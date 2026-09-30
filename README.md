# Transaction Processing Engine
This is a simple transaction processing engine that updates client account balances. 

## Events
The system processes five types of events:

### Deposits
A deposit is a credit to the client's asset account. A deposit increases the available and total funds of the client's account.

A deposit can be disputed: in that case, associated funds are held while waiting for either a resolve or a chargeback.

A deposit looks like:

```csv
type,client,tx,amount
deposit,1,1,1.0
```

### Withdrawals
A withdrawal is a debit to the client's asset account. A withdrawal decreases the available and total funds of the client's account.

A withdrawal fails if the client's account does not have sufficient available funds.

A withdrawal looks like:

```csv
type,client,tx,amount
withdrawal,1,2,0.5
```

### Disputes
A dispute represents a client's claim that a deposit was erroneous and should be reversed. A dispute holds funds until it is either resolved or chargedback.

Only deposits can be disputed.

A dispute looks like: 
```csv
type,client,tx,amount
dispute,1,1,
```

Where `tx` is the transaction identifier for the deposit under dispute.

Note that the `amount` field is empty, as the disputed amount is the one associated with the deposit.

### Resolves
A resolve is a possible resolution to a dispute. A resolve releases the associated held funds from a previous dispute.

A resolve looks like:
```csv
type,client,tx,amount
resolve,1,1,
```

Where `tx` is the transaction identifier for the deposit under dispute which is now resolved.

### Chargebacks
A chargeback is a possible resolution to a dispute. A chargeback represents the client reversing a deposit and funds that were held have now been withdrawn.

A chargeback looks like:

```csv
type,client,tx,amount
chargeback,1,1,
```

Where `tx` is the transaction identifier for the deposit under dispute which is now chargedback.

As part of fraud analysis, **a chargeback immediately freezes the client's account**, preventing any further operations. This is a terminal state in this system.

