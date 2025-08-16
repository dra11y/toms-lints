#![feature(rustc_private)]
#![feature(let_chains)]
#![warn(unused_extern_crates)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_ast::{Block, Stmt};
use rustc_errors::Applicability;
use rustc_hir::Item;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::BytePos;

dylint_linting::declare_early_lint! {
    /// ### What it does
    /// Checks for nested if-then-else statements and other branching that is too many levels deep.
    ///
    /// ### Why is this bad?
    /// Deeply nested code is hard to read and maintain, leading to confusion and bugs.
    ///
    /// ### Example
    /// ```rust
    /// if condition1 {
    ///     // Do something
    ///     if condition2 {
    ///         if condition3 {
    ///             // Do something
    ///         } else {
    ///             // Do nothing
    ///         }
    ///     } else {
    ///         // Do nothing
    ///     }
    /// }
    /// ```
    ///
    /// Use instead:
    /// ```rust
    ///
    /// if !condition1 {
    ///     return;
    /// }
    /// if !condition2 {
    ///     return;
    /// }
    ///
    /// if condition3 {
    ///     // Do something
    /// } else {
    ///     // Do nothing
    /// }
    /// ```
    pub NESTING_TOO_DEEP,
    Warn,
    "nested if-then-else and other branching should be simplified"
}

impl<'tcx> EarlyLintPass<'tcx> for NestingTooDeep {
    fn check_block(&mut self, cx: &EarlyContext<'tcx>, block: &Block);
    fn check_stmt(&mut self, cx: &EarlyContext<'tcx>, stmt: &Stmt);
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
