# Investigating swarm testing in Hegel and Proptest

# Prompt 1:

Investigate Hegel (hypothesis-based property-based testing) support for swarm testing.

# Output 1:

**Hegel already supports swarm testing for this project’s state machine.** The project pins `hegeltest` 0.47.4 in [Cargo.toml](/Users/crs/Development/payment-processing/Cargo.toml:14). Its engine selects a different enabled subset of rules for each test case, then chooses among those rules throughout that case. The existing [state-machine test](/Users/crs/Development/payment-processing/src/payment_processor/property_tests.rs:361) uses that engine, so no flag or new generator is needed. Hegel has supported this since 0.19.1. [Hegel changelog](https://docs.rs/crate/hegeltest/0.40.0/source/CHANGELOG.md)

The six `#[rule(weight = ...)]` values in [property_tests.rs](/Users/crs/Development/payment-processing/src/payment_processor/property_tests.rs:54) still apply, but only among rules enabled for a given case. They guide selection; they do not guarantee overall event proportions. [Engine changelog](https://docs.rs/crate/hegeltest-c/0.43.0/source/CHANGELOG.md)

There is also a second, automatic form of swarm behavior: integer draws vary their mix of boundary, small, and ordinary values by test case. [Hegel changelog](https://docs.rs/crate/hegeltest/0.40.0/source/CHANGELOG.md)

The practical next step is to revise the “Swarm testing” item in [README.md](/Users/crs/Development/payment-processing/README.md:303): it describes a feature already in use. Swarm selection does not guarantee coverage of a particular dispute lifecycle, so focused scenarios remain useful for those sequences. I inspected the pinned library and project code; I did not change files or run the test suite.

# Prompt 2:

Investigate proptest, an alternative to hegel. Does it also support swarm testing? Does it do it by default?

# Output 2:

**Proptest can do swarm testing, but it does not do Hegel-style swarm testing by default.**

| | Hegel state machine | Proptest state machine |
| --- | --- | --- |
| Case-wide rule subset | Engine selects one automatically | You define it in the strategy |
| Rule weights | Apply among enabled rules | `prop_oneof!` supports explicit weights |

In Proptest, `prop_oneof!` chooses an alternative **for each generated transition**; its weights do not automatically select a subset to keep enabled for the whole test case. The state-machine API lets you build that behavior: generate a feature mask in `init_state()`, keep it in the reference state, and have `transitions(state)` choose only eligible event types. This is an inference from the documented [state-machine generation API](https://proptest-rs.github.io/proptest/proptest/state-machine.html) and [`prop_oneof!` behavior](https://docs.rs/proptest/latest/proptest/macro.prop_oneof.html).

For this payment processor, that could produce cases focused on deposits and withdrawals, or on the dispute lifecycle. You would need to ensure each mask permits useful sequences—for example, a case intended to exercise disputes also needs a way to create deposits. The benefit over Hegel’s automatic rule subset is explicit control; the cost is writing and maintaining the selection logic.
