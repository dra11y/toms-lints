#![feature(rustc_private)]
#![warn(unused_extern_crates)]

mod config;
mod context;
mod debug;

extern crate rustc_ast;
extern crate rustc_span;

const DESCRIPTION: &str = "excessive nesting";

use anyhow::bail;
use config::{Config, HELP_MESSAGE};
use context::{Context, ContextKind, NestingLint, Reason};
use debug::debug_expr_kind;
use dylint_linting::config_or_default;
use rustc_ast::{Arm, AssocItem, Expr, ExprKind, Item, ItemKind, ModKind, NodeId};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;
use std::collections::HashSet;

/// Lint for detecting nesting that is too deep
pub struct NestingDepth {
    config: Config,
    contexts: Vec<Context>,
    lints: Vec<NestingLint>,
    skipped_macro_ids: HashSet<NodeId>,
    checked_ids: HashSet<NodeId>,
    else_if_expr_ids: HashSet<NodeId>,
    else_block_expr_ids: HashSet<NodeId>,
    current_nesting_lint: Option<NestingLint>,
    inside_fn: bool,
}

impl Default for NestingDepth {
    fn default() -> Self {
        Self {
            config: config_or_default(env!("CARGO_PKG_NAME")),
            contexts: vec![],
            lints: vec![],
            skipped_macro_ids: HashSet::new(),
            checked_ids: HashSet::new(),
            else_if_expr_ids: HashSet::new(),
            else_block_expr_ids: HashSet::new(),
            current_nesting_lint: None,
            inside_fn: false,
        }
    }
}

dylint_linting::impl_early_lint! {
    /// ### What it does
    /// Checks for nested if-then-else statements and other branching that is too many levels deep.
    ///
    /// ### Why is this bad?
    /// Deeply nested code is hard to read and maintain, leading to confusion and bugs.
    ///
    /// ### Examples
    /// ```rust,no_run
    /// # let result: Result<i32, &str> = Ok(42);
    /// # let option: Option<i32> = Some(10);
    /// # let condition3 = true;
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
    /// ```rust,no_run
    /// # let result: Result<i32, &str> = Ok(42);
    /// # let option: Option<i32> = Some(10);
    /// # let condition3 = true;
    /// let Ok(value) = result else {
    ///     return;
    /// };
    /// let Some(inner) = option else {
    ///     return;
    /// };
    ///
    /// if condition3 {
    ///     // Do something
    /// } else {
    ///     // Do nothing
    /// }
    /// ```
    pub NESTING_DEPTH,
    Warn,
    DESCRIPTION,
    NestingDepth::default()
}

impl NestingDepth {
    fn depth(&self) -> usize {
        self.contexts
            .iter()
            .skip(1)
            .filter(|c| c.count_depth())
            .count()
    }

    fn push_context(&mut self, cx: &EarlyContext<'_>, kind: ContextKind, id: NodeId, span: Span) {
        let source_map = cx.sess().source_map();
        let ctx = Context::new(kind.clone(), id, span);
        self.contexts.push(ctx);
        let depth = self.depth();

        self.debug_visit(cx, &format!("PUSH CONTEXT: {kind}"), span);

        if depth <= self.config.max_depth {
            return;
        }

        let outer_span = self.contexts.get(1).map(|ctx| ctx.span);

        let mut lint = self.current_nesting_lint.get_or_insert(NestingLint {
            outer_span,
            span,
            kind,
            reason: Reason::Depth(depth),
        });
        lint.reason = Reason::Depth(depth);
    }

    fn push_current_lints(&mut self, cx: &EarlyContext<'_>, ctx: &mut Context) {
        if let Some(lint) = self.current_nesting_lint.take() {
            self.lints.push(lint);
        }

        if let Some(lint) = ctx.consec_if_else_lint.take() {
            self.lints.push(lint);
        }
    }

    fn pop_context_unchecked(&mut self, cx: &EarlyContext<'_>) -> Context {
        let ctx = self.contexts.pop().expect("pop context unchecked");
        self.debug_visit(
            cx,
            &format!("POP CONTEXT: {} {}", ctx.id, ctx.kind),
            ctx.span,
        );
        ctx
    }

