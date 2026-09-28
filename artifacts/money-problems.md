# Money implementation: issues and ambiguities

## 1. Property-test amount ranges

- Category: `other`; phase: caller migration.
- Ambiguity: The existing Hegel bounds (`1..1_000_000` for deposits and `1..500_000` for withdrawals) were bare `i64` values. Changing the generator to minor units without scaling would reduce the tested monetary range by 10,000.
- Decision: Preserve the original whole-unit lower and upper bounds by multiplying both by 10,000, then generate minor-unit integers so fractional amounts are covered.
- Lesson: Label stored units explicitly at every integer boundary.

## 2. Float inputs cannot prove decimal intent

- Category: `design-flaw`; phase: type construction.
- Ambiguity: An `f64` has no record of the decimal text that produced it. Even a fallible conversion cannot tell whether a rounded float originally had more than four decimal places.
- Decision: `TryFrom<f64>` parses the float's shortest round-trip decimal representation and rejects excess precision; exact decimal text remains the recommended input for banking data. This limitation is inherent and remains open for any future ingestion interface.
- Lesson: Accept source decimal text before conversion to binary floating point when exact user intent matters.

## 3. Failed events previously created accounts

- Category: `integration`; phase: checked processor arithmetic.
- Issue: The old handlers called `BTreeMap::entry(...).or_default()` before validating the event. With checked arithmetic, that could leave a newly created account after an error, contrary to the specification's unchanged-state requirement.
- Fix: Compute against a local account value, then insert it and change deposit status only after every check succeeds.
- Lesson: Error paths must be checked for mutations as well as numerical overflow.

## 4. Transaction amount sign

- Category: `other`; phase: processor migration.
- Ambiguity: Existing event handling does not reject zero or negative deposit and withdrawal amounts. The specification assigns transaction validation to the event boundary but does not define its policy.
- Decision: Preserve the current behavior for this migration. A separate validation rule is needed before accepting untrusted transaction events.
- Lesson: The signed `Money` type is required for balances and should not by itself determine valid transaction inputs.

## 5. Strict Clippy verification

- Category: `integration`; phase: verification.
- Issue: `cargo clippy --all-targets -- -D warnings` fails on existing dead-code warnings for event variants used by tests but not the demo binary, plus existing `collapsible_if` and `needless_borrow` warnings.
- Fix: Removed avoidable unused-item warnings from the new `Money` API. The remaining warnings are unrelated to the money migration; regular tests and formatting remain the verification gates for this change.
- Lesson: A strict warning gate needs a clean baseline or targeted lint allowances before it can be used as a required project check.
