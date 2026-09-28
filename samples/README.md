# Sample inputs and outputs

Every input CSV in this directory has a same-stem `.output` file beside it.
For successful runs, that file contains the binary's exact standard output:
the final account CSV. For malformed inputs, standard output is empty, so the
`.output` file says that no account CSV was produced and includes the exit
status and standard-error diagnostic.

- `transactions.csv` is the original example.
- [`readable/`](readable/README.md) has short scenarios to work through by hand.
- `generated/` has longer Hegel-generated transaction sequences and manifests.

Regenerate all `.output` files after changing the processor or the inputs:

```sh
python3 scripts/update-sample-outputs.py
```
