use std::cmp::Ordering;

use rustc_ast::ExprKind;
use rustc_lint::{EarlyContext, LintContext};
use rustc_span::{RemapPathScopeComponents, Span, source_map::SourceMap};
use serde::Deserialize;

use crate::NestingDepth;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct SpanRange {
    file: String,
    start_line: usize,
    end_line: usize,
}

impl SpanRange {
    pub fn intersects(&self, other: &SpanRange) -> bool {
        if self.file != other.file {
            return false;
        }
        match self.start_line.cmp(&other.end_line) {
            Ordering::Greater => false,
            Ordering::Equal => true,
            Ordering::Less => !matches!(self.end_line.cmp(&other.start_line), Ordering::Less),
        }
    }
}

impl NestingDepth {
    pub fn debug_visit(&self, cx: &EarlyContext<'_>, method: &str, span: Span) {
        if !self.config.debug {
            return;
        }

        self.debug_visit_with(cx, method, span, false, None);
    }

    pub fn debug_visit_extra(&self, cx: &EarlyContext<'_>, method: &str, span: Span, extra: &str) {
        if !self.config.debug {
            return;
        }

        self.debug_visit_with(cx, method, span, false, Some(extra));
    }

    pub fn debug_visit_with(
        &self,
        cx: &EarlyContext<'_>,
        method: &str,
        span: Span,
        code: bool,
        extra: Option<&str>,
    ) {
        // return;
        if !self.config.debug {
            return;
        }
        if self
            .config
            .debug_span_range
            .as_ref()
            .is_some_and(|s| !s.intersects(&self.debug_span_info(cx, span)))
        {
            return;
        }
        let info = self.debug_span_info(cx, span);
        let code = code.then(|| self.debug_code(cx, span));
        let depth = self.depth();
        let span = self.debug_span(cx, span);
        let extra = match extra {
            Some(extra) => format!("{extra} "),
            None => String::new(),
        };
        let debug_str = format!(
            "{}[{depth:2}] {method} {extra}{span} {}",
            "  ".repeat(depth),
            code.unwrap_or_default(),
        );
        println!("{debug_str}");
    }

    pub fn debug_span_info(&self, cx: &EarlyContext<'_>, span: Span) -> SpanRange {
        debug_span_info(span, cx.sess().source_map())
    }

    pub fn debug_span(&self, cx: &EarlyContext<'_>, span: Span) -> String {
        if self.config.debug {
            debug_span(span, cx.sess().source_map())
        } else {
            String::new()
        }
    }

    fn debug_code(&self, cx: &EarlyContext<'_>, span: Span) -> String {
        if !self.config.debug {
            return String::new();
        }
        cx.sess()
            .source_map()
            .span_to_snippet(span)
            .unwrap_or_default()
    }
}

pub fn debug_span_info(span: Span, source_map: &SourceMap) -> SpanRange {
    let location_start = source_map.span_to_location_info(span);
    let location_end = source_map.span_to_location_info(span.shrink_to_hi());
    let file = location_start
        .0
        .map(|f| {
            f.name
                .display(RemapPathScopeComponents::DIAGNOSTICS)
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_default();
    SpanRange {
        file,
        start_line: location_start.1,
        end_line: location_end.1,
    }
}

pub fn debug_span(span: Span, source_map: &SourceMap) -> String {
    let location = source_map.span_to_location_info(span);
    let file = location
        .0
        .map(|f| {
            f.name
                .display(RemapPathScopeComponents::DIAGNOSTICS)
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_default();
    format!("{file}:{}:{}", location.1, location.2)
}

pub const fn debug_expr_kind(kind: &ExprKind) -> &'static str {
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
