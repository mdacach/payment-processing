# Creating sample CSVs from property-testing event generation

# Prompt 1:

I want you to investigate the current property-based testing we have in this crate.

I want to generate events (from the rules) and port sequences of them to CSV files. What would be the best way of doing that?

The CSV file should ultimately look like this:
type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0

# Output 1:

The Hegel rules in `PaymentModel` construct and apply events without retaining their order. I recommended capturing each event before applying it, then exporting a completed, single-case run rather than writing files during the 5,000-case property test or inside rules. The export should retain rejected inputs, use `tx` for referred IDs in dispute, resolve, and chargeback rows, leave their amount blank, and format money as exact decimal text.

# Prompt 2:

Create an implementation plan for exporting the generated events like these CSV files. We will want to have a few different CSV files, maybe based on their transaction types, or length, or values, or number of clients.

# Output 2:

I created the [event CSV export plan](/Users/crs/Development/payment-processing/artifacts/event-csv-export-plan.md). It proposes capturing events from the existing Hegel rules, exporting each completed run through a dedicated test, and defining six profiles with different event mixes, lengths, amounts, and client counts. Each file must pass checks against its profile because rule weights alone do not guarantee its contents.

# Prompt 3:

Some CSV-related functionality has been implemented in the main binary. Investigate if any of it is reusable here, and update the plan accordingly.

# Output 3:

I updated the [event CSV export plan](/Users/crs/Development/payment-processing/artifacts/event-csv-export-plan.md) to reuse the binary's transaction CSV format and existing `csv` dependency. The plan calls for moving the transaction reader from `main.rs` into a shared module and adding an event writer there. The binary's existing writer produces account balances, so its row mapping does not apply to generated events. I also added a check that generated files can be read back and processed by the binary.

# Prompt 4:

Go ahead and implement the changes as specified.

# Output 4:

I implemented the [event CSV export plan](/Users/crs/Development/payment-processing/artifacts/event-csv-export-plan.md). The binary and exporter now share a [transaction CSV codec](/Users/crs/Development/payment-processing/src/event_csv.rs). The Hegel model captures events before processing them and uses configurable profiles to produce six CSV fixtures with manifests in [samples/generated](/Users/crs/Development/payment-processing/samples/generated). The [regeneration script](/Users/crs/Development/payment-processing/scripts/generate-event-csv.sh) reads the recorded seeds.

The [implementation notes](/Users/crs/Development/payment-processing/artifacts/event-csv-export-problems.md) document two issues found and fixed: excessive account locking in the long profile and possible random-dispute ID collisions.

Verification passed: the full test suite, all six export runs, CLI replay, build, formatting, and shell syntax checks. Regenerating all 12 fixture and manifest files produced byte-identical results. The new fixture files are present in the workspace but have not been committed.

# Prompt 5:

Do you need the trace to be an Rc?

# Output 5:

No. I used `Rc` because `Machine::run` consumes the model, while the export test needs to read the trace afterward. Since the run is single threaded, the model could instead borrow a `RefCell<Vec<TraceEntry>>` owned by the test. That would remove the `Rc`, at the cost of adding a lifetime to `PaymentModel`.

# Prompt 6:

I want you to generate more CSV samples. For these samples, focus on readability. These are the samples a human will work through manually to sanity check the system.
Create a new sibling folder (to generated) to store the readable/ samples.

# Output 6:

Added six short, hand-checkable CSVs in [samples/readable](/Users/crs/Development/payment-processing/samples/readable/README.md). They cover basic balances, insufficient funds, disputes, resolution, chargeback locking, and decimal truncation. The README gives the expected balances and rejected rows for each.

I ran every sample through the binary; all outputs matched the documented results.

# Prompt 7:

Add a few more readable samples.

# Output 7:

Added three more hand-checkable CSVs for independent client accounts, repeated dispute actions, and an unknown transaction reference. Their expected balances and rejected rows are in the [readable samples README](/Users/crs/Development/payment-processing/samples/readable/README.md).

I ran all three through the binary; each matched the documented result.

# Prompt 8:

Add a couple of ill-forme and malformed samples.

# Output 8:

Added four samples in [samples/readable](/Users/crs/Development/payment-processing/samples/readable/README.md): two valid CSVs with rejected payment actions, and two malformed inputs that exit before processing. The README lists the expected balances or errors.

I ran all four through the binary and verified their exit status, stdout, and record-specific diagnostics.

# Prompt 9:

For each sample in samples, run the system and persist its output with a similar name, together with the samples themselves.

For the samples that do not produce output (like malformed), say so in the would-be output file.

# Output 9:

Ran all 20 input CSVs and saved a same-stem `.output` file beside each one. The 18 successful runs contain account CSV; the two malformed samples contain a note explaining that no account CSV was produced, plus the exit status and error.

The [sample README](/Users/crs/Development/payment-processing/samples/README.md) explains the convention. [update-sample-outputs.py](/Users/crs/Development/payment-processing/scripts/update-sample-outputs.py) regenerates the files; a second run produced byte-identical outputs.

# Prompt 10:

1. create a new branch called "sample-generator".
2. move all of the changes related to this change to that branch.
3. checkout main again.
4. pull in the samples (and their associated artifacts) into main.
5. create an ai usage transcript with the prompts we used when creating the sample-exporter feature.

# Output 10:

Created `sample-generator` for the exporter implementation and returned to `main`. Main commit `92c9167` brings over all 20 sample CSVs, their `.output` files, generated manifests, sample documentation, the output refresh script, and [AI usage transcript 12](/Users/crs/Development/payment-processing/ai-usage-transcripts/12-sample-exporter.md). The exporter code and generation script remain on `sample-generator`.

The six main-branch CLI tests pass, and every sample has a companion output. I left the unrelated untracked notes untouched. `cargo fmt --check` still flags an import-order issue already present in `main`’s [src/main.rs](/Users/crs/Development/payment-processing/src/main.rs:3).
