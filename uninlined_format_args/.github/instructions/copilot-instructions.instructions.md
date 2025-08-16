---
applyTo: '**/*.rs'
---
## Purpose of lint
simple identifiers get inlined, complex expressions remain as arguments but their placeholders stay in the format string

## Testing:
* run `j test` INSTEAD OF `cargo test`
* There is no `--bless` argument for tests
* You are FORBIDDEN to use `cargo test` or `j test` with arguments such as 2>&1, head, grep, or any other filtering commands.
