use serde::Deserialize;
use serde_inline_default::serde_inline_default;

use crate::debug::SpanInfo;

pub const HELP_MESSAGE: &str = "use early returns and guard clauses to reduce nesting";

/// Default maximum nesting levels
const DEFAULT_MAX_DEPTH: usize = 3;

/// Default maximum items in an if-block
const DEFAULT_MAX_ITEMS: usize = 10;

/// Default maximum consecutive if-else statements
const DEFAULT_MAX_CONSEC_IF_ELSE: usize = 3;

const DEFAULT_DEBUG: bool = cfg!(debug_assertions);

/// Lint configuration
#[serde_inline_default]
#[derive(Deserialize)]
pub struct Config {
    #[serde_inline_default(DEFAULT_MAX_DEPTH)]
    pub max_depth: usize,
    #[serde_inline_default(DEFAULT_MAX_ITEMS)]
    pub max_items: usize,
    #[serde_inline_default(DEFAULT_MAX_CONSEC_IF_ELSE)]
    pub max_consec_if_else: usize,
    #[serde_inline_default(DEFAULT_DEBUG)]
    pub debug: bool,
    #[serde(default)]
    pub debug_span_info: Option<SpanInfo>,
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
