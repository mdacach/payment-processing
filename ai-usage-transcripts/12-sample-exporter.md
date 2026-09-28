# Exporting and curating payment samples

The [event CSV export plan](/Users/crs/Development/payment-processing/artifacts/event-csv-export-plan.md) was already present when this sequence of prompts began.

# Prompt 1:

Go ahead and implement the changes as specified.

# Output 1:

I implemented the [event CSV export plan](/Users/crs/Development/payment-processing/artifacts/event-csv-export-plan.md). The binary and exporter now share a [transaction CSV codec](/Users/crs/Development/payment-processing/src/event_csv.rs). The Hegel model captures events before processing them and uses configurable profiles to produce six CSV fixtures with manifests in [samples/generated](/Users/crs/Development/payment-processing/samples/generated). The [regeneration script](/Users/crs/Development/payment-processing/scripts/generate-event-csv.sh) reads the recorded seeds.

The [implementation notes](/Users/crs/Development/payment-processing/artifacts/event-csv-export-problems.md) document two issues found and fixed: excessive account locking in the long profile and possible random-dispute ID collisions.

Verification passed: the full test suite, all six export runs, CLI replay, build, formatting, and shell syntax checks. Regenerating all 12 fixture and manifest files produced byte-identical results. The new fixture files are present in the workspace but have not been committed.

# Prompt 2:

Do you need the trace to be an Rc?

# Output 2:

No. I used `Rc` because `Machine::run` consumes the model, while the export test needs to read the trace afterward. Since the run is single threaded, the model could instead borrow a `RefCell<Vec<TraceEntry>>` owned by the test. That would remove the `Rc`, at the cost of adding a lifetime to `PaymentModel`.

# Prompt 3:

I want you to generate more CSV samples. For these samples, focus on readability. These are the samples a human will work through manually to sanity check the system.
Create a new sibling folder (to generated) to store the readable/ samples.

# Output 3:

Added six short, hand-checkable CSVs in [samples/readable](/Users/crs/Development/payment-processing/samples/readable/README.md). They cover basic balances, insufficient funds, disputes, resolution, chargeback locking, and decimal truncation. The README gives the expected balances and rejected rows for each.

I ran every sample through the binary; all outputs matched the documented results.

# Prompt 4:

Add a few more readable samples.

# Output 4:

Added three more hand-checkable CSVs for independent client accounts, repeated dispute actions, and an unknown transaction reference. Their expected balances and rejected rows are in the [readable samples README](/Users/crs/Development/payment-processing/samples/readable/README.md).

I ran all three through the binary; each matched the documented result.

# Prompt 5:

Add a couple of ill-forme and malformed samples.

# Output 5:

Added four samples in [samples/readable](/Users/crs/Development/payment-processing/samples/readable/README.md): two valid CSVs with rejected payment actions, and two malformed inputs that exit before processing. The README lists the expected balances or errors.

I ran all four through the binary and verified their exit status, stdout, and record-specific diagnostics.

# Prompt 6:

For each sample in samples, run the system and persist its output with a similar name, together with the samples themselves.

For the samples that do not produce output (like malformed), say so in the would-be output file.

# Output 6:

Ran all 20 input CSVs and saved a same-stem `.output` file beside each one. The 18 successful runs contain account CSV; the two malformed samples contain a note explaining that no account CSV was produced, plus the exit status and error.

The [sample README](/Users/crs/Development/payment-processing/samples/README.md) explains the convention. [update-sample-outputs.py](/Users/crs/Development/payment-processing/scripts/update-sample-outputs.py) regenerates the files; a second run produced byte-identical outputs.
