#![allow(unused)]
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_span;

use std::{cmp::Ordering, collections::HashSet};

use dylint_linting::config_or_default;
use rustc_ast::{
    AssocItemKind, Block, Crate, Expr, ExprKind, Item, ItemKind, LocalKind, ModKind, NodeId, Stmt,
    StmtKind,
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

const HELP_MESSAGE: &str = "use early returns and guard clauses to reduce nesting";

/// Lint configuration
#[serde_inline_default]
#[derive(Deserialize)]
struct Config {
    #[serde_inline_default(DEFAULT_MAX_DEPTH)]
    max_depth: usize,
    #[serde_inline_default(DEFAULT_MAX_ITEMS)]
    max_items: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
            max_items: DEFAULT_MAX_ITEMS,
        }
    }
}

/// Lint for detecting nesting that is too deep
pub struct NestingDepth {
    config: Config,
}

impl Default for NestingDepth {
    fn default() -> Self {
        Self {
            config: config_or_default(env!("CARGO_PKG_NAME")),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ContextKind {
    Func,
    Closure,
    Static,

    If,
    Else,
    Let,
    Loop,
    While,
    For,
    Match,
}

struct Context<'a> {
    span: Span,
    kind: ContextKind,
    source_map: &'a SourceMap,
}

impl<'a> Context<'a> {
    fn new(kind: ContextKind, span: Span, source_map: &'a SourceMap) -> Self {
        Self {
            span,
            kind,
            source_map,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Reason {
    Depth(usize),
}

impl Reason {
    fn label(&self) -> &'static str {
        match self {
            Reason::Depth(_) => "nesting depth",
        }
    }

    fn message(&self, config: &Config) -> String {
        let label = self.label();
        match self {
            Reason::Depth(depth) => format!(
                "{label}: {max} max allowed, reaches {max_1} to {depth} levels",
                max = config.max_depth,
                max_1 = config.max_depth + 1
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Lint {
    span: Span,
    kind: ContextKind,
    reason: Reason,
}

struct NestingDepthVisitor<'a> {
    config: &'a Config,
    contexts: Vec<Context<'a>>,
    source_map: &'a SourceMap,
    lints: Vec<Lint>,
    current_lint: Option<Lint>,
    inside_fn: bool,
}

impl<'a> Visitor<'a> for NestingDepthVisitor<'a> {
    type Result = ();

    fn visit_block(&mut self, block: &'a Block) -> Self::Result {
        for stmt in &block.stmts {
            self.visit_stmt(stmt);
        }
    }

    fn visit_crate(&mut self, krate: &'a Crate) -> Self::Result {
        for item in &krate.items {
            self.visit_item(item);
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) -> Self::Result {
        let depth = self.depth();

        match &expr.kind {
            ExprKind::Let(_pat, let_expr, _span, _recovered) => {
                self.push_context(ContextKind::Let, expr.span);
                self.visit_expr(let_expr);
                self.pop_context();
            }
            ExprKind::If(_if_expr, _if_block, _else_expr) => {
                self.process_if(expr);
            }
            ExprKind::While(while_expr, block, label) => {}
            ExprKind::ForLoop {
                pat,
                iter,
                body,
                label,
                kind,
            } => {
                self.push_context(ContextKind::For, expr.span);
                self.visit_block(body);
                self.pop_context();
            }
            ExprKind::Loop(block, _label, _span) => {
                self.push_context(ContextKind::Loop, expr.span);
                self.visit_block(block);
                self.pop_context();
            }
            ExprKind::Match(_match_expr, arms, _match_kind) => {
                for arm in arms {
                    if let Some(body) = &arm.body {
                        self.push_context(ContextKind::Match, body.span);
                        self.visit_expr(body);
                        self.pop_context();
                    }
                }
            }
            ExprKind::Closure(closure) => {
                self.push_context(ContextKind::Closure, closure.body.span);
                self.visit_expr(&closure.body);
            }
            ExprKind::Block(block, label) => {
                self.visit_block(block);
            }
            ExprKind::Gen(_capture_by, block, _gen_block_kind, _span) => {
                self.visit_block(block);
            }
            ExprKind::TryBlock(block) => {
                self.visit_block(block);
            }
            ExprKind::Call(call_expr, _) => {
                self.visit_expr(call_expr);
            }
            _kind => {}
        }
    }

    fn visit_item(&mut self, item: &'a Item) -> Self::Result {
        match &item.kind {
            ItemKind::Static(static_item) => {
                if let Some(expr) = &static_item.expr {
                    self.push_context(ContextKind::Static, item.span);
                    self.visit_expr(expr);
                    self.pop_context();
                }
            }
            ItemKind::Fn(func) => self.process_fn(func, item.span),
            ItemKind::Mod(_, _, ModKind::Loaded(items, _, span)) => {
                for item in items {
                    self.visit_item(item);
                }
            }
            ItemKind::Trait(tr) => {
                for item in &tr.items {
                    if let AssocItemKind::Fn(func) = &item.kind {
                        self.process_fn(func, item.span);
                    }
                }
            }
            ItemKind::Impl(imp) => {
                for item in &imp.items {
                    if let AssocItemKind::Fn(func) = &item.kind {
                        self.process_fn(func, item.span);
                    }
                }
            }
            _ => {}
        }
    }

    fn visit_stmt(&mut self, stmt: &'a Stmt) -> Self::Result {
        match &stmt.kind {
            StmtKind::Let(local) => match &local.kind {
                LocalKind::Decl => {}
                LocalKind::Init(expr) => self.visit_expr(expr),
                LocalKind::InitElse(expr, block) => {
                    self.visit_expr(expr);
                    self.visit_block(block);
                }
            },
            StmtKind::Item(item) => self.visit_item(item),
            StmtKind::Expr(expr) | StmtKind::Semi(expr) => self.visit_expr(expr),
            _ => {}
        }
    }
}

impl<'a> NestingDepthVisitor<'a> {
    fn new(config: &'a Config, source_map: &'a SourceMap) -> Self {
        Self {
            config,
            source_map,
            contexts: vec![],
            lints: vec![],
            current_lint: None,
            inside_fn: false,
        }
    }

    fn depth(&self) -> usize {
        self.contexts.len()
    }

    fn debug_span(&self, span: Span) -> String {
        debug_span(span, self.source_map)
    }

    fn debug_code(&self, span: Span) -> String {
        self.source_map.span_to_snippet(span).unwrap_or_default()
    }

    fn push_context(&mut self, kind: ContextKind, span: Span) {
        let ctx = Context::new(kind, span, self.source_map);
        let Context { span, kind, .. } = ctx;
        self.contexts.push(ctx);
        let depth = self.depth();
        if depth <= self.config.max_depth {
            return;
        }
        let mut lint = self.current_lint.get_or_insert(Lint {
            span,
            kind,
            reason: Reason::Depth(depth),
        });
        lint.reason = Reason::Depth(depth);
    }

    fn pop_context(&mut self) {
        let depth = self.depth();
        let Some(ctx) = self.contexts.pop() else {
            return;
        };
        if depth > self.config.max_depth {
            return;
        }
        if let Some(lint) = self.current_lint.take() {
            self.lints.push(lint);
        }
    }

    fn process_if(&mut self, expr: &'a Expr) {
        match &expr.kind {
            ExprKind::If(_, block, else_expr) => {
                self.push_context(ContextKind::If, block.span);
                self.visit_block(block);
                self.pop_context();

                if let Some(else_expr) = else_expr {
                    self.process_if(else_expr);
                }
            }
            ExprKind::Block(block, _) => {
                self.push_context(ContextKind::Else, block.span);
                self.visit_block(block);
                self.pop_context();
            }
            other => unreachable!("else expression is not a block or if: {other:?}"),
        }
    }

    fn process_fn(&mut self, func: &'a rustc_ast::Fn, span: Span) {
        let Some(body) = &func.body else {
            return;
        };
        let was_inside_fn = self.inside_fn;
        if was_inside_fn {
            self.push_context(ContextKind::Func, span);
        }
        self.inside_fn = true;
        self.visit_block(body);
        self.pop_context();
        self.inside_fn = was_inside_fn;
    }
}

impl EarlyLintPass for NestingDepth {
    #[inline(always)]
    fn check_crate(&mut self, cx: &EarlyContext<'_>, cr: &rustc_ast::Crate) {
        let source_map = cx.sess().source_map();
        let mut visitor = NestingDepthVisitor::new(&self.config, source_map);
        visitor.visit_crate(cr);
        for lint in visitor.lints {
            let level = Level::Warn;
            cx.span_lint(NESTING_DEPTH, lint.span, |diag| {
                diag.primary_message(lint.reason.message(&self.config));
                diag.help(HELP_MESSAGE);
            });
        }
    }
}

fn debug_span(span: Span, source_map: &SourceMap) -> String {
    let location = source_map.span_to_location_info(span);
    let file = location
        .0
        .map(|f| {
            f.name
                .display(FileNameDisplayPreference::Remapped)
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_default();
    format!("{file}:{}:{}", location.1, location.2)
}

impl<'a> std::fmt::Debug for Context<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let location = debug_span(self.span, self.source_map);
        write!(f, "{:?} @ {location}", self.kind)
    }
}

const fn debug_expr_kind(kind: &ExprKind) -> &'static str {
    match kind {
        ExprKind::Array(..) => "Array",
        ExprKind::ConstBlock(..) => "ConstBlock",
        ExprKind::Call(..) => "Call",
        ExprKind::MethodCall(..) => "MethodCall",
        ExprKind::Tup(..) => "Tup",
        ExprKind::Binary(..) => "Binary",
        ExprKind::Unary(..) => "Unary",
        ExprKind::Lit(..) => "Lit",
        ExprKind::Cast(..) => "Cast",
        ExprKind::Type(..) => "Type",
        ExprKind::Let(..) => "Let",
        ExprKind::If(..) => "If",
        ExprKind::While(..) => "While",
        ExprKind::ForLoop { .. } => "ForLoop",
        ExprKind::Loop(..) => "Loop",
        ExprKind::Match(expr, thin_vec, ..) => "Match",
        ExprKind::Closure(..) => "Closure",
        ExprKind::Block(block, ..) => "Block",
        ExprKind::Gen(capture_by, block, gen_block_kind, ..) => "Gen",
        ExprKind::Await(expr, ..) => "Await",
        ExprKind::Use(expr, ..) => "Use",
        ExprKind::TryBlock(..) => "TryBlock",
        ExprKind::Assign(expr, expr1, ..) => "Assign",
        ExprKind::AssignOp(spanned, expr, ..) => "AssignOp",
        ExprKind::Field(expr, ..) => "Field",
        ExprKind::Index(expr, expr1, ..) => "Index",
        ExprKind::Range(expr, expr1, ..) => "Range",
        ExprKind::Underscore => "Underscore",
        ExprKind::Path(qself, ..) => "Path",
        ExprKind::AddrOf(borrow_kind, mutability, ..) => "AddrOf",
        ExprKind::Break(label, ..) => "Break",
        ExprKind::Continue(..) => "Continue",
        ExprKind::Ret(..) => "Ret",
        ExprKind::InlineAsm(..) => "InlineAsm",
        ExprKind::OffsetOf(ty, ..) => "OffsetOf",
        ExprKind::MacCall(..) => "MacCall",
        ExprKind::Struct(..) => "Struct",
        ExprKind::Repeat(expr, ..) => "Repeat",
        ExprKind::Paren(..) => "Paren",
        ExprKind::Try(..) => "Try",
        ExprKind::Yield(..) => "Yield",
        ExprKind::Yeet(..) => "Yeet",
        ExprKind::Become(..) => "Become",
        ExprKind::IncludedBytes(..) => "IncludedBytes",
        ExprKind::FormatArgs(..) => "FormatArgs",
        ExprKind::UnsafeBinderCast(unsafe_binder_cast_kind, expr, ..) => "UnsafeBinderCast",
        ExprKind::Err(..) => "Err",
        ExprKind::Dummy => "Dummy",
    }
}

#[test]
fn ui() {
    dylint_uitesting::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
