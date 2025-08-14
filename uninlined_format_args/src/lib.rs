#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_span;

use rustc_ast::{Expr, ExprKind, FormatArgPositionKind, FormatArgumentKind};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};

dylint_linting::declare_early_lint! {
    /// ### What it does
    /// Effectively runs the uninlined_format_args clippy lint on any macro that expands to use format_args!
    ///
    /// ### Why is this bad?
    /// Uninlined format arguments are hard to read. In 3rd party crates, they are not linted like they are
    /// in std with clippy. This results in inconsistent formatting. This lint fills the gap
    /// by linting 3rd party macros for uninlined_format_args like clippy does for std.
    ///
    /// ### Example
    /// ```rust
    /// fn main() {
    ///    let a = 42;
    ///    let b = Some("test");
    ///    tracing::warn!("This should lint: {}", a);
    ///    tracing::error!("So should this: {}, {:?}", a, b);
    ///    tracing::info!("But these are OK: {a}, {b:?}");
    /// }
    /// ```
    pub UNINLINED_FORMAT_ARGS,
    Warn,
    "format arguments should be inlined for readability and consistency"
}

impl EarlyLintPass for UninlinedFormatArgs {
    fn check_expr(&mut self, cx: &EarlyContext, expr: &Expr) {
        let ExprKind::FormatArgs(format_args) = &expr.kind else {
            return;
        };

        // println!("=== FormatArgs structure ===");
        // println!("All args: {:?}", format_args.arguments.all_args().len());

        // Check each placeholder in the template
        for (_i, placeholder) in format_args.template.iter().enumerate() {
            let rustc_ast::FormatArgsPiece::Placeholder(placeholder) = placeholder else {
                continue;
            };

            // println!("--- Placeholder {} ---", i);

            match &placeholder.argument.kind {
                FormatArgPositionKind::Implicit => {
                    self.check_implicit_placeholder(cx, placeholder, &format_args.arguments);
                }
                FormatArgPositionKind::Named => {
                    // println!(
                    //     "Found INLINED (named) format argument (OK) at {:?}",
                    //     placeholder.span
                    // );
                }
                FormatArgPositionKind::Number => {
                    // println!(
                    //     "Found INLINED (numbered) format argument (OK) at {:?}",
                    //     placeholder.span
                    // );
                }
            }
        }
    }
}

impl UninlinedFormatArgs {
    fn check_implicit_placeholder(
        &self,
        cx: &EarlyContext,
        placeholder: &rustc_ast::FormatPlaceholder,
        arguments: &rustc_ast::FormatArguments,
    ) {
        // println!("Found IMPLICIT format argument at {:?}", placeholder.span);
        // println!("  Argument index: {:?}", placeholder.argument.index);

        let Ok(arg_index) = placeholder.argument.index else {
            // println!("  -> Invalid argument index");
            return;
        };

        let Some(format_arg) = arguments.by_index(arg_index) else {
            // println!("  -> Could not find argument at index {}", arg_index);
            return;
        };

        // println!("  Argument kind: {:?}", format_arg.kind);
        // println!("  Argument expr kind: {:?}", format_arg.expr.kind);

        let FormatArgumentKind::Normal = &format_arg.kind else {
            match &format_arg.kind {
                FormatArgumentKind::Named(_) => {
                    // println!("  -> No lint: Already named")
                }
                FormatArgumentKind::Captured(_) => {
                    // println!("  -> No lint: Already captured/inlined")
                }
                FormatArgumentKind::Normal => unreachable!(),
            }
            return;
        };

        // This is a normal argument that could potentially be inlined
        let rustc_ast::ExprKind::Path(None, path) = &format_arg.expr.kind else {
            // println!("  -> No lint: Not a simple path");
            return;
        };

        if path.segments.len() != 1 {
            // println!("  -> No lint: Complex path");
            return;
        }

        // println!(
        //     "  -> SHOULD LINT: Simple variable '{}' could be inlined",
        //     path.segments[0].ident
        // );

        // Emit the actual lint
        if let Some(span) = placeholder.span {
            cx.lint(UNINLINED_FORMAT_ARGS, |lint| {
                lint.span(span)
                    .note("format argument should be inlined")
                    .help("consider using the variable name directly in the placeholder");
            });
        }
    }
}

// impl<'tcx> LateLintPass<'tcx> for UninlinedFormatArgs {
//     fn check_expr(&mut self, _cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {}
// }

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
