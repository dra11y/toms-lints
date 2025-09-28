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
    Block,
    Static,
    Mod,
    Trait,
    Impl,

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
    consec_if_else_span: Option<Span>,
    consec_if_else_count: usize,
    consec_if_else_lint: Option<Lint>,
}

impl<'a> Context<'a> {
    fn new(kind: ContextKind, span: Span, source_map: &'a SourceMap) -> Self {
        Self {
            span,
            kind,
            source_map,
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct Lint {
    outer_span: Option<Span>,
    span: Span,
    kind: ContextKind,
    reason: Reason,
}

struct NestingDepthVisitor<'a> {
    config: &'a Config,
    contexts: Vec<Context<'a>>,
    source_map: &'a SourceMap,
    lints: Vec<Lint>,
    current_nesting_lint: Option<Lint>,
    inside_fn: bool,
}

impl<'a> NestingDepthVisitor<'a> {
    fn debug_visit(&self, method: &str, span: Span) {
        if !self.config.debug {
            return;
        }

        self.debug_visit_with(method, span, false, None);
    }

    fn debug_visit_with(&self, method: &str, span: Span, code: bool, extra: Option<&str>) {
        if !self.config.debug {
            return;
        }
        if self
            .config
            .debug_span_info
            .as_ref()
            .is_some_and(|s| !s.contains(&self.debug_span_info(span)))
        {
            return;
        }
        let info = self.debug_span_info(span);
        let code = code.then(|| self.debug_code(span));
        let depth = self.depth();
        let span = self.debug_span(span);
        let mut debug_str = String::new();
        for _ in 1..=depth {
            debug_str.push_str("  ");
        }
        debug_str.push_str(method);
        debug_str.push_str(" [");
        debug_str.push_str(&depth.to_string());
        debug_str.push(']');
        debug_str.push_str(" [");
        debug_str.push_str(&span);
        debug_str.push(']');
        if let Some(code) = code {
            debug_str.push_str(&code);
        }
        println!("{debug_str}");
    }
}

impl<'a> Visitor<'a> for NestingDepthVisitor<'a> {
    type Result = ();

    fn visit_block(&mut self, block: &'a Block) -> Self::Result {
        self.debug_visit("block", block.span);
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
        self.debug_visit(&format!("expr {}", debug_expr_kind(&expr.kind)), expr.span);

        if !matches!(expr.kind, ExprKind::If(..)) {
            self.reset_if_else();
        }

        match &expr.kind {
            ExprKind::Let(_pat, let_expr, _span, _recovered) => {
                self.push_context(ContextKind::Let, expr.span);
                self.visit_expr(let_expr);
                self.pop_context();
            }
            ExprKind::If(_if_expr, _if_block, _else_expr) => {
                self.process_if(expr);
            }
            ExprKind::While(while_expr, block, label) => {
                self.push_context(ContextKind::While, expr.span);
                self.visit_expr(while_expr);
                self.visit_block(block);
                self.pop_context();
            }
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
                // self.push_context(ContextKind::Match, body.span);
                for arm in arms {
                    if let Some(body) = &arm.body {
                        self.visit_expr(body);
                    }
                }
                // self.pop_context();
            }
            ExprKind::Closure(closure) => {
                self.visit_expr(&closure.body);
            }
            ExprKind::Block(block, _label) => {
                if self.inside_fn {
                    self.push_context(ContextKind::Block, block.span);
                }
                self.visit_block(block);
                if self.inside_fn {
                    self.pop_context();
                }
            }
            ExprKind::Gen(_capture_by, block, _gen_block_kind, _span) => {
                self.visit_block(block);
            }
            ExprKind::TryBlock(block) => {
                self.visit_block(block);
            }
            ExprKind::Call(_call_expr, args) => {
                // TODO: do we even need to visit call expr?
                // self.visit_expr(call_expr);
                for arg in args {
                    self.visit_expr(arg);
                }
                // println!("CALL ARGS len {}: {args:#?}", args.len());
                // self.visit_expr(call_expr);
            }
            kind => {
                // println!(
                //     "\n\n*** MISSING EXPR KIND: {} @ {} ***\n\n",
                //     debug_expr_kind(kind),
                //     self.debug_span(expr.span)
                // );
            }
        }
    }

    fn visit_item(&mut self, item: &'a Item) -> Self::Result {
        self.debug_visit("item", item.span);

        match &item.kind {
            ItemKind::Static(static_item) => {
                if let Some(expr) = &static_item.expr {
                    self.push_context(ContextKind::Static, item.span);
                    self.debug_visit("item Static expr", expr.span);
                    self.visit_expr(expr);
                    self.pop_context();
                }
            }
            ItemKind::Fn(func) => {
                self.push_context(ContextKind::Func, item.span);
                self.process_fn(func, item.span);
                self.pop_context();
            }
            ItemKind::Mod(_, _, ModKind::Loaded(items, _, span)) => {
                self.push_context(ContextKind::Mod, item.span);
                for item in items {
                    self.visit_item(item);
                }
                self.pop_context();
            }
            ItemKind::Trait(tr) => {
                self.push_context(ContextKind::Trait, item.span);
                for item in &tr.items {
                    if let AssocItemKind::Fn(func) = &item.kind {
                        self.process_fn(func, item.span);
                    }
                }
                self.pop_context();
            }
            ItemKind::Impl(imp) => {
                self.push_context(ContextKind::Impl, item.span);
                for item in &imp.items {
                    if let AssocItemKind::Fn(func) = &item.kind {
                        self.process_fn(func, item.span);
                    }
                }
                self.pop_context();
            }
            kind => {
                // println!(
                //     "\n\n*** MISSING ITEM KIND: {kind:?} @ {} ***\n\n",
                //     self.debug_span(item.span)
                // );
            }
        }
    }