    fn pop_context(
        &mut self,
        cx: &EarlyContext<'_>,
        kind: &ContextKind,
        id: &NodeId,
    ) -> Result<(), anyhow::Error> {
        let depth = self.depth();
        let mut ctx = self.pop_context_unchecked(cx);

        if !matches!(&ctx.kind, kind) {
            bail!(
                "pop context kind mismatch: expected {kind}, got {}",
                ctx.kind
            );
        }

        if ctx.id != *id {
            bail!(
                "pop context id mismatch: kind: {kind}, expected {id}, got {}",
                ctx.id
            );
        }

        self.push_current_lints(cx, &mut ctx);

        Ok(())
    }

    fn should_check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) -> bool {
        matches!(
            item.kind,
            // ItemKind::Static(_) |
            ItemKind::Fn(..)
                | ItemKind::Mod(_, _, ModKind::Loaded(..))
                | ItemKind::Trait(..)
                | ItemKind::Impl(..)
        ) && self.should_check_id(cx, item.id, item.span)
    }

    /// Returns `true` if the node is not from a macro expansion and can be checked
    fn should_check_id(&mut self, cx: &EarlyContext<'_>, id: NodeId, span: Span) -> bool {
        if self.checked_ids.contains(&id) {
            return true;
        }
        if self.skipped_macro_ids.contains(&id) {
            return false;
        }
        if span.ctxt().in_external_macro(cx.sess().source_map()) {
            self.skipped_macro_ids.insert(id);
            return false;
        }
        self.checked_ids.insert(id);
        true
    }
}

