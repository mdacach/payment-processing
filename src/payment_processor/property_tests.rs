use std::collections::BTreeSet;

use super::*;
use hegel::{TestCase, generators as gs};

// TODO: think about an oracle to test against.
// TODO: come up with more invariants.
struct PaymentModel {
    /// The system under test. The PaymentProcessor processes events generated
    /// by the property-based test, and its state is checked against the invariants.
    processor: PaymentProcessor,
    /// A pool of previous deposits that are eligible to be disputed.
    ///
    /// Keeping track of disputable deposits allows the generation of actionable
    /// dispute events (disputes that refer to a valid deposit).
    disputable_deposits: hegel::stateful::Pool<(ClientId, TxId)>,
    /// A pool of deposits that are currently under dispute.
    ///
    /// Keeping track of deposits under dispute allows the generation of
    /// actionable resolves and chargebacks (that refer to a deposit
    /// that can in fact be resolved or charged back).
    currently_disputed_deposits: hegel::stateful::Pool<(ClientId, TxId)>,
    /// The set of already used transaction identifiers.
    ///
    /// Allows for the generation of new transaction identifiers.
    ///
    /// Note that `hegel::stateful::Pool` is not used in this case because it
    /// does not allow for efficiently checking whether a value is contained in
    /// the pool.
    used_tx_ids: BTreeSet<TxId>,

    /// A snapshot of the previous state for the system under test.
    ///
    /// Comparing current and previous state allows checking of invariants
    /// between transitions (an invariant would normally only be checked against
    /// the current state).
    previous_state: Option<ModelSnapshot>,
}

struct ModelSnapshot {
    processor: PaymentProcessor,
}

impl From<&PaymentModel> for ModelSnapshot {
    fn from(value: &PaymentModel) -> Self {
        Self {
            processor: value.processor.clone(),
        }
    }
}

/// TODO: add documentation for an overview of the rules and invariants. also
///       mention the chosen weights for the rules.
#[hegel::state_machine]
impl PaymentModel {
    /// Generates a deposit event with valid amounts and unique transaction identifier.
    #[rule(weight = 10)]
    fn deposit(&mut self, tc: TestCase) {
        let tx_id = self.draw_unused_tx_id(&tc);
        let client_id = self.draw_client_id(&tc);
        // Considering the fixed-point underlying type, this range represents
        // 0.0001 (the smallest allowed value) to 1_000_000.0000 (a very big
        // deposit!).
        let amount = Money::from_mantissa(
            tc.draw(gs::integers::<i64>().min_value(1).max_value(10_000_000_000)),
        );

        tc.note(&format!("deposit for client {client_id}, amount {amount}"));
        let deposit = Event::Deposit {
            client_id,
            tx_id,
            amount,
        };
        let result = self.processor.on_event(deposit);
        tc.note(&format!("result: {result:?}"));

        tc.note(&format!(
            "updated account: {:?}",
            self.processor.account(client_id)
        ));

        // TODO: in case of an error, the deposit should not be marked as eligible to be disputed.

        // Mark this deposit as eligible to be disputed later.
        self.disputable_deposits.add((client_id, tx_id));
    }

    /// Generates a withdrawal event with valid amounts and unique transaction identifier.
    #[rule(weight = 6)]
    fn withdrawal(&mut self, tc: TestCase) {
        let tx_id = self.draw_unused_tx_id(&tc);
        let client_id = self.draw_client_id(&tc);
        // Considering the fixed-point underlying type, this range represents
        // 0.0001 (the smallest allowed value) to 500_000.0000 (a very big
        // withdrawal!).
        let amount = Money::from_mantissa(
            tc.draw(gs::integers::<i64>().min_value(1).max_value(5_000_000_000)),
        );

        tc.note(&format!(
            "withdrawal for client {client_id}, amount {amount}"
        ));
        let withdrawal = Event::Withdrawal {
            client_id,
            tx_id,
            amount,
        };
        let result = self.processor.on_event(withdrawal);
        tc.note(&format!("result: {result:?}"));

        tc.note(&format!(
            "updated account: {:?}",
            self.processor.account(client_id)
        ));
    }

