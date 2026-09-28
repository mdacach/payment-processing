# CSV transaction CLI implementation plan

Status: Implemented

## Goal

Run `cargo run -- transactions.csv > accounts.csv`. The binary parses the entire CSV into an ordered collection of events, then sends them to `PaymentProcessor` in file order and writes final account balances as CSV to standard output. Keep the processor independent of CSV and command-line concerns.

## Behavior to implement

- Accept exactly one command-line argument: the input CSV path. Report missing or extra arguments and file-open errors on standard error with a nonzero exit status.
- Read a header with `type`, `client`, `tx`, and `amount`; accept spaces around headers and values. Parse and collect every row before processing any event. Preserve input order and record numbers for diagnostics. This deliberately materializes the input; streaming can be compared as a later improvement.
- Map `deposit`, `withdrawal`, `dispute`, `resolve`, and `chargeback` to the existing `Event` variants. Parse client IDs as `u16`, transaction IDs as `u32`, and money directly from decimal text, never through `f64`. Require an amount for deposits and withdrawals; allow a blank amount for the other three types. If an amount has more than four fractional digits, discard the extra digits without rounding before converting it to `Money`. Verify the fixed-point crate's parser and formatter behavior so truncation is explicit and cannot silently round.
- Treat malformed CSV, unknown event types, invalid IDs, and invalid or missing required amounts as input errors. Include the input path and record number in the diagnostic; exit nonzero. Keep standard output free of diagnostics.
- Pass the collected events to `PaymentProcessor::on_event` in input order. For a transaction the processor rejects (such as the sample's insufficient-funds withdrawal), log a contextual warning to standard error and continue. Such a row must not alter the account balance.
- After processing, write `client,available,held,total,locked` and one row per account to standard output. Use the processor's `BTreeMap` order for deterministic client order. Always format monetary values with exactly four digits after the decimal point (for example, `2.0000`); write `false`/`true` for lock state. The sample should yield client 1 with `1.5000` and client 2 with `2.0000` available and total, both with `0.0000` held and unlocked.
- Initialize `tracing` in the binary with a subscriber that writes to standard error. Log the input path and parsed row count, rejected transactions with their record numbers and reasons, and a completion summary. Keep standard output exclusively for account CSV.

## Implementation steps

1. [x] Add `csv`, `tracing`, and `tracing-subscriber` to [Cargo.toml](../Cargo.toml). Use the CSV reader and writer so quoted fields, line endings, and CSV output are handled consistently.
2. [x] Add a read-only account iterator to [PaymentProcessor](../src/payment_processor.rs), for example `accounts(&self) -> impl Iterator<Item = (ClientId, &Account)>`. Keep the map itself private.
3. [x] Replace the hard-coded example in [main.rs](../src/main.rs) with argument handling, complete CSV parsing into an ordered `Vec` of events with record numbers, ordered calls to `on_event`, and final CSV writing. Put parsing in a small private binary module if `main.rs` becomes crowded. Use `anyhow` context for path and record diagnostics. Format output from the fixed-point representation so it has exactly four fractional digits.
4. [x] Set up `tracing` output on standard error and add the input, rejection, and completion events described above.
5. [x] Add a sample input file and document the command, fixed four-place output, truncation, memory tradeoff, and rejected-transaction policy in [README.md](../README.md).
6. [x] Add integration tests that run the binary with temporary CSV files. Cover the supplied sample and exact four-place output; each of the three dispute-related events; whitespace; excess-precision truncation without rounding; malformed rows; and a rejected transaction followed by a successful one. Assert stdout, stderr logging, and exit status where relevant.

## Verification

- `cargo fmt --check`
- `cargo test`
- `cargo build`
- Run the documented sample command and compare the resulting account CSV with the expected balances.

## Scope boundary

This feature wires CSV input and output to the current processor. Existing processor rules, including which events it rejects, remain the source of truth. Changes to transaction semantics or balance arithmetic should be handled separately if the new integration tests expose them.
