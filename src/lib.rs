#![feature(rustc_private)]
#![warn(unused_extern_crates)]

dylint_linting::dylint_library!();

extern crate rustc_lint;
extern crate rustc_session;

#[expect(clippy::no_mangle_with_rust_abi)]
#[unsafe(no_mangle)]
pub fn register_lints(sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    eol_comments::register_lints(sess, lint_store);
    // nesting_too_deep::register_lints(sess, lint_store);
    control_flow::register_lints(sess, lint_store);
    uninlined_format_args::register_lints(sess, lint_store);
}
