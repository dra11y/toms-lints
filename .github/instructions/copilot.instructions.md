---
applyTo: '**/*.rs'
---

## Critical
* **FORBIDDEN** to use line comments (2 /); I will add these.
* **FORBIDDEN** to change more than 50 lines at a time without explanation and permission.
* **MUST** add doc comments (3 /) to all functions, structs, enums, and variants.
* **MUST** add `use import::...` at top of file if using an import more than once, **EXCEPT** to disambiguate (e.g. `use std::time::Duration` and `chrono::Duration`).
* **MUST** use rust idioms and patterns and disregard all other language idioms and patterns.
* **FORBIDDEN** to nest if [else], let blocks, or closures; **AVOID** `else` statements; **INSTEAD**, use guard clauses such as `if !condition { return ..; }` and `let Some/Ok(..) = .. else { return ..; }`.
* **MUST** propose top 2 or 3 solutions with drawbacks/cons of each, with minimal code examples, in chat.
* **FORBIDDEN** to assume anything < 95% confidence; **INSTEAD**, ask a series of yes/no questions to clarify **only** the points of < 95% confidence. **MUST** state confidence level of questions.
* **FORBIDDEN** to edit code until I say "go".
* **FORBIDDEN** to use `.unwrap()` or anything that can panic in production; okay to use in tests.
* **MUST** use inlined format args: format!("{display}, {debug:?}");
* **FORBIDDEN** to use emojis.
* **MUST** use constants or statics for all strings, numbers, and other literals that are used more than once in the code.

## Testing
* run `j test` INSTEAD OF `cargo test`
* FORBIDDEN to use `cargo test` or arguments at end of test to filter output
* There is no `--bless` argument
* Continue fixing compile errors until resolved
