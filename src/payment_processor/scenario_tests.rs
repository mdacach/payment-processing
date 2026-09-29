use super::*;

fn money(units: i64) -> Money {
    Money::try_from(units).expect("test amount is representable")
}

fn assert_balances(account: &Account, available: Money, held: Money, total: Money, locked: bool) {
    assert_eq!(account.available(), available);
    assert_eq!(account.held(), held);
    assert_eq!(account.total(), total);
    assert_eq!(account.is_locked(), locked);
}

#[test]
fn deposit_after_chargeback_does_not_mutate_locked_account() {
    let mut processor = PaymentProcessor::default();

    processor
        .on_event(Event::Deposit {
            client_id: 0,
            tx_id: 0,
            amount: Money::try_from(1).expect("1 is representable"),
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
        account_before.is_locked(),
        "chargeback should lock the account"
    );

    // Hegel found that another deposit, even with the same transaction ID,
    // changes the balance of this locked account.
    // (That's because I wasn't, in fact, locking accounts. Just marking them
    // so.)
    let result = processor.on_event(Event::Deposit {
        client_id: 0,
        tx_id: 0,
        amount: Money::try_from(1).expect("1 is representable"),
    });

    let account_after = processor
        .account(0)
        .expect("locked account should still exist");
    assert_eq!(account_after.total(), account_before.total());
    assert_eq!(account_after.available(), account_before.available());
    assert_eq!(account_after.held(), account_before.held());
    assert_eq!(account_after.is_locked(), account_before.is_locked());
    assert!(result.is_err(), "deposit into a locked account should fail");
}

#[test]
fn fractional_amounts_keep_four_decimal_places() {
    let mut processor = PaymentProcessor::default();
    processor
        .on_event(Event::Deposit {
            client_id: 1,
            tx_id: 1,
            amount: Money::from_mantissa(10_001), // 1.0001
        })
        .expect("fractional deposit should succeed");
    processor
        .on_event(Event::Withdrawal {
            client_id: 1,
            tx_id: 2,
            amount: Money::from_mantissa(1), // 0.0001
        })
        .expect("smallest fractional withdrawal should succeed");

    let account = processor.account(1).expect("account should exist");
    let one = Money::try_from(1).expect("1 is representable");
    assert_eq!(account.total(), one);
    assert_eq!(account.available(), one);
    assert_eq!(account.held(), Money::default());
}

#[test]
fn spent_deposit_can_be_disputed_and_charged_back() {
    let mut processor = PaymentProcessor::default();
    processor
        .on_event(Event::Deposit {
            client_id: 1,
            tx_id: 10,
            amount: money(5),
        })
        .expect("deposit should succeed");
    processor
        .on_event(Event::Withdrawal {
            client_id: 1,
            tx_id: 11,
            amount: money(5),
        })
        .expect("withdrawal should succeed");
    processor
        .on_event(Event::Dispute {
            client_id: 1,
            referred_tx_id: 10,
        })
        .expect("spent deposit should still be disputable");

    assert_balances(
        processor.account(1).unwrap(),
        money(-5),
        money(5),
        money(0),
        false,
    );

    processor
        .on_event(Event::Chargeback {
            client_id: 1,
            referred_tx_id: 10,
        })
        .expect("chargeback should succeed");

    assert_balances(
        processor.account(1).unwrap(),
        money(-5),
        money(0),
        money(-5),
        true,
    );
}

#[test]
fn wrong_client_dispute_does_not_change_deposit_or_accounts() {
    let mut processor = PaymentProcessor::default();
    processor
        .on_event(Event::Deposit {
            client_id: 1,
            tx_id: 10,
            amount: money(5),
        })
        .expect("first deposit should succeed");
    processor
        .on_event(Event::Deposit {
            client_id: 2,
            tx_id: 20,
            amount: money(1),
        })
        .expect("second deposit should succeed");

    assert!(
        processor
            .on_event(Event::Dispute {
                client_id: 2,
                referred_tx_id: 10,
            })
            .is_err()
    );
    assert_balances(
        processor.account(1).unwrap(),
        money(5),
        money(0),
        money(5),
        false,
    );
    assert_balances(
        processor.account(2).unwrap(),
        money(1),
        money(0),
        money(1),
        false,
    );

    processor
        .on_event(Event::Dispute {
            client_id: 1,
            referred_tx_id: 10,
        })
        .expect("owner should still be able to dispute the deposit");
    assert_balances(
        processor.account(1).unwrap(),
        money(0),
        money(5),
        money(5),
        false,
    );
}

#[test]
fn deposit_above_limit_is_rejected_without_mutation() {
    let mut processor = PaymentProcessor::default();
    assert!(
        processor
            .on_event(Event::Deposit {
                client_id: 1,
                tx_id: 9,
                amount: money(0),
            })
            .is_err()
    );
    assert!(processor.account(1).is_none());

    processor
        .on_event(Event::Deposit {
            client_id: 1,
            tx_id: 10,
            amount: money(10_000_000),
        })
        .expect("maximum permitted deposit should succeed");

    assert!(
        processor
            .on_event(Event::Deposit {
                client_id: 1,
                tx_id: 11,
                amount: Money::from_mantissa(100_000_000_001),
            })
            .is_err()
    );
    assert_balances(
        processor.account(1).unwrap(),
        money(10_000_000),
        money(0),
        money(10_000_000),
        false,
    );
    assert!(
        processor
            .on_event(Event::Dispute {
                client_id: 1,
                referred_tx_id: 11,
            })
            .is_err(),
        "rejected deposit must not become disputable"
    );
}

#[test]
fn minimum_and_maximum_deposits_and_withdrawals_are_accepted() {
    let mut processor = PaymentProcessor::default();
    for (tx_id, amount) in [(10, Money::from_mantissa(1)), (11, money(10_000_000))] {
        processor
            .on_event(Event::Deposit {
                client_id: 1,
                tx_id,
                amount,
            })
            .expect("boundary deposit should succeed");
    }
    for (tx_id, amount) in [(12, money(10_000_000)), (13, Money::from_mantissa(1))] {
        processor
            .on_event(Event::Withdrawal {
                client_id: 1,
                tx_id,
                amount,
            })
            .expect("boundary withdrawal should succeed");
    }
    assert_balances(
        processor.account(1).unwrap(),
        money(0),
        money(0),
        money(0),
        false,
    );
}

#[test]
fn zero_and_above_limit_withdrawals_are_rejected_without_mutation() {
    let mut processor = PaymentProcessor::default();
    processor
        .on_event(Event::Deposit {
            client_id: 1,
            tx_id: 10,
            amount: money(10_000_000),
        })
        .expect("deposit should succeed");

    for (tx_id, amount) in [(11, money(0)), (12, Money::from_mantissa(100_000_000_001))] {
        assert!(
            processor
                .on_event(Event::Withdrawal {
                    client_id: 1,
                    tx_id,
                    amount,
                })
                .is_err()
        );
        assert_balances(
            processor.account(1).unwrap(),
            money(10_000_000),
            money(0),
            money(10_000_000),
            false,
        );
    }
}

#[test]
fn negative_withdrawal_is_rejected_without_creating_funds() {
    let mut processor = PaymentProcessor::default();
    assert!(
        processor
            .on_event(Event::Withdrawal {
                client_id: 1,
                tx_id: 10,
                amount: money(-10),
            })
            .is_err()
    );

    if let Some(account) = processor.account(1) {
        assert_balances(account, money(0), money(0), money(0), false);
    }
}

#[test]
fn chargeback_can_settle_an_existing_dispute_after_account_locks() {
    let mut processor = PaymentProcessor::default();
    for (tx_id, amount) in [(10, 5), (11, 3)] {
        processor
            .on_event(Event::Deposit {
                client_id: 1,
                tx_id,
                amount: money(amount),
            })
            .expect("deposit should succeed");
        processor
            .on_event(Event::Dispute {
                client_id: 1,
                referred_tx_id: tx_id,
            })
            .expect("dispute should succeed");
    }

    processor
        .on_event(Event::Chargeback {
            client_id: 1,
            referred_tx_id: 10,
        })
        .expect("first chargeback should succeed");
    assert_balances(
        processor.account(1).unwrap(),
        money(0),
        money(3),
        money(3),
        true,
    );

    processor
        .on_event(Event::Chargeback {
            client_id: 1,
            referred_tx_id: 11,
        })
        .expect("outstanding dispute should still be settled after lock");
    assert_balances(
        processor.account(1).unwrap(),
        money(0),
        money(0),
        money(0),
        true,
    );
}
