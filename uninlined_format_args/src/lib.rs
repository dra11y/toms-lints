#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_lint_defs;
extern crate rustc_middle;
extern crate rustc_parse;
extern crate rustc_span;
extern crate rustc_type_ir;

const PRIMARY_MESSAGE: &str = "variables can be used directly in the `format!` string";
const FRIVOLOUS_REASSIGNMENT_MESSAGE: &str = "frivolous reassignment in format arguments";
const CHANGE_MESSAGE: &str = "change this to";
const HELP_MESSAGE: &str = "for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#uninlined_format_args";

use rustc_ast::{
    Expr, ExprKind, FormatArgPositionKind, FormatArgs, FormatArgsPiece, FormatArgumentKind,
    FormatPlaceholder,
    token::{Delimiter, IdentIsRaw, LitKind, TokenKind},
    tokenstream::{TokenStream, TokenTree},
};
use rustc_lint::{EarlyContext, EarlyLintPass, Level, LintContext};
use rustc_lint_defs::Applicability;
use rustc_parse::new_parser_from_source_str;
use rustc_span::{BytePos, ExpnKind, FileName, MacroKind, Span, hygiene};

/// from clippy_utils: https://github.com/rust-lang/rust-clippy/blob/master/clippy_utils/src/macros.rs#L456
/// Span of the `:` and format specifiers
///
/// ```ignore
/// format!("{:.}"), format!("{foo:.}")
///           ^^                  ^^
/// ```
pub fn format_placeholder_format_span(placeholder: &FormatPlaceholder) -> Option<Span> {
    let base = placeholder.span?.data();

    // `base.hi` is `{...}|`, subtract 1 byte (the length of '}') so that it points before the closing
    // brace `{...|}`
    Some(Span::new(
        placeholder.argument.span?.hi(),
        base.hi - BytePos(1),
        base.ctxt,
        base.parent,
    ))
}

// /// from clippy_utils: https://github.com/rust-lang/rust-clippy/blob/master/clippy_utils/src/macros.rs#L481
// /// Span covering the format string and values
// ///
// /// ```ignore
// /// format("{}.{}", 10, 11)
// /// //     ^^^^^^^^^^^^^^^
// /// ```
// pub fn format_args_inputs_span(format_args: &FormatArgs) -> Span {
//     match format_args.arguments.explicit_args() {
//         [] => format_args.span,
//         [.., last] => format_args
//             .span
//             .to(hygiene::walk_chain(last.expr.span, format_args.span.ctxt())),
//     }
// }

/// from clippy_utils: https://github.com/rust-lang/rust-clippy/blob/master/clippy_utils/src/macros.rs#L497
/// Returns the [`Span`] of the value at `index` extended to the previous comma, e.g. for the value
/// `10`
///
/// ```ignore
/// format("{}.{}", 10, 11)
/// //            ^^^^
/// ```
pub fn format_arg_removal_span(format_args: &FormatArgs, index: usize) -> Option<Span> {
    let ctxt = format_args.span.ctxt();

    let current = hygiene::walk_chain(format_args.arguments.by_index(index)?.expr.span, ctxt);

    let prev = if index == 0 {
        format_args.span
    } else {
        hygiene::walk_chain(format_args.arguments.by_index(index - 1)?.expr.span, ctxt)
    };

    Some(current.with_lo(prev.hi()))
}

dylint_linting::declare_early_lint! {
    /// ### What it does
    /// Effectively runs the uninlined_format_args clippy lint on any macro that expands to use format_args!
    ///
    /// ### Why is this bad?
    /// Uninlined format arguments are hard to read. In 3rd party crates, they are not linted like they are
    /// in std with clippy. This results in inconsistent formatting. This lint fills the gap
    /// by linting 3rd party macros for uninlined_format_args like clippy does for std.
    ///
    /// ### Example
    /// ```ignore
    /// fn main() {
    ///    let a = 42;
    ///    let b = Some("test");
    ///    tracing::warn!("This should lint: {}", a);
    ///    tracing::error!("So should this: {}, {:?}", a, b);
    ///    tracing::info!("But these are OK: {a}, {b:?}");
    /// }
    /// ```
    pub UNINLINED_FORMAT_ARGS,
    Warn,
    "format arguments should be inlined for readability and consistency"
}

