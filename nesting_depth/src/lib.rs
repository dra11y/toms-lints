#![allow(unused)]
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

mod debug;

extern crate rustc_ast;
extern crate rustc_span;

use std::{cmp::Ordering, collections::HashSet};

use debug::{SpanInfo, debug_expr_kind, debug_span};
use dylint_linting::config_or_default;
use rustc_ast::{
    Arm, AssocItem, AssocItemKind, Block, Crate, Expr, ExprKind, HasNodeId, Item, ItemKind,
    LocalKind, ModKind, NodeId, Stmt, StmtKind,
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
    current_nesting_lint: Option<Lint>,
    inside_fn: bool,
}

impl Default for NestingDepth {
    fn default() -> Self {
        Self {
            config: config_or_default(env!("CARGO_PKG_NAME")),
            contexts: vec![],
            lints: vec![],
            current_nesting_lint: None,
            inside_fn: false,
        }
    }
}

const DESCRIPTION: &str = "excessive nesting";

dylint_linting::impl_pre_expansion_lint! {
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

#[derive(Debug, Clone)]
enum ContextKind {
    Item(ItemKind),
    Expr(ExprKind),
    If,
    Else,
    Match,
    Block,
    BlockExpr(Box<Block>),
    While,
    For,
    Loop,
}

impl ContextKind {
    fn count_depth(&self) -> bool {
        !matches!(self, ContextKind::Match | ContextKind::Item(..))
    }

    fn descr(&self) -> &'static str {
        match self {
            ContextKind::Item(kind) => kind.descr(),
            ContextKind::Expr(kind) => debug_expr_kind(kind),
            ContextKind::If => "if",
            ContextKind::Else => "else",
            ContextKind::Match => "match",
            ContextKind::Block => "block",
            ContextKind::BlockExpr(block) => "block expr",
            ContextKind::While => "while",
            ContextKind::For => "for",
            ContextKind::Loop => "loop",
        }
    }
}

impl PartialEq for ContextKind {
    fn eq(&self, other: &Self) -> bool {
        matches!(self, other)
    }
}

