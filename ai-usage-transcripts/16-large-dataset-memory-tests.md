# Large dataset memory tests

A sort of "automated" testing is also a good use for tool-calling LLMs.

# Prompt 1:

Create the 1GiB file and test the memory usage of the system. Create a report about it.

# Output 1:

I generated an exact 1 GiB CSV under `ignored/benchmarks/deposits-1gib.csv`, containing 41,725,111 unique deposits of `0.0001` for one client. I ran both release and debug builds using macOS `/usr/bin/time -l`. Both processed all rows successfully with no rejections and produced a final balance of `4172.5111`. Peak resident memory was 1.792 GiB for release and 1.793 GiB for debug; elapsed times were 13.33 s and 144.16 s, respectively. I saved the methodology, raw-result paths, and limits in `artifacts/one-gib-deposits-memory-report.md`. I did not change the README.

# Prompt 2:

Run a release build (only) with a 10GiB file in that format, now.

# Output 2:

I generated an exact 10 GiB CSV under `ignored/benchmarks/deposits-10gib.csv`, containing 401,797,382 unique deposits of `0.0001` for one client. I built and ran only the release profile. The run succeeded in 137.01 s, processed every row with zero rejections, and produced a final balance of `40179.7382`. macOS `/usr/bin/time -l` reported a peak resident set size of 15,639,347,200 bytes (14.56 GiB) and zero swaps. The output CSV and raw timing log remain under `ignored/benchmarks/`.