impl EarlyLintPass for UninlinedFormatArgs {
    fn check_expr(&mut self, cx: &EarlyContext, expr: &Expr) {
        if cx.get_lint_level(UNINLINED_FORMAT_ARGS).level == Level::Allow {
            return;
        }

        let ExprKind::FormatArgs(format_args) = &expr.kind else {
            return;
        };

        let callsite = expr.span.source_callsite();

        let mut data = expr.span.ctxt().outer_expn_data();
        let mut outer_expn_data = data.call_site.ctxt().outer_expn_data();
        while !outer_expn_data.is_root() {
            data = outer_expn_data;
            outer_expn_data = data.call_site.ctxt().outer_expn_data();
        }
        if !matches!(data.kind, ExpnKind::Macro(MacroKind::Bang, _)) {
            // println!("SKIP: not a bang macro callsite: {:?}", data.kind);
            return;
        }

        let mut fixes = Vec::new(); // First check for frivolous reassignments by parsing the macro source
        if let Some(args) = parse_macro_call_args(cx, callsite)
            && has_frivolous_reassignment(&args)
            && let Some(rewritten) = reinline_entire_invocation(cx, callsite)
        {
            cx.span_lint(UNINLINED_FORMAT_ARGS, callsite, move |lint| {
                lint.primary_message(FRIVOLOUS_REASSIGNMENT_MESSAGE);
                lint.help(HELP_MESSAGE);
                lint.span_suggestion_verbose(
                    callsite,
                    CHANGE_MESSAGE,
                    rewritten,
                    Applicability::MachineApplicable,
                );
            });
            return;
        }

        for placeholder in format_args.template.iter() {
            let FormatArgsPiece::Placeholder(placeholder) = placeholder else {
                continue;
            };

            let FormatArgPositionKind::Implicit = placeholder.argument.kind else {
                continue;
            };

            let Ok(arg_index) = placeholder.argument.index else {
                continue;
            };

            let Some(format_arg) = format_args.arguments.by_index(arg_index) else {
                continue;
            };

            let FormatArgumentKind::Normal = format_arg.kind else {
                continue;
            };

            let ExprKind::Path(None, path) = &format_arg.expr.kind else {
                continue;
            };

            let [segment] = path.segments.as_slice() else {
                continue;
            };

            let Some(placeholder_span) = placeholder.span else {
                continue;
            };

            let Some(arg_removal_span) = format_arg_removal_span(format_args, arg_index) else {
                continue;
            };

            let identifier = &segment.ident;

            if identifier.span.from_expansion() {
                let Some(rewritten) = reinline_entire_invocation(cx, callsite) else {
                    continue;
                };

                cx.span_lint(UNINLINED_FORMAT_ARGS, callsite, move |lint| {
                    lint.primary_message(PRIMARY_MESSAGE);
                    lint.help(HELP_MESSAGE);
                    lint.span_suggestion_verbose(
                        callsite,
                        CHANGE_MESSAGE,
                        rewritten,
                        Applicability::MachineApplicable,
                    );
                });
                return;
            }

            let format_spec = format_placeholder_format_span(placeholder)
                .and_then(|spec_span| cx.sess().source_map().span_to_snippet(spec_span).ok())
                .unwrap_or_default();
            // Adjust any positional indices in the format spec that are affected by removing this argument.
            let adjusted_spec = adjust_positional_indices(&format_spec, arg_index);
            let identifier_name = identifier.name;
            let suggestion = format!("{{{identifier_name}{adjusted_spec}}}");

            if !placeholder_span.is_empty() {
                fixes.push((placeholder_span, suggestion));
            }
            if !arg_removal_span.is_empty() {
                fixes.push((arg_removal_span, String::new()));
            }
        }

        if fixes.is_empty() {
            return;
        }

        cx.span_lint(UNINLINED_FORMAT_ARGS, callsite, move |lint| {
            lint.primary_message(PRIMARY_MESSAGE);
            lint.help(HELP_MESSAGE);
            lint.multipart_suggestion(CHANGE_MESSAGE, fixes, Applicability::MachineApplicable);
        });
    }
}

