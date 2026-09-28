# Transaction ID collisions in the state machine

Pretty good. I missed that the draw is biased to integer numbers, thus
significantly increasing the chance of a collision. But the overall idea was
correct — the processing doesn't care enough about duplicated ids to cause
failures. In any case, we should generate unique ones to improve the testing.

# Prompt:

Look at my draw_unused_tx_id. The way the code runs right now, I'm not actually discarding tx ids (because I'm not populating them when used). 

Does that impact my testing? My thoughts are:

- No tx id colision will be generated through a normal run, because hegel is drawing them randomly and their state space is too big. Even more so because previous runs were running with a small amount of steps — meaning a small amount of generated tx ids. 
- The problem would be apparent only when debugging a shrinked failing case. Hegel would probably shrink all tx ids to 0, meaning in the failing case they would all be the same (but probably not in the original failing case exposing a bug, so that's still fine).
- The system does not care much about repeated tx ids anyway. Tx ids are only used to identify deposits, and because deposits are stored separately, a different conflicting-id tx would not overwrite it anyway. So repeated tx ids might change the desired deposit to be disputed, but that would still be consistent semantically and thus wouldn't be considered a bug.

I want you to double-check my reasoning, and provide any further thoughts.

# Output:

Your reasoning has one important gap: **Hegel’s integer draws are not uniform over all `u32` values**. Its generator deliberately favors small numbers, boundaries, and other interesting values. So collisions can happen during an ordinary run, especially with your current 100,000-step setting; you should not use the birthday probability for uniform 32-bit IDs here. The installed engine’s [integer sampling code](/Users/crs/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/hegeltest-c-0.43.7/src/native/core/state.rs:242) shows that bias.

Your shrinking intuition is partly right. IDs tend to shrink toward `0`, so a minimal failing sequence may contain repeated zeros. But shrinking **re-executes the test and must preserve a failure**; it does not simply change every ID to zero after finding a bug. A collision may also be present in the original generated case. [Hegel’s shrinking overview](https://docs.rs/hegeltest/latest/hegel/)

The collision’s effect depends on the event types:

- **Withdrawal + deposit with the same ID:** currently harmless *for lookup*, because [withdrawal ignores its `tx_id`](/Users/crs/Development/payment-processing/src/payment_processor.rs:94).
- **Deposit + deposit with the same ID:** consequential. [The second deposit overwrites `deposits[tx_id]`](/Users/crs/Development/payment-processing/src/payment_processor.rs:76), while both amounts remain in account balances. A later dispute may therefore find the second deposit’s client, amount, and status even if your Hegel pool selected the first deposit.
- **Your test model then drifts:** its pools treat both deposits as separate eligible transactions. The `dispute` rule moves an entry to `currently_disputed_deposits` regardless of whether `on_event` succeeded. That can make later resolve or chargeback actions refer to a transaction that was never successfully disputed. [Dispute rule](/Users/crs/Development/payment-processing/src/payment_processor/property_tests.rs:78)

So repeated IDs are useful **if you deliberately want to test duplicate-ID behavior**. But they undermine this state machine’s intended “valid deposit → dispute → resolution” sequences, and your balance invariants alone may not reveal the mismatch.

I’d separate the two concerns: make this state machine assign unique IDs, then add a focused property test for duplicate IDs and decide whether the processor should reject, ignore, or replace them. When you enforce uniqueness, record an ID when an event uses it; also add deposits to the eligible pool only when processing succeeds. A counter is the simplest way to guarantee unique IDs without a retry loop that may struggle against Hegel’s bias toward small values.
