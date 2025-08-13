#![feature(rustc_private)]
#![feature(let_chains)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use rustc_hir::{Expr, Item, Stmt};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

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
        self.check_for_eol_comments(cx, item.span);
    }

    fn check_stmt(&mut self, cx: &LateContext<'tcx>, stmt: &'tcx Stmt<'tcx>) {
        self.check_for_eol_comments(cx, stmt.span);
    }
}

impl EolComments {
    fn check_for_eol_comments(&self, cx: &LateContext<'_>, span: Span) {
        let source_map = cx.sess().source_map();

        // Get the source text for this span
        let snippet = match source_map.span_to_snippet(span) {
            Ok(snippet) => snippet,
            Err(_) => return,
        };

        // Process each line in the snippet
        for (line_idx, line) in snippet.lines().enumerate() {
            self.check_line_for_eol_comment(cx, span, line, line_idx);
        }
    }

    fn check_line_for_eol_comment(
        &self,
        cx: &LateContext<'_>,
        span: Span,
        line: &str,
        line_idx: usize,
    ) {
        // Skip empty lines and lines that start with comments
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
            return;
        }

        // Look for end-of-line comments (both // and /* styles)
        let comment_pos = {
            let slash_slash = line.find("//");
            let slash_star = line.find("/*");

            match (slash_slash, slash_star) {
                (Some(ss), Some(st)) => ss.min(st),
                (Some(ss), None) => ss,
                (None, Some(st)) => st,
                (None, None) => return,
            }
        };

        // Check if the // is inside a string literal
        if self.is_inside_string_literal(line, comment_pos) {
            return;
        }

        let before_comment = line[..comment_pos].trim();

        // Flag any line that has code before a comment
        if !before_comment.is_empty() {
            cx.lint(EOL_COMMENTS, |lint| {
                lint.span(span)
                    .note("end-of-line comment should be moved to its own line above the code")
                    .help("consider moving this comment to its own line above the code");
            });
        }
    }

    fn is_inside_string_literal(&self, line: &str, comment_pos: usize) -> bool {
        let before_comment = &line[..comment_pos];
        let mut in_string = false;
        let mut in_char = false;
        let mut escaped = false;

        for ch in before_comment.chars() {
            match ch {
                '"' if !in_char && !escaped => in_string = !in_string,
                '\'' if !in_string && !escaped => in_char = !in_char,
                '\\' if (in_string || in_char) && !escaped => escaped = true,
                _ => escaped = false,
            }
        }

        in_string || in_char
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
