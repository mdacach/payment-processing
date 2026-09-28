use std::collections::BTreeSet;

use super::*;
use hegel::{generators as gs, TestCase};

// TODO: think about an oracle to test against.
// TODO: come up with more invariants.
struct PaymentModel {
    processor: PaymentProcessor,
    disputable_deposits: hegel::stateful::Pool<(ClientId, TxId)>,
    currently_disputed_deposits: hegel::stateful::Pool<(ClientId, TxId)>,
    used_tx_ids: BTreeSet<TxId>,

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

#[hegel::state_machine]
impl PaymentModel {
    #[rule(weight = 10)]
    fn deposit(&mut self, tc: TestCase) {
        let tx_id = self.draw_unused_tx_id(&tc);
        let client_id = self.draw_client_id(&tc);
        let amount = tc.draw(gs::integers::<Money>().min_value(1).max_value(1_000_000));

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

        // Mark this deposit as eligible to be disputed later.
        self.disputable_deposits.add((client_id, tx_id));
    }

    #[rule(weight = 6)]
    fn withdrawal(&mut self, tc: TestCase) {
        let tx_id = self.draw_unused_tx_id(&tc);
        let client_id = self.draw_client_id(&tc);
        let amount = tc.draw(gs::integers::<Money>().min_value(1).max_value(500_000));

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

    #[rule(weight = 3)]
    fn dispute(&mut self, tc: TestCase) {
        // TODO: also generate disputes that refer a non-deposit or a deposit
        //       that is already being disputed.

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

    #[rule(weight = 2)]
    fn resolve(&mut self, tc: TestCase) {
        // TODO: also generate resolves that refer a non-deposit or a deposit
        //       that is not being disputed.

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

    #[rule(weight = 2)]
    fn chargeback(&mut self, tc: TestCase) {
        // TODO: also generate chargebacks that refer a non-deposit or a deposit
        //       that is not being disputed.

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

    // TODO: need to review all of these invariants. which is good, because they
    //       will fail soon.
    #[invariant(always_run)]
    fn total_is_non_negative(&self, _: TestCase) {
        for account in self.processor.accounts.values() {
            assert!(account.total >= 0); // TODO: might change if we allow negative totals.
        }
    }

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

                // TODO: could use an Eq implementation here, but a bit weird, maybe.
                assert_eq!(previous_account.available, current_account.available);
                assert_eq!(previous_account.held, current_account.held);
                assert_eq!(previous_account.total, current_account.total);
                assert_eq!(previous_account.is_locked, current_account.is_locked);
            }
        }
    }

    // Transition checks require knowledge of the previous state (to compare
    // with the current one). This is a hacky way of persisting that state:
    // because an always_run invariant runs after every step, and because it
    // allows for mutable state, we can make it save the state for the future.
    // But this isn't ideal — if we removed the always_run annotation, this
    // invariant would only run after _some_ steps, and the state tracking would
    // be inconsistent.
    // TODO: investigate a better way of doing this.
    #[invariant(always_run)]
    fn save_snapshot(&mut self, _: TestCase) {
        self.previous_state = Some(ModelSnapshot::from(&*self));
    }

    #[hegel::test_helper]
    fn draw_unused_tx_id(&mut self, tc: &TestCase) -> TxId {
        // TODO: an observant reader might see that used tx ids are never updated,
        //       which defeats the purpose of this function. I'm letting it go because
        //       I want to see whether the proptests will catch that failure eventually
        //       (when two transactions with the same id cause havoc).

        // This is a naive way of drawing an available transaction id,
        // but it shall suffice for now.
        loop {
            let candidate_tx_id = tc.draw(gs::integers::<TxId>());

            if self.used_tx_ids.contains(&candidate_tx_id) {
                tc.note(&format!(
                    "transaction ID {candidate_tx_id} is already used; retrying"
                ));
                continue;
            }

            tc.note(&format!("selected transaction ID {candidate_tx_id}"));
            return candidate_tx_id;
        }
    }

    #[hegel::test_helper]
    fn draw_client_id(&mut self, tc: &TestCase) -> ClientId {
        tc.draw_named(
            "client_id",
            gs::integers::<ClientId>().min_value(0).max_value(10),
        )
    }
}

#[hegel::test]
fn state_machine_run(tc: TestCase) {
    let processor = PaymentProcessor::default();
    let model = PaymentModel {
        processor,
        disputable_deposits: hegel::stateful::pool(&tc),
        currently_disputed_deposits: hegel::stateful::pool(&tc),
        used_tx_ids: Default::default(),
        previous_state: Default::default(),
    };
    hegel::stateful::machine(model).run(tc)
}
