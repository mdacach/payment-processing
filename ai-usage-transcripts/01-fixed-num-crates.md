# Surveying some Rust "fixed-num" crates

Dealing with money requires precision. A straightforward enough way is to simply use integers, representing the number of cents (or desired precision). With this prompt I wanted to survey Rust crates that could be alternatives for dealing with fixed precision values.

At the end of the day, I found the output verbose (but I asked for it, kind of) and not super comprehensive. Manual web searches found me some other contenders, but overall none of those seemed to be a clear cut winner (nor without drawbacks) and I will probably simply use integers anyway. Also because I will only be doing simple additions and subtractions, so no fancier operations will be needed.

# Prompt:

Investigate Rust crates for working with fixed precision values. Find at least five such crates, and give me examples to show their usage ergonomics.

The domain is a payment processor which requires precision of up to four places after the decimal.

(This will be used to decide whether I will use a ready-made crate, or simply create a money-unit-type and represent it all as integers in the code.)


# Output:

For a payment processor with a hard requirement of **no more than 4 decimal places**, I’d separate the Rust ecosystem into two families:

- **Runtime-scale decimals** — e.g. `rust_decimal`, `fpdec`, `fastnum`. Convenient, but a value can carry 0, 2, 4, 10… fractional digits unless your own `Money` wrapper enforces the rule.
- **Type-level fixed-scale decimals** — e.g. `fixnum`, `rust-fixed-point-decimal`. The 4-decimal invariant can be encoded in the type itself.

That makes `fixnum` especially interesting for your “crate vs just store integer units” decision, because conceptually it is very close to the integer approach.

## Summary

| Crate | Representation | Scale enforced by type? | Allocation | Payment ergonomics |
|---|---|---:|---:|---|
| `rust_decimal` | 96-bit integer + runtime scale | No | No | Excellent |
| `fixnum` | integer × `10^-P` | **Yes** | No | Excellent, explicit |
| `fpdec` | `i128` + runtime scale | No | No | Very good |
| `fastnum` | fixed-size coefficient + runtime scale/context | No | No | Powerful, heavier |
| `decimal-rs` | high-precision coefficient + scale | No | No | Reasonable |
| `rust-fixed-point-decimal` | `i128` + const-generic scale | **Yes** | No | Nice model, but discontinued |
| `bigdecimal` | `BigInt` + scale | No | **Yes** | Overkill here |

One crate I would **not** use for this particular purpose is the popular `fixed` crate. It is **binary** fixed point: fractional units are powers of 1/2, not powers of 1/10. Therefore `0.0001` is generally not exactly representable. :chatgpt-content-reference{index="0"}

---

# 1. `rust_decimal`

This is probably the most obvious general-purpose choice.

Current `rust_decimal` represents a decimal as a **96-bit integer plus a decimal scale from 0–28**. It has compile-time decimal literals, checked arithmetic, configurable rounding, serde support, and integrations for PostgreSQL/Diesel/etc. :chatgpt-content-reference{index="1"}

```rust
use rust_decimal::{Decimal, RoundingStrategy};
use rust_decimal_macros::dec;

let balance = dec!(1250.1234);
let deposit = dec!(100.2500);

let new_balance = balance
    .checked_add(deposit)
    .expect("overflow");

assert_eq!(new_balance, dec!(1350.3734));
```

For a computation producing more than four places:

```rust
use rust_decimal::{RoundingStrategy};
use rust_decimal_macros::dec;

let amount = dec!(12.3456);
let rate = dec!(0.015);

let fee = (amount * rate)
    .round_dp_with_strategy(
        4,
        RoundingStrategy::MidpointNearestEven,
    );

assert_eq!(fee, dec!(0.1852));
```

`round_dp()` uses midpoint-to-even by default, while `round_dp_with_strategy()` lets you make the policy explicit. :chatgpt-content-reference{index="2"}

### The important downside

Nothing prevents this:

```rust
let amount = dec!(12.345678901234);
```

So:

```rust
struct Money(Decimal);
```