/// Adjust positional argument indices within a format specifier string after the
/// argument at `removed_index` is inlined and removed from the argument list.
///
/// We look for occurrences of `<digits>$` and, for each numeric value `n` greater
/// than `removed_index`, decrement it by one to reflect the shifted indices.
/// If we encounter an index exactly equal to `removed_index`, we leave the spec
/// unchanged (best-effort); this represents an unsupported pattern like
/// `format!("{:0$}", x)` where width reuses the same argument. In such a case,
/// the original suggestion (without index adjustment) would become invalid, so
/// by not altering the spec we effectively avoid producing an incorrect
/// negative shift. (Future improvement: skip producing a suggestion for that
/// placeholder.)
fn adjust_positional_indices(spec: &str, removed_index: usize) -> String {
    if spec.is_empty() {
        return String::new();
    }

    // Fast path: if there's no '$', nothing to do.
    if !spec.as_bytes().contains(&b'$') {
        return spec.to_string();
    }

    let bytes = spec.as_bytes();
    let mut i = 0;
    let mut out = String::with_capacity(spec.len());
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            // If next char is '$', treat this as positional index.
            if i < bytes.len() && bytes[i] == b'$' {
                let num_str = &spec[start..i];
                if let Ok(num) = num_str.parse::<usize>() {
                    if num == removed_index {
                        // Unsupported: width/precision referencing the removed argument itself.
                        // Return original spec unchanged to avoid producing invalid format.
                        return spec.to_string();
                    }
                    if num > removed_index {
                        let new_num = num - 1;
                        out.push_str(&new_num.to_string());
                        out.push('$');
                        i += 1; // Skip '$'
                        continue;
                    }
                }
                // Fall through: leave original digits and '$' untouched
                out.push_str(num_str);
                out.push('$');
                i += 1;
                continue;
            } else {
                // Not a positional index; just copy digits.
                out.push_str(&spec[start..i]);
                continue;
            }
        }
        // Copy current char and advance.
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Parses macro call arguments from the source code at the given span
fn parse_macro_call_args(cx: &EarlyContext, callsite: Span) -> Option<Vec<String>> {
    let snippet = cx.sess().source_map().span_to_snippet(callsite).ok()?;

    let mut parser = new_parser_from_source_str(
        &cx.sess().psess,
        FileName::anon_source_code(&snippet),
        snippet.clone(),
    )
    .ok()?;

    let parsed_expr = parser.parse_expr().ok()?;
    let ExprKind::MacCall(mac_call) = &parsed_expr.kind else {
        return None;
    };

    parse_macro_args(&mac_call.args.tokens)
}

/// Checks if the macro arguments contain frivolous reassignments like `name = name`
fn has_frivolous_reassignment(args: &[String]) -> bool {
    use std::collections::{HashMap, HashSet};
    let mut mapping = HashMap::new();
    let mut lefts = HashSet::new();
    let mut rights = HashSet::new();
    for arg in args.iter().skip(1) {
        if let Some(eq_pos) = arg.find('=') {
            let before_eq = arg[..eq_pos].trim();
            let after_eq = arg[eq_pos + 1..].trim();
            let before_word = before_eq.split_whitespace().last().unwrap_or("");
            let after_word = after_eq.split_whitespace().next().unwrap_or("");
            if is_simple_identifier(before_word) && is_simple_identifier(after_word) {
                mapping.insert(before_word, after_word);
                lefts.insert(before_word);
                rights.insert(after_word);
            }
        }
    }
    // If any mapping would cause a duplicate placeholder, do not lint
    for (from, to) in &mapping {
        if lefts.contains(to) && from != to {
            // Would cause ambiguity or duplicate placeholder
            return false;
        }
    }
    !mapping.is_empty()
}

