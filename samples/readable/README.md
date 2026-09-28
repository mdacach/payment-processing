# Readable payment examples

These small, hand-written inputs are for checking the processor by eye. Read
each CSV from top to bottom; `tx` names a new transaction for deposits and
withdrawals and refers to an earlier deposit for dispute actions. Every file
can be run with:

```sh
cargo run -- samples/readable/01-basic-balances.csv
```

The binary writes final account rows to standard output and rejection logs to
standard error. Balances below are listed as **available / held / total**;
`locked` is `false` unless stated otherwise. The CSV output always uses four
digits after the decimal point.

Each input also has a `.output` file beside it with the actual account CSV.
For malformed inputs, that file explains why no account CSV was produced.

| Input | What to check by hand | Final account state | Rejected row |
| --- | --- | --- | --- |
| `01-basic-balances.csv` | Client 1 gets 10, spends 4, then gets 1.5; client 2 keeps 5. | Client 1: `7.5000 / 0.0000 / 7.5000`; client 2: `5.0000 / 0.0000 / 5.0000` | None |
| `02-insufficient-funds.csv` | A withdrawal of 3 fails against 2 available; the later withdrawal of 0.5 succeeds. | Client 1: `1.5000 / 0.0000 / 1.5000` | Record 3: insufficient funds |
| `03-dispute-held.csv` | Disputing the 10 deposit moves 10 from available to held without changing total. | Client 1: `2.0000 / 10.0000 / 12.0000` | None |
| `04-dispute-resolved.csv` | Resolving returns the held 10 to available; the 3 withdrawal leaves 7. | Client 1: `7.0000 / 0.0000 / 7.0000` | None |
| `05-chargeback-lock.csv` | Chargeback removes the held 10 and locks the account; the last deposit is rejected. | Client 1: `2.0000 / 0.0000 / 2.0000`, locked | Record 6: locked account |
| `06-truncate-precision.csv` | `1.23456` becomes `1.2345` and `0.23459` becomes `0.2345`; subtraction leaves exactly 1. | Client 1: `1.0000 / 0.0000 / 1.0000` | None |
| `07-independent-clients.csv` | Client 1's deposit is held while client 2 withdraws from a separate account. | Client 1: `0.0000 / 4.0000 / 4.0000`; client 2: `4.0000 / 0.0000 / 4.0000` | None |
| `08-repeated-dispute-actions.csv` | A second dispute is rejected; after resolution, a second resolve is rejected. | Client 1: `5.0000 / 0.0000 / 5.0000` | Records 4 and 6 |
| `09-unknown-transaction.csv` | A dispute for transaction 99 is rejected; the later withdrawal still succeeds. | Client 1: `3.0000 / 0.0000 / 3.0000` | Record 3: only deposits can be disputed |

## Invalid input examples

The first two files contain well-formed CSV rows but invalid payment actions.
The processor logs a warning, continues, and writes final account CSV. The
last two contain invalid input; the binary exits with an error before
processing any event and writes no account CSV.

| Input | What to check by hand | Expected result |
| --- | --- | --- |
| `10-ill-formed-wrong-client.csv` | Client 2 tries to dispute client 1's deposit. | Record 4 is rejected; client 1 finishes at `5.0000 / 0.0000 / 5.0000`, client 2 at `2.0000 / 0.0000 / 2.0000`. |
| `11-ill-formed-resolve-without-dispute.csv` | A deposit is resolved without first being disputed. | Record 3 is rejected; the later withdrawal succeeds, leaving client 1 at `3.0000 / 0.0000 / 3.0000`. |
| `12-malformed-amount.csv` | Record 3 has text in the amount. | Nonzero exit, an invalid-amount error identifying record 3, and empty standard output. |
| `13-malformed-column-count.csv` | Record 3 is missing the amount column. | Nonzero exit, a CSV record-length error identifying record 3, and empty standard output. |
