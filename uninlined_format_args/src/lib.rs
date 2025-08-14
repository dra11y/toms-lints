#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_lint_defs;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_ast::{
    Expr, ExprKind, FormatAlignment, FormatArgPositionKind, FormatArgumentKind, FormatCount,
    FormatDebugHex, FormatSign, FormatTrait,
};
use rustc_lint::{EarlyContext, EarlyLintPass, Level, LintContext};
use rustc_lint_defs::Applicability;
// use rustc_middle::lint::LintLevelSource;

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
        if cx.get_lint_level(UNINLINED_FORMAT_ARGS).level == Level::Allow {
            return;
        }

        let ExprKind::FormatArgs(format_args) = &expr.kind else {
            return;
        };

        for placeholder in format_args.template.iter() {
            let rustc_ast::FormatArgsPiece::Placeholder(placeholder) = placeholder else {
                continue;
            };

            if placeholder.argument.kind == FormatArgPositionKind::Implicit {
                self.check_implicit_placeholder(cx, placeholder, &format_args.arguments);
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
        let Ok(arg_index) = placeholder.argument.index else {
            return;
        };

        let Some(format_arg) = arguments.by_index(arg_index) else {
            return;
        };

        if !matches!(format_arg.kind, FormatArgumentKind::Normal) {
            return;
        }

        // This is a normal argument that could potentially be inlined
        let ExprKind::Path(None, path) = &format_arg.expr.kind else {
            return;
        };

        if path.segments.len() != 1 {
            return;
        }

        let Some(span) = placeholder.span else {
            return;
        };

        let variable_name = &path.segments[0].ident;

        let format_spec = self.build_format_spec(placeholder);
        let suggestion = format!("{{{variable_name}{format_spec}}}");

        // error: variables can be used directly in the `format!` string
        //   --> crate/src/module.rs:84:5
        //    |
        // 84 |     println!("idle timeout = {}", idle_timeout_seconds);
        //    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        //    |
        //    = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#uninlined_format_args
        // note: the lint level is defined here
        //   --> crate/src/lib.rs:1:9
        //    |
        //  1 | #![deny(clippy::uninlined_format_args)]
        //    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        // help: change this to
        //    |
        // 84 -     println!("idle timeout = {}", idle_timeout_seconds);
        // 84 +     println!("idle timeout = {idle_timeout_seconds}");
        //    |
        //
        //    = note: `#[warn(uninlined_format_args)]` on by default
        //
        // note: the lint level is defined here
        //   --> chromium-operator/src/api/chromium_session_handlers.rs:1:9
        //    |
        // 1  | #![deny(clippy::uninlined_format_args)]
        //    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        // let level_and_src = cx.get_lint_level(UNINLINED_FORMAT_ARGS);

        cx.span_lint(UNINLINED_FORMAT_ARGS, span, |lint| {
            lint.primary_message("variables can be used directly in the `format!` string");
            lint.help("for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#uninlined_format_args");
            lint.span_suggestion(
                span,
                "change this to",
                suggestion,
                Applicability::MachineApplicable,
            );

            // // Add notes about lint level similar to clippy
            // match level_and_src.src {
            //     LintLevelSource::Default => {
            //         // let level: &'static str = level_and_src.level.as_str();
            //         // lint.note(format!("`#[{level}({})]` on by default", UNINLINED_FORMAT_ARGS.name_lower()));
            //     },
            //     LintLevelSource::Node { span, .. } => {
            //         lint.span_note(span, "the lint level is defined here");
            //     },
            //     LintLevelSource::CommandLine(_symbol, level) => {
            //         lint.span_note(span, "the lint level is defined on the command line");
            //     },
            // }
        });
    }

    fn build_format_spec(&self, placeholder: &rustc_ast::FormatPlaceholder) -> String {
        let mut spec = String::new();
        let options = &placeholder.format_options;

        // Check if we have any format options OR a non-default format trait
        let has_options = options.fill.is_some()
            || options.alignment.is_some()
            || options.sign.is_some()
            || options.alternate
            || options.zero_pad
            || options.width.is_some()
            || options.precision.is_some()
            || options.debug_hex.is_some()
            || !matches!(placeholder.format_trait, FormatTrait::Display);

        if !has_options {
            return spec;
        }

        spec.push(':');

        // Fill character (must come before alignment)
        if let Some(fill) = options.fill {
            spec.push(fill);
        }

        // Alignment
        if let Some(alignment) = options.alignment {
            match alignment {
                FormatAlignment::Left => spec.push('<'),
                FormatAlignment::Right => spec.push('>'),
                FormatAlignment::Center => spec.push('^'),
            }
        }

        // Sign
        if let Some(sign) = options.sign {
            match sign {
                FormatSign::Plus => spec.push('+'),
                FormatSign::Minus => spec.push('-'),
            }
        }

        // Alternate flag
        if options.alternate {
            spec.push('#');
        }

        // Zero padding
        if options.zero_pad {
            spec.push('0');
        }

        // Width
        if let Some(width) = &options.width {
            match width {
                FormatCount::Literal(n) => spec.push_str(&n.to_string()),
                FormatCount::Argument(_pos) => {
                    // For argument-based width like {:.width$}, we can't inline this
                    // TODO: Handle this case - for now skip
                }
            }
        }

        // Precision
        if let Some(precision) = &options.precision {
            spec.push('.');
            match precision {
                FormatCount::Literal(n) => spec.push_str(&n.to_string()),
                FormatCount::Argument(_pos) => {
                    // For argument-based precision like {:.precision$}, we can't inline this
                    // TODO: Handle this case - for now skip
                }
            }
        }

        // Debug hex modifier (for Debug trait)
        if let Some(debug_hex) = options.debug_hex {
            match debug_hex {
                FormatDebugHex::Lower => spec.push_str("x?"),
                FormatDebugHex::Upper => spec.push_str("X?"),
            }
        } else {
            // Format trait specifier
            match placeholder.format_trait {
                FormatTrait::Display => {} // No specifier needed
                FormatTrait::Debug => spec.push('?'),
                FormatTrait::LowerExp => spec.push('e'),
                FormatTrait::UpperExp => spec.push('E'),
                FormatTrait::Octal => spec.push('o'),
                FormatTrait::Pointer => spec.push('p'),
                FormatTrait::Binary => spec.push('b'),
                FormatTrait::LowerHex => spec.push('x'),
                FormatTrait::UpperHex => spec.push('X'),
            }
        }

        spec
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