fn reinline_entire_invocation(cx: &EarlyContext, callsite: Span) -> Option<String> {
    let snippet = cx.sess().source_map().span_to_snippet(callsite).ok()?;

    // Parse the entire macro call expression
    let mut parser = new_parser_from_source_str(
        &cx.sess().psess,
        FileName::anon_source_code(&snippet),
        snippet.clone(),
    )
    .ok()?;

    let parsed_expr = match parser.parse_expr() {
        Ok(expr) => expr,
        Err(_e) => {
            return None;
        }
    };

    // Extract the macro call and parse arguments
    let ExprKind::MacCall(mac_call) = &parsed_expr.kind else {
        return None;
    };
    let args = parse_macro_args(&mac_call.args.tokens)?;
    if args.is_empty() {
        return None;
    }

    // First argument should be the format string
    let format_string = args.first()?;

    let remaining_args: Vec<String> = args.iter().skip(1).cloned().collect();

    let format_content = extract_string_content(format_string)?;

    // First, handle frivolous reassignments and build a mapping
    use std::collections::{HashMap, HashSet};
    let mut replacements = HashMap::new();
    let mut filtered_args = Vec::new();
    let mut lefts = HashSet::new();
    let mut rights = HashSet::new();
    for arg in &remaining_args {
        if let Some(eq_pos) = arg.find('=') {
            let before_eq = arg[..eq_pos].trim();
            let after_eq = arg[eq_pos + 1..].trim();
            let before_word = before_eq.split_whitespace().last().unwrap_or("");
            let after_word = after_eq.split_whitespace().next().unwrap_or("");
            if is_simple_identifier(before_word) && is_simple_identifier(after_word) {
                lefts.insert(before_word);
                rights.insert(after_word);
                replacements.insert(before_word.to_string(), after_word.to_string());
                continue;
            }
        }
        filtered_args.push(arg.clone());
    }
    // Use FormatArgs to collect all placeholder names
    let mut original_placeholders = std::collections::HashSet::new();
    if let Some(format_args) = match &parsed_expr.kind {
        rustc_ast::ExprKind::FormatArgs(fa) => Some(fa),
        _ => None,
    } {
        for piece in &format_args.template {
            if let rustc_ast::FormatArgsPiece::Placeholder(ph) = piece {
                match &ph.argument.kind {
                    rustc_ast::FormatArgPositionKind::Implicit => {
                        if let Ok(idx) = ph.argument.index
                            && let Some(arg) = format_args.arguments.by_index(idx)
                            && let rustc_ast::ExprKind::Path(_, path) = &arg.expr.kind
                            && let Some(seg) = path.segments.first()
                        {
                            original_placeholders.insert(seg.ident.name.to_string());
                        }
                    }
                    rustc_ast::FormatArgPositionKind::Named => {
                        // For named, get the name from ph.argument.index if it is Err(symbol)
                        if let Err(sym) = ph.argument.index {
                            original_placeholders.insert(sym.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // Simulate the replacements and check for duplicate placeholders
    let mut simulated_placeholders = original_placeholders.clone();
    for (from, to) in &replacements {
        if from == to {
            continue;
        }
        if original_placeholders.contains(to) {
            // Would create a duplicate placeholder
            return None;
        }
        simulated_placeholders.remove(from);
        simulated_placeholders.insert(to.clone());
    }
    // Apply replacements to the format string
    let mut format_content = format_content;
    for (name, value) in &replacements {
        format_content = format_content.replace(&format!("{{{name}}}"), &format!("{{{value}}}"));
        let pattern = format!("{{{name}:");
        if let Some(start) = format_content.find(&pattern)
            && let Some(end) = format_content[start..].find('}')
        {
            let full_placeholder = &format_content[start..start + end + 1];
            let spec = &full_placeholder[pattern.len()..full_placeholder.len() - 1];
            let replacement = format!("{{{value}:{spec}}}");
            format_content = format_content.replace(full_placeholder, &replacement);
        }
    }
    // After replacements, check for duplicate placeholders
    let mut seen = std::collections::HashSet::new();
    let mut dup = false;
    let mut chars = format_content.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if let Some('{') = chars.peek().copied() {
                chars.next(); // skip
                continue;
            }
            let mut name = String::new();
            let mut found = false;
            while let Some(nc) = chars.peek().copied() {
                if nc == '}' || nc == ':' {
                    found = true;
                    break;
                }
                name.push(nc);
                chars.next();
            }
            if found && !name.is_empty() && !seen.insert(name.clone()) {
                dup = true;
                break;
            }
        }
    }
    if dup {
        return None;
    }

    // Process the format string, replacing positional `{}` with inlined variables and leaving complex arguments as explicit arguments

    // Walk through the format string, handling each placeholder
    let mut arg_iter = filtered_args.iter();
    let mut out_fmt = String::with_capacity(format_content.len() + filtered_args.len() * 4);
    let mut chars = format_content.chars().peekable();
    let mut remaining_complex_args = Vec::new();
    let mut any_changes = false;

    while let Some(c) = chars.next() {
        if c == '{' {
            if let Some('{') = chars.peek().copied() {
                // Escaped {{ - keep as is
                out_fmt.push('{');
                out_fmt.push(chars.next().unwrap());
                continue;
            }

            // Find the placeholder content
            let mut placeholder = String::new();
            for nc in chars.by_ref() {
                if nc == '}' {
                    break;
                }
                placeholder.push(nc);
            }

            if placeholder.is_empty() {
                // Empty placeholder {} - try to inline the next argument if it's a simple identifier
                if let Some(arg) = arg_iter.next() {
                    if is_simple_identifier(arg) {
                        // For string literals, extract the content and inline that directly
                        if let Some(content) = extract_string_content(arg) {
                            // For string literals, extract the content and inline it directly
                            // Need to escape quotes in the content for the format string
                            let escaped_content = content.replace('"', "\\\"");
                            out_fmt.push_str(&escaped_content);
                        } else {
                            // Regular identifier - wrap in {}
                            out_fmt.push('{');
                            out_fmt.push_str(arg);
                            out_fmt.push('}');
                        }
                        any_changes = true;
                    } else {
                        // Not a simple identifier, keep the {} and add to remaining args
                        out_fmt.push_str("{}");
                        remaining_complex_args.push(arg.clone());
                        // Don't set any_changes = true here - we're keeping the same structure
                    }
                } else {
                    out_fmt.push_str("{}");
                }
            } else {
                // Placeholder has content (like {:?} or {foo}) - try to inline if it's just a format specifier

                // Check if this is just a format specifier (starts with :)
                if placeholder.starts_with(':') {
                    // This is a format specifier like :? - try to inline the next argument
                    if let Some(arg) = arg_iter.next() {
                        // Only simple identifiers (NOT string literals) can be inlined with format specifiers
                        if is_simple_identifier(arg) && !is_string_literal(arg) {
                            // Regular identifier - can be inlined with format specifier
                            out_fmt.push('{');
                            out_fmt.push_str(arg);
                            out_fmt.push_str(&placeholder);
                            out_fmt.push('}');
                            any_changes = true;
                        } else {
                            // String literals or complex expressions cannot be inlined with format specifiers
                            out_fmt.push('{');
                            out_fmt.push_str(&placeholder);
                            out_fmt.push('}');
                            remaining_complex_args.push(arg.clone());
                            // Don't set any_changes = true here - we're keeping the same structure
                        }
                    } else {
                        out_fmt.push('{');
                        out_fmt.push_str(&placeholder);
                        out_fmt.push('}');
                    }
                } else {
                    // This is already an inlined variable (like {foo}) - keep as is, don't consume arguments
                    out_fmt.push('{');
                    out_fmt.push_str(&placeholder);
                    out_fmt.push('}');
                }
            }
        } else if c == '}' {
            if let Some('}') = chars.peek().copied() {
                // Escaped }} - keep as is
                out_fmt.push('}');
                out_fmt.push(chars.next().unwrap());
            } else {
                out_fmt.push(c);
            }
        } else {
            out_fmt.push(c);
        }
    }

    // If there were any non-inlinable placeholders but no changes, don't suggest anything
    if !any_changes && replacements.is_empty() {
        return None;
    }

    // Collect any remaining unused arguments - these should be added to the remaining complex args
    remaining_complex_args.extend(arg_iter.cloned());

    // Reconstruct the macro call with the new format string and remaining complex arguments
    let macro_name = cx
        .sess()
        .source_map()
        .span_to_snippet(mac_call.path.span)
        .ok()?;
    let result = if remaining_complex_args.is_empty() {
        format!(r#"{macro_name}!("{out_fmt}")"#)
    } else {
        let args_str = remaining_complex_args.join(", ");
        format!(r#"{macro_name}!("{out_fmt}", {args_str})"#)
    };
    Some(result)
}

fn parse_macro_args(tokens: &TokenStream) -> Option<Vec<String>> {
    let mut args = Vec::new();
    let mut current_arg = String::new();
    let mut paren_depth: i32 = 0;
    let mut brace_depth: i32 = 0;
    let mut bracket_depth: i32 = 0;

    for token_tree in tokens.iter() {
        match token_tree {
            TokenTree::Token(token, _) => {
                match &token.kind {
                    TokenKind::Comma
                        if paren_depth == 0 && brace_depth == 0 && bracket_depth == 0 =>
                    {
                        if !current_arg.trim().is_empty() {
                            args.push(current_arg.trim().to_string());
                        }
                        current_arg.clear();
                    }
                    TokenKind::OpenParen => {
                        paren_depth += 1;
                        current_arg.push('(');
                    }
                    TokenKind::CloseParen => {
                        paren_depth = paren_depth.saturating_sub(1);
                        current_arg.push(')');
                    }
                    TokenKind::OpenBrace => {
                        brace_depth += 1;
                        current_arg.push('{');
                    }
                    TokenKind::CloseBrace => {
                        brace_depth = brace_depth.saturating_sub(1);
                        current_arg.push('}');
                    }
                    TokenKind::OpenBracket => {
                        bracket_depth += 1;
                        current_arg.push('[');
                    }
                    TokenKind::CloseBracket => {
                        bracket_depth = bracket_depth.saturating_sub(1);
                        current_arg.push(']');
                    }
                    TokenKind::Literal(lit) => {
                        // Extract the actual literal text
                        let literal_text = match lit.kind {
                            LitKind::Str => {
                                format!("\"{}\"", lit.symbol)
                            }
                            LitKind::StrRaw(n) => {
                                if n == 0 {
                                    format!("r\"{}\"", lit.symbol)
                                } else {
                                    let hashes = "#".repeat(n as usize);
                                    format!("r{hashes}\"{}\"{hashes}", lit.symbol)
                                }
                            }
                            LitKind::Integer => lit.symbol.to_string(),
                            LitKind::Float => lit.symbol.to_string(),
                            _ => lit.symbol.to_string(),
                        };
                        current_arg.push_str(&literal_text);
                    }
                    TokenKind::Ident(name, _) => {
                        current_arg.push_str(&name.to_string());
                    }
                    TokenKind::Lifetime(symbol, _is_raw) => {
                        current_arg.push('\'');
                        current_arg.push_str(&symbol.to_string());
                    }
                    TokenKind::NtLifetime(ident, _is_raw) => {
                        current_arg.push('\'');
                        current_arg.push_str(&ident.name.to_string());
                    }
                    TokenKind::NtIdent(ident, is_raw) => {
                        if *is_raw == IdentIsRaw::Yes {
                            current_arg.push_str(&format!("r#{}", ident.name));
                        } else {
                            current_arg.push_str(&ident.name.to_string());
                        }
                    }
                    // Handle all other common tokens using static mapping
                    kind => {
                        if let Some(token_str) = token_kind_to_static_str(&token.kind) {
                            current_arg.push_str(token_str);
                            continue;
                        }
                        unimplemented!(
                            "Unhandled token kind in uninlined_format_args lint: {kind:?}"
                        )
                    }
                }
            }
            TokenTree::Delimited(_, _, delim_token, delimited_tokens) => {
                // Handle delimited token groups (like parentheses, braces, brackets)
                let open = delimiter_to_static_str(delim_token, true);
                let close = delimiter_to_static_str(delim_token, false);
                current_arg.push_str(open);
                // Recursively parse the delimited tokens
                if let Some(inner_args) = parse_macro_args(delimited_tokens) {
                    current_arg.push_str(&inner_args.join(", "));
                }
                current_arg.push_str(close);
            }
        }
    }

    if !current_arg.trim().is_empty() {
        args.push(current_arg.trim().to_string());
    }

    Some(args)
}

fn extract_string_content(lit: &str) -> Option<String> {
    let lit = lit.trim();
    if lit.starts_with('"') && lit.ends_with('"') && lit.len() >= 2 {
        Some(lit[1..lit.len() - 1].to_string())
    } else if lit.starts_with("r#\"") && lit.ends_with("\"#") && lit.len() >= 4 {
        Some(lit[3..lit.len() - 2].to_string())
    } else if lit.starts_with("r\"") && lit.ends_with("\"") && lit.len() >= 3 {
        Some(lit[2..lit.len() - 1].to_string())
    } else {
        None
    }
}

fn is_simple_identifier(arg: &str) -> bool {
    let arg = arg.trim();

    // Check if it's a simple identifier (letters, digits, underscore, starting with letter or underscore)
    if !arg.is_empty()
        && arg.chars().all(|c| c.is_alphanumeric() || c == '_')
        && arg
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
    {
        return true;
    }

    // Check if it's a string literal that can be inlined
    if arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2 {
        return true;
    }

    // Check if it's a raw string literal
    if (arg.starts_with("r#\"") && arg.ends_with("\"#"))
        || (arg.starts_with("r\"") && arg.ends_with("\""))
    {
        return true;
    }

    false
}

fn is_string_literal(arg: &str) -> bool {
    let arg = arg.trim();
    arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2
        || (arg.starts_with("r#\"") && arg.ends_with("\"#"))
        || (arg.starts_with("r\"") && arg.ends_with("\""))
}

const fn delimiter_to_static_str(delim: &Delimiter, open: bool) -> &'static str {
    use Delimiter::*;
    match delim {
        Parenthesis if open => "(",
        Parenthesis => ")",
        Brace if open => "{",
        Brace => "}",
        Bracket if open => "[",
        Bracket => "]",
        Invisible(_) => "",
    }
}

const fn token_kind_to_static_str(kind: &TokenKind) -> Option<&'static str> {
    use TokenKind::*;
    match kind {
        Eq => Some("="),
        Lt => Some("<"),
        Le => Some("<="),
        EqEq => Some("=="),
        Ne => Some("!="),
        Ge => Some(">="),
        Gt => Some(">"),
        AndAnd => Some("&&"),
        OrOr => Some("||"),
        Bang => Some("!"),
        Tilde => Some("~"),
        Plus => Some("+"),
        Minus => Some("-"),
        Star => Some("*"),
        Slash => Some("/"),
        Percent => Some("%"),
        Caret => Some("^"),
        And => Some("&"),
        Or => Some("|"),
        Shl => Some("<<"),
        Shr => Some(">>"),
        PlusEq => Some("+="),
        MinusEq => Some("-="),
        StarEq => Some("*="),
        SlashEq => Some("/="),
        PercentEq => Some("%="),
        CaretEq => Some("^="),
        AndEq => Some("&="),
        OrEq => Some("|="),
        ShlEq => Some("<<="),
        ShrEq => Some(">>="),
        At => Some("@"),
        Dot => Some("."),
        DotDot => Some(".."),
        DotDotDot => Some("..."),
        DotDotEq => Some("..="),
        Comma => Some(","),
        Semi => Some(";"),
        Colon => Some(":"),
        PathSep => Some("::"),
        RArrow => Some("->"),
        LArrow => Some("<-"),
        FatArrow => Some("=>"),
        Pound => Some("#"),
        Dollar => Some("$"),
        Question => Some("?"),
        SingleQuote => Some("'"),
        OpenParen => Some("("),
        CloseParen => Some(")"),
        OpenBrace => Some("{"),
        CloseBrace => Some("}"),
        OpenBracket => Some("["),
        CloseBracket => Some("]"),
        // Invisible delimiters produce no output
        OpenInvisible(_) => Some(""),
        // Invisible delimiters produce no output
        CloseInvisible(_) => Some(""),
        // This shouldn't reach here due to earlier match, but fallback
        Literal(_) => None,
        // This shouldn't reach here due to earlier match, but fallback
        Ident(_, _) => None,
        // Non-terminal ident, rare in macro args
        NtIdent(_, _) => None,
        // This would need dynamic formatting, fallback for now
        Lifetime(_, _) => None,
        // Non-terminal lifetime, rare in macro args
        NtLifetime(_, _) => None,
        // Doc comments shouldn't appear in macro args
        DocComment(_, _, _) => None,
        // End of file, shouldn't appear in macro args
        Eof => Some(""),
    }
}

#[test]
fn ui() {
    dylint_uitesting::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
