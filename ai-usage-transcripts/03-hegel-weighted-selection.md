# Investigating Hegel weighted selection APIs

Not impressed with the output. Maybe it's bad because Hegel is so new. Something
like https://github.com/hegeldev/hegel-skill could be interesting, but it's
geared towards the testing itself, while here I just wanted to survey the API.

(edit: it completely missed the "state machine"-way that is a much better fit for my needs.)

# Prompt:

My Hegel composite generator generates a subset of variants from my enum's domain. How do I control which variants it generates, and the weight of each one? I want some variants to be more likely than others.

# Output:

For your current generator, replace:

```rust
let is_deposit = gs::booleans();
```

with:

```rust
let is_deposit = gs::weighted_booleans(0.8);
```

This gives `true` a probability of `0.8`, favoring deposits over withdrawals. The rest of your generator—including unique transaction IDs—stays the same. I verified this API in your installed Hegel 0.47.4 [source](/Users/crs/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/hegeltest-0.47.4/src/generators/misc.rs:70).

**The branches you construct control the allowed variants.** Since your `if` only constructs `Deposit` and `Withdrawal`, other variants cannot appear.

You can make the probability configurable:

```rust
#[hegel::composite]
fn transaction_events(tc: &TestCase, deposit_probability: f64) -> Vec<Event> {
    let is_deposit = gs::weighted_booleans(deposit_probability);
    // Rest of your existing generator...
}
```

Then draw with:

```rust
let events = tc.draw(transaction_events(0.8).print_as_debug());
```

For **three or more variants**, use conditional weighted choices. For example, weights `6:3:1`:

```rust
let event = if tc.draw(gs::weighted_booleans(0.6)) {
    // Deposit: 60%
    Event::Deposit { client_id, tx_id, amount }
} else if tc.draw(gs::weighted_booleans(0.75)) {
    // Withdrawal: 40% × 75% = 30%
    Event::Withdrawal { client_id, tx_id, amount }
} else {
    // Dispute: 40% × 25% = 10%
    Event::Dispute { client_id, referred_tx_id }
};
```

Here, `referred_tx_id` should come from an earlier eligible transaction if you want valid disputes.

These probabilities guide generation; they don’t guarantee an exact percentage in each vector or in the final shrunk counterexample.

# Prompt:

Is there no API to "pick one from a set"?

# Output:

Yes—**`gs::sampled_from(...)` picks from values**, and **`hegel::one_of!(...)` picks from generators**. I should have mentioned these earlier.

For your existing composite, after drawing the fields:

```rust
let event = tc.draw(
    gs::sampled_from(vec![
        Event::Deposit { client_id, tx_id, amount },
        Event::Withdrawal { client_id, tx_id, amount },
    ])
    .print_as_debug(),
);
```

Or, if you have separate generators for each variant:

```rust
let event = tc.draw(
    hegel::one_of!(
        deposits(),
        withdrawals(),
    )
    .print_as_debug(),
);
```

Only the listed alternatives can appear.

**Neither API accepts explicit weights in your installed Hegel 0.47.4.** For two alternatives, `weighted_booleans(p)` remains the direct way to control their relative likelihood. Repeating entries in `sampled_from` creates more selectable positions for those values, but its index generation is not a guarantee of a particular frequency.

Verified in the installed [selection API source](/Users/crs/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/hegeltest-0.47.4/src/generators/combinators.rs:14).

# Prompt:

How does these APIs compare with proptest's?

# Output:

**Proptest has a more direct API for your weighted-variant use case:** `prop_oneof!` accepts explicit weights.

| Task | Hegel 0.47.4 | Proptest |
|---|---|---|
| Pick an existing value | `gs::sampled_from(values)` | `prop::sample::select(values)` |
| Pick a generator/strategy | `hegel::one_of!(a(), b())` | `prop_oneof![a(), b()]` |
| Weight alternatives | No explicit weights in `one_of!` | `prop_oneof![8 => a(), 2 => b()]` |
| Build a domain value | `#[hegel::composite]` with `tc.draw(...)` | Tuple strategies with `.prop_map(...)`, or `prop_compose!` |

Proptest’s `select` selects uniformly from a collection. Its unweighted `prop_oneof!` selects strategies uniformly; the weighted form selects according to their relative weights. [Select docs](https://docs.rs/proptest/latest/proptest/sample/fn.select.html), [prop_oneof! docs](https://docs.rs/proptest/latest/proptest/macro.prop_oneof.html)

For your events, a Proptest strategy could look like this:

```rust
use proptest::prelude::*;

fn event(tx_id: u32) -> impl Strategy<Value = Event> {
    let fields = || (1u16..=5, 1i64..=1_000_000);

    prop_oneof![
        8 => fields().prop_map(move |(client_id, amount)| {
            Event::Deposit { client_id, tx_id, amount }
        }),
        2 => fields().prop_map(move |(client_id, amount)| {
            Event::Withdrawal { client_id, tx_id, amount }
        }),
    ]
}
```

Only those two variants appear, with deposits selected about 80% of the time during initial generation. Adding another variant is another weighted entry.

To retain **unique IDs within a vector**, generate events and assign IDs afterward:

```rust
fn transaction_events() -> impl Strategy<Value = Vec<Event>> {
    prop::collection::vec(event(0), 1..=100).prop_map(|mut events| {
        for (index, event) in events.iter_mut().enumerate() {
            match event {
                Event::Deposit { tx_id, .. }
                | Event::Withdrawal { tx_id, .. } => {
                    *tx_id = u32::try_from(index + 1).unwrap();
                }
                _ => unreachable!("strategy only generates deposits and withdrawals"),
            }
        }
        events
    })
}
```

One distinction: **weights control initial generation, while ordering influences shrinking**. Proptest can shrink toward earlier alternatives, so list simpler variants first; a shrunk failure need not reflect the configured proportions. [Shrinking behavior](https://docs.rs/proptest/latest/proptest/macro.prop_oneof.html)

Hegel’s composite style is convenient when generation involves ordinary branching and successive dependent draws. Proptest expresses dependencies through strategy composition such as `prop_flat_map` and `prop_compose!`, but its weighted selection syntax is clearer for this particular task. [Composition docs](https://docs.rs/proptest/latest/proptest/macro.prop_compose.html)