#[derive(Clone)]
struct Context {
    span: Span,
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
    fn new(kind: ContextKind, span: Span) -> Self {
        Self {
            span,
            kind,
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

    fn push_context(&mut self, cx: &EarlyContext<'_>, kind: ContextKind, span: Span) {
        let source_map = cx.sess().source_map();
        let ctx = Context::new(kind.clone(), span);
        self.contexts.push(ctx);
        let depth = self.depth();

        let debug_str = format!(
            "{}[{depth:2}] {} {}",
            "  ".repeat(depth),
            kind.descr(),
            debug_span(span, source_map)
        );
        println!("push {debug_str}");

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

    fn replace_context_kind(&mut self, cx: &EarlyContext<'_>, kind: ContextKind) {
        if let Some(mut ctx) = self.contexts.last_mut() {
            println!("REPLACE KIND: {} -> {}", ctx.kind.descr(), kind.descr());
            *ctx = Context {
                kind,
                ..ctx.clone()
            }
        }
    }

    fn pop_context(&mut self, cx: &EarlyContext<'_>, kind: &ContextKind) {
        let depth = self.depth();
        let Some(mut ctx) = self.contexts.pop() else {
            return;
        };

        // // Just pop Block because check_block_post doesn't exist.
        // let mut ctx = if matches!(ctx.kind, ContextKind::Block) {
        //     let Some(ctx) = self.contexts.pop() else {
        //         return;
        //     };
        //     ctx
        // } else {
        //     ctx
        // };

        if !matches!(&ctx.kind, kind) {
            eprintln!(
                "MISMATCH ITEM CONTEXT: item kind {} vs context kind {}",
                ctx.kind.descr(),
                kind.descr()
            );
        }

        if let Some(lint) = self.current_nesting_lint.take() {
            self.debug_visit(cx, "pop_context add nesting lint", ctx.span);
            self.lints.push(lint);
        }

        if let Some(lint) = ctx.consec_if_else_lint.take() {
            self.debug_visit(cx, "pop_context add if/else lint", ctx.span);
            self.lints.push(lint);
        }
    }
}

fn should_check_item(item: &Item) -> bool {
    matches!(
        item.kind,
        // ItemKind::Static(_) |
        ItemKind::Fn(..)
            | ItemKind::Mod(_, _, ModKind::Loaded(..))
            | ItemKind::Trait(..)
            | ItemKind::Impl(..)
    )
}

fn expr_context_kind(expr: &Expr, post: bool, last_ctx: Option<&Context>) -> Option<ContextKind> {
    let last_is_if = last_ctx.is_some_and(|c| matches!(c.kind, ContextKind::If));
    match &expr.kind {
        ExprKind::If(expr, block, expr1) => Some(ContextKind::If),
        ExprKind::Match(expr, arms, match_kind) => Some(ContextKind::Match),
        ExprKind::Block(..) if last_is_if => Some(ContextKind::Else),
        ExprKind::Block(body, _) => Some(ContextKind::BlockExpr(body.clone())),
        ExprKind::While(_, body, _) => Some(ContextKind::While),
        ExprKind::ForLoop { body, .. } => Some(ContextKind::For),
        ExprKind::Loop(body, _, _) => Some(ContextKind::Loop),
        ExprKind::Gen(_, body, _, _) | ExprKind::TryBlock(body) => {
            Some(ContextKind::BlockExpr(body.clone()))
        }
        ExprKind::Array(..) => None,
        ExprKind::ConstBlock(..) => None,
        ExprKind::Call(..) => None,
        ExprKind::MethodCall(..) => None,
        ExprKind::Tup(..) => None,
        ExprKind::Binary(..) => None,
        ExprKind::Unary(..) => None,
        ExprKind::Lit(..) => None,
        ExprKind::Cast(..) => None,
        ExprKind::Type(..) => None,
        ExprKind::Let(..) => None,
        ExprKind::Closure(..) => None,
        ExprKind::Await(..) => None,
        ExprKind::Use(..) => None,
        ExprKind::Assign(..) => None,
        ExprKind::AssignOp(..) => None,
        ExprKind::Field(..) => None,
        ExprKind::Index(..) => None,
        ExprKind::Range(..) => None,
        ExprKind::Underscore => None,
        ExprKind::Path(..) => None,
        ExprKind::AddrOf(..) => None,
        ExprKind::Break(..) => None,
        ExprKind::Continue(..) => None,
        ExprKind::Ret(..) => None,
        ExprKind::InlineAsm(..) => None,
        ExprKind::OffsetOf(..) => None,
        ExprKind::MacCall(..) => None,
        ExprKind::Struct(..) => None,
        ExprKind::Repeat(..) => None,
        ExprKind::Paren(..) => None,
        ExprKind::Try(..) => None,
        ExprKind::Yield(..) => None,
        ExprKind::Yeet(..) => None,
        ExprKind::Become(..) => None,
        ExprKind::IncludedBytes(..) => None,
        ExprKind::FormatArgs(..) => None,
        ExprKind::UnsafeBinderCast(..) => None,
        ExprKind::Err(..) => None,
        ExprKind::Dummy => None,
    }
}

// _ => should_check_expr(expr).then(|| ContextKind::Expr(expr.kind.clone())),

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
        if !should_check_item(item) {
            return;
        }

        self.push_context(cx, ContextKind::Item(item.kind.clone()), item.span);
        self.debug_visit_extra(cx, "check_item", item.span, item.kind.descr());
    }

    #[inline(always)]
    fn check_item_post(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if !should_check_item(item) {
            return;
        }

        self.debug_visit_extra(cx, "check_item_post", item.span, item.kind.descr());
        self.pop_context(cx, &ContextKind::Item(item.kind.clone()));
    }

    #[inline(always)]
    fn check_arm(&mut self, cx: &EarlyContext<'_>, arm: &Arm) {
        // println!("CHECK ARM");
    }

    // #[inline(always)]
    // fn check_block(&mut self, cx: &EarlyContext<'_>, b: &rustc_ast::Block) {
    //     if self
    //         .contexts
    //         .last()
    //         .is_none_or(|c| !matches!(c.kind, ContextKind::If | ContextKind::Else))
    //     {
    //         self.push_context(cx, ContextKind::Block, b.span);
    //     }
    //     self.debug_visit(cx, "check_block", b.span);
    // }

    #[inline(always)]
    fn check_expr(&mut self, cx: &EarlyContext<'_>, expr: &Expr) {
        // self.debug_visit_extra(
        //     cx,
        //     "check_expr start",
        //     expr.span,
        //     debug_expr_kind(&expr.kind),
        // );

        let Some(kind) = expr_context_kind(expr, false, self.contexts.last()) else {
            return;
        };

        let descr = kind.descr();
        if matches!(kind, ContextKind::Else) {
            self.contexts.pop();
            self.replace_context_kind(cx, ContextKind::Else);
        } else {
            self.push_context(cx, kind, expr.span);
        }

        self.debug_visit_extra(cx, "check_expr", expr.span, descr);
    }

    #[inline(always)]
    fn check_expr_post(&mut self, cx: &EarlyContext<'_>, expr: &Expr) {
        let Some(kind) = expr_context_kind(expr, true, self.contexts.last()) else {
            return;
        };

        let descr = kind.descr();
        self.debug_visit_extra(cx, "check_expr_post", expr.span, descr);
        self.pop_context(cx, &kind);
    }

    #[inline(always)]
    fn check_trait_item(&mut self, cx: &EarlyContext<'_>, _: &AssocItem) {
        println!("CHECK TRAIT ITEM");
    }

    #[inline(always)]
    fn check_trait_item_post(&mut self, cx: &EarlyContext<'_>, _: &AssocItem) {
        println!("CHECK TRAIT ITEM POST");
    }
}

#[test]
fn ui() {
    dylint_uitesting::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
