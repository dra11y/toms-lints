#![allow(unused)]
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashSet;

use dylint_linting::config_or_default;
use rustc_hir::{
    Block, Body, Expr, ExprKind, FnDecl, HirId, ImplItemKind, ItemKind, LoopSource, MatchSource,
    Node, StmtKind, TraitItemKind, def_id::LocalDefId, intravisit::FnKind,
};
use rustc_lint::{LateContext, LateLintPass, Level, LintContext};
use rustc_span::{ExpnKind, Span};

/// Default maximum nesting levels
const DEFAULT_MAX_DEPTH: usize = 3;

const HELP_MESSAGE: &str = "use early returns and guard clauses to reduce nesting";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExprKindKind {
    AddrOf,
    Array,
    Assign,
    AssignOp,
    Become,
    Binary,
    Block,
    Break,
    Call,
    Cast,
    Closure,
    ConstBlock,
    Continue,
    DropTemps,
    Err,
    Field,
    If,
    Index,
    InlineAsm,
    Let,
    Lit,
    Loop,
    Match,
    MethodCall,
    OffsetOf,
    Path,
    Repeat,
    Ret,
    Struct,
    Tup,
    Type,
    Unary,
    UnsafeBinderCast,
    Use,
    Yield,
}

impl std::fmt::Display for ExprKindKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<ExprKind<'_>> for ExprKindKind {
    fn from(value: ExprKind<'_>) -> Self {
        match value {
            ExprKind::AddrOf(..) => ExprKindKind::AddrOf,
            ExprKind::Array(..) => ExprKindKind::Array,
            ExprKind::Assign(..) => ExprKindKind::Assign,
            ExprKind::AssignOp(..) => ExprKindKind::AssignOp,
            ExprKind::Become(..) => ExprKindKind::Become,
            ExprKind::Binary(..) => ExprKindKind::Binary,
            ExprKind::Block(..) => ExprKindKind::Block,
            ExprKind::Break(..) => ExprKindKind::Break,
            ExprKind::Call(..) => ExprKindKind::Call,
            ExprKind::Cast(..) => ExprKindKind::Cast,
            ExprKind::Closure(..) => ExprKindKind::Closure,
            ExprKind::ConstBlock(..) => ExprKindKind::ConstBlock,
            ExprKind::Continue(..) => ExprKindKind::Continue,
            ExprKind::DropTemps(..) => ExprKindKind::DropTemps,
            ExprKind::Err(..) => ExprKindKind::Err,
            ExprKind::Field(..) => ExprKindKind::Field,
            ExprKind::If(..) => ExprKindKind::If,
            ExprKind::Index(..) => ExprKindKind::Index,
            ExprKind::InlineAsm(..) => ExprKindKind::InlineAsm,
            ExprKind::Let(..) => ExprKindKind::Let,
            ExprKind::Lit(..) => ExprKindKind::Lit,
            ExprKind::Loop(..) => ExprKindKind::Loop,
            ExprKind::Match(..) => ExprKindKind::Match,
            ExprKind::MethodCall(..) => ExprKindKind::MethodCall,
            ExprKind::OffsetOf(..) => ExprKindKind::OffsetOf,
            ExprKind::Path(..) => ExprKindKind::Path,
            ExprKind::Repeat(..) => ExprKindKind::Repeat,
            ExprKind::Ret(..) => ExprKindKind::Ret,
            ExprKind::Struct(..) => ExprKindKind::Struct,
            ExprKind::Tup(..) => ExprKindKind::Tup,
            ExprKind::Type(..) => ExprKindKind::Type,
            ExprKind::Unary(..) => ExprKindKind::Unary,
            ExprKind::UnsafeBinderCast(..) => ExprKindKind::UnsafeBinderCast,
            ExprKind::Use(..) => ExprKindKind::Use,
            ExprKind::Yield(..) => ExprKindKind::Yield,
        }
    }
}

/// Lint configuration
#[derive(serde::Deserialize)]
struct Config {
    max_depth: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }
}

/// Lint for detecting nesting that is too deep
pub struct NestingTooDeep {
    config: Config,
    outer_span: Option<Span>,
    max_depth: usize,
}