    /// Generates a dispute event that refers to a valid deposit that can be disputed.
    #[rule(weight = 3)]
    fn dispute_eligible_deposit(&mut self, tc: TestCase) {
        // A dispute for a non-disputable transaction is still interesting input,
        // but for now let's simply avoid those.

        // Destructuring it immediately makes Hegel not annotate the draw by its name.
        let disputed_deposit = tc.draw(self.disputable_deposits.values_consumed());
        let (client_id, tx_id) = disputed_deposit;

        tc.note(&format!(
            "dispute for client {client_id}, deposit_id {tx_id}"
        ));

        let dispute = Event::Dispute {
            client_id,
            referred_tx_id: tx_id,
        };
        let result = self.processor.on_event(dispute);
        tc.note(&format!("result: {result:?}"));

        self.currently_disputed_deposits.add((client_id, tx_id));

        tc.note(&format!(
            "updated account: {:?}",
            self.processor.account(client_id)
        ));
    }

    /// Generates a dispute event that refers to a random transaction.
    ///
    /// Note that the referred transaction is probably non-existent (or might
    /// not be allowed, such as disputing a withdrawal), and thus this rule
    /// exercises the part of the system that must ignore such ill-formed
    /// requests.
    #[rule(weight = 1)]
    fn dispute_random(&mut self, tc: TestCase) {
        tc.event("dispute random");
        let client_id = self.draw_client_id(&tc);
        let random_tx_id = tc.draw(gs::integers::<TxId>());

        tc.note(&format!(
            "dispute for client {client_id}, random_tx_id {random_tx_id}; tx_id probably doesn't exist"
        ));

        let dispute = Event::Dispute {
            client_id,
            referred_tx_id: random_tx_id,
        };
        let result = self.processor.on_event(dispute);
        tc.note(&format!("result: {result:?}"));

        tc.note(&format!(
            "updated account: {:?}",
            self.processor.account(client_id)
        ));
    }

    // TODO: I think I like generating resolves and chargebacks for random transactions as well.

    /// Generates a resolve event that refers to a currently-disputed deposit
    /// that can be resolved.
    #[rule(weight = 2)]
    fn resolve(&mut self, tc: TestCase) {
        // Destructuring it immediately makes Hegel not annotate the draw by its name.
        let resolved_deposit = tc.draw(self.currently_disputed_deposits.values_consumed());
        let (client_id, tx_id) = resolved_deposit;

        tc.note(&format!(
            "resolve for client {client_id}, deposit_id {tx_id}"
        ));

        let resolve = Event::Resolve {
            client_id,
            referred_tx_id: tx_id,
        };
        let result = self.processor.on_event(resolve);
        tc.note(&format!("result: {result:?}"));

        tc.note(&format!(
            "updated account: {:?}",
            self.processor.account(client_id)
        ));
    }

    /// Generates a chargeback event that refers to a currently-disputed deposit
    /// that can be chargedback.
    #[rule(weight = 2)]
    fn chargeback(&mut self, tc: TestCase) {
        // Destructuring it immediately makes Hegel not annotate the draw by its name.
        let chargedback_deposit = tc.draw(self.currently_disputed_deposits.values_consumed());
        let (client_id, tx_id) = chargedback_deposit;

        tc.note(&format!(
            "chargeback for client {client_id}, deposit_id {tx_id}"
        ));

        let chargeback = Event::Chargeback {
            client_id,
            referred_tx_id: tx_id,
        };
        let result = self.processor.on_event(chargeback);
        tc.note(&format!("result: {result:?}"));

        // TODO: there should be a more ergonomic way of printing something after every rule.
        tc.note(&format!(
            "updated account: {:?}",
            self.processor.account(client_id)
        ));
    }

    // TODO: need to review the decision below and document it in the README.

    // "total is non negative" is not necessarily true, as it depends on how we
    // deal with disputes with insufficient funds. Suppose the following
    // scenario:
    //
    // 1. deposit 500
    // 2. withdrawal 500
    // 3. dispute-deposit
    //
    // Allowing the dispute means allowing a negative available balance. If that
    // dispute is then charged back, the account total becomes negative.
    //
    // I see two ways of dealing with this:
    // A. do not allow disputes if there aren't enough available funds.
    // B. do not allow chargebacks if there aren't enough available funds.
    //
    // And I don't know which one to pick yet. Will need to think more about it. But anyway,
    // this invariant is commented out for the time being.
    // #[invariant(always_run)]
    // fn total_is_non_negative(&self, _: TestCase) {
    //     for account in self.processor.accounts.values() {
    //         assert!(account.total >= 0); // TODO: might change if we allow negative totals.
    //     }
    // }

