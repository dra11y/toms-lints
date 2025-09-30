#![allow(unused)]
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

mod debug;

extern crate rustc_ast;
extern crate rustc_span;

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use anyhow::{Context as _, bail};
use debug::{SpanInfo, debug_expr_kind, debug_span};
use dylint_linting::config_or_default;
use rustc_ast::{
    Arm, AssocItem, AssocItemKind, Block, Crate, Expr, ExprKind, HasNodeId, Item, ItemKind,
    LocalKind, ModKind, NodeId, StaticItem, Stmt, StmtKind,
    visit::{FnKind, Visitor},
};
use rustc_lint::{EarlyContext, EarlyLintPass, Level, LintContext};
use rustc_span::{ExpnKind, FileNameDisplayPreference, Span, source_map::SourceMap};
use serde::Deserialize;
use serde_inline_default::serde_inline_default;

/// Default maximum nesting levels
const DEFAULT_MAX_DEPTH: usize = 3;

/// Default maximum items in an if-block
const DEFAULT_MAX_ITEMS: usize = 10;

/// Default maximum consecutive if-else statements
const DEFAULT_MAX_CONSEC_IF_ELSE: usize = 3;

const HELP_MESSAGE: &str = "use early returns and guard clauses to reduce nesting";

const DEFAULT_DEBUG: bool = cfg!(debug_assertions);

/// Lint configuration
#[serde_inline_default]
#[derive(Deserialize)]
struct Config {
    #[serde_inline_default(DEFAULT_MAX_DEPTH)]
    max_depth: usize,
    #[serde_inline_default(DEFAULT_MAX_ITEMS)]
    max_items: usize,
    #[serde_inline_default(DEFAULT_MAX_CONSEC_IF_ELSE)]
    max_consec_if_else: usize,
    #[serde_inline_default(DEFAULT_DEBUG)]
    debug: bool,
    #[serde(default)]
    debug_span_info: Option<SpanInfo>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
            max_items: DEFAULT_MAX_ITEMS,
            max_consec_if_else: DEFAULT_MAX_CONSEC_IF_ELSE,
            debug: DEFAULT_DEBUG,
            debug_span_info: None,
        }
    }
}

/// Lint for detecting nesting that is too deep
pub struct NestingDepth {
    config: Config,
    contexts: Vec<Context>,
    lints: Vec<Lint>,
    skipped_macro_ids: HashSet<NodeId>,
    checked_ids: HashSet<NodeId>,
    /// Map of block id to (IfBranchKind, outer if expr span)
    if_else_blocks: HashMap<NodeId, (IfBranchKind, Span)>,
    current_nesting_lint: Option<Lint>,
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
            if_else_blocks: HashMap::new(),
            current_nesting_lint: None,
            inside_fn: false,
        }
    }
}

const DESCRIPTION: &str = "excessive nesting";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum IfBranchKind {
    If,
    ElseIf,
    Else,
}

impl IfBranchKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            IfBranchKind::If => "if",
            IfBranchKind::ElseIf => "else if",
            IfBranchKind::Else => "else",
        }
    }
}

impl std::fmt::Display for IfBranchKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ContextKind {
    Item,
    Func,
    If(IfBranchKind, Span),
    MatchArm,
    Block,
    While,
    For,
    Loop,
}

impl std::fmt::Display for ContextKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.descr())
    }
}

impl ContextKind {
    fn count_depth(&self) -> bool {
        // / !matches!(self, ContextKind::Match | ContextKind::Item(..))
        true
    }

    fn descr(&self) -> &'static str {
        match self {
            ContextKind::Item => "item",
            ContextKind::Func => "func",
            ContextKind::If(IfBranchKind::If, _) => "if",
            ContextKind::If(IfBranchKind::Else, _) => "else",
            ContextKind::If(IfBranchKind::ElseIf, _) => "else-if",
            ContextKind::MatchArm => "match-arm",
            ContextKind::Block => "block",
            ContextKind::While => "while",
            ContextKind::For => "for",
            ContextKind::Loop => "loop",
        }
    }
}

#[derive(Clone)]
struct Context {
    span: Span,
    id: NodeId,
    kind: ContextKind,
    consec_if_else_span: Option<Span>,
    consec_if_else_count: usize,
    consec_if_else_lint: Option<Lint>,
}

impl Context {
    fn count_depth(&self) -> bool {
        self.kind.count_depth()
    }
}

