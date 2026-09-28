use super::*;

#[test]
fn deposit_after_chargeback_does_not_mutate_locked_account() {
    let mut processor = PaymentProcessor::default();

    processor
        .on_event(Event::Deposit {
            client_id: 0,
            tx_id: 0,
            amount: "1".parse().unwrap(),
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
        amount: "1".parse().unwrap(),
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

#[test]
fn processes_four_place_amounts_exactly() {
    let mut processor = PaymentProcessor::default();
    processor
        .on_event(Event::Deposit {
            client_id: 1,
            tx_id: 1,
            amount: "1.5050".parse().unwrap(),
        })
        .unwrap();
    processor
        .on_event(Event::Withdrawal {
            client_id: 1,
            tx_id: 2,
            amount: "0.0001".parse().unwrap(),
        })
        .unwrap();
    let account = processor.account(1).unwrap();
    assert_eq!(account.total.to_string(), "1.5049");
    assert_eq!(account.available, account.total);
    assert_eq!(account.held, Money::ZERO);

    processor
        .on_event(Event::Dispute {
            client_id: 1,
            referred_tx_id: 1,
        })
        .unwrap();
    let account = processor.account(1).unwrap();
    assert_eq!(account.available.to_string(), "-0.0001");
    assert_eq!(account.held.to_string(), "1.5050");
    assert_eq!(
        account.total,
        account.available.checked_add(account.held).unwrap()
    );
}

#[test]
fn deposit_overflow_does_not_change_account_or_deposits() {
    let mut processor = PaymentProcessor::default();
    processor.accounts.insert(
        1,
        Account {
            total: Money::from_minor_units(i64::MAX),
            available: Money::from_minor_units(i64::MAX),
            held: Money::ZERO,
            is_locked: false,
        },
    );

    let result = processor.on_event(Event::Deposit {
        client_id: 1,
        tx_id: 1,
        amount: Money::from_minor_units(1),
    });
    assert!(result.is_err());
    assert_eq!(processor.account(1).unwrap().total.minor_units(), i64::MAX);
    assert_eq!(
        processor.account(1).unwrap().available.minor_units(),
        i64::MAX
    );
    assert!(processor.deposits.is_empty());
}

#[test]
fn dispute_overflow_leaves_deposit_processed() {
    let mut processor = PaymentProcessor::default();
    processor.accounts.insert(
        1,
        Account {
            total: Money::from_minor_units(i64::MIN),
            available: Money::from_minor_units(i64::MIN),
            held: Money::ZERO,
            is_locked: false,
        },
    );
    processor.deposits.insert(
        1,
        DepositInfo {
            client_id: 1,
            amount: Money::from_minor_units(1),
            status: DepositStatus::Processed,
        },
    );

    assert!(
        processor
            .on_event(Event::Dispute {
                client_id: 1,
                referred_tx_id: 1,
            })
            .is_err()
    );
    let account = processor.account(1).unwrap();
    assert_eq!(account.available.minor_units(), i64::MIN);
    assert_eq!(account.held, Money::ZERO);
    assert!(matches!(
        processor.deposits.get(&1).unwrap().status,
        DepositStatus::Processed
    ));
}

#[test]
fn overflow_in_second_balance_update_is_atomic() {
    let mut processor = PaymentProcessor::default();
    processor.accounts.insert(
        1,
        Account {
            total: Money::from_minor_units(i64::MAX),
            available: Money::ZERO,
            held: Money::from_minor_units(i64::MAX),
            is_locked: false,
        },
    );
    processor.deposits.insert(
        1,
        DepositInfo {
            client_id: 1,
            amount: Money::from_minor_units(1),
            status: DepositStatus::Processed,
        },
    );

    assert!(
        processor
            .on_event(Event::Dispute {
                client_id: 1,
                referred_tx_id: 1,
            })
            .is_err()
    );
    let account = processor.account(1).unwrap();
    assert_eq!(account.available, Money::ZERO);
    assert_eq!(account.held.minor_units(), i64::MAX);
    assert!(matches!(
        processor.deposits.get(&1).unwrap().status,
        DepositStatus::Processed
    ));
}
