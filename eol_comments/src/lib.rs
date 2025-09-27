#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_errors::Applicability;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::BytePos;

dylint_linting::declare_late_lint! {
    /// ### What it does
    /// Checks for comments at the end of lines with code.
    ///
    /// ### Why is this bad?
    /// End-of-line comments are harder to read, especially on longer lines.
    /// AI LLMs are notorious for generating unhelpful EOL comments. This lint gets rid of them.
    ///
    /// ### Example
    /// ```rust
    /// let x = 42; // changed to 42
    /// ```
    ///
    /// Use instead:
    /// ```rust
    /// // changed to 42
    /// let x = 42;
    /// ```
    pub EOL_COMMENTS,
    Warn,
    "end-of-line comments should be moved or removed"
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

                        // Find the start of whitespace before the comment
                        let mut whitespace_start = i;
                        while whitespace_start > 0 {
                            let prev_char = line.chars().nth(whitespace_start - 1).unwrap_or('\0');
                            if prev_char.is_whitespace() {
                                whitespace_start -= 1;
                            } else {
                                break;
                            }
                        }

                        let lo = span.lo() + BytePos((base) as u32);
                        let hi = span.lo() + BytePos((base + line.len()) as u32);
                        let sub = span.with_lo(lo).with_hi(hi);

                        // Check if this is a /* block comment
                        let is_block_comment = next == '*';

                        cx.span_lint(EOL_COMMENTS, sub, |lint| {
                            lint.note(EOL_COMMENTS.desc)
                                .help("consider removing or moving this comment");

                            if is_block_comment {
                                // For block comments, suggest adding a newline before the comment
                                let whitespace_before = &line[whitespace_start..i];
                                let comment_text = &line[i..];
                                let suggestion = format!(
                                    "{}\n{}{}",
                                    whitespace_before, whitespace_before, comment_text
                                );
                                lint.span_suggestion_verbose(
                                    sub,
                                    "move block comment to its own line",
                                    suggestion,
                                    Applicability::MachineApplicable,
                                );
                            } else {
                                // For line comments, suggest moving the comment to the previous line
                                let code_part = &line[..whitespace_start].trim_end();
                                let comment_part = &line[i..].trim_start();
                                let suggestion = format!(
                                    "{}\n{}",
                                    comment_part, code_part
                                );
                                lint.span_suggestion_verbose(
                                    sub,
                                    "move comment to previous line",
                                    suggestion,
                                    Applicability::MachineApplicable,
                                );
                            }
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
    dylint_uitesting::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
