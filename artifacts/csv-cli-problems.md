# CSV CLI implementation issues

No unforeseen issues arose during implementation.

The planned check of the fixed-point crate confirmed that its decimal parser rejects values with more than four fractional digits. The binary validates the entire decimal input, removes surplus fractional digits, and then calls that parser. Its display formatter supports exactly four fractional digits with `{:.4}`.
