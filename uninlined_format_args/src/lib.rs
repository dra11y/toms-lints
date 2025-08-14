#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_lint_defs;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_ast::{
    Expr, ExprKind, FormatArgPositionKind, FormatArgs, FormatArgsPiece, FormatArgumentKind,
    FormatPlaceholder, MacCall,
};
use rustc_lint::{EarlyContext, EarlyLintPass, Level, LintContext};
use rustc_lint_defs::Applicability;
use rustc_span::{BytePos, Span, hygiene};

/// from clippy_utils: https://github.com/rust-lang/rust-clippy/blob/master/clippy_utils/src/macros.rs#L456
/// Span of the `:` and format specifiers
///
/// ```ignore
/// format!("{:.}"), format!("{foo:.}")
///           ^^                  ^^
/// ```
pub fn format_placeholder_format_span(placeholder: &FormatPlaceholder) -> Option<Span> {
    let base = placeholder.span?.data();

    // `base.hi` is `{...}|`, subtract 1 byte (the length of '}') so that it points before the closing
    // brace `{...|}`
    Some(Span::new(
        placeholder.argument.span?.hi(),
        base.hi - BytePos(1),
        base.ctxt,
        base.parent,
    ))
}

/// from clippy_utils: https://github.com/rust-lang/rust-clippy/blob/master/clippy_utils/src/macros.rs#L481
/// Span covering the format string and values
///
/// ```ignore
/// format("{}.{}", 10, 11)
/// //     ^^^^^^^^^^^^^^^
/// ```
pub fn format_args_inputs_span(format_args: &FormatArgs) -> Span {
    match format_args.arguments.explicit_args() {
        [] => format_args.span,
        [.., last] => format_args
            .span
            .to(hygiene::walk_chain(last.expr.span, format_args.span.ctxt())),
    }
}

/// from clippy_utils: https://github.com/rust-lang/rust-clippy/blob/master/clippy_utils/src/macros.rs#L497
/// Returns the [`Span`] of the value at `index` extended to the previous comma, e.g. for the value
/// `10`
///
/// ```ignore
/// format("{}.{}", 10, 11)
/// //            ^^^^
/// ```
pub fn format_arg_removal_span(format_args: &FormatArgs, index: usize) -> Option<Span> {
    let ctxt = format_args.span.ctxt();

    let current = hygiene::walk_chain(format_args.arguments.by_index(index)?.expr.span, ctxt);

    let prev = if index == 0 {
        format_args.span
    } else {
        hygiene::walk_chain(format_args.arguments.by_index(index - 1)?.expr.span, ctxt)
    };

    Some(current.with_lo(prev.hi()))
}

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
    fn check_mac(&mut self, _cx: &EarlyContext, _mac: &MacCall) {
        // does not work outside of pre_expansion!!!
        // println!("mac: {mac:#?}");
    }

    fn check_expr(&mut self, cx: &EarlyContext, expr: &Expr) {
        if cx.get_lint_level(UNINLINED_FORMAT_ARGS).level == Level::Allow {
            return;
        }

        let ExprKind::FormatArgs(format_args) = &expr.kind else {
            return;
        };

        let mut fixes = Vec::new();

        for placeholder in format_args.template.iter() {
            let FormatArgsPiece::Placeholder(placeholder) = placeholder else {
                continue;
            };

            let FormatArgPositionKind::Implicit = placeholder.argument.kind else {
                continue;
            };

            let Ok(arg_index) = placeholder.argument.index else {
                continue;
            };

            let Some(format_arg) = format_args.arguments.by_index(arg_index) else {
                continue;
            };

            let FormatArgumentKind::Normal = format_arg.kind else {
                continue;
            };

            let ExprKind::Path(None, path) = &format_arg.expr.kind else {
                continue;
            };

            let [segment] = path.segments.as_slice() else {
                continue;
            };

            let Some(placeholder_span) = placeholder.span else {
                continue;
            };

            let Some(arg_removal_span) = format_arg_removal_span(format_args, arg_index) else {
                continue;
            };

            let variable_name = &segment.ident;
            let format_spec = format_placeholder_format_span(placeholder)
                .and_then(|spec_span| cx.sess().source_map().span_to_snippet(spec_span).ok())
                .unwrap_or_default();
            let suggestion = format!("{{{variable_name}{format_spec}}}");

            fixes.push((placeholder_span, suggestion));
            fixes.push((arg_removal_span, String::new()));
        }

        if fixes.is_empty() {
            return;
        }

        cx.span_lint(UNINLINED_FORMAT_ARGS, expr.span.source_callsite(), |lint| {
            lint.primary_message("variables can be used directly in the `format!` string");
            lint.help("for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#uninlined_format_args");
            lint.multipart_suggestion(
                "change this to",
                fixes,
                Applicability::MachineApplicable,
            );
        });
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