impl EarlyLintPass for NestingDepth {
    #[inline(always)]
    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, a: &rustc_ast::Crate) {
        for lint in &self.lints {
            cx.span_lint(NESTING_DEPTH, lint.span, |diag| {
                if let Some(outer_span) = lint.outer_span {
                    diag.span_label(outer_span, lint.reason.outer_context_label());
                }
                diag.primary_message(lint.reason.message(&self.config));
                diag.help(HELP_MESSAGE);
            });
        }
    }

    #[inline(always)]
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if !self.should_check_item(cx, item) {
            return;
        }

        match &item.kind {
            ItemKind::Static(item) => {
                let Some(expr) = &item.expr else {
                    return;
                };
            }
            ItemKind::Fn(func) => {
                if let Some(body) = &func.body {

                    // self.push_context(cx, ContextKind::Func, , span);
                }
            }
            ItemKind::Mod(_, ident, ModKind::Loaded(items, inline, spans)) => todo!(),
            ItemKind::Trait(_) => todo!(),
            ItemKind::Impl(_) => todo!(),
            _ => return,
        }

        self.push_context(cx, ContextKind::Item, item.id, item.span);
        self.debug_visit_extra(cx, "ENTER item", item.span, item.kind.descr());
    }

    #[inline(always)]
    fn check_item_post(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if !self.checked_ids.contains(&item.id) {
            return;
        }

        self.debug_visit_extra(cx, "EXIT item", item.span, item.kind.descr());
        self.pop_context(cx, &ContextKind::Item, &item.id)
            .expect("pop item context");
    }

    #[inline(always)]
    fn check_arm(&mut self, cx: &EarlyContext<'_>, arm: &Arm) {
        // println!("CHECK ARM");
    }

    #[inline(always)]
    fn check_expr(&mut self, cx: &EarlyContext<'_>, expr: &Expr) {
        if !self.should_check_id(cx, expr.id, expr.span) {
            return;
        }

        // println!("CHECK EXPR ID: {} {}", expr.id, debug_expr_kind(&expr.kind));

        match &expr.kind {
            // enter the `if` or `else-if` block context
            ExprKind::If(_cond, if_or_else_if_block, else_expr) => {
                let kind = if self.else_if_expr_ids.contains(&expr.id) {
                    ContextKind::ElseIf
                } else {
                    ContextKind::Then
                };
                self.push_context(cx, ContextKind::If, expr.id, expr.span);
                self.push_context(cx, kind, if_or_else_if_block.id, if_or_else_if_block.span);
                self.debug_visit(
                    cx,
                    &format!("ENTER IF: {} {}", expr.id, if_or_else_if_block.id),
                    expr.span,
                );
                if let Some(else_expr) = else_expr {
                    self.debug_visit(
                        cx,
                        &format!(
                            "  with ELSE: {} {}",
                            else_expr.id,
                            debug_expr_kind(&else_expr.kind)
                        ),
                        else_expr.span,
                    );
                    match &else_expr.kind {
                        ExprKind::If(..) => {
                            self.else_if_expr_ids.insert(else_expr.id);
                        }
                        ExprKind::Block(..) => {
                            self.else_block_expr_ids.insert(else_expr.id);
                        }
                        _ => unreachable!("impossible else expr kind"),
                    }
                }
            }
            // enter the `else` block context
            ExprKind::Block(block, _) => {
                if self.else_block_expr_ids.contains(&expr.id) {
                    self.debug_visit(
                        cx,
                        &format!("ENTER ELSE: {} {}", expr.id, block.id),
                        expr.span,
                    );
                    if matches!(
                        self.contexts.last().map(|c| c.kind),
                        Some(ContextKind::Then | ContextKind::ElseIf)
                    ) {
                        self.pop_context_unchecked(cx);
                    }
                    self.push_context(cx, ContextKind::Else, expr.id, expr.span);
                    return;
                }
                self.debug_visit(
                    cx,
                    &format!("ENTER EXPR BLOCK: {} {}", expr.id, block.id),
                    expr.span,
                );
                self.push_context(cx, ContextKind::ExprBlock, expr.id, expr.span);
            }
            _ => {}
        }
    }

    #[inline(always)]
    fn check_expr_post(&mut self, cx: &EarlyContext<'_>, expr: &Expr) {
        if !self.checked_ids.contains(&expr.id) {
            return;
        }

        match &expr.kind {
            // EXIT the `if` or `else-if` block context
            ExprKind::If(_cond, if_block, else_expr) => {
                self.debug_visit(
                    cx,
                    &format!("EXIT IF: {} {}", expr.id, if_block.id),
                    expr.span,
                );
                if let Some(else_expr) = else_expr {
                    self.debug_visit(
                        cx,
                        &format!(
                            "  with ELSE: {} {}",
                            else_expr.id,
                            debug_expr_kind(&else_expr.kind)
                        ),
                        expr.span,
                    );
                }
                while matches!(
                    self.contexts.last().map(|c| c.kind),
                    Some(
                        ContextKind::If
                            | ContextKind::Then
                            | ContextKind::ElseIf
                            | ContextKind::Else
                    )
                ) {
                    let ctx = self.pop_context_unchecked(cx);
                    self.debug_visit(
                        cx,
                        &format!("POP CONTEXT: {} {}", ctx.id, ctx.kind),
                        ctx.span,
                    );
                    if ctx.id == expr.id {
                        break;
                    }
                }
            }
            // EXIT the `else` block context
            ExprKind::Block(block, _) => {
                if self.else_block_expr_ids.contains(&expr.id) {
                    self.debug_visit(
                        cx,
                        &format!("EXIT ELSE: {} {}", expr.id, block.id),
                        expr.span,
                    );
                    self.pop_context(cx, &ContextKind::Else, &expr.id)
                        .expect("pop else context");
                    return;
                }
                self.debug_visit(
                    cx,
                    &format!("EXIT EXPR BLOCK: {} {}", expr.id, block.id),
                    expr.span,
                );
                self.pop_context(cx, &ContextKind::ExprBlock, &expr.id)
                    .expect("pop expr block context");
            }
            _ => {}
        }
    }

    #[inline(always)]
    fn check_trait_item(&mut self, cx: &EarlyContext<'_>, _: &AssocItem) {
        // println!("CHECK TRAIT ITEM");
    }

    #[inline(always)]
    fn check_trait_item_post(&mut self, cx: &EarlyContext<'_>, _: &AssocItem) {
        // println!("CHECK TRAIT ITEM POST");
    }
}

#[test]
fn ui() {
    dylint_uitesting::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
