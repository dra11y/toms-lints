# Nesting Depth Lint

### What it does
Detects code whose structural nesting (blocks, matches, if/else chains, etc.) exceeds a configurable maximum depth.

### Why is this bad?
Deeply nested code is harder to read, reason about, and maintain. Flattening control flow with early returns and guard clauses usually yields clearer code.

### Configuration
Add a `[[lints]]` entry for `nesting_depth` (per Dylint config conventions) and supply any of the keys below (all optional):

```toml
[lints.nesting_depth]
# Maximum allowed nesting depth (excluding initial item / fn context)
max_depth = 3

# Ignore closures when counting depth
ignore_closures = true

# Maximum allowed items (statements + expr) in a single then-block
max_then_items = 20

# Maximum allowed consecutive if/else-if/else branches under a single root if
max_consec_if_else = 10

# Names of macros whose expanded bodies should NOT contribute to nesting depth.
# Each entry matches the macro's local invocation name (after any `use as` rename).
# Example for Yew UI code:
ignore_macros = ["html"]

# Enable internal debug logging
debug = false
```

If a macro name is re-exported or renamed (e.g. `use yew::html as h; h!{}`) then add the renamed identifier (`"h"`).

### Known problems
Re-export detection is name-based only; full canonical macro path matching is not yet implemented.

### Example

Before (counts deep nesting inside `html!`):

```rust,ignore
match api.state() {
	ApiState::Ok(resp) => html! { <div><span>{format!("{resp:?}")}</span></div> },
	_ => html! { <p>{"loading"}</p> }
}
```

With `ignore_macros = ["html"]`, the internal structure of each `html!` expansion is treated as a leaf and does not inflate the nesting depth.

Use early returns / guard clauses to flatten instead of stacking nested `if`/`match` constructs.
