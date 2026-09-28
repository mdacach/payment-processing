# Investigating Hegel stateful generation

This was a good answer, and pointed to relevant resources, but it was still a
bit weird. #[hegel::state_machine] was the most interesting part, but it only
talked briefly about it. In any case, it was enough for me to pick it up and
start looking, so it was definitely helpful.

# Prompt:

In my case, I want to generate dispute events that refer to previous deposits transaction ids. So we need stateful generation, what does Hegel provide for that?

# Output:

Hegel provides **pools of previously generated values** and **state machines with weighted actions**. For your case, a pool can hold `(client_id, tx_id)` pairs from deposits, so disputes reference both the correct transaction and its owner.

You can use a pool directly inside your existing composite generator:

```rust
use hegel::{generators as gs, stateful, TestCase};

#[hegel::composite]
fn transaction_events(tc: &TestCase) -> Vec<Event> {
    let mut eligible_deposits = stateful::pool::<(u16, u32)>(tc);
    let mut events = Vec::new();
    let mut next_tx_id = 1u32;

    let count = tc.draw(
        gs::integers::<usize>().min_value(1).max_value(100),
    );

    for _ in 0..count {
        // Generate a deposit when no dispute target exists.
        let deposit = eligible_deposits.is_empty()
            || tc.draw(gs::weighted_booleans(0.7));

        let event = if deposit {
            let client_id = tc.draw(
                gs::integers::<u16>().min_value(1).max_value(5),
            );
            let amount = tc.draw(
                gs::integers::<i64>().min_value(1).max_value(1_000_000),
            );
            let tx_id = next_tx_id;
            next_tx_id += 1;

            eligible_deposits.add((client_id, tx_id));

            Event::Deposit { client_id, tx_id, amount }
        } else {
            let (client_id, referred_tx_id) =
                tc.draw(eligible_deposits.values_consumed());

            Event::Dispute { client_id, referred_tx_id }
        };

        events.push(event);
    }

    events
}
```

This generates deposits and disputes with three guarantees:

- Deposit transaction IDs are unique within the sequence.
- Every dispute references an earlier deposit belonging to the same client.
- Each deposit is disputed at most once.

The pool offers two selection methods:

| Method | Effect |
|---|---|
| `values_reusable()` | Selects a reference without removing the value; useful for testing repeated disputes. |
| `values_consumed()` | Selects and removes a value; useful for tracking deposits still eligible for dispute. |

Pool choices participate in Hegel’s replay and shrinking. Drawing from an empty pool rejects the case, which is why the example checks emptiness first. [Installed pool documentation](/Users/crs/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/hegeltest-0.47.4/src/stateful.rs:224)

**For testing the processor as the sequence develops**, use `#[hegel::state_machine]`. Each action can execute an event, update pools, and check results:

```rust
#[hegel::state_machine]
impl PaymentModel {
    #[rule(weight = 7)]
    fn deposit(&mut self, tc: TestCase) {
        // Generate and execute a deposit.
        // Add its (client_id, tx_id) to eligible_deposits on success.
    }

    #[rule(weight = 3)]
    fn dispute(&mut self, tc: TestCase) {
        // Draw an eligible deposit before making any mutations.
        // Execute the dispute and move it into an active_disputes pool.
    }

    #[invariant(always_run)]
    fn balances_agree(&self, _: TestCase) {
        // Check total == available + held for every account.
    }
}
```

Run an initialized model with:

```rust
stateful::machine(model).steps(100).run(tc);
```

**State-machine rules do support explicit weights**—unlike `one_of!`. They guide action selection without guaranteeing exact proportions. `always_run` checks the invariant after every rule. [Stateful API](https://docs.rs/hegeltest/latest/hegel/stateful/index.html)

For your eventual deposit → dispute → resolve/chargeback lifecycle, I’d use a state machine with separate pools for eligible deposits and active disputes. That lets each action select transactions in the appropriate state and update eligibility after successful execution.