would still need an invariant such as:

```rust
impl Money {
    fn try_new(value: Decimal) -> Result<Self, Error> {
        if value.scale() > 4 {
            return Err(Error::TooManyFractionalDigits);
        }

        Ok(Self(value))
    }
}
```

This is a significant architectural distinction: `Decimal` gives you **correct decimal arithmetic**, but not your domain's `scale <= 4` invariant.

### My view

Very attractive if you want:

- mature decimal ergonomics;
- PostgreSQL `NUMERIC`;
- serde;
- convenient parsing/display;
- occasional intermediate calculations with >4 digits.

You would almost certainly still want a domain wrapper:

```rust
struct Money(Decimal);
```

rather than exposing `Decimal` everywhere.

---

# 2. `fixnum`

This one is particularly relevant to you.

`fixnum` stores a value in an integer and specifies decimal precision in the type. For example:

```rust
type Money = FixedPoint<i64, U4>;
```

means effectively:

```text
stored integer / 10_000
```

The crate explicitly describes its representation as an integer pre-multiplied by `10^PRECISION`; its API emphasizes **checked arithmetic and explicit rounding**. :chatgpt-content-reference{index="3"}

For your system:

```rust
use fixnum::{
    FixedPoint,
    fixnum,
    typenum::U4,
    ops::*,
};

type Money = FixedPoint<i64, U4>;

let balance: Money = fixnum!(1250.1234, 4);
let deposit: Money = fixnum!(100.2500, 4);

let balance = balance.cadd(deposit)?;

assert_eq!(balance, fixnum!(1350.3734, 4));
```

The particularly nice part for payment systems is multiplication/division:

```rust
let a: Money = fixnum!(10.0000, 4);
let b: Money = fixnum!(0.3333, 4);

let result =
    a.rmul(b, RoundMode::Nearest)?;
```

Rounding is not silently hidden inside `*`; `rmul` requires a rounding policy. :chatgpt-content-reference{index="4"}

Parsing can also enforce exact representability:

```rust
let amount =
    Money::from_str_exact("12.3456")?;

// Error: cannot exactly represent at scale 4
let invalid =
    Money::from_str_exact("12.34567")?;
```

That `from_str_exact` behavior is particularly good for API boundaries. Ordinary `FromStr` may round; `from_str_exact` rejects values that cannot be represented exactly. :chatgpt-content-reference{index="5"}

### Wrapper ergonomics

The authors explicitly support wrapping fixed-point types:

```rust
type RawMoney = FixedPoint<i64, U4>;

struct Money(RawMoney);
```

and their literal macro is designed to work with wrapper types. :chatgpt-content-reference{index="6"}

### Range

With:

```rust
FixedPoint<i64, U4>
```

your raw integer is an `i64`, so approximately:

```text
±922,337,203,685,477.5807
```

That is probably already vastly larger than an individual ledger amount requires.

Using `i128` makes the range effectively irrelevant for most conventional payment systems.

### My view

This is the crate I would investigate most closely if your alternative is:

```rust
struct Money(i64); // ten-thousandths
```

because `fixnum` is essentially that representation **plus a well-defined arithmetic library**.

It buys you:

- compile-time scale;
- parsing/formatting;
- checked arithmetic;
- widened intermediate operations;
- explicit rounding;
- serde;
- fewer home-grown arithmetic bugs.

The API is less pretty than `rust_decimal`, but I actually consider that an advantage in ledger code:

```rust
a.cadd(b)?
a.rmul(b, RoundMode::Nearest)?
a.rdiv(b, RoundMode::Floor)?
```

The code makes failure and rounding visible.

---

# 3. `fpdec`

`fpdec` is aimed explicitly at business quantities and money. It stores an `i128` coefficient plus a runtime count of fractional digits, currently supporting up to 18 fractional digits. :chatgpt-content-reference{index="7"}

Basic use is pleasant:

