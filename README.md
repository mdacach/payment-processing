- only deposits can be disputed
- a deposit can only be disputed and resolved/chargedback once; further events referring to that deposit are ignored.
  - explain why.
- scenario where user deposits. withdraws. disputes the deposit: should the dispute auto-fail because of the lack of funds? or should we let the dispute go through and maybe lose money if it's resolved?

## Run the CSV processor

The [sample directory](samples/README.md) includes inputs and their saved
outputs.

```sh
cargo run -- samples/transactions.csv > accounts.csv
```

The binary takes exactly one input file path. It parses the whole CSV before
processing any transaction, so memory use grows with the number of rows. The
header is `type,client,tx,amount`, and rows may contain `deposit`,
`withdrawal`, `dispute`, `resolve`, or `chargeback`. The last three types refer
to an earlier transaction by `tx` and leave `amount` blank. Spaces around
fields are accepted.

Amounts are parsed as decimal text. Digits beyond the fourth fractional place
are truncated, never rounded. The output CSV has columns
`client,available,held,total,locked`; every balance has exactly four decimal
places, and clients appear in ascending ID order. For the sample above, the
output is:

```csv
client,available,held,total,locked
1,1.5000,0.0000,1.5000,false
2,2.0000,0.0000,2.0000,false
```

Invalid CSV or an invalid field stops the run with an error on standard error.
Transactions rejected by the processor, such as the sample's final withdrawal
for insufficient funds, are logged on standard error; processing continues.
Tracing also reports the input row count and completion summary there, keeping
standard output usable as a CSV file.

## Generated transaction fixtures

For short examples that are easy to calculate by hand, see
[`samples/readable/`](samples/readable/README.md).

The six CSVs in `samples/generated/` are versioned input fixtures produced by
the Hegel state-machine rules. Each has a matching `.manifest` with the seed,
Hegel version, profile parameters, and observed event counts. They contain
every event sent to the processor, including rejected events, in execution
order. The `tx` field names a new transaction for deposits and withdrawals and
the referenced transaction for disputes, resolves, and chargebacks. The latter
three have an empty `amount` field.

| Profile | Rows | Main coverage |
| --- | ---: | --- |
| `small-deposits` | 24 | One client, deposits only |
| `mixed-short` | 40 | Deposits and withdrawals across three clients |
| `withdrawal-heavy` | 80 | Many withdrawals, including rejected ones |
| `dispute-lifecycle` | 80 | All five event types and account locking |
| `tiny-amounts` | 50 | Four-place amounts near zero |
| `many-clients-long` | 800 | Longer mixed sequence across ten clients |

Regenerate the full set with `./scripts/generate-event-csv.sh`. The script
reads each profile's seed from its manifest (currently `1` for every file).
With the pinned dependencies in
`Cargo.lock`, running it again should produce byte-identical CSV and manifest
files. To inspect the account output from one generated input:

```sh
cargo run -- samples/generated/dispute-lifecycle.csv > accounts.csv
```

For an ad hoc export with a different seed, use the ignored test directly:

```sh
PAYMENT_EXPORT_PROFILE=tiny-amounts \
PAYMENT_EXPORT_OUTPUT=samples/generated/adhoc/example.csv \
PAYMENT_EXPORT_SEED=42 \
cargo test --lib payment_processor::property_tests::export_generated_case -- --ignored --exact --nocapture
```

The `adhoc/` directory is ignored by Git. Export validates the profile's row,
client, event-mix, amount, and rejection requirements before writing. The CSV
and manifest are written through temporary files, so a failed case or failed
validation does not create a partial fixture.