    fn visit_stmt(&mut self, stmt: &'a Stmt) -> Self::Result {
        self.debug_visit("stmt", stmt.span);

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
            current_nesting_lint: None,
            inside_fn: false,
        }
    }

    fn depth(&self) -> usize {
        self.contexts.len().saturating_sub(1)
    }

    fn debug_span_info(&self, span: Span) -> SpanInfo {
        debug_span_info(span, self.source_map)
    }

    fn debug_span(&self, span: Span) -> String {
        if self.config.debug {
            debug_span(span, self.source_map)
        } else {
            String::new()
        }
    }

    fn debug_code(&self, span: Span) -> String {
        if !self.config.debug {
            return String::new();
        }
        self.source_map.span_to_snippet(span).unwrap_or_default()
    }

    fn inc_if_else(&mut self, span: Span) {
        let depth = self.depth();

        let Some(ctx) = self.contexts.last_mut() else {
            return;
        };

        let outer_span = *ctx.consec_if_else_span.get_or_insert(span);

        ctx.consec_if_else_count += 1;
        let count = ctx.consec_if_else_count;

        if count > self.config.max_consec_if_else {
            let mut lint = ctx.consec_if_else_lint.get_or_insert(Lint {
                outer_span: Some(outer_span),
                span,
                // TODO: use context kind or If/Else?
                kind: ctx.kind,
                reason: Reason::ConsecIfElse(count),
            });
            lint.reason = Reason::ConsecIfElse(count);
        }

        if self.config.debug {
            let debug_span = self.debug_span(span);
            println!(
                "{}inc_if_else # {} [d={depth}] at {debug_span}",
                "  ".repeat(depth),
                count,
            );
        }
    }

    fn reset_if_else(&mut self) {
        if let Some(ctx) = self.contexts.last_mut() {
            ctx.consec_if_else_count = 0;
            ctx.consec_if_else_span = None;
            ctx.consec_if_else_lint = None;
        }
    }

    fn push_context(&mut self, kind: ContextKind, span: Span) {
        let ctx = Context::new(kind, span, self.source_map);
        let Context { span, kind, .. } = ctx;
        self.contexts.push(ctx);
        let depth = self.depth();
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

    fn pop_context(&mut self) {
        let depth = self.depth();
        if let Some(mut ctx) = self.contexts.pop()
            && let Some(lint) = ctx.consec_if_else_lint.take()
        {
            self.lints.push(lint);
        }
        if depth <= self.config.max_depth
            && let Some(lint) = self.current_nesting_lint.take()
        {
            self.lints.push(lint);
        }
    }

    fn process_if(&mut self, expr: &'a Expr) {
        self.debug_visit("process_if", expr.span);
        self.inc_if_else(expr.span);

        match &expr.kind {
            ExprKind::If(_if_expr, block, else_expr) => {
                self.push_context(ContextKind::If, expr.span);
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
            other => unreachable!("if/else must be a block or if: {other:?}"),
        }
    }

    fn process_fn(&mut self, func: &'a rustc_ast::Fn, span: Span) {
        self.debug_visit("process_fn", func.sig.span);

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
            // let spans = match lint.outer_span {
            //     Some(outer_span) => vec![outer_span, lint.span],
            //     None => vec![lint.span],
            // };
            cx.span_lint(NESTING_DEPTH, lint.span, |diag| {
                if let Some(outer_span) = lint.outer_span {
                    diag.span_label(outer_span, lint.reason.outer_context_label());
                }
                diag.primary_message(lint.reason.message(&self.config));
                diag.help(HELP_MESSAGE);
            });
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
struct SpanInfo {
    file: String,
    start_line: usize,
    end_line: usize,
}

impl SpanInfo {
    fn contains(&self, other: &SpanInfo) -> bool {
        if self.file != other.file {
            return false;
        }
        self.start_line <= other.start_line && self.end_line >= other.end_line
    }
}

fn debug_span_info(span: Span, source_map: &SourceMap) -> SpanInfo {
    let location_start = source_map.span_to_location_info(span);
    let location_end = source_map.span_to_location_info(span.shrink_to_hi());
    let file = location_start
        .0
        .map(|f| {
            f.name
                .display(FileNameDisplayPreference::Remapped)
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_default();
    SpanInfo {
        file,
        start_line: location_start.1,
        end_line: location_end.1,
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
