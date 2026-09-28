# Fixing compiler errors after the fixed-point Money change

Simple usage fixing the straightforward compiler errors after changing the
underlying money type.

# Prompt:

I've changed the type `Money` to be a fixed point decimal instead of a simple i64.

I want you to investigate the compiler errors created by this change, create a report, fix them as you see fit, and document your changes.

# Output:

The `Money` migration now compiles. I replaced integer amount literals with explicit fixed-point construction and changed the Hegel tests to generate integer mantissas, then convert them to `Money`. I added a regression test for a `0.0001` withdrawal and documented the changes in the [migration report](/Users/crs/Development/payment-processing/artifacts/money-compiler-migration-report.md).

`cargo check --all-targets`, both regression tests, formatting, and whitespace checks pass. The existing state-machine test also passed at its current settings.

The report records a remaining gap: the existing [money specification](/Users/crs/Development/payment-processing/artifacts/money-spec.md) calls for checked balance arithmetic and a fuller `Money` API. Those are outside this compiler fix and are not yet implemented.
