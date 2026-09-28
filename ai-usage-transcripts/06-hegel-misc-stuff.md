# Hegel: pools, failure reports, and multiple failures

Misc short prompts about Hegel in general. Not too interesting, but certainly
helpful. New models do a much better job of linking their references too, so
checking their answers is a lot easier (and the reference itself is relevant).

# Prompt 1:

so that's a disadvantage of not using pool? if i was using pool, i would see which event has been selected there?

# Output 1:

**Yes, drawing from a pool would show the selected value**—in your case, the `(client_id, tx_id)` pair, rather than the whole event.

For example, inside your rule:

```rust
let selected = tc.draw(self.disputable_deposits.values_consumed());
let (client_id, tx_id) = selected;
```

The trace would look roughly like:

```text
Step 2: dispute {
  let selected = (0, 0);
}
```

Hegel records that choice for **replay and shrinking**, too. [Pool documentation](https://docs.rs/hegeltest/latest/src/hegel/stateful.rs.html)

But the underlying disadvantage is **using `rand` outside Hegel**, rather than keeping your own collection. You could keep the `BTreeSet` and draw its index through `tc.draw(...)`; that would make the choice reproducible and shrinkable, though the trace would show an index rather than the selected pair. A pool packages that selection and value reporting for you.

# Prompt 2:

does Hegel's values_consumed() draw a value by random?

# Output 2:

Yes, in the sense that **Hegel chooses one of the values currently in the pool** when you call `tc.draw(pool.values_consumed())`. It returns that value and removes it, so it cannot be drawn again from that pool.

The choice is controlled by Hegel: it can be recorded, replayed, and shrunk. It is **not** a separate `rand` call, and I would not assume every pooled value has an equal probability of being chosen. If the pool is empty, the draw rejects that test case. [Hegel’s pool documentation](https://docs.rs/hegeltest/latest/src/hegel/stateful.rs.html)

# Prompt 3:

can i check if a value is in a pool?

# Output 3:

Not directly. In your installed Hegel version, `Pool<T>` provides `is_empty()` and `len()`, but no `contains()` or iterator over its values. It lets Hegel draw a reusable or consumed value.

If your model needs membership checks, keep a separate `BTreeSet` or `HashSet` alongside the pool:

```rust
pool.add(key);
known_keys.insert(key);

// Later:
if known_keys.contains(&key) {
    // ...
}
```

When you consume a value, remove it from the set too. Treat the set as your model’s lookup index and the pool as Hegel’s source of replayable choices. [Pool API](https://docs.rs/hegeltest/latest/src/hegel/stateful.rs.html)

# Prompt 4:

how to annotate hegel tests so that debugging is easier?

# Output 4:

For your state machine, the most useful annotation is **`#[hegel::test_helper]` on your draw helpers**. That changes anonymous `draw_1` entries into names that explain the choice:

```rust
#[hegel::test_helper]
fn draw_client_id(&mut self, tc: &TestCase) -> ClientId {
    let client_id = tc.draw(gs::integers::<ClientId>().min_value(0).max_value(10));
    client_id
}
```

Then a failure reports a name such as `client_id_1` instead of `draw_2`. Hegel already names draws assigned directly inside a `#[rule]` method. [Helper documentation](https://docs.rs/hegeltest/latest/hegel/attr.test_helper.html)

For context that **isn’t a draw**, add a note at the point you know what happened:

```rust
tc.note(&format!(
    "chargeback: client={client_id}, deposit={tx_id}, accounts={:?}",
    self.processor.accounts
));
```

Notes appear in the minimal failing example. This is particularly helpful for showing the selected transaction and account balances before or after an event. [Debugging documentation](https://docs.rs/hegeltest/latest/hegel/)

When you have a specific failure to investigate, paste the emitted `#[hegel::reproduce_failure("…")]` attribute below `#[hegel::test]` to replay that case while debugging. Remove it when you want Hegel to explore new cases again. [Reproduction documentation](https://docs.rs/hegeltest/latest/hegel/attr.reproduce_failure.html)

# Prompt 5:

I've added #[hegel::test_helper] to dispute(), but it's not naming the draws. Why?

# Output 5:

It’s the **tuple pattern**, not the lack of an annotation. Hegel’s naming rewrite recognizes `let name = tc.draw(...)`, but your `dispute()` uses:

```rust
let (client_id, tx_id) = tc.draw(self.disputable_deposits.values_consumed());
```

Change it to a named binding, then destructure it:

```rust
let selected_deposit = tc.draw(self.disputable_deposits.values_consumed());
let (client_id, tx_id) = selected_deposit;
```

The trace should then name `selected_deposit` and show the selected pair.

You can remove `#[hegel::test_helper]` from `dispute()`: `#[hegel::state_machine]` already rewrites draws directly inside `#[rule]` methods. `test_helper` is for separate helper functions that the state-machine macro cannot see. I checked the installed 0.47.4 macro: its naming logic specifically requires a single identifier on the left of `tc.draw(...)`.

# Prompt 6:

Look at `draw_unused_tx_id()`: there, I repeatedly draw transactions (because if they've already been used, I will discard them). 
As written, Hegel will annotate each draw as tx_id_N. How do I make it clearer what's happening in the test? Should I add some kind of annotation?

# Output 6:

I’d name each draw **`candidate_tx_id_N`** and add a note explaining its outcome. The number tells you the order; the note tells you why the loop continued.

```rust
#[hegel::test_helper]
fn draw_unused_tx_id(&mut self, tc: &TestCase) -> TxId {
    loop {
        let candidate_tx_id = tc.draw(gs::integers::<TxId>());

        if self.used_tx_ids.contains(&candidate_tx_id) {
            tc.note(&format!("transaction ID {candidate_tx_id} was already used; retrying"));
            continue;
        }

        tc.note(&format!("selected transaction ID {candidate_tx_id}"));
        return candidate_tx_id;
    }
}
```

`#[hegel::test_helper]` gives the repeated draws the binding name; `tc.note` adds the meaning, and normally appears only in the final failing trace. You don’t need an annotation on every draw. [Hegel test-helper docs](https://docs.rs/hegeltest/latest/hegel/attr.test_helper.html), [note API](https://docs.rs/hegeltest/latest/hegel/struct.TestCase.html)

One thing to verify separately: the selected ID must be inserted into `used_tx_ids` somewhere, or future calls won’t recognize it as used.

# Prompt 7:

where else should I add tc.note?

# Output 7:

I’d add `tc.note` where the trace cannot tell you **what the processor did with an event**. Your draw names already show the chosen amounts and deposits; the missing information is the `on_event` result and the resulting account state.

For example, in `dispute()`:

```rust
let result = self.processor.on_event(dispute);
tc.note(&format!(
    "dispute for client {client_id}, deposit {tx_id}: {result:?}; account: {:?}",
    self.processor.account(client_id),
));
```

The same pattern is useful in `deposit`, `withdrawal`, `resolve`, and `chargeback`, especially because those rules currently discard the `Result`. It would reveal cases where the processor rejected an event but the model still updated a pool as though it succeeded.

I would **not** add notes to every invariant or repeat every drawn value. Give failing assertions context directly instead—for example, include the client ID and account in `total_is_non_negative`. Keep `tc.note` for decisions and outcomes that Hegel’s draw trace cannot infer. Its notes appear with the failing example; your current `dbg!` calls print during ordinary runs too, so replacing those with focused notes would make failures easier to read.

# Prompt 8:

can I do something like tracing syntax for printing stuff like client information?

# Output 8:

Yes, you can get close with Rust’s format-string capture syntax:

```rust
tc.note(&format!(
    "client_id={client_id:?} account={:?}",
    self.processor.account(client_id)
));
```

Or, for a multiline account dump:

```rust
tc.note(&format!(
    "client_id={client_id:?}\naccount={:#?}",
    self.processor.account(client_id)
));
```

Unlike `tracing::debug!(?client_id, ?account)`, `tc.note` takes a **string**, so it has no structured-field shorthand. Its advantage here is that the note appears alongside Hegel’s minimized failing example. If you use `tracing` instead, you’d need a subscriber configured for tests, and its output would generally include all explored cases.

# Prompt 9:

can i make hegel continue running tests even after the first failure? i want to gather many failures to see what to debug first.

# Output 9:

Yes. In your installed Hegel 0.47.4, enable **multiple failure reporting** on the test:

```rust
#[hegel::test(report_multiple_failures = true, test_cases = 500)]
fn state_machine_run(tc: TestCase) {
    // ...
}
```

Hegel will continue looking for failures and report **distinct failure origins** it finds, each with its own example. That should help you compare failures from different rules or invariants. It does not make one test-case execution continue past a panic: if an invariant fails at step 3, that particular sequence stops there. It also won’t guarantee finding every bug, and several examples may share one root cause. [Hegel settings documentation](https://docs.rs/hegeltest/latest/hegel/docs/settings/index.html), [multiple-failure setting](https://docs.rs/hegeltest/latest/hegel/struct.Settings.html#method.report_multiple_failures)

# Prompt 10:

doesn't seem like it's working. maybe because it's reproducing past failures first?

# Output 10:

Yes—that’s likely the reason. I checked Hegel 0.47.4’s runner: **if it finds a failing example during database reuse, it skips fresh generation**, even with `report_multiple_failures = true`. My previous answer missed that limitation.

To explore for additional failures without deleting your saved examples, omit the `Reuse` phase for this test:

```rust
#[hegel::test(
    report_multiple_failures = true,
    test_cases = 500,
    phases = [
        hegel::Phase::Explicit,
        hegel::Phase::Generate,
        hegel::Phase::Target,
        hegel::Phase::Shrink,
    ]
)]
fn state_machine_run(tc: TestCase) {
    // ...
}
```

Also check that you haven’t left a `#[hegel::reproduce_failure("…")]` attribute on the test; that explicitly runs only the recorded example. This setting allows Hegel to *look* for other failures, but it still reports only distinct failures it discovers, not every failing sequence. [Hegel settings documentation](https://docs.rs/hegeltest/latest/hegel/docs/settings/index.html)
