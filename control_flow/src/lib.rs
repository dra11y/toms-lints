#![allow(unused)]
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_span;

use std::collections::HashSet;

use dylint_linting::config_or_default;
use rustc_ast::{
    AssocItemKind, Block, Crate, Expr, ExprKind, Item, ItemKind, ModKind, NodeId, Stmt, StmtKind,
    visit::{FnKind, Visitor},
};
use rustc_lint::{EarlyContext, EarlyLintPass, Level, LintContext};
use rustc_span::{ExpnKind, FileNameDisplayPreference, Span, source_map::SourceMap};
use serde_inline_default::serde_inline_default;

/// Default maximum nesting levels
const DEFAULT_MAX_DEPTH: usize = 3;

/// Default maximum items in an if-block
const DEFAULT_MAX_ITEMS: usize = 10;

const HELP_MESSAGE: &str = "use early returns and guard clauses to reduce nesting";

/// Lint configuration
#[serde_inline_default]
#[derive(serde::Deserialize)]
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
pub struct ControlFlow {
    config: Config,
}

impl Default for ControlFlow {
    fn default() -> Self {
        Self {
            config: config_or_default(env!("CARGO_PKG_NAME")),
        }
    }
}

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
    pub CONTROL_FLOW,
    Warn,
    "nested if-then-else and other branching should be simplified",
    ControlFlow::default()
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

impl ContextKind {
    fn count_depth(&self) -> bool {
        !matches!(
            self,
            ContextKind::Func | ContextKind::Closure | ContextKind::Static,
        )
    }
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

struct MyVisitor<'a> {
    contexts: Vec<Context<'a>>,
    source_map: &'a SourceMap,
}

impl<'a> Visitor<'a> for MyVisitor<'a> {
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
        fn print_expr(depth: usize, kind: &ExprKind, span: Span, source_map: &SourceMap) {
            if depth < 2 {
                return;
            }
            let location = debug_span(span, source_map);
            let kind = debug_expr_kind(kind);
            print!("{}", " ".repeat(depth * 4));
            println!("    EXPR {depth} {kind} @ {location}");
        }

        match &expr.kind {
            ExprKind::Let(_pat, let_expr, _span, _recovered) => {
                print_expr(self.depth(), &expr.kind, expr.span, self.source_map);
                self.push_context(ContextKind::Let, expr.span);
                self.visit_expr(let_expr);
                self.pop_context();
                //
            }
            ExprKind::If(_if_expr, _if_block, _else_expr) => {
                self.process_if(expr);
            }
            ExprKind::While(while_expr, block, label) => {
                print_expr(self.depth(), &expr.kind, expr.span, self.source_map);

                //
            }
            ExprKind::ForLoop {
                pat,
                iter,
                body,
                label,
                kind,
            } => {
                print_expr(self.depth(), &expr.kind, expr.span, self.source_map);

                //
            }
            ExprKind::Loop(block, label, span) => {
                print_expr(self.depth(), &expr.kind, expr.span, self.source_map);

                //
            }
            ExprKind::Match(match_expr, thin_vec, match_kind) => {
                print_expr(self.depth(), &expr.kind, expr.span, self.source_map);

                //
            }
            ExprKind::Closure(closure) => {
                print_expr(self.depth(), &expr.kind, expr.span, self.source_map);

                //
            }
            ExprKind::Block(block, label) => {
                println!("BLOCK {}", self.depth());
                print_expr(self.depth(), &expr.kind, expr.span, self.source_map);
                //
            }
            ExprKind::Gen(capture_by, block, gen_block_kind, span) => {
                print_expr(self.depth(), &expr.kind, expr.span, self.source_map);
                //
            }
            ExprKind::TryBlock(block) => {
                print_expr(self.depth(), &expr.kind, expr.span, self.source_map);
                //
            }
            _ => {
                // print!("    ---- SKIP ----: ");
                // print_expr(self.depth(), &expr.kind, expr.span, self.source_map);
            }
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
            _ => {}
        }
    }

    fn visit_fn(&mut self, kind: FnKind<'a>, span: Span, _: NodeId) -> Self::Result {
        match kind {
            FnKind::Fn(_, _, func) => self.process_fn(func, span),
            FnKind::Closure(_, _, _, expr) => {
                //
                self.push_context(ContextKind::Closure, span);
                self.visit_expr(expr);
                self.pop_context();
            }
        }
    }

    fn visit_stmt(&mut self, stmt: &'a Stmt) -> Self::Result {
        match &stmt.kind {
            StmtKind::Item(item) => self.visit_item(item),
            StmtKind::Expr(expr) => self.visit_expr(expr),
            _ => {}
        }
    }
}

impl<'a> MyVisitor<'a> {
    fn new(source_map: &'a SourceMap) -> Self {
        Self {
            contexts: Vec::new(),
            source_map,
        }
    }

    fn depth(&self) -> usize {
        self.contexts
            .iter()
            .filter(|c| c.kind.count_depth())
            .count()
    }

    fn debug_span(&self, span: Span) -> String {
        debug_span(span, self.source_map)
    }

    fn push_context(&mut self, kind: ContextKind, span: Span) {
        let ctx = Context::new(kind, span, self.source_map);
        // println!("PUSH CONTEXT: {} {ctx:?}", self.depth());
        self.contexts.push(ctx);
    }

    fn pop_context(&mut self) {
        let _ctx = self.contexts.pop();
        // if let Some(ctx) = ctx {
        //     println!("POP CONTEXT: {} {ctx:?}", self.depth());
        // } else {
        //     eprintln!("POP CONTEXT: {} NONE", self.depth());
        // }
    }

    fn process_if(&mut self, expr: &'a Expr) {
        match &expr.kind {
            ExprKind::If(_, block, else_expr) => {
                self.push_context(ContextKind::If, block.span);
                println!("process_if: IF {}", self.depth());
                self.visit_block(block);
                self.pop_context();

                if let Some(else_expr) = else_expr {
                    println!("process_if: ELSE {}", self.depth());
                    self.process_if(else_expr);
                }
            }
            ExprKind::Block(block, _) => {
                self.push_context(ContextKind::Else, block.span);
                println!("process_if: ELSE BLOCK {}", self.depth());
                self.visit_block(block);
                self.pop_context();
            }
            other => unreachable!("else expression is not a block or if: {other:?}"),
        }
    }

    fn process_fn(&mut self, func: &'a rustc_ast::Fn, span: Span) {
        if let Some(body) = &func.body {
            self.push_context(ContextKind::Func, span);
            self.visit_block(body);
            self.pop_context();
        }
    }
}

impl EarlyLintPass for ControlFlow {
    #[inline(always)]
    fn check_crate(&mut self, cx: &EarlyContext<'_>, cr: &rustc_ast::Crate) {
        let source_map = cx.sess().source_map();
        let mut visitor = MyVisitor::new(source_map);
        visitor.visit_crate(cr);
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