impl Context {
    fn new(kind: ContextKind, id: NodeId, span: Span) -> Self {
        Self {
            span,
            kind,
            id,
            consec_if_else_span: None,
            consec_if_else_count: 0,
            consec_if_else_lint: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Reason {
    Depth(usize),
    ConsecIfElse(usize),
}

impl Reason {
    fn outer_context_label(&self) -> &'static str {
        match self {
            Reason::Depth(_) => "outer nested context",
            Reason::ConsecIfElse(_) => "first if in sequence",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Reason::Depth(_) => "nesting depth",
            Reason::ConsecIfElse(_) => "consecutive if-else statements",
        }
    }

    fn message(&self, config: &Config) -> String {
        let label = self.label();
        match self {
            Reason::Depth(depth) => {
                let max_1 = config.max_depth + 1;
                let levels_desc = if *depth > max_1 {
                    format!("{max_1} to {depth} levels")
                } else {
                    format!("{depth} levels")
                };
                format!(
                    "{label}: {max} max allowed, {levels_desc} found",
                    max = config.max_depth,
                )
            }
            Reason::ConsecIfElse(count) => {
                format!(
                    "{label}: {max} max allowed, {count} found",
                    max = config.max_consec_if_else,
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Lint {
    outer_span: Option<Span>,
    span: Span,
    kind: ContextKind,
    reason: Reason,
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

        let debug_str = format!(
            "{}[{depth:2}] {} {}",
            "  ".repeat(depth),
            kind.descr(),
            debug_span(span, source_map)
        );

        if depth <= self.config.max_depth {
            return;
        }

        let outer_span = self.contexts.get(1).map(|ctx| ctx.span);

        let mut lint = self.current_nesting_lint.get_or_insert(Lint {
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

    fn pop_context(
        &mut self,
        cx: &EarlyContext<'_>,
        kind: &ContextKind,
        id: &NodeId,
    ) -> Result<(), anyhow::Error> {
        let depth = self.depth();
        let mut ctx = self.contexts.pop().context("No context exists")?;

        if ctx.id != *id {
            bail!("pop context id mismatch: expected {id}, got {}", ctx.id);
        }

        if ctx.kind != *kind {
            bail!(
                "pop context kind mismatch: expected {kind}, got {}",
                ctx.kind
            );
        }

        self.push_current_lints(cx, &mut ctx);

        Ok(())
    }

    fn enter_if_expr(
        &mut self,
        cx: &EarlyContext<'_>,
        expr: &Expr,
        if_or_else_if_block: &Block,
        else_expr: Option<&Expr>,
    ) {
        {
            // mark `if` block, iff not already marked as `else if`.
            let (kind, _) = *self
                .if_else_blocks
                .entry(if_or_else_if_block.id)
                .or_insert((IfBranchKind::If, expr.span));
            self.push_context(
                cx,
                ContextKind::If(kind, expr.span),
                if_or_else_if_block.id,
                if_or_else_if_block.span,
            );
            self.debug_visit_extra(
                cx,
                &format!("ENTER {kind} {}", self.debug_span(cx, expr.span)),
                if_or_else_if_block.span,
                &if_or_else_if_block.id.to_string(),
            );
        }

        if let Some(else_expr) = else_expr {
            let (id, kind) = match &else_expr.kind {
                ExprKind::If(_expr, else_if_block, _) => (else_if_block.id, IfBranchKind::ElseIf),
                ExprKind::Block(else_block, _) => (else_block.id, IfBranchKind::Else),
                _ => unreachable!("else expr must be ExprKind::If or ExprKind::Block"),
            };
            self.if_else_blocks.insert(id, (kind, expr.span));
        }
    }

    fn did_enter_if_block(&mut self, cx: &EarlyContext<'_>, expr: &Expr, block: &Block) -> bool {
        let Some((last_context_id, ContextKind::If(last_if_branch_kind, if_expr_span), last_span)) =
            self.contexts.last().map(|c| (c.id, c.kind, c.span))
        else {
            return false;
        };

        let Some((if_branch_kind, span)) = self.if_else_blocks.get(&block.id).copied() else {
            return false;
        };

        self.if_else_blocks.remove(&last_context_id);

        self.debug_visit_extra(
            cx,
            &format!("EXIT IF FOR ELSE: {last_if_branch_kind}"),
            last_span,
            &last_context_id.to_string(),
        );
        self.pop_context(
            cx,
            &ContextKind::If(last_if_branch_kind, if_expr_span),
            &last_context_id,
        );
        self.push_context(
            cx,
            ContextKind::If(if_branch_kind, if_expr_span),
            block.id,
            expr.span,
        );
        self.debug_visit_extra(
            cx,
            &format!("ENTER BLOCK {if_branch_kind}"),
            block.span,
            &block.id.to_string(),
        );

        true
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
        self.pop_context(cx, &ContextKind::Item, &item.id);
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

        match &expr.kind {
            // enter the `if` or `else-if` block context
            ExprKind::If(_cond, if_or_else_if_block, else_expr) => {
                self.enter_if_expr(cx, expr, if_or_else_if_block, else_expr.as_deref());
            }
            // enter the `else` block context
            ExprKind::Block(block, _) => {
                if self.did_enter_if_block(cx, expr, block) {
                    return;
                }
                self.push_context(cx, ContextKind::Block, block.id, block.span);
                self.debug_visit(cx, "ENTER block", block.span);
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
                // TODO: exit the if only if needed
            }
            // EXIT the `else` block context
            ExprKind::Block(block, _) => {
                if let Some((if_branch_kind, span)) = self.if_else_blocks.remove(&block.id) {
                    self.debug_visit_extra(
                        cx,
                        &format!("EXIT {if_branch_kind}"),
                        block.span,
                        &block.id.to_string(),
                    );
                    self.pop_context(cx, &ContextKind::If(if_branch_kind, block.span), &block.id);
                    return;
                }

                self.debug_visit(cx, "ENTER block", block.span);
                self.pop_context(cx, &ContextKind::Block, &block.id);
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
