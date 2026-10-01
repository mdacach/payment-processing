# Transaction Processing Engine
This is a simple transaction processing engine that maintains account balances.
Transactions are deserialized from CSV format, and the core system processes
them one-by-one, in file order. This README contains descriptions of the system
as well as future possible improvements.

## Events
The unit of processing of the system is an "event". Events can be usual
transactions, like deposits, but also requests related to a transaction, such as
disputes.

Event processing can fail. Some are expected, like a withdrawal request with
insufficient funds, but others aren't — like a deposit that numerically
overflows the internal representation of the balance (a well-desired problem!).

An "event" can be of the following five types.

### Deposits
A deposit is a credit to the client's asset account. A deposit increases the available and total funds of the client's account.

A deposit can be disputed: in that case, associated funds are held while waiting for either a resolve or a chargeback.

In CSV format, a deposit must look like:

```csv
type,client,tx,amount
deposit,1,1,1.0
```

### Withdrawals
A withdrawal is a debit to the client's asset account. A withdrawal decreases the available and total funds of the client's account.

A withdrawal fails if the client's account does not have sufficient available funds.

In CSV format, a withdrawal must look like:

```csv
type,client,tx,amount
withdrawal,1,2,0.5
```

### Disputes
A dispute represents a client's claim that a deposit was erroneous and should be reversed. A dispute holds funds until it is either resolved or chargedback.

Only deposits can be disputed.

In CSV format, a dispute must look like: 
```csv
type,client,tx,amount
dispute,1,1,
```

Where `tx` is the transaction identifier for the deposit under dispute.

Note that the `amount` field is empty, as the disputed amount is the one associated with the deposit.

### Resolves
A resolve is a possible resolution to a dispute. A resolve releases the associated held funds from a previous dispute.

In CSV format, a resolve must look like: 
```csv
type,client,tx,amount
resolve,1,1,
```

Where `tx` is the transaction identifier for the deposit under dispute which is now resolved.

### Chargebacks
A chargeback is a possible resolution to a dispute. A chargeback represents the client reversing a deposit and funds that were held have now been withdrawn.

In CSV format, a chargeback must look like: 
```csv
type,client,tx,amount
chargeback,1,1,
```

Where `tx` is the transaction identifier for the deposit under dispute which is now chargedback.

As part of fraud analysis, **a chargeback immediately freezes the client's
account**, preventing any further operations. A frozen account is a terminal
state, modeling something that would require human intervention in a payment
system.

