use rustc_ast::NodeId;
use rustc_span::Span;

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextKind {
    Item,
    Func,
    If,
    Then,
    ElseIf,
    Else,
    MatchArm,
    Block,
    ExprBlock,
    While,
    For,
    Loop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reason {
    Depth(usize),
    ConsecIfElse(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NestingLint {
    pub outer_span: Option<Span>,
    pub span: Span,
    pub kind: ContextKind,
    pub reason: Reason,
}

#[derive(Clone)]
pub struct Context {
    pub span: Span,
    pub id: NodeId,
    pub kind: ContextKind,
    pub consec_if_else_span: Option<Span>,
    pub consec_if_else_count: usize,
    pub consec_if_else_lint: Option<NestingLint>,
}

impl Context {
    pub fn count_depth(&self) -> bool {
        self.kind.count_depth()
    }

    pub fn new(kind: ContextKind, id: NodeId, span: Span) -> Self {
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

impl ContextKind {
    pub fn count_depth(&self) -> bool {
        match self {
            ContextKind::Item => true,
            ContextKind::Func => true,
            ContextKind::If => false,
            ContextKind::Then => true,
            ContextKind::Else => true,
            ContextKind::ElseIf => true,
            ContextKind::MatchArm => true,
            ContextKind::Block => true,
            ContextKind::ExprBlock => true,
            ContextKind::While => true,
            ContextKind::For => true,
            ContextKind::Loop => true,
        }
    }

    pub fn descr(&self) -> &'static str {
        match self {
            ContextKind::Item => "item",
            ContextKind::Func => "func",
            ContextKind::If => "if",
            ContextKind::Then => "then",
            ContextKind::Else => "else",
            ContextKind::ElseIf => "else-if",
            ContextKind::MatchArm => "match-arm",
            ContextKind::Block => "block",
            ContextKind::ExprBlock => "expr-block",
            ContextKind::While => "while",
            ContextKind::For => "for",
            ContextKind::Loop => "loop",
        }
    }
}

impl Reason {
    pub fn outer_context_label(&self) -> &'static str {
        match self {
            Reason::Depth(_) => "outer nested context",
            Reason::ConsecIfElse(_) => "first if in sequence",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Reason::Depth(_) => "nesting depth",
            Reason::ConsecIfElse(_) => "consecutive if-else statements",
        }
    }

    pub fn message(&self, config: &Config) -> String {
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

impl std::fmt::Display for ContextKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.descr())
    }
}
