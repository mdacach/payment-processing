use std::collections::BTreeSet;

use super::*;
use hegel::{generators as gs, TestCase};

// TODO: think about an oracle to test against.
// TODO: come up with more invariants.
#[derive(Debug, Default)]
struct PaymentModel {
    processor: PaymentProcessor,
    eligible_deposits: BTreeSet<(ClientId, TxId)>,
    used_tx_ids: BTreeSet<TxId>,
}

#[hegel::state_machine]
impl PaymentModel {
    #[rule(weight = 7)]
    fn deposit(&mut self, tc: TestCase) {
        let tx_id = self.draw_unused_tx_id(&tc);
        let client_id = self.draw_client_id(&tc);
        let amount = tc.draw(gs::integers::<Money>().min_value(1).max_value(1_000_000));

        let deposit = dbg!(Event::Deposit {
            client_id,
            tx_id,
            amount,
        });
        let _ = self.processor.on_event(deposit);
    }

    #[rule(weight = 2)]
    fn withdrawal(&mut self, tc: TestCase) {
        let tx_id = self.draw_unused_tx_id(&tc);
        let client_id = self.draw_client_id(&tc);
        let amount = tc.draw(gs::integers::<Money>().min_value(1).max_value(500_000));

        let withdrawal = dbg!(Event::Withdrawal {
            client_id,
            tx_id,
            amount,
        });
        let _ = dbg!(self.processor.on_event(withdrawal));
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

    fn draw_unused_tx_id(&mut self, tc: &TestCase) -> TxId {
        // This is a naive way of drawing an available transaction id,
        // but it shall suffice for now.
        loop {
            let candidate = tc.draw(gs::integers::<TxId>());

            if !self.used_tx_ids.contains(&candidate) {
                break candidate;
            }
        }
    }

    fn draw_client_id(&mut self, tc: &TestCase) -> ClientId {
        tc.draw(gs::integers::<ClientId>().min_value(0).max_value(10))
    }
}

#[hegel::test]
fn state_machine_run(tc: TestCase) {
    let processor = PaymentProcessor::default();
    let model = PaymentModel {
        processor,
        ..Default::default()
    };
    hegel::stateful::machine(model).run(tc)
}
