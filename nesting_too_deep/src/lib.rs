#![feature(rustc_private)]
#![feature(let_chains)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashSet;

use dylint_linting::config_or_default;
use rustc_hir::{
    Block, ExprKind, HirId, ImplItem, ImplItemKind, Item, ItemKind, Node, TraitItem, TraitItemKind,
};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

/// Default maximum nesting levels
const DEFAULT_MAX_LEVELS: u32 = 3;

const HELP_MESSAGE: &str =
    "consider using early returns, guard clauses, or extracting functions to reduce nesting";

/// Lint configuration
#[derive(serde::Deserialize)]
struct Config {
    levels: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            levels: DEFAULT_MAX_LEVELS,
        }
    }
}

/// Lint for detecting nesting that is too deep
pub struct NestingTooDeep {
    config: Config,
    /// Track spans we've already reported to avoid duplicates
    emitted_spans: HashSet<Span>,
}

impl Default for NestingTooDeep {
    fn default() -> Self {
        Self {
            config: config_or_default(env!("CARGO_PKG_NAME")),
            emitted_spans: HashSet::new(),
        }
    }
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    /// Checks for nested if-then-else statements and other branching that is too many levels deep.
    ///
    /// ### Why is this bad?
    /// Deeply nested code is hard to read and maintain, leading to confusion and bugs.
    ///
    /// ### Examples
    /// ```rust
    /// if let Ok(value) = result {
    ///     // Do something
    ///     if let Some(inner) = option {
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
    /// let Ok(value) = result?;
    /// let Some(inner) = option {
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
    "nested if-then-else and other branching should be simplified",
    NestingTooDeep::default()
}

impl<'tcx> LateLintPass<'tcx> for NestingTooDeep {
    fn check_block(&mut self, cx: &LateContext<'tcx>, block: &'tcx Block<'tcx>) {
        let (top_level_span, depth) = self.find_top_level_and_count_depth(cx, block.hir_id);

        if depth < self.config.levels {
            return;
        }

        if self.emitted_spans.contains(&top_level_span) {
            return;
        }

        self.emitted_spans.insert(top_level_span);

        cx.span_lint(NESTING_TOO_DEEP, top_level_span, |lint| {
            lint.primary_message(format!(
                "nested structure is {depth} levels deep (max: {})",
                self.config.levels
            ))
            .help(HELP_MESSAGE);
        });
    }
}

impl NestingTooDeep {
    /// Walk up parent chain to find top-level nesting construct and count depth
    fn find_top_level_and_count_depth(&self, cx: &LateContext<'_>, hir_id: HirId) -> (Span, u32) {
        let mut depth = 0;
        let mut top_level_span = cx.tcx.hir_span(hir_id);

        // Walk up parent chain within current function
        for (parent_id, _) in cx.tcx.hir_parent_iter(hir_id) {
            let parent_node = cx.tcx.hir_node(parent_id);

            let is_nesting_node = match parent_node {
                Node::Expr(expr) => matches!(
                    expr.kind,
                    ExprKind::If(..)
                        | ExprKind::Loop(..)
                        | ExprKind::Match(..)
                        | ExprKind::Closure(..)
                ),
                _ => false,
            };
            if is_nesting_node {
                depth += 1;
                top_level_span = cx.tcx.hir_span(parent_id);
            }

            let is_function_boundary = matches!(
                parent_node,
                Node::Item(Item {
                    kind: ItemKind::Fn { .. },
                    ..
                }) | Node::TraitItem(TraitItem {
                    kind: TraitItemKind::Fn(..),
                    ..
                }) | Node::ImplItem(ImplItem {
                    kind: ImplItemKind::Fn(..),
                    ..
                })
            );

            if is_function_boundary {
                break;
            }
        }

        (top_level_span, depth)
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
