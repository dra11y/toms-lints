use serde::Deserialize;
use serde_inline_default::serde_inline_default;

use crate::debug::SpanRange;

pub const HELP_MESSAGE: &str = "use early returns and guard clauses to reduce nesting";

/// Default maximum nesting levels
const DEFAULT_MAX_DEPTH: usize = 3;

/// Default maximum items in an if-then block
const DEFAULT_MAX_THEN_ITEMS: usize = 10;

/// Default maximum consecutive if-else statements
const DEFAULT_MAX_CONSEC_IF_ELSE: usize = 3;

const DEFAULT_DEBUG: bool = cfg!(debug_assertions);

/// Lint configuration
#[serde_inline_default]
#[derive(Deserialize)]
pub struct Config {
    /// Maximum allowed nesting depth
    #[serde_inline_default(DEFAULT_MAX_DEPTH)]
    pub max_depth: usize,

    /// Maximum allowed items in an if-then block
    #[serde_inline_default(DEFAULT_MAX_THEN_ITEMS)]
    pub max_then_items: usize,

    /// Maximum allowed consecutive if-else statements
    #[serde_inline_default(DEFAULT_MAX_CONSEC_IF_ELSE)]
    pub max_consec_if_else: usize,

    /// Enable debug output
    #[serde_inline_default(DEFAULT_DEBUG)]
    pub debug: bool,

    /// Optional span range to limit debug output
    #[serde(default)]
    pub debug_span_range: Option<SpanRange>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
            max_then_items: DEFAULT_MAX_THEN_ITEMS,
            max_consec_if_else: DEFAULT_MAX_CONSEC_IF_ELSE,
            debug: DEFAULT_DEBUG,
            debug_span_range: None,
        }
    }
}
