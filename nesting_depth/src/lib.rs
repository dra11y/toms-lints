#![allow(unused)]
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
use rustc_ast::{Arm, AssocItem, Crate, Expr, ExprKind, Inline, Item, ItemKind, ModKind, NodeId};
use rustc_lint::{EarlyContext, EarlyLintPass, Level, LintContext};
use rustc_span::{ExpnKind, Span};
use std::collections::HashSet;

/// Lint for detecting nesting that is too deep
pub struct NestingDepth {
    config: Config,
    contexts: Vec<Context>,
    lints: Vec<NestingLint>,
    skipped_macro_ids: HashSet<NodeId>,
    checked_ids: HashSet<NodeId>,
    /// Call site spans (macro invocation spans) for ignored macros. Any node whose span
    /// is fully contained inside one of these will be skipped, even if its span is not
    /// marked as coming from an expansion (e.g. tokens originating from macro input).
    ignored_macro_call_sites: Vec<Span>,
    else_if_expr_ids: HashSet<NodeId>,
    else_block_expr_ids: HashSet<NodeId>,
    current_nesting_lint: Option<NestingLint>,
    closure_ids: HashSet<NodeId>,
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
            ignored_macro_call_sites: vec![],
            else_if_expr_ids: HashSet::new(),
            else_block_expr_ids: HashSet::new(),
            closure_ids: HashSet::new(),
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
    /// Returns true if the span (or any of its parent expansions) originates from one of the
    /// configured ignored macros. Matching is performed against the macro's local expansion
    /// name (identifier as written at call site after any `use as` rename).
    fn span_in_ignored_macro(&mut self, span: Span) -> bool {
        if self.config.ignore_macros.is_empty() {
            return false;
        }
        // Quick path: if span not from expansion, it still might be inside a macro invocation's
        // call site span (macro input tokens). Check containment first.
        if self.span_within_ignored_callsite(span) {
            return true;
        }
        if !span.from_expansion() {
            return false;
        }
        // Walk outward via successive call_site spans until we leave expansion chain.
        let mut cur = span;
        while cur.from_expansion() {
            let data = cur.ctxt().outer_expn_data();
            if let ExpnKind::Macro(_, name) = data.kind {
                let macro_name = name.as_str();
                if self.config.ignore_macros.iter().any(|m| m == macro_name) {
                    // Record call site span (invocation) so that any non-expansion spans inside
                    // the macro input are also skipped later.
                    let call_site = data.call_site;
                    if !self
                        .ignored_macro_call_sites
                        .iter()
                        .any(|s| s.lo() == call_site.lo() && s.hi() == call_site.hi())
                    {
                        self.ignored_macro_call_sites.push(call_site);
                    }
                    return true;
                }
            }
            let call_site = data.call_site;
            if call_site == cur || !call_site.from_expansion() {
                break;
            }
            cur = call_site;
        }
        false
    }

    fn span_within_ignored_callsite(&self, span: Span) -> bool {
        self.ignored_macro_call_sites.iter().any(|site| {
            // simple containment check on byte positions
            span.lo() >= site.lo() && span.hi() <= site.hi()
        })
    }
    fn depth(&self) -> usize {
        self.contexts
            .iter()
            .skip(1)
            .filter(|c| c.kind.count_depth(&self.config))
            .count()
    }