    // TODO: these invariants are kind of the same thing. not sure if it's worth keeping the three of them.

    #[invariant(always_run)]
    fn available_is_total_minus_held(&self, _: TestCase) {
        for account in self.processor.accounts.values() {
            assert_eq!(account.available, account.total - account.held);
        }
    }

    #[invariant(always_run)]
    fn held_is_total_minus_available(&self, _: TestCase) {
        for account in self.processor.accounts.values() {
            assert_eq!(account.held, account.total - account.available);
        }
    }

    #[invariant(always_run)]
    fn total_is_available_plus_held(&self, _: TestCase) {
        for account in self.processor.accounts.values() {
            assert_eq!(account.total, account.available + account.held);
        }
    }

    #[invariant(always_run)]
    fn locked_accounts_are_not_mutable(&self, _: TestCase) {
        let Some(previous) = &self.previous_state else {
            return;
        };

        let current_accounts = &self.processor.accounts;

        for (id, previous_account) in &previous.processor.accounts {
            if previous_account.is_locked {
                let current_account = current_accounts
                    .get(&id)
                    .unwrap_or_else(|| panic!("previously locked account {id} is missing!"));

                // TODO: consider using an Eq implementation here instead of
                //       comparing each field separately.
                assert_eq!(previous_account.available, current_account.available);
                assert_eq!(previous_account.held, current_account.held);
                assert_eq!(previous_account.total, current_account.total);
                assert_eq!(previous_account.is_locked, current_account.is_locked);
            }
        }
    }

    // TODO: investigate a better way of doing this.

    /// Saves the current state of the system under test for future checks.
    ///
    /// Transition checks require comparing the current and previous state of
    /// the system.
    ///
    /// This is a hacky way of persisting the previous state: because
    /// `#[invariant(always_run)]` after _every_ step, we can create a fake
    /// invariant that simply saves the current state for future use by other
    /// invariants.
    ///
    /// But this isn't ideal — if always_run annotation is removed, this code
    /// would only run after _some_ steps, and the state tracking would be
    /// inconsistent.
    #[invariant(always_run)]
    fn save_snapshot(&mut self, _: TestCase) {
        self.previous_state = Some(ModelSnapshot::from(&*self));
    }

    /// Helper function to draw an unused transaction identifier.
    #[hegel::test_helper]
    fn draw_unused_tx_id(&mut self, tc: &TestCase) -> TxId {
        // This is a naive way of drawing an available transaction id,
        // but it shall suffice.
        loop {
            // Using tc.draw() for the randomness is important because it allows
            // for reproducibility (when compared to using an external
            // randomness generator like `rand`). The other advantage is that
            // Hegel automatically annotates the draws when showing the output
            // of a failing case.
            let candidate_tx_id = tc.draw(gs::integers::<TxId>());

            if self.used_tx_ids.contains(&candidate_tx_id) {
                tc.note(&format!(
                    "transaction ID {candidate_tx_id} is already used; retrying"
                ));
                continue;
            }

            tc.note(&format!("selected transaction ID {candidate_tx_id}"));
            self.used_tx_ids.insert(candidate_tx_id);
            return candidate_tx_id;
        }
    }

    /// Helper function to draw a valid client identifier.
    #[hegel::test_helper]
    fn draw_client_id(&mut self, tc: &TestCase) -> ClientId {
        // TODO: consider allowing a larger number of clients.
        tc.draw_named(
            "client_id",
            gs::integers::<ClientId>().min_value(0).max_value(10),
        )
    }
}

/// The main property-based test.
///
/// The `TooSlow` health check is suppressed because tests with many steps will naturally be slow.
#[hegel::test(report_multiple_failures = true, test_cases = 5000, suppress_health_check = [hegel::HealthCheck::TooSlow])]
fn state_machine_run(tc: TestCase) {
    let processor = PaymentProcessor::default();
    let model = PaymentModel {
        processor,
        disputable_deposits: hegel::stateful::pool(&tc),
        currently_disputed_deposits: hegel::stateful::pool(&tc),
        used_tx_ids: Default::default(),
        previous_state: Default::default(),
    };
    hegel::stateful::machine(model).steps(1000).run(tc)
}
