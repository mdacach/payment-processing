# Money type: lightweight specification

## Purpose and representation

Replace `type Money = i64` in `src/types.rs` with an opaque `Money` value that stores a signed `i64` count of ten-thousandths of a currency unit. For example, `1.5050` is stored as `15_050`. The type has no floating-point fields or floating-point arithmetic. It represents values from `-922337203685477.5808` through `922337203685477.5807`.

Derive `Copy`, `Clone`, `Debug`, `Default`, `Eq`, `PartialEq`, `Ord`, and `PartialOrd`; zero is the default. Keep the field private. Negative values are necessary because disputes and chargebacks can make account balances negative. Whether *transaction amounts* must be positive belongs to event validation, not the `Money` representation.

## Construction and operations

- Provide `Money::ZERO`, `Money::from_minor_units(i64)`, and `Money::minor_units() -> i64`. The unit name must be explicit: these are ten-thousandths, not cents or whole currency units.
- Make exact decimal text the preferred external input: `FromStr` (or an equivalent `parse_decimal`) accepts an optional sign, digits, and at most four fractional digits; it right-pads the fraction to four digits. It rejects malformed text, excess fractional digits, and out-of-range values. Convert with checked integer arithmetic, including the `i64::MIN` edge case. Examples: `"1.5" -> 15_000`, `"1.5050" -> 15_050`, `"0.0001" -> 1`; `"1.23456"` is an error.
- Provide a **fallible** `TryFrom<f64>` because existing callers may supply floats. Reject NaN, infinity, out-of-range values, and values whose shortest round-trip decimal representation has more than four fractional digits. Normalize scientific notation before applying the decimal parser. Do not silently round an input with extra decimal places. This accepts ordinary literals such as `1.5050_f64`; it may reject values produced by prior floating-point calculations, such as `0.1 + 0.2`. A float cannot preserve the user's original decimal spelling or guarantee all four places at large magnitudes, so banking ingestion should retain the source decimal text and parse that instead.
- Provide `checked_add` and `checked_sub`, returning `Option<Money>` or a typed overflow error. All payment-processing balance changes must use these methods and return an error on overflow. Do not implement wrapping or saturating arithmetic. `Add`/`Sub` and assignment operators are optional only if their overflow behavior is explicit; they must not be used for fallible account updates.
- Format values from integer units, with four fractional digits and correct handling of `i64::MIN`. This makes logs and future output unambiguous. Display formatting must never convert through a float.

Once constructed, addition, subtraction, comparison, and formatting are exact within the `i64` range. No currency code or cross-currency conversion is in scope.

## Implementation plan

1. Add the opaque type, decimal parser, float adapter, formatter, and checked arithmetic in `src/types.rs` (or a small `money` module re-exported from there). Unit-test zero, signs, four-place values, rejected fifth places, non-finite floats, boundaries, and overflow.
2. Replace integer amount literals in `src/main.rs` and regression tests with explicit constructors. Update Hegel generators in `src/payment_processor/property_tests.rs` to generate bounded minor-unit integers and wrap them in `Money`; preserve the intended ranges by deciding whether their current `1..1_000_000` limits mean whole units or minor units.
3. In `src/payment_processor.rs`, precompute every affected balance with checked arithmetic before changing the account or deposit status. Return an error without partial mutation if any calculation overflows. Cover deposit, withdrawal, dispute, resolve, and chargeback paths.
4. Add processor checks at the new precision: `0.0001` transfers, arithmetic identities (`total = available + held`), overflow rejection with unchanged state, and negative balances after a withdrawal followed by a dispute. Run the existing regression and state-machine tests; inspect any failures before changing their expectations.

## Refactoring risk

**Medium for the type replacement; high if float input is treated as an accuracy guarantee.** The project is small and `Money` is confined to `src/types.rs`, `src/event.rs`, `src/payment_processor.rs`, `src/main.rs`, and processor tests. There is currently no file, network, or database amount format to migrate. The compiler will expose most sites that assumed an `i64` (literals, Hegel integer generators, and `+=`/`-=`).

The main behavioral risks are less visible: changing the meaning of existing integers by a factor of 10,000; accepting a float whose intended decimal has already been lost; arithmetic overflow; and mutating one field or deposit status before a later checked calculation fails. Explicit unit names, exact decimal ingestion, atomic precomputation, and boundary tests address those risks. The existing test changes in the working tree should be preserved during implementation.

## Implementation status

Implemented on 2026-09-28. All four plan steps are complete; implementation decisions and remaining input-policy ambiguities are recorded in `artifacts/money-problems.md`. The full test suite passes.