```rust
use fpdec::{Dec, Decimal};

let balance = Dec!(1250.1234);
let deposit = Dec!(100.2500);

let result = balance + deposit;

assert_eq!(result.to_string(), "1350.3734");
```

Parsing:

```rust
use fpdec::Decimal;
use std::str::FromStr;

let amount =
    Decimal::from_str("38.2070")?;

assert_eq!(amount.to_string(), "38.2070");
```

It exposes checked variants for addition/subtraction/multiplication/division, and dedicated traits for rounded multiplication/division, quantization, and rounding. :chatgpt-content-reference{index="8"}

Conceptually:

```rust
use fpdec::{Dec, MulRounded};

let amount = Dec!(12.3456);
let rate = Dec!(0.015);

// Conceptually: explicitly select the fractional
// precision of the resulting monetary value.
let fee = amount.mul_rounded(rate, 4);
```

The exact API is somewhat more trait-heavy than `rust_decimal`, but the design is appealing: exact integer-backed decimal arithmetic, with rounding surfaced as an operation.

### Important difference from `fixnum`

These are both valid `Decimal` values:

```rust
Dec!(1.23)
Dec!(1.23456789)
```

because precision is a property of the **value**, not its Rust type.

So once again I'd write:

```rust
struct Money(Decimal);
```

and validate `n_frac_digits() <= 4` at construction.

### Maturity

`fpdec 0.14.1` was released in March 2026. Its docs still describe it as “work in progress, but most of the API is stable.” :chatgpt-content-reference{index="9"}

I would consider it, but for production payments I would scrutinize its release history and implementation more closely than `rust_decimal`.

---

# 4. `fastnum`

`fastnum` is more ambitious.

It provides fixed-size decimal types such as:

```rust
D128
D256
D512
```

without heap allocation. Its decimals track scale/sign plus exceptional conditions including rounded, inexact, overflow, underflow, NaN and infinity. :chatgpt-content-reference{index="10"}

Usage is excellent:

```rust
use fastnum::*;

let balance = dec128!(1250.1234);
let deposit = dec128!(100.2500);

let result = balance + deposit;

assert_eq!(result, dec128!(1350.3734));
```

Literal arithmetic looks natural:

```rust
use fastnum::*;

let amount = dec128!(12.3456);
let rate = dec128!(0.015);

let fee = (amount * rate).round(4);
```

One interesting feature is that significant trailing zeroes are retained:

```rust
use fastnum::*;

assert_eq!(
    dec128!(1.30) + dec128!(1.20),
    dec128!(2.50)
);

assert_eq!(
    dec128!(1.30) * dec128!(1.20),
    dec128!(1.5600)
);
```

That behavior is deliberate. :chatgpt-content-reference{index="11"}

It also has an unusually broad integration surface: serde, SQLx, PostgreSQL, MySQL, Diesel and `tokio-postgres`. :chatgpt-content-reference{index="12"}

### Interesting payment-system feature: arithmetic signals

You can detect that an operation rounded:

```rust
let result = /* operation */;

if result.is_op_rounded() {
    // operation discarded digits
}
```

The same machinery exposes inexact/overflow/underflow states. :chatgpt-content-reference{index="13"}

That is potentially useful in financial code.

### What gives me pause

It behaves closer to a full-fledged IEEE-style **decimal floating-point environment**.

It supports things such as:

```text
NaN
+Infinity
-Infinity
```

For a `Money` value, I usually don't want those states to be representable at all.

So you would definitely want:

```rust
struct Money(D128);
```

with validation.

Also, it solves a substantially larger problem than your requirement.

For a ledger where all stored amounts have four places, it may be more machinery than you need.

---

# 5. `decimal-rs`

`decimal-rs` offers a high-precision decimal with up to 38 significant digits. It has a runtime scale and offers rounding/truncation/normalization operations. :chatgpt-content-reference{index="14"}

Ergonomics:

```rust
use decimal_rs::Decimal;

let balance: Decimal =
    "1250.1234".parse()?;

let deposit: Decimal =
    "100.2500".parse()?;

let result = balance + deposit;

assert_eq!(
    result.to_string(),
    "1350.3734"
);
```

