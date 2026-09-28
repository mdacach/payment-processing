# Converting the binary crate into a library

# Prompt:

This is a binary crate. I want you to transform it into a library crate, with the main.rs code being a binary that constructs a couple of events and calls into the library code.

# Output:

The crate now has a library in [src/lib.rs](/Users/crs/Development/payment-processing/src/lib.rs) that exposes the events, processor, account, and money types. [src/main.rs](/Users/crs/Development/payment-processing/src/main.rs) is a small binary example: it deposits 10, withdraws 8, and prints `Balance: 2`.

Verification passed: all 3 tests, `cargo fmt --check`, `git diff --check`, and `cargo run --quiet`.
