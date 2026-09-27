use super::*;
use hegel::{generators as gs, Generator, TestCase};

// TODO: should generate random client_ids and random tx_ids, so the code doesn't
//       rely on them being small and ordered.
// TODO: there's certainly a more elegant way of coding this. should weigh
//       differently deposits and withdrawals.
#[hegel::composite]
fn transaction_events(tc: &TestCase) -> Vec<Event> {
    let is_deposit = gs::booleans();
    let client_id = gs::integers::<u16>().min_value(1).max_value(5);
    let amount = gs::integers::<i64>().min_value(1).max_value(1_000_000);

    let input_fields = gs::tuples!(is_deposit, client_id, amount);
    let inputs = tc.draw(gs::vecs(input_fields).min_size(1).max_size(100));

    inputs
        .into_iter()
        .enumerate()
        .map(|(tx_id, (is_deposit, client_id, amount))| {
            let tx_id = tx_id as u32;
            if is_deposit {
                Event::Deposit {
                    client_id,
                    tx_id,
                    amount,
                }
            } else {
                Event::Withdrawal {
                    client_id,
                    tx_id,
                    amount,
                }
            }
        })
        .collect()
}

#[hegel::test]
fn test_event_sequence(tc: TestCase) {
    let mut processor = PaymentProcessor::default();

    let events = tc.draw(transaction_events().print_as_debug());

    for event in events {
        let _ = processor.on_event(event);

        // TODO: come up with good invariants.
        // TODO: consider an oracle reference model and state machine testing.
        for account in processor.accounts.values() {
            assert!(account.total >= 0); // TODO: might change if we allow negative totals.
            assert!(account.available >= 0);
            assert_eq!(account.available + account.held, account.total);
        }
    }
}