Rounding:

```rust
let value: Decimal =
    "12.345678".parse()?;

let money = value.round(4);

assert_eq!(money.to_string(), "12.3457");
```

Internally it exposes the scale:

```rust
assert_eq!(money.scale(), 4);
```

and can normalize to a requested scale. :chatgpt-content-reference{index="15"}

Serde is optional. :chatgpt-content-reference{index="16"}

### My view

Technically reasonable, but it doesn't provide an obvious advantage for this use case over `rust_decimal`.

I'd only choose it if benchmarking or some specific compatibility requirement favored it.

---

# 6. `rust-fixed-point-decimal`

This one is worth studying because its **design is almost exactly what you are considering**.

It represents:

```rust
Decimal<P>
```

with an `i128` coefficient and compile-time decimal scale.

So:

```rust
Decimal<4>
```

really means four fractional decimal places. :chatgpt-content-reference{index="17"}

Usage:

```rust
use rust_fixed_point_decimal::*;

let amount =
    Decimal::<4>::from_str("38.207")?;

assert_eq!(
    amount.to_string(),
    "38.2070"
);
```

And this is especially interesting:

```rust
let x = Dec!(17.5);
let y = Dec!(6.47);

let result: Decimal<4> =
    x.mul_rounded(y);
```

The **result type itself decides the scale**. :chatgpt-content-reference{index="18"}

The compiler can even derive output scales. If you multiply:

```text
Decimal<1> × Decimal<2>
```

the result is naturally:

```text
Decimal<3>
```

because the scale is known statically. :chatgpt-content-reference{index="19"}

This is intellectually very appealing.

Unfortunately, the crate itself says:

> development ... has been ceased, in favor of fpdec.rs

It also depends on incomplete/nightly const-generic functionality, and its own docs explain that the large number of generic instantiations and awkward arithmetic type relationships outweighed the advantages. :chatgpt-content-reference{index="20"}

So I would **not use it**, but I would absolutely study its design.

It demonstrates both why type-level decimal scale is appealing and where it can become cumbersome.

---

# Bonus: `bigdecimal`

`BigDecimal` is another established option:

```rust
use bigdecimal::BigDecimal;
use std::str::FromStr;

let balance =
    BigDecimal::from_str("1250.1234")?;

let deposit =
    BigDecimal::from_str("100.2500")?;

let result = balance + deposit;
```

It is backed by a `BigInt` plus decimal scale and offers effectively arbitrary precision. :chatgpt-content-reference{index="21"}

For your use case, I wouldn't pick it.

You know your precision requirement and likely know a sensible maximum magnitude. Paying for dynamically-sized integers is unnecessary complexity.

---

# How these compare with `struct Money(i64)`

Suppose you simply establish:

```rust
/// Number of 1/10,000 currency units.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct Money(i64);
```

Then:

```rust
impl Money {
    pub const SCALE: i64 = 10_000;

    pub const fn from_units(units: i64) -> Self {
        Self(units)
    }

    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }
}
```

This is extremely compelling for a ledger.

Your database can also store:

```text
BIGINT amount
```

and there is never ambiguity about what:

```text
123456
```

means:

```text
12.3456
```

## But multiplication/division are where the crate starts earning its keep

Imagine calculating:

```text
12.3456 × 1.5%
```

Raw integer implementation becomes something along these lines:

```rust
let amount = 123_456_i64; // 12.3456
let rate   = 150_i64;     // perhaps 0.0150

let intermediate =
    i128::from(amount) * i128::from(rate);

// Now:
// - what scale does intermediate have?
// - how do you round?
// - what happens on overflow?
// - how do negative midpoint cases behave?
// - how do you convert back?
```

You now have to implement exactly the sort of machinery `fixnum` already provides.

This is why I see the real comparison as:

```text
custom Money(i64)
```

versus

```text
Money(FixedPoint<i64, U4>)
```

rather than:

```text
integer vs decimal
```

