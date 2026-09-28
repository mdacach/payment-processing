# Turning a Hegel failure into a regression test

Thank you Codex gpt-6-sol medium. These use-cases are just no-brainer productivity wins.
Created test was accepted verbatim, but I added a comment on why the failure was happening.

# Prompt 1:

Hegel has just generated a very interesting failure, but I'm finding it hard to refer to it. Should I maybe extract it into a regression test myself?

# Output 1:

Yes. If the failure reveals a meaningful payment scenario, turn Hegel’s **minimized event sequence** into a named, ordinary Rust test. That gives you a stable way to refer to it and documents the behavior you expect:

```rust
#[test]
fn chargeback_after_withdrawing_deposit_records_debt() {
    let mut processor = PaymentProcessor::default();

    processor.on_event(Event::Deposit { /* values from trace */ }).unwrap();
    processor.on_event(Event::Withdrawal { /* ... */ }).unwrap();
    processor.on_event(Event::Dispute { /* ... */ }).unwrap();
    processor.on_event(Event::Chargeback { /* ... */ }).unwrap();

    // Assert the specific behavior you decide is correct.
}
```

Use the actual minimized values, check each result rather than discarding it, and name the test after the **scenario and expected behavior**. If an earlier event is irrelevant, leave it out. The test should fail for the reason you intend before you fix the code.

Keep the `#[hegel::reproduce_failure("…")]` blob while investigating, then remove it so the property test explores new cases again. Hegel only guarantees blob compatibility within a specific version, so a readable example test is better long-term documentation. [Hegel reproduction documentation](https://docs.rs/hegeltest/latest/hegel/attr.reproduce_failure.html)

# Prompt 2:

Please create:

1. a mod for regression tests
2. a regression test for the scenario I'm pasting below, based on hegel's run.

```text
running 1 test
Checking invariants on the initial state.
Step 1: deposit {
  let candidate_tx_id_1 = 0;
  selected transaction ID 0
  let client_id_1 = 0;
  let amount = 1;
  deposit for client 0, amount 1
  result: Ok(())
  updated account: Some(Account { total: 1, available: 1, held: 0, is_locked: false })
}
Step 2: dispute {
  let disputed_deposit = (0, 0);
  dispute for client 0, deposit_id 0
  result: Ok(())
  updated account: Some(Account { total: 1, available: 0, held: 1, is_locked: false })
}
Step 3: chargeback {
  let chargedback_deposit = (0, 0);
  chargeback for client 0, deposit_id 0
  result: Ok(())
  updated account: Some(Account { total: 0, available: 0, held: 0, is_locked: true })
}
Step 4: deposit {
  let candidate_tx_id_1 = 0;
  selected transaction ID 0
  let client_id_1 = 0;
  let amount = 1;
  deposit for client 0, amount 1
  result: Ok(())
  updated account: Some(Account { total: 1, available: 1, held: 0, is_locked: true })
}
Invariant locked_accounts_are_not_mutable failed:
thread 'payment_processor::property_tests::state_machine_run' (2) panicked at src/payment_processor/property_tests.rs:225:17:
assertion `left == right` failed
  left: 0
 right: 1

To reproduce this failure, add the attribute below #[hegel::test]:
    #[hegel::reproduce_failure("AXicdYzBDQAgDAKB2I8LuP+m+hASTfppCXdhASidg0nkqZjMS5jwutbMFe5m8He6jQ1ITADt")]
test payment_processor::property_tests::state_machine_run ... FAILED
```

# Output 2:

Created a [regression test module](/Users/crs/Development/payment-processing/src/payment_processor/regression_tests.rs) and registered it in [payment_processor.rs](/Users/crs/Development/payment-processing/src/payment_processor.rs:222). The test replays Hegel’s deposit → dispute → chargeback → deposit sequence, including the reused transaction ID, and checks that the locked account stays unchanged.

The targeted test **fails as intended**: the final deposit changes the account total from `0` to `1`. The new file passes `rustfmt --check`. I left the processor behavior unchanged.
