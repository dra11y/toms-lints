use rustc_ast::NodeId;
use rustc_span::Span;

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextKind {
    Func,
    Mod,
    Trait,
    Impl,
    If,
    Then,
    ElseIf,
    Else,
    Match,
    Closure,
    Block,
    ExprBlock,
    While,
    For,
    Loop,
}

impl ContextKind {
    pub fn count_depth(&self, config: &Config) -> bool {
        match self {
            ContextKind::Func => true,
            ContextKind::Mod => false,
            ContextKind::Trait => false,
            ContextKind::Impl => false,
            ContextKind::If => false,
            ContextKind::Then => true,
            ContextKind::ElseIf => false,
            ContextKind::Else => false,
            ContextKind::Match => true,
            ContextKind::Closure => !config.ignore_closures,
            ContextKind::Block => true,
            ContextKind::ExprBlock => true,
            ContextKind::While => true,
            ContextKind::For => true,
            ContextKind::Loop => true,
        }
    }

    pub fn is_if_or_if_branch(&self) -> bool {
        matches!(
            self,
            ContextKind::If | ContextKind::Then | ContextKind::ElseIf | ContextKind::Else
        )
    }

    pub fn is_if_branch(&self) -> bool {
        matches!(
            self,
            ContextKind::Then | ContextKind::ElseIf | ContextKind::Else
        )
    }
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
    /// Count of consecutive if/else-if/else branches in the current block.
    pub consec_if_else_count: usize,
    /// Count of consecutive if/else-if branches in the current if-else chain.
    pub consec_if_branch_count: usize,
}

impl Context {
    pub fn new(kind: ContextKind, id: NodeId, span: Span) -> Self {
        Self {
            span,
            kind,
            id,
            consec_if_else_count: 0,
            consec_if_branch_count: 0,
        }
    }
}

impl ContextKind {
    pub fn descr(&self) -> &'static str {
        match self {
            ContextKind::Func => "func",
            ContextKind::If => "if",
            ContextKind::Then => "then",
            ContextKind::Else => "else",
            ContextKind::ElseIf => "else-if",
            ContextKind::Match => "match",
            ContextKind::Closure => "closure",
            ContextKind::Block => "block",
            ContextKind::ExprBlock => "expr-block",
            ContextKind::While => "while",
            ContextKind::For => "for",
            ContextKind::Loop => "loop",
            ContextKind::Mod => "mod",
            ContextKind::Trait => "trait",
            ContextKind::Impl => "impl",
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
