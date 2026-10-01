use super::*;
use crate::types::Balance;

fn amount(units: i64) -> TransactionAmount {
    TransactionAmount::try_from_mantissa(units * 10_000).expect("positive test amount")
}

fn balance(units: i64) -> Balance {
    Balance::from_mantissa(units * 10_000)
}

fn amount_mantissa(mantissa: i64) -> TransactionAmount {
    TransactionAmount::try_from_mantissa(mantissa).expect("positive test amount")
}

fn assert_balances(
    account: &Account,
    available: Balance,
    held: Balance,
    total: Balance,
    locked: bool,
) {
    assert_eq!(account.available(), available);
    assert_eq!(account.held(), held);
    assert_eq!(account.total(), total);
    assert_eq!(account.is_locked(), locked);
}

#[test]
fn spent_deposit_can_be_disputed_and_charged_back() {
    let mut processor = PaymentProcessor::default();
    processor
        .on_event(Event::Deposit {
            client_id: 1,
            tx_id: 10,
            amount: amount(5),
        })
        .expect("deposit should succeed");
    processor
        .on_event(Event::Withdrawal {
            client_id: 1,
            tx_id: 11,
            amount: amount(5),
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
        balance(-5),
        balance(5),
        balance(0),
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
        balance(-5),
        balance(0),
        balance(-5),
        true,
    );
}

#[test]
fn insufficient_funds_has_transaction_context() {
    let mut processor = PaymentProcessor::default();
    let error = processor
        .on_event(Event::Withdrawal {
            client_id: 4,
            tx_id: 17,
            amount: amount(3),
        })
        .unwrap_err();
    assert_eq!(
        error,
        ProcessorError::InsufficientFunds {
            client_id: 4,
            tx_id: 17,
            available: balance(0),
            requested: amount(3),
        }
    );
    assert_balances(
        processor.account(4).unwrap(),
        balance(0),
        balance(0),
        balance(0),
        false,
    );
}

#[test]
fn dispute_lookup_combines_missing_owner_and_state() {
    let mut processor = PaymentProcessor::default();
    processor
        .on_event(Event::Deposit {
            client_id: 1,
            tx_id: 10,
            amount: amount(5),
        })
        .unwrap();

    for (client_id, referred_tx_id) in [(1, 99), (2, 10)] {
        assert_eq!(
            processor
                .on_event(Event::Dispute {
                    client_id,
                    referred_tx_id
                })
                .unwrap_err(),
            ProcessorError::DisputableDepositNotFound {
                client_id,
                referred_tx_id
            }
        );
    }
    processor
        .on_event(Event::Dispute {
            client_id: 1,
            referred_tx_id: 10,
        })
        .unwrap();
    assert_eq!(
        processor
            .on_event(Event::Dispute {
                client_id: 1,
                referred_tx_id: 10
            })
            .unwrap_err(),
        ProcessorError::DisputableDepositNotFound {
            client_id: 1,
            referred_tx_id: 10
        }
    );
    assert_balances(
        processor.account(1).unwrap(),
        balance(0),
        balance(5),
        balance(5),
        false,
    );
}

#[test]
fn settlement_lookup_combines_missing_owner_and_state() {
    let mut processor = PaymentProcessor::default();
    processor
        .on_event(Event::Deposit {
            client_id: 1,
            tx_id: 10,
            amount: amount(5),
        })
        .unwrap();
    for (client_id, referred_tx_id) in [(1, 99), (2, 10), (1, 10)] {
        for event in [
            Event::Resolve {
                client_id,
                referred_tx_id,
            },
            Event::Chargeback {
                client_id,
                referred_tx_id,
            },
        ] {
            assert_eq!(
                processor.on_event(event).unwrap_err(),
                ProcessorError::EligibleDepositNotFound {
                    client_id,
                    referred_tx_id,
                }
            );
        }
    }
    assert_balances(
        processor.account(1).unwrap(),
        balance(5),
        balance(0),
        balance(5),
        false,
    );
}

#[test]
fn balance_overflow_identifies_field_and_does_not_record_deposit() {
    let mut processor = PaymentProcessor::default();
    let maximum = amount_mantissa(i64::MAX);
    processor
        .on_event(Event::Deposit {
            client_id: 1,
            tx_id: 10,
            amount: maximum,
        })
        .unwrap();
    let error = processor
        .on_event(Event::Deposit {
            client_id: 1,
            tx_id: 11,
            amount: amount_mantissa(1),
        })
        .unwrap_err();
    assert_eq!(
        error,
        ProcessorError::BalanceOverflow {
            client_id: 1,
            tx_id: 11,
            balance: crate::BalanceField::Total,
        }
    );
    assert_balances(
        processor.account(1).unwrap(),
        Balance::from_mantissa(i64::MAX),
        balance(0),
        Balance::from_mantissa(i64::MAX),
        false,
    );
    assert!(!processor.deposits.contains_key(&11));
}

#[test]
fn locked_account_rejects_deposits_and_settlement_without_mutation() {
    let mut processor = PaymentProcessor::default();
    for (tx_id, units) in [(10, 5), (11, 3)] {
        processor
            .on_event(Event::Deposit {
                client_id: 1,
                tx_id,
                amount: amount(units),
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
        balance(0),
        balance(3),
        balance(3),
        true,
    );

    // A deposit using an existing transaction ID once changed a locked account.
    // The remaining disputed deposit must also stay unsettled after the lock.
    for event in [
        Event::Deposit {
            client_id: 1,
            tx_id: 10,
            amount: amount(1),
        },
        Event::Chargeback {
            client_id: 1,
            referred_tx_id: 11,
        },
    ] {
        assert_eq!(
            processor.on_event(event).unwrap_err(),
            ProcessorError::AccountLocked { client_id: 1 }
        );
        assert_balances(
            processor.account(1).unwrap(),
            balance(0),
            balance(3),
            balance(3),
            true,
        );
        assert!(matches!(
            processor.deposits.get(&10).map(|info| &info.status),
            Some(DepositStatus::Chargedback)
        ));
        assert!(matches!(
            processor.deposits.get(&11).map(|info| &info.status),
            Some(DepositStatus::Disputed)
        ));
    }
}
