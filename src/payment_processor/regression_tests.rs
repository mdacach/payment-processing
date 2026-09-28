use super::*;

#[test]
fn deposit_after_chargeback_does_not_mutate_locked_account() {
    let mut processor = PaymentProcessor::default();

    processor
        .on_event(Event::Deposit {
            client_id: 0,
            tx_id: 0,
            amount: 1,
        })
        .expect("initial deposit should succeed");
    processor
        .on_event(Event::Dispute {
            client_id: 0,
            referred_tx_id: 0,
        })
        .expect("dispute should succeed");
    processor
        .on_event(Event::Chargeback {
            client_id: 0,
            referred_tx_id: 0,
        })
        .expect("chargeback should succeed");

    let account_before = processor.account(0).expect("account should exist").clone();
    assert!(
        account_before.is_locked,
        "chargeback should lock the account"
    );

    // Hegel found that another deposit, even with the same transaction ID,
    // changes the balance of this locked account.
    // (That's because I wasn't, in fact, locking accounts. Just marking them
    // so.)
    let result = processor.on_event(Event::Deposit {
        client_id: 0,
        tx_id: 0,
        amount: 1,
    });

    let account_after = processor
        .account(0)
        .expect("locked account should still exist");
    assert_eq!(account_after.total, account_before.total);
    assert_eq!(account_after.available, account_before.available);
    assert_eq!(account_after.held, account_before.held);
    assert_eq!(account_after.is_locked, account_before.is_locked);
    assert!(result.is_err(), "deposit into a locked account should fail");
}
