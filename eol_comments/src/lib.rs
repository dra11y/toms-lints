#![feature(rustc_private)]
#![feature(let_chains)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::BytePos;

dylint_linting::declare_late_lint! {
    /// ### What it does
    /// Checks for comments at the end of lines with code and suggests moving them
    /// to their own line above the code for better readability.
    ///
    /// ### Why is this bad?
    /// End-of-line comments make code harder to read, especially with longer lines.
    /// They also make it difficult to format code consistently and may be missed
    /// when scanning code. Comments on their own line are easier to read.
    ///
    /// ### Example
    /// ```rust
    /// let x = 42; // The Answer to the Ultimate Question of Life, The Universe, and Everything
    /// ```
    ///
    /// Use instead:
    /// ```rust
    /// // The Answer to the Ultimate Question of Life, The Universe, and Everything
    /// let x = 42;
    /// ```
    pub EOL_COMMENTS,
    Warn,
    "end-of-line comments should be moved to their own line above the code"
}

impl<'tcx> LateLintPass<'tcx> for EolComments {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let span = item.span;
        let sm = cx.sess().source_map();
        let Ok(snippet) = sm.span_to_snippet(span) else {
            return;
        };

        let mut base = 0usize;
        for line in snippet.lines() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                base += line.len() + 1;
                continue;
            }
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                base += line.len() + 1;
                continue;
            }

            let mut in_string = false;
            let mut in_char = false;
            let mut escaped = false;
            for (i, &b) in line.as_bytes().iter().enumerate() {
                let c = b as char;
                if escaped {
                    escaped = false;
                    continue;
                }
                match c {
                    '\\' if in_string || in_char => {
                        escaped = true;
                    }
                    '"' if !in_char => {
                        in_string = !in_string;
                    }
                    '\'' if !in_string => {
                        in_char = !in_char;
                    }
                    '/' if !in_string && !in_char => {
                        let Some(&next) = line.as_bytes().get(i + 1) else {
                            continue;
                        };
                        let next = next as char;
                        if !(next == '/' || next == '*') {
                            continue;
                        }
                        if line[..i].trim().is_empty() {
                            break;
                        }
                        let lo = span.lo() + BytePos((base + i) as u32);
                        let hi = span.lo() + BytePos((base + line.len()) as u32);
                        let sub = span.with_lo(lo).with_hi(hi);
                        cx.lint(EOL_COMMENTS, |lint| {
                            lint.span(sub)
                                .note("end-of-line comment should be moved to its own line above the code")
                                .help("consider moving this comment to its own line above the code");
                        });
                        break;
                    }
                    _ => {}
                }
            }
            base += line.len() + 1;
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