    fn push_context(&mut self, cx: &EarlyContext<'_>, kind: ContextKind, id: NodeId, span: Span) {
        let ctx = Context::new(kind.clone(), id, span);
        self.contexts.push(ctx);
        self.debug_visit(cx, &format!("PUSH CONTEXT: {id} {kind}"), span);

        let depth = self.depth();
        if depth <= self.config.max_depth {
            return;
        }

        let outer_span = self.contexts.get(1).map(|ctx| ctx.span);

        let lint = self.current_nesting_lint.get_or_insert(NestingLint {
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

        if ctx.consec_if_branch_count > self.config.max_consec_if_else {
            let outer_span = self.contexts.get(1).map(|ctx| ctx.span);
            self.lints.push(NestingLint {
                outer_span,
                span: ctx.span,
                kind: ContextKind::If,
                reason: Reason::ConsecIfElse(ctx.consec_if_branch_count),
            });
        }
    }

    fn find_root_if_parent(&mut self) -> Option<&mut Context> {
        let mut iter = self.contexts.iter_mut().rev();
        iter.reduce(|mut acc, ctx| {
            if ctx.kind.is_if_or_if_branch() {
                acc = ctx;
            }
            acc
        })
    }

    fn pop_context_unchecked(&mut self, cx: &EarlyContext<'_>) -> Context {
        let mut ctx = self.contexts.pop().expect("pop context unchecked");

        self.debug_visit(
            cx,
            &format!("POP CONTEXT: {} {}", ctx.id, ctx.kind),
            ctx.span,
        );

        if ctx.kind.is_if_branch() {
            if let Some(if_parent) = self.find_root_if_parent() {
                match ctx.kind {
                    ContextKind::If => {
                        // if_parent.consec_if_else_count += 1;
                    }
                    ContextKind::Then | ContextKind::ElseIf | ContextKind::Else => {
                        // These can only exist within a ContextKind::If,
                        // and are automatically "reset" when the current If is popped.
                        if_parent.consec_if_branch_count += 1;
                    }
                    _ => {
                        // if_parent.consec_if_else_count = 0;
                    }
                }
            } else {
                panic!("DID NOT FIND IF PARENT: {} {}", ctx.kind, ctx.id);
            }
        }

        self.push_current_lints(cx, &mut ctx);

        ctx
    }

    fn pop_context(&mut self, cx: &EarlyContext<'_>, id: &NodeId) -> Result<(), anyhow::Error> {
        let mut ctx = self.pop_context_unchecked(cx);

        if ctx.id != *id {
            bail!(
                "pop context id mismatch: kind: {}, expected {id}, got {}",
                ctx.kind,
                ctx.id
            );
        }

        Ok(())
    }

    /// Returns `true` if the node is not from a macro expansion and can be checked
    fn should_check_id(&mut self, cx: &EarlyContext<'_>, id: NodeId, span: Span) -> bool {
        if cx.get_lint_level(NESTING_DEPTH).level == Level::Allow {
            return false;
        }
        if self.checked_ids.contains(&id) {
            return true;
        }
        if self.skipped_macro_ids.contains(&id) {
            return false;
        }
        // Ignore nodes whose spans originate from an ignored macro expansion.
        if self.span_in_ignored_macro(span) {
            self.skipped_macro_ids.insert(id);
            return false;
        }
        // Also skip if span lies within any previously recorded ignored macro call site.
        if self.span_within_ignored_callsite(span) {
            self.skipped_macro_ids.insert(id);
            return false;
        }
        if span.ctxt().in_external_macro(cx.sess().source_map()) {
            self.skipped_macro_ids.insert(id);
            return false;
        }
        self.checked_ids.insert(id);
        true
    }

    fn item_kind(&mut self, cx: &EarlyContext<'_>, item: &Item) -> Option<ContextKind> {
        match &item.kind {
            ItemKind::Fn(_) => Some(ContextKind::Func),
            ItemKind::Mod(_, _, ModKind::Loaded(_, Inline::Yes, _)) => Some(ContextKind::Mod),
            ItemKind::Trait(_) => Some(ContextKind::Trait),
            ItemKind::Impl(_) => Some(ContextKind::Impl),
            _ => None,
        }
        .filter(|_| self.should_check_id(cx, item.id, item.span))
    }
}

impl EarlyLintPass for NestingDepth {
    #[inline(always)]
    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _krate: &Crate) {
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
        let Some(kind) = self.item_kind(cx, item) else {
            return;
        };

        self.push_context(cx, kind, item.id, item.span);
        self.debug_visit_extra(cx, "ENTER item", item.span, item.kind.descr());
    }

    #[inline(always)]
    fn check_item_post(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if !self.checked_ids.contains(&item.id) {
            return;
        }

        self.debug_visit_extra(cx, "EXIT item", item.span, item.kind.descr());
        self.pop_context(cx, &item.id).expect("pop item context");
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
                self.push_context(cx, kind, if_or_else_if_block.id, expr.span);
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
            ExprKind::Closure(closure) => {
                self.closure_ids.insert(closure.body.id);
                self.debug_visit(
                    cx,
                    &format!(
                        "ENTER CLOSURE: {} {}",
                        closure.body.id,
                        debug_expr_kind(&closure.body.kind)
                    ),
                    expr.span,
                );
            }
            ExprKind::Block(block, _) => {
                if self.else_block_expr_ids.contains(&expr.id) {
                    // entered `else` block context
                    // branch wrappers (ContextKind::If) are only popped in post ExprKind::If
                    // DO NOT pop context here.
                    self.debug_visit(
                        cx,
                        &format!("ENTER ELSE: {} {}", expr.id, block.id),
                        expr.span,
                    );
                    self.push_context(cx, ContextKind::Else, expr.id, expr.span);
                    return;
                }
                if block.stmts.is_empty() {
                    return;
                }
                if self.closure_ids.contains(&expr.id) {
                    self.debug_visit(
                        cx,
                        &format!("ENTER CLOSURE BLOCK: {} {}", expr.id, block.id),
                        expr.span,
                    );
                    self.push_context(cx, ContextKind::Closure, expr.id, expr.span);
                    return;
                }
                self.debug_visit(
                    cx,
                    &format!("ENTER EXPR BLOCK: {} {}", expr.id, block.id),
                    expr.span,
                );
                self.push_context(cx, ContextKind::ExprBlock, expr.id, expr.span);
            }
            ExprKind::Match(..) => {
                self.debug_visit(cx, &format!("ENTER MATCH: {}", expr.id), expr.span);
                self.push_context(cx, ContextKind::Match, expr.id, expr.span);
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
                    if ctx.id == expr.id {
                        break;
                    }
                }
            }
            ExprKind::Block(block, _) => {
                if self.else_block_expr_ids.contains(&expr.id) {
                    // EXIT the `else` block context
                    self.debug_visit(
                        cx,
                        &format!("EXIT ELSE: {} {}", expr.id, block.id),
                        expr.span,
                    );
                    self.pop_context(cx, &expr.id).expect("pop else context");
                    return;
                }
                if block.stmts.is_empty() {
                    return;
                }
                self.debug_visit(
                    cx,
                    &format!("EXIT EXPR BLOCK: {} {}", expr.id, block.id),
                    expr.span,
                );
                self.pop_context(cx, &expr.id)
                    .expect("pop expr block context");
            }
            ExprKind::Match(..) => {
                self.debug_visit(cx, &format!("EXIT MATCH: {}", expr.id), expr.span);
                self.pop_context(cx, &expr.id).expect("pop match context");
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
