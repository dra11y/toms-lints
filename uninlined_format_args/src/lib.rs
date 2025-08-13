#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::BytePos;

dylint_linting::declare_late_lint! {
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

impl<'tcx> LateLintPass<'tcx> for UninlinedFormatArgs {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        todo!();
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