#### Aside: Fraud
A malicious actor can abuse such a system in the following way:
1. deposits fiat funds (which can be charged back);
2. purchases and withdraws cryptocoins (which are immutable transactions; can't be chargedback);
3. reverses fiat deposit through a chargeback.

The fiat deposit is reversed back to the malicious actor account, but there's no
way to get the on-chain transaction reversed.

Note that in this system, freezing the account after a chargeback does not help.
Instead, the reason behind the freezing is to prevent _repeated_ frauds. A way
of preventing the fraud in the first place is to reject a chargeback if the
account has insufficient funds, but that in turn would doubly hurm a legitimate
defrauded user.

> [!Note]
Internally, allowing a chargeback even with insufficient funds means that the
available balance in an account can be negative, meaning the system is "owed".



## Implementation notes

Payment processing systems are highly complex. For the purposes of this project,
many assumptions and simplifications are made. Among them:

- All transaction identifiers are required to be globally unique.
- Each client has a single account, and that account is identifiable through the client's id.
  - CSV rows with the same `client` field refers to further transactions to that same account.
- Deposit and withdrawal amounts must be at least 0.0001 currency units.
  - But are otherwise limited only by the underlying numeric representation.
- Deposit and withdrawal amounts must have at most four decimal places of precision.
  - Amounts with higher degrees of precision are truncated during CSV processing. [^1]
- An account's balance may become negative after a dispute or chargeback. 
- Only deposit transactions can be disputed.
  - Preference is given to consistent dispute semantics. Under those semantics,
    a disputed withdrawal could end up withdrawing funds twice, which doesn't
    make sense. Thus, disputed withdrawals are rejected by the system.
- Each deposit transaction can only be disputed once.
- A disputed deposit can only be settled (resolved or charged back) once.
- A frozen account (after a chargeback) can never be unfrozen.
- No further operations are permitted for a frozen account, including disputes, resolutions and chargebacks.
  - Allowing previously disputed deposits to be settled could also be sensible,
    but it's not allowed in this version.

## Input

The input file must be in CSV format with header `type,client,tx,amount` (as
exemplified in [Events](#events)). Extra whitespace is fine. Rows are processed in
the order they appear in the file, sequentially.

The rows are processed in a streaming fashion, one-at-a-time. This allows for
larger CSV input files, but doesn't completely solve memory usage issues because
of the system's internal state of client data and deposits.

A sample CSV file can look like this:

```csv
type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0
```
> _file available in: ([`samples/transactions.csv`](samples/transactions.csv))._

For this particular scenario, the final withdrawal is rejected due to
insufficient funds (client 2 only has `2.0000` available at that moment).

Malformed CSV files are rejected.

## Output

The output of the binary is a CSV with one row per client account. The output
for the example above is:

A sample output can look like this:
```csv
client,available,held,total,locked
1,1.5000,0.0000,1.5000,false
2,2.0000,0.0000,2.0000,false
```
> _the output for the sample above, and available in [`samples/transactions.output`](samples/transactions.output)._


| Column | Description |
| --- | --- |
| `client` | The client identifier for this account. |
| `available` | Funds that are available for withdrawal. Equal to `total - held`. |
| `held` | Funds that are held for dispute. Equal to `total - available`. |
| `total` | The total funds that are available or held. Equal to `available + held`. |
| `locked` | Whether the account is locked. An account is locked if a charge back occurs. |

Balances are always written with four decimal places.

Note that the ordering of the rows is not guaranteed (but in the current version
will be always ordered by increasing client identifiers).

### Samples
The samples/ folder contains generated input files based on the property-based test's
generator. The readable/ subfolder contains smaller and easier-to-read samples.

Note that samples are only supplemental. Correctness testing mostly comes from
the property-based tests in
[`src/payment_processor/property_tests.rs`](src/payment_processor/property_tests.rs),
which generate thousands of complex events at a time.

## Running the system

The binary takes exactly one input file path, a CSV file as described in
[Input](#input). The binary writes its output to stdout, as described in
[Output](#output). This can then be piped into a file (`accounts.csv` in the
command below).

```sh
cargo run -- samples/transactions.csv > accounts.csv
```

Transactions rejected by the processor are skipped. Processing continues until
the end of the file. Logs are disabled by default. `RUST_LOG=warn` can be set to
see rejected transactions on stderr, or `RUST_LOG=info` to also see processing
summaries.

Malformed CSVs or invalid fields immediately stop the run with an error. 

# Extra Stuff

## Further considerations

### Security
Manipulating money requires extreme care. Some of the best-practices are
well-known, such as using precise numeric representations instead of floating
point arithmetic. In this section, I would like to talk about some of the less
obvious considerations that went into this implementation.

#### Preventing invalid states
Rust is very good at preventing invalid states. See [Make Illegal States
Unrepresentable](https://corrode.dev/blog/illegal-state/) for instance.
The heavy use of new-types that are valid by construction is an example of this
idea. See also: [Parse, don't
validate](https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/)

#### Careful when using external dependencies

In the realm of deterministic testing, external dependencies might introduce
nondeterminism that can be hard to spot or workaround (see the [disadvantages of
deterministic simulation testing](https://www.polarsignals.com/blog/posts/2025/07/08/dst-rust#disadvantages)).
For critical projects, external dependencies need to audited and chosen with care.
An owned implementation might be preferable, for more control.

After exploring AI-generated own type implementation (which was too ugly: AI:
link to branch with experiment), I opted to use an external dependency to
represent the currency amounts. I then chose `primitive_fixed_point_decimal`
based on [Comparison and Benchmarking of Rust Decimal
Crates][https://wubingzheng.github.io/en/Decimal-Crates-Comparison.html]. 

(But in a production project, more effort should have been devoted for both
attempting an owned implementation as well as choosing the crate to use).


#### The Witness Pattern
> This one is pretty cool. See Will Crichton's [Typed Design Patterns for the
Functional Era](https://arxiv.org/pdf/2307.07069) for more cool stuff like this
one.

As per the requirements of the system, a chargeback immediately freezes an
accoun as part of fraud analysis. Ensuring that frozen accounts are indeed
non-operable is an important security concern (otherwise, a fraudster could
continue operating and incurring losses).

The most direct translation of this requirement into code is a check
`is_locked?` before any operations. 

```ruby
process(event):
    account = find_or_create_account(event.client)
    if account.is_locked?:
        reject event
    else:
        handle(event, account)
```

But this design has an important drawback — it requires constant attention. If
someone forgets to check the account status before the operation, nothing else
matters. The operation-handling itself has no knowledge of whether the account
is locked or not, so it can't prevent it.

A natural next idea would be to move the `is_locked?` closer to the
operation-processing, maybe _inside_ it:

```ruby
handle_deposit(account, deposit):
    if account.is_locked?: reject deposit
    credit(account, deposit.amount)

handle_withdrawal(account, withdrawal):
    if account.is_locked?: reject withdrawal
    debit(account, withdrawal.amount)

...
```

Chances of forgetting the check are smaller, and they would be easier to spot
during audits. But it introduces a bunch of code duplication, and doesn't look
great.

A better idea is to use a neat pattern known as "The Witness". [^2]
Or the variant known as "Guards". [^3]

Here, the `ActiveAccountGuard` type itself is a witness of the account being
active (because it's the only way it could have been constructed). Because the
guard itself exclusively borrows the account, we also know that no one else
could have made it frozen while we use it.

```rust
struct ActiveAccountGuard<'a> {
    account: &'a mut Account,
}

impl Account {
    fn try_active(&mut self) -> Result<ActiveAccountGuard<'_>, SomeError> {
        ...
    }
}

impl ActiveAccountGuard<'_> {
    fn deposit(&mut self, amount: TransactionAmount) -> Result<(), SomeError> { /* ... */ }
}

let active = account.try_active()?;
```

### Correctness
#### Property-based testing with Hegel
I also wanted to experiment with the new Hegel
([announcement](https://antithesis.com/blog/2026/hegel/),
[website](https://hegel.dev/)) in this project.

The main test of this project is a Hegel state machine test
([stateful](https://docs.rs/hegeltest/latest/hegel/stateful/index.html)) that
generates sequences of events by the system and processes them. Invariants
(`#[invariant(always_run)]`) are checked after every event processed
(`#[rule]`).

| Invariant | Check |
| --- | --- |
| Available balance | `available = total - held` for every account. |
| Held balance | `held = total - available` for every account. |
| Total balance | `total = available + held` for every account. |
| Locked accounts are immutable | If an account was locked before the event, its available, held, total, and locked values are unchanged afterward. |

Property-based testing is great, [but I knew that already](https://github.com/mdacach/kv-store). I did not know that Hegel was so cool, though.
It's still in beta, so possibly not the best pick for important projects, but I found it elegant and plan to use it more myself.

As a side note, it's extremely cool that Hegel supports Swarm Testing by default! 
See this excellent presentation [Will Wilson on Swarm
Testing](https://www.youtube.com/watch?v=wzfC7Q-xNik) about the topic.


## Possible Future Improvements:
There's plenty more we could do here. On my mind at this moment:

Model payment lifecycle with [deterministic simulation
testing](https://antithesis.com/docs/resources/deterministic_simulation_testing/)
and fault injection. Or maybe model checking.
Currently a "deposit" event is synchronously and instantaneously considered "successful". A more accurate model would
be to hand that over to some processing system that could asynchronously analyze it and reject it for different ways (maybe fraud suspicions).
Or that processing system could fail — meaning we now need to consider retries
and [durable execution](https://restate.dev/blog/building-a-modern-durable-execution-engine-from-first-principles).
Or that we need to make sure our system is resilient to out-of-order delivery,
assuming this is over the network. There's really a lot here.

Some degree of concurrency. Currently the system simply processes each event one
at a time, in file order. This is good because it's simple, and makes the system
much easier to test. But it, of course, doesn't scale well. While making it
fully concurrent (or even distributed) is a big beast of complexity, a smaller
degree of concurrency could be achieved with not-so-complex changes — if we note
that client transactions are always independent. Because client X's transactions
never impact client Y's state, we can process them in parallel. We could
partition the workload per client identifier, for example.
- And then there's a lot of stuff we could explore, like [lock-free data structures](https://github.com/rigtorp/awesome-lockfree).


[^1]: Rounding the values could be less secure: consider a case with repeated deposits of 0.00009 would repeatedly rounding them to 0.0001, creating (a small amount, but) virtual money! It's unlikely that this could be abused — will hardly be economical for the attacker, but maybe? So we better truncate instead.
[^2]: Not to be confused with the amazing game [The Witness](https://store.steampowered.com/app/210970/The_Witness/) that I still need to play.
[^3]: Not to be confused with the amazing book [Guards! Guards!](https://terrypratchett.com/books/guards-guards/) that I still need to read.