#### Fraud
A malicious actor can abuse chargebacks in the following way:
1. deposits fiat funds (which can be charged back).
2. purchases and withdraws cryptocoins (which are immutable transactions; can't be chargedback).
3. reverses fiat deposit through a chargeback.

Although freezing the account after a chargeback does not prevent the fraud, a
chargedback account is considered suspicious and requires intervention before
being allowed to continue operating. That might prevent further malicious
transactions.

Note that preventing the chargeback would avoid the fraud, but also prevent a
legitimate defrauded user from getting their money back. It's a tricky
situation. The system here chooses to allow a chargeback to go through even with
insufficient funds, in order for a better user experience. This allows balances
to become negative.


## System assumptions

- Each client has a single account, and that account is identifiable through the client's id.
- Deposit and withdrawal amounts must be at least 0.0001 currency units.
  - `TransactionAmount` enforces this minimum for both CSV and library callers.
- `Balance` may be negative after a dispute or chargeback.
- Only deposits can be disputed. TODO: why.
- A deposit can only be disputed once.
- A disputed deposit can only be resolved or charged back once.
- Negative available balances are allowed (such as the fraud scenario TODO: maybe change this).
- All transaction ids are globally unique.
- A frozen account (after a chargeback) can never be unfrozen.
- A frozen account does not permit any further operation, including disputes, resolutions and chargebacks.
  - Note that allowing previously disputed deposits to be resolved is a possibility, but is not done here.

## Input

The input file must be in CSV format with header `type,client,tx,amount` (as
exemplified in [Events](#events)). Whitespace is accepted. Rows are processed in
the order they appear in the file, sequentially. All of the records are
materialized before processing. Streaming is a possible future improvement.

For example, [`samples/transactions.csv`](samples/transactions.csv):

```csv
type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0
```

The final withdrawal is rejected due to insufficient funds — client 2 only has `2.0000` available.

### Requirements
Deposit and withdrawal amounts are truncated to four decimal places before validation.
Values that become zero, negative values, and values outside the fixed-point
representation are rejected before any account output is written.
All transaction identifiers `tx` are globally unique in a file. Conflicting transaction identifiers might cause inconsistent state.

Malformed CSV files are rejected.

## Output

The output is a CSV with one row per client account. The output for the example above is:

```csv
client,available,held,total,locked
1,1.5000,0.0000,1.5000,false
2,2.0000,0.0000,2.0000,false
```

| Column | Description |
| --- | --- |
| `client` | The client identifier for this account. |
| `available` | Funds that are available for withdrawal. Equal to `total - held`. |
| `held` | Funds that are held for dispute. Equal to `total - available`. |
| `total` | The total funds that are available or held. Equal to `available + held`. |
| `locked` | Whether the account is locked. An account is locked if a charge back occurs. |

Balances are always written with four decimal places.

Note that ordering of the rows is not guaranteed.

### Sample files
The samples/ folder contains generated CSVs based on the property-based test's
generator. A set of smaller samples are kept in samples/readable/ and can be
manually worked through.

Note that samples are only supplemental. Robust testing comes from the
property-based tests in (TODO: link to file), which generate thousands and
thousands of complex event sequences at a time.

## Running the system

```sh
cargo run -- samples/transactions.csv > accounts.csv
```

The binary takes exactly one input file path, a CSV file as described in [Input](#input).
The binary writes its output to stdout, which can then be piped into a file (as `accounts.csv` in the example command).

Transactions rejected by the processor are logged as warnings, but otherwise
skipped. Processing continues until the end of the file.

Extra information is logged to stderr, like row count and rejected transactions.
Setting the environment variable `RUST_LOG=off` suppresses these logs.

Malformed CSVs or invalid fields immediately stop the run with an error. 

## Further considerations

### Security
Dealing with money is always tricky. Some approaches are well-known, such as not using floats. In this section I talk about some of the less obvious considerations that went into this project.
- dealing with money is tricky — precision points (don't use float).
#### Preventing invalid events
- new types.
- unsigned values (disallowing negative withdrawal).
- account handling atomicity through accessor functions.
#### Considering external dependencies
This project uses the `primitive_fixed_point_decimal` crate for handling the
financial numbers. In a security-critical project, choosing an external
dependency needs to be done with care. In very high stakes, the dependency
should be carefully audited, and rolling our own should be considered. For
expediency (and after exploring with an AI-generated own type implementation in
TODO: link to other branch), I chose to use the ready-made crate mentioned
above. See the author's [Comparison and Benchmarking of Rust Decimal
Crates][https://wubingzheng.github.io/en/Decimal-Crates-Comparison.html] for
more information.

#### The Witness Pattern
Accounts with chargebacks are frozen as part of fraud analysis. Ensuring that
frozen accounts are indeed non-operable is an important security concern
(otherwise, a fraudster could continue operating and incurring losses). The most
direct translation of this requirement into code is a check `is_locked?` before
any operations. 

For example, a check at each call site would look like this:

```text
process(event):
    account = find_or_create_account(event.client)
    if account.is_locked?:
        reject event
    else:
        handle(event, account)
```

But this design has an important drawback — it requires consistent attention. If
someone forgets to check the account status before the operation, the operation
will go through successfully. The operation-handling itself has no knowledge of
whether the account is locked or not, so it can't prevent it.

A natural next idea would be to move the `is_locked?` check _inside_ the
operation-processing, like this:

```text
handle_deposit(account, deposit):
    if account.is_locked?: reject deposit
    credit(account, deposit.amount)

handle_withdrawal(account, withdrawal):
    if account.is_locked?: reject withdrawal
    debit(account, withdrawal.amount)

// Every other operation handler repeats the same check.
```

This is an improvement, as chances of forgetting the check are smaller, but it
still doesn't solve it completely and duplicates code in many places.

A better idea is to use a neat pattern described in Will Crichton's [Typed
Design Patterns for the Functional Era](https://arxiv.org/pdf/2307.07069), The
Witness.

In particular, this implementation uses a variant of The Witness pattern with a
borrowed guard. Because the guard must be acquired before an operation, and
because the guard can only be acquired for an active account, we can be sure
that operations can't be performed on frozen accounts.

Something like this:
```rust
pub(super) struct ActiveAccountGuard<'a> {
    account: &'a mut Account,
}

impl Account {
    fn try_active(&mut self) -> Result<ActiveAccountGuard<'_>, AccountError> {
        if self.is_locked {
            return Err(AccountError::Locked);
        }
        Ok(ActiveAccountGuard { account: self })
    }
}

impl ActiveAccountGuard<'_> {
    fn deposit(&mut self, amount: TransactionAmount) -> Result<(), AccountError> { /* ... */ }
    //         ^^^^ only implemented for ActiveAccountGuard
}

//  vvvvvv note that because of the &mut borrow, no one else can access that account
//  vvvvvv and we're sure it remains active.
let active = account.try_active()?;
```

### Correctness
#### Property-based testing with Hegel
In this project, I wanted to experiment with the new Hegel ([announcement][https://antithesis.com/blog/2026/hegel/], [website][https://hegel.dev/]).
The main test of this project is a Hegel state machine test
([stateful][https://docs.rs/hegeltest/latest/hegel/stateful/index.html]) that
generates sequences of events allowed by the system and processes them. Invariants (`#[invariant(always_run)]`) are checked after every event processed (`#[rule]`).

The invariants are:

| Invariant | What the property test checks after each generated event |
| --- | --- |
| Available balance | `available = total - held` for every account. |
| Held balance | `held = total - available` for every account. |
| Total balance | `total = available + held` for every account. |
| Locked accounts are immutable | If an account was locked before the event, its available, held, total, and locked values are unchanged afterward. |

The test also saves a snapshot after each event to compare locked accounts on
the next step; that snapshot is test bookkeeping, not a business invariant.
The proposed `total >= 0` invariant is commented out. A client can deposit and
withdraw the same funds, then dispute the deposit. The dispute makes available
funds negative, and a subsequent chargeback can make total funds negative.

My thoughts: Hegel is elegant and enjoyable. It's still in beta, so it's probably not the best fit for important projects, but I liked it and plan to experiment further with it.

It's also super cool that Hegel supports Swarm Testing by default. 

See this excellent presentation [Wil Willson on Swarm Testing][https://www.youtube.com/watch?v=wzfC7Q-xNik] for more details on Swarm Testing.


## Possible Future Improvements:
- Payment lifecycle with deterministic simulation testing and fault injection.
  - This project completely papers over the whole payment lifecycle (waiting for a payment to complete, possibly needing to retry it, yada yada...). Modeling something like that would be super cool with deterministic simulation testing and fault injection.
- Concurrency (difficult, but could do something partitioned by client, for instance).
  - Making something concurrent is always difficult. In this case, simply concurrently processing the events create a whole new swarm of problems. But one key observation here is that clients are independent. A client's transaction only ever affects its account, so as long as we process those events in the expected order, we're free to concurrently process the workload. This could allow designs where workloads are partitioned by client keys, for instance.
- Streaming processing.
  - This wouldn't be a big lift, actually. The system already processes events one at a time, so it would be simply a matter of batching them. My initial plan to contrast the materialized version (this one) with a future vectorized (streamed; batched) one with memory usage comparisons, but I didn't get to it in time.
