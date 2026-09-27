# Investigating the shiny new Hegel Rust for property-based testing

 Seems like a good opportunity to test out Hegel (https://hegel.dev/) for property-based testing.
 I thought LLM's overview was fine. 

# Prompt:

Give me an overview of how Hegel Rust works, and examples of how I could use it.

# Output:

**Hegel Rust is a property-based testing library:** you describe a rule your code should obey, and Hegel generates inputs, runs the test repeatedly, and simplifies failures into small counterexamples. It belongs to the Hypothesis-based Hegel family. [Hegel Rust](https://github.com/hegeldev/hegel-rust)

For example, instead of testing only “deposit 100, withdraw 30, balance is 70,” you can test “withdrawing any affordable amount leaves the correct balance.”

**How it works**

1. `#[hegel::test]` runs your test repeatedly.
2. `tc.draw(generator)` produces inputs such as integers, strings, collections, or your own domain objects.
3. Ordinary Rust assertions describe the property.
4. If an assertion fails or your code panics, Hegel **shrinks** the generated choices while preserving the failure, then reports a simpler example.

Internally, the Rust API communicates with a shared engine that handles generation, shrinking, and an example database. The current engine is written in Rust. Passing a test is evidence from the explored cases, not a proof over every possible input. [How Hegel works](https://hegel.dev/explanation/how-hegel-works)

Install and run it with:

```sh
cargo add --dev hegeltest
cargo test
```

The package is named `hegeltest`, but Rust imports use `hegel`. Tests run 100 cases by default; `#[hegel::test(test_cases = 1000)]` increases that. [Getting started](https://docs.rs/hegeltest/latest/hegel/)

**Example: deposit and withdrawal in your project**

Your current `PaymentProcessor` is a good fit. This illustrative test could go inside a `#[cfg(test)]` module in [payment_processor.rs](/Users/crs/Development/payment-processing/src/payment_processor.rs), where it can inspect private account fields:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use hegel::{generators as gs, TestCase};

    #[hegel::test]
    fn affordable_withdrawal_preserves_balances(tc: TestCase) {
        let deposit = tc.draw(
            gs::integers::<i64>().min_value(1).max_value(1_000_000),
        );
        let withdrawal = tc.draw(
            gs::integers::<i64>().min_value(1).max_value(deposit),
        );

        let mut processor = PaymentProcessor::default();

        processor.on_event(Event::Deposit {
            client_id: 1,
            tx_id: 1,
            amount: deposit,
        }).unwrap();

        processor.on_event(Event::Withdrawal {
            client_id: 1,
            tx_id: 2,
            amount: withdrawal,
        }).unwrap();

        let account = processor.account(1).unwrap();
        assert_eq!(account.total, deposit - withdrawal);
        assert_eq!(account.available, account.total);
        assert_eq!(account.held, 0);
    }
}
```

Notice that the withdrawal generator depends on the generated deposit. You can construct valid scenarios directly this way. Hegel also provides collection generators and combinators such as `map`, `filter`, and `flat_map`. [Generator API](https://docs.rs/hegeltest/latest/hegel/generators/index.html)

The amount bounds here keep this test focused on ordinary balance behavior. Overflow deserves its own property.

**Example: finding an input-validation bug**

Suppose your intended rule is that negative withdrawals must be rejected. Inside the same test module:

```rust
#[hegel::test]
fn negative_withdrawals_are_rejected(tc: TestCase) {
    let amount = tc.draw(
        gs::integers::<i64>().min_value(-1_000_000).max_value(-1),
    );
    let mut processor = PaymentProcessor::default();

    let result = processor.on_event(Event::Withdrawal {
        client_id: 1,
        tx_id: 1,
        amount,
    });

    assert!(result.is_err());
}
```

From inspecting your implementation, this would fail: it currently subtracts a negative amount, increasing the balance, and returns `Ok(())`. A small counterexample is `amount = -1`. I haven’t installed Hegel or executed these snippets.

**Other useful properties for this processor**

| Property | What to generate and check |
|---|---|
| Rejected withdrawals preserve balances | Generate a deposit and a larger withdrawal; check the error and unchanged balances. |
| Accounting stays consistent | Generate event sequences; check `total == available + held` after every operation. |
| Clients are isolated | Interleave operations for several clients; check one client’s events cannot change another’s balances. |
| Dispute followed by resolve restores balances | Generate a valid deposit, dispute it, then resolve it. |
| Duplicate transactions follow your policy | Reuse transaction IDs and check rejection or idempotency, once that policy is defined. |
| Money parsing round-trips | Once implemented, check `parse(format(amount)) == amount` across representable values. |

For longer workflows, Hegel has **stateful testing**: define actions with `#[rule]`, and checks with `#[invariant]`. It generates sequences of actions against your state. Use `#[invariant(always_run)]` when a balance invariant must be checked after every action; ordinary invariants are sampled between the initial and final checks. [Stateful testing](https://docs.rs/hegeltest/latest/hegel/stateful/index.html)

I’d start with affordable withdrawals, insufficient-funds rejection, and negative-amount validation. Once dispute, resolve, and chargeback are implemented, a state machine comparing your processor against a small reference model would be especially valuable.

# Prompt:

How do I draw elements of my own domain? Specifically, I want to create a `vec` of `Event`s. Additionally, I want it to only produce `Deposit`s and `Withdrawal`s, rather than all possible variants.

# Output:

Define a generator for one `Event`, then pass it to `gs::vecs(...)`. For your two variants, a **composite generator** lets you draw the fields and choose which variant to construct. [Composite generator docs](https://docs.rs/hegeltest/latest/hegel/attr.composite.html)

```rust
use crate::event::Event;
use hegel::{generators as gs, Generator, TestCase};

#[hegel::composite]
fn deposit_or_withdrawal(tc: &TestCase) -> Event {
    let client_id = tc.draw(
        gs::integers::<u16>().min_value(1).max_value(10),
    );
    let tx_id = tc.draw(gs::integers::<u32>());
    let amount = tc.draw(
        gs::integers::<i64>().min_value(1).max_value(1_000_000),
    );

    if tc.draw(gs::booleans()) {
        Event::Deposit { client_id, tx_id, amount }
    } else {
        Event::Withdrawal { client_id, tx_id, amount }
    }
}

#[hegel::test]
fn test_event_sequence(tc: TestCase) {
    let events: Vec<Event> = tc.draw(
        gs::vecs(deposit_or_withdrawal().print_as_debug())
            .min_size(1)
            .max_size(100),
    );

    // Feed events into your processor and assert your properties.
}
```

The macro turns `deposit_or_withdrawal` into a generator constructor: call **`deposit_or_withdrawal()` without passing `tc`**. Hegel supplies the test case when drawing.

- Only the two explicitly constructed variants can appear.
- Each vector contains 1–100 events.
- The small client range encourages repeated operations on the same accounts.
- `.print_as_debug()` uses your existing `Event: Debug` implementation for failure output. [Printing generators](https://docs.rs/hegeltest/latest/hegel/generators/trait.Generator.html#method.print_as_debug)

This generates independent events: transaction IDs can repeat, and withdrawals can exceed available funds. If you want **only successful transaction sequences**, the generator needs to track balances as it builds the vector and constrain each withdrawal accordingly.

# Prompt:

Make it so the transaction IDs do not repeat.

# Output:

Assign IDs from each event’s position in the generated vector. This guarantees uniqueness within each test case, including after shrinking:

```rust
use crate::event::Event;
use hegel::{generators as gs, Generator, TestCase};

#[hegel::composite]
fn transaction_events(tc: &TestCase) -> Vec<Event> {
    let fields = tc.draw(
        gs::vecs(gs::tuples!(
            gs::booleans(),
            gs::integers::<u16>().min_value(1).max_value(10),
            gs::integers::<i64>().min_value(1).max_value(1_000_000),
        ))
        .min_size(1)
        .max_size(100),
    );

    fields
        .into_iter()
        .enumerate()
        .map(|(index, (is_deposit, client_id, amount))| {
            let tx_id = u32::try_from(index + 1).unwrap();

            if is_deposit {
                Event::Deposit { client_id, tx_id, amount }
            } else {
                Event::Withdrawal { client_id, tx_id, amount }
            }
        })
        .collect()
}

#[hegel::test]
fn test_event_sequence(tc: TestCase) {
    let events = tc.draw(transaction_events().print_as_debug());

    // Process events and assert your properties.
}
```

The event kinds, clients, amounts, and vector length vary; transaction IDs are always `1, 2, …, events.len()`.

# Prompt:

Give me short examples on how to check invariants.

# Output:

Check invariants **after each event**, so a later operation cannot hide an earlier violation. Inside your existing test module:

```rust
#[hegel::test]
fn balances_stay_consistent(tc: TestCase) {
    let events = tc.draw(transaction_events().print_as_debug());
    let mut processor = PaymentProcessor::default();

    for event in events {
        // Insufficient funds is an expected rejection.
        let _ = processor.on_event(event);

        for account in processor.accounts.values() {
            assert_eq!(
                account.total,
                account.available + account.held,
            );
            assert!(account.available >= 0);
            assert_eq!(account.held, 0); // No dispute events generated.
        }
    }
}
```

You can also check a **transition invariant**: rejected operations must preserve balances.

```rust
for event in events {
    let balances = |p: &PaymentProcessor| {
        p.accounts
            .iter()
            .map(|(&id, a)| (id, (a.total, a.available, a.held)))
            .collect::<BTreeMap<_, _>>()
    };

    let before = balances(&processor);
    let result = processor.on_event(event);

    if result.is_err() {
        assert_eq!(balances(&processor), before);
    }
}
```

That second check is stronger: it also forbids creating an account on rejection. Your current withdrawal implementation creates an empty account before checking funds, so it can fail this property.