impl Default for NestingTooDeep {
    fn default() -> Self {
        Self {
            config: config_or_default(env!("CARGO_PKG_NAME")),
            outer_span: None,
            max_depth: 0,
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
    pub NESTING_TOO_DEEP,
    Warn,
    "nested if-then-else and other branching should be simplified",
    NestingTooDeep::default()
}

impl<'tcx> LateLintPass<'tcx> for NestingTooDeep {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        fn_kind: FnKind<'tcx>,
        _fn_decl: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _span: Span,
        def_id: LocalDefId,
    ) {
        // For closures, check if they should be skipped (function body context)
        if matches!(fn_kind, FnKind::Closure)
            && self.is_closure_in_body(cx, fn_kind, body, _span, def_id)
        {
            // println!("🚫 SKIPPING function body closure in check_fn");
            return;
        }
        // println!("✅ PROCESSING static context closure (LazyLock) in check_fn");

        let name = match fn_kind {
            FnKind::ItemFn(ident, _generics, _fn_header) => {
                format!("ITEM {}", self.snippet_first_line(cx, ident.span))
            }
            FnKind::Method(ident, _fn_sig) => {
                format!("METHOD {}", self.snippet_first_line(cx, ident.span))
            }
            FnKind::Closure => format!("CLOSURE {}", self.snippet_first_line(cx, _span)),
        };
        // println!("======================== CHECK FN {name}");

        let body_expr = match body.value.kind {
            ExprKind::Closure(closure) => cx.tcx.hir_body(closure.body).value,
            _ => body.value,
        };

        self.check_expr_for_nesting(cx, body_expr, 0);
    }
}

impl NestingTooDeep {
    fn is_closure_in_body<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        fn_kind: FnKind<'tcx>,
        body: &'tcx Body<'tcx>,
        _span: Span,
        def_id: LocalDefId,
    ) -> bool {
        // For closures, check if they're in a static context (like LazyLock)
        // vs function body context (already handled by expression traversal)
        if !matches!(fn_kind, FnKind::Closure) {
            return false;
        }

        let mut current_id = def_id.to_def_id();

        while let Some(parent_id) = cx.tcx.opt_parent(current_id) {
            let Some(parent_local_id) = parent_id.as_local() else {
                current_id = parent_id;
                continue;
            };

            let parent_hir_id = cx.tcx.local_def_id_to_hir_id(parent_local_id);

            // Get the HIR node using hir_node method

            match cx.tcx.hir_node(parent_hir_id) {
                Node::Item(item) => match item.kind {
                    ItemKind::Static(..) => return false,
                    ItemKind::Fn { .. } => return true,
                    _ => {}
                },
                Node::ImplItem(impl_item) => {
                    if let ImplItemKind::Fn(..) = impl_item.kind {
                        return true;
                    }
                }
                Node::TraitItem(trait_item) => {
                    if let TraitItemKind::Fn(..) = trait_item.kind {
                        return true;
                    }
                }
                _ => {}
            }

            current_id = parent_id;
        }

        false
    }

