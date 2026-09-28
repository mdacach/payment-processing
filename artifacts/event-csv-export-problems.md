# Event CSV export implementation notes

## Profile tuning

- Category: design-flaw
- Phase: generate and review the initial set
- Issue: the first `many-clients-long` candidate used the property test's chargeback weight. Chargebacks locked clients early, so 694 of 800 input events were rejected. That left too little useful activity in the latter part of the file.
- Solution: lower only that profile's chargeback weight from `2.0` to `0.1`, regenerate with the recorded seed, and verify that the final file still contains all five event types. Rejections fell to 200 of 800 rows in the final fixture while all ten clients remained represented.
- Lesson: rule weights are hints; inspect observed acceptance and event counts before retaining a fixture.

## Random dispute collisions

- Category: integration
- Phase: trace capture and pool reconciliation
- Issue: a randomly chosen dispute ID could collide with a generated deposit. The processor would accept it while the Hegel pool still listed that deposit as eligible, making later pool draws stale.
- Solution: draw random dispute IDs only from IDs not used for generated deposits or withdrawals. This keeps the rule deliberately invalid and leaves the eligible and disputed pools consistent with accepted processor transitions.
- Lesson: when a generator is intended to create invalid references, enforce that condition against the model's known IDs rather than relying on a low collision probability.
