#![feature(rustc_private)]
#![warn(unused_extern_crates)]

dylint_linting::dylint_library!();

extern crate rustc_lint;
extern crate rustc_session;

use declare_clippy_lint::LintListBuilder;
use dylint_internal::env;
use std::env::{remove_var, set_var};

/// All of the Clippy lints as a Dylint library
#[expect(clippy::no_mangle_with_rust_abi)]
#[unsafe(no_mangle)]
pub fn register_lints(sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    if let Ok(clippy_disable_docs_links) = env::var(env::CLIPPY_DISABLE_DOCS_LINKS)
        && let Ok(val) = serde_json::from_str::<Option<String>>(&clippy_disable_docs_links)
    {
        if let Some(val) = val {
            unsafe {
                set_var(env::CLIPPY_DISABLE_DOCS_LINKS, val);
            }
        } else {
            unsafe {
                remove_var(env::CLIPPY_DISABLE_DOCS_LINKS);
            }
        }
    }

    let mut list_builder = LintListBuilder::default();
    list_builder.insert(clippy_lints::declared_lints::LINTS);
    list_builder.register(lint_store);

    let conf_path = clippy_config::lookup_conf_file();
    let conf = clippy_config::Conf::read(sess, &conf_path);
    clippy_lints::register_lint_passes(lint_store, conf);
}

// TODO: find a way to test clippy -- only some of its ui/**/*.rs files work with current ui_test
// #[cfg(test)]
// mod tests {
//     use dylint_internal::CommandExt;

//     #[test]
//     fn ui_test() {
//         extern crate rustc_lint;
//         use rustc_lint::Level;

//         let lint_store = rustc_lint::new_lint_store(false);
//         let allow_rustc_flags = lint_store
//             .get_lints()
//             .iter()
//             .filter(|lint| {
//                 // disable builtin lints
//                 lint.default_level == Level::Warn
//                 // -Awarnings disables all clippy lints too!
//                 && lint.name != rustc_lint::builtin::WARNINGS.name
//             })
//             .map(|lint| format!("-A{}", lint.name_lower().replace('_', "-")))
//             .collect::<Vec<_>>();

//         // use std::process::Command;
//         // use tempfile::Builder as TempDirBuilder;

//         // let tempdir = TempDirBuilder::new()
//         //     .disable_cleanup(true)
//         //     .tempdir()
//         //     .unwrap();
//         // let root = tempdir.path();
//         // let ui = root.join("tests/ui");
//         // let root_str = root.to_str().unwrap();

//         // Command::new("git")
//         //     .args([
//         //         "clone",
//         //         "--depth",
//         //         "1",
//         //         "https://github.com/rust-lang/rust-clippy",
//         //         root_str,
//         //     ])
//         //     .status()
//         //     .expect("clone rust-clippy");

//         // // Remove all clippy:: qualifiers from #[allow(...)], #[deny(...)] etc.
//         // Command::new("sh")
//         //     .args(["-c", "sed -r -i '' 's/clippy:://g' *.rs"])
//         //     .current_dir(&ui)
//         //     .status()
//         //     .expect("run sed");

//         unsafe {
//             std::env::set_var("BLESS", "1");
//         }

//         dylint_uitesting::ui::Test::src_base(env!("CARGO_PKG_NAME"), "tests/ui")
//             .normalize_codes(true)
//             .rustc_flags(allow_rustc_flags)
//             .rustc_flags(["-Dwarnings"])
//             .run();
//     }
// }