    fn snippet_collapsed(&self, cx: &LateContext<'_>, span: Span) -> String {
        let snippet = self.snippet(cx, span);
        snippet
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn snippet_first_line(&self, cx: &LateContext<'_>, span: Span) -> String {
        self.snippet(cx, span)
            .lines()
            .next()
            .unwrap_or_default()
            .to_string()
    }

    fn snippet(&self, cx: &LateContext<'_>, span: Span) -> String {
        cx.sess()
            .source_map()
            .span_to_snippet(span)
            .unwrap_or_default()
    }

    fn print_span(&self, cx: &LateContext<'_>, label: &str, span: Span) {
        println!("{label} {}", self.snippet(cx, span));
    }

    fn set_outer_span(&mut self, span: Span) {
        if self
            .outer_span
            .is_none_or(|current_span| span.contains(current_span))
        {
            self.outer_span = Some(span);
        }
    }

    /// Recursively check expressions for nesting constructs
    fn check_expr_for_nesting(&mut self, cx: &LateContext<'_>, expr: &Expr<'_>, depth: usize) {
        let kind_kind = ExprKindKind::from(expr.kind);

        'block: {
            let is_dummy = expr.span.is_dummy();
            let in_derive = expr.span.in_derive_expansion();
            let is_empty = expr.span.is_empty();
            let is_macro_expansion =
                matches!(expr.span.ctxt().outer_expn_data().kind, ExpnKind::Macro(..));

            if is_dummy || in_derive || is_empty || is_macro_expansion {
                if is_macro_expansion {
                    let snippet = self.snippet_first_line(cx, expr.span);
                    // println!("    SKIP macro_expansion: {snippet}");
                }
                break 'block;
            }

            match expr.kind {
                ExprKind::If(_if_expr, then_expr, else_expr) => {
                    self.set_outer_span(expr.span);

                    const MAX_ITEMS: usize = 10;
                    const ELSE_MORE_THAN_THEN_MIN: usize = 6;
                    const ELSE_MORE_THAN_THEN_RATIO: f64 = 2.0;

                    enum ThenElseReason {
                        ThenTooMany,
                        ElseTooMany,
                        ThenLargerThanElse,
                    }

                    impl ThenElseReason {
                        fn message(&self, then_items: usize, else_items: usize) -> String {
                            match self {
                                ThenElseReason::ThenTooMany => {
                                    format!(
                                        "if 'then' block has too many items: {then_items} (max: {MAX_ITEMS})"
                                    )
                                }
                                ThenElseReason::ElseTooMany => {
                                    format!(
                                        "if 'else' block has too many items: {else_items} (max: {MAX_ITEMS})"
                                    )
                                }
                                ThenElseReason::ThenLargerThanElse => {
                                    format!(
                                        "if 'then' block has significantly more items ({then_items}) than 'else' block ({else_items})"
                                    )
                                }
                            }
                        }
                    }

                    let then_items = if let ExprKind::Block(block, _label) = then_expr.kind {
                        block.stmts.len() + if block.expr.is_some() { 1 } else { 0 }
                    } else {
                        1
                    };

                    let else_items = else_expr
                        .map(|els| {
                            if let ExprKind::Block(block, _label) = els.kind {
                                block.stmts.len() + if block.expr.is_some() { 1 } else { 0 }
                            } else {
                                1
                            }
                        })
                        .unwrap_or(0);

                    let reason = if else_items > ELSE_MORE_THAN_THEN_MIN
                        && then_items as f64 > else_items as f64 * ELSE_MORE_THAN_THEN_RATIO
                    {
                        Some(ThenElseReason::ThenLargerThanElse)
                    } else if then_items > 10 {
                        Some(ThenElseReason::ThenTooMany)
                    } else if else_items > 10 {
                        Some(ThenElseReason::ElseTooMany)
                    } else {
                        None
                    };

                    if let Some(reason) = reason
                        && Level::Allow
                            != cx
                                .tcx
                                .lint_level_at_node(NESTING_TOO_DEEP, expr.hir_id)
                                .level
                    {
                        cx.span_lint(NESTING_TOO_DEEP, expr.span, |lint| {
                            lint.primary_message(reason.message(then_items, else_items))
                                .help(HELP_MESSAGE);
                        });
                    }

                    self.check_expr_for_nesting(cx, then_expr.peel_blocks(), depth + 1);
                    if let Some(else_expr) = else_expr {
                        self.check_expr_for_nesting(cx, else_expr.peel_blocks(), depth + 1);
                    }
                }
                ExprKind::Loop(block, _label, loop_source, span) => {
                    let depth = match loop_source {
                        // While desugars to an extra ExprKind::If
                        LoopSource::While => depth,
                        LoopSource::Loop => depth + 1,
                        LoopSource::ForLoop => {
                            // let for_loop = self.snippet(cx, expr.span);
                            // if for_loop.contains("(server_id, snapshot)") {
                            //     println!(
                            //         "FOR LOOP! SELF CURRENT SPAN: {:?}   EXPR SPAN: {:?}",
                            //         self.current_span, expr.span
                            //     );
                            // }
                            depth + 1
                        }
                    };
                    self.set_outer_span(expr.span);
                    self.check_block_for_nesting(cx, block, depth);
                }
                ExprKind::DropTemps(inner_expr) => {
                    // println!("DESUGAR DROP TEMPS!");
                    self.check_expr_for_nesting(cx, inner_expr, depth);
                }
                ExprKind::Match(expr, arms, match_source) => {
                    self.set_outer_span(expr.span);
                    for arm in arms {
                        // self.print_span(cx, &format!("MATCH ARM depth={depth}"), arm.span);
                        // Don't count match itself as a level of nesting
                        self.check_expr_for_nesting(cx, arm.body, depth);
                    }
                }
                ExprKind::Closure(closure) => {
                    self.set_outer_span(expr.span);
                    let body_expr = cx.tcx.hir_body(closure.body).value;
                    let kind_kind = ExprKindKind::from(body_expr.kind);
                    // println!("CLOSURE! {kind_kind} {}", self.snippet(cx, expr.span));
                    self.check_expr_for_nesting(cx, body_expr, depth + 1);
                }
                ExprKind::Block(block, _label) => {
                    let is_empty = block.stmts.is_empty();
                    let is_none = block.expr.is_none();
                    if is_empty && is_none {
                        // println!("EMPTY BLOCK!");
                        break 'block;
                    }
                    self.check_block_for_nesting(cx, block, depth);
                }
                ExprKind::AddrOf(borrow_kind, mutability, expr) => break 'block,
                ExprKind::Array(exprs) => break 'block,
                ExprKind::Assign(expr, expr1, span) => break 'block,
                ExprKind::AssignOp(spanned, expr, expr1) => break 'block,
                ExprKind::Become(expr) => break 'block,
                ExprKind::Binary(spanned, expr, expr1) => break 'block,
                ExprKind::Break(..) => break 'block,
                ExprKind::Call(fn_expr, _args) => break 'block,
                ExprKind::Cast(expr, ty) => break 'block,
                ExprKind::ConstBlock(const_block) => break 'block,
                ExprKind::Continue(destination) => break 'block,
                ExprKind::Err(error_guaranteed) => break 'block,
                ExprKind::Field(expr, ident) => break 'block,
                ExprKind::Index(expr, expr1, span) => break 'block,
                ExprKind::InlineAsm(inline_asm) => break 'block,
                ExprKind::Let(let_expr) => break 'block,
                ExprKind::Lit(..) => break 'block,
                ExprKind::MethodCall(path_segment, expr, exprs, span) => break 'block,
                ExprKind::OffsetOf(ty, idents) => break 'block,
                ExprKind::Path(..) => break 'block,
                ExprKind::Repeat(expr, const_arg) => break 'block,
                ExprKind::Ret(expr) => break 'block,
                ExprKind::Struct(qpath, expr_fields, struct_tail_expr) => break 'block,
                ExprKind::Tup(exprs) => break 'block,
                ExprKind::Type(expr, ty) => break 'block,
                ExprKind::Unary(un_op, expr) => break 'block,
                ExprKind::UnsafeBinderCast(unsafe_binder_cast_kind, expr, ty) => break 'block,
                ExprKind::Use(expr, span) => break 'block,
                ExprKind::Yield(expr, yield_source) => break 'block,
            }
        }

        if depth > self.max_depth {
            self.max_depth = depth;
        }

        if depth == 0 {
            if self.max_depth > self.config.max_depth
                && let Some(span) = self.outer_span
                && Level::Allow
                    != cx
                        .tcx
                        .lint_level_at_node(NESTING_TOO_DEEP, expr.hir_id)
                        .level
            {
                cx.span_lint(NESTING_TOO_DEEP, span, |lint| {
                    lint.primary_message(format!(
                        "nested structure is {} levels deep (max: {})",
                        self.max_depth, self.config.max_depth
                    ))
                    .help(HELP_MESSAGE);
                });
            }

            // println!("    CLEAR current_span");
            self.outer_span = None;
            self.max_depth = 0;
        }
    }

    /// Check a block for nesting constructs
    fn check_block_for_nesting(&mut self, cx: &LateContext<'_>, block: &Block<'_>, depth: usize) {
        // self.print_span(cx, "BLOCK", block.span);

        for stmt in block.stmts {
            if let StmtKind::Expr(expr) | StmtKind::Semi(expr) = &stmt.kind {
                self.check_expr_for_nesting(cx, expr, depth);
            }

            if let StmtKind::Let(local) = &stmt.kind {
                // println!("LET EXPR: OUTER SPAN: {:?}", self.current_span);
                self.set_outer_span(local.span);

                if let Some(init_expr) = &local.init {
                    self.check_expr_for_nesting(cx, init_expr, depth);
                }

                if let Some(els_block) = &local.els {
                    self.check_block_for_nesting(cx, els_block, depth);
                }
            }
        }

        if let Some(expr) = &block.expr {
            self.check_expr_for_nesting(cx, expr, depth);
        }
    }
}

#[test]
fn ui() {
    dylint_uitesting::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