`fixnum` **is an integer representation**, with the annoying and subtle fixed-point arithmetic already implemented.

---

# One architectural point I would strongly recommend

Regardless of crate choice, I would **not use the numeric crate type as your domain type**.

Don't do:

```rust
struct Account {
    balance: Decimal,
}
```

Prefer:

```rust
struct Money(/* underlying numeric representation */);
```

and expose only valid operations.

For example:

```rust
pub struct Money(FixedPoint<i64, U4>);

impl Money {
    pub fn checked_add(
        self,
        rhs: Money,
    ) -> Result<Money, MoneyError> {
        self.0
            .cadd(rhs.0)
            .map(Money)
            .map_err(MoneyError::from)
    }
}
```

Then you can make illegal things inconvenient or impossible.

You might additionally distinguish:

```rust
Money
Percentage
ExchangeRate
FeeRate
```

because:

```rust
Money + Money
```

makes sense, whereas:

```rust
Money * Money
```

almost certainly doesn't.

A generic decimal type cannot protect you from that.

---

# An important question: does “up to 4 decimals” mean `<=4`, or exactly scale 4 internally?

I would seriously consider making the **internal representation always exactly four places**, even if the external currency normally has fewer.

Thus:

```text
USD 12.34     -> 123400
BRL 12.34     -> 123400
some asset 12.3456 -> 123456
```

rather than carrying variable scales around.

That gives you one canonical representation:

```text
Money(i64)    // 10^-4 units
```

or:

```rust
type RawMoney = FixedPoint<i64, U4>;
```

It makes equality, hashing, persistence, replay, ledger balancing, and DST/model checking considerably simpler.

Formatting is a presentation concern:

```text
123400 internally
   ↓
"12.34 USD"
```

You don't need `"12.3400"` to be the user-visible format.

---

# My shortlist

For this specific system, I would prototype **three implementations**, not seven:

### `fixnum`

```rust
type RawMoney =
    FixedPoint<i64, U4>;

struct Money(RawMoney);
```

This is my most interesting crate option because the representation closely matches the domain invariant. Checked operations and explicit rounding are first-class concepts. :chatgpt-content-reference{index="22"}

### `rust_decimal`

```rust
struct Money(Decimal);
```

with construction enforcing:

```rust
decimal.scale() <= 4
```

This wins on ecosystem and ease of use. `rust_decimal` also has direct database integrations and a very comfortable literal/rounding API. :chatgpt-content-reference{index="23"}

### Your own integer

```rust
struct Money(i64);
```

where:

```text
1 == 0.0001
```

This gives the strongest simplicity story, especially if the ledger mostly performs:

```text
credit
debit
transfer
hold
release
refund
chargeback
```

because almost all of those are just exact addition/subtraction.

---

## Where I currently lean

For the **ledger/accounting core**, I'd lean toward either:

```rust
struct Money(i64);
```

or

```rust
struct Money(FixedPoint<i64, U4>);
```

rather than allowing a general runtime-scale decimal throughout the system.

The reason isn't performance. It's **state-space reduction**.

If your invariant is:

> every monetary amount is an integer number of 1/10,000 units

then values cannot accidentally become:

```text
12.12345
12.123456789
1E-12
NaN
Infinity
```

and there is no per-value scale metadata at all.

For the sort of payment processor you're designing—with deposits, withdrawals, disputes, ledger invariants, idempotency and reconciliation—that simplicity is quite valuable.

`fixnum` looks like the best middle ground: **the semantics of integer minor units without having to reimplement fixed-point multiplication, division, parsing and rounding yourself**. :chatgpt-content-reference{index="24"}

If I were evaluating this for production, my next step would be to compare **`fixnum` vs `rust_decimal` vs a ~100-line `Money(i64)` implementation** against the actual operations a payment processor needs—credit/debit, percentage fees, proportional refunds, FX, allocation/splitting, serialization, and PostgreSQL representation. That exercise will make the trade-off much clearer than comparing generic decimal APIs.
