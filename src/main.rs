use payment_processing::{Event, Money, PaymentProcessor};

fn main() {
    let mut processor = PaymentProcessor::default();

    processor
        .on_event(Event::Deposit {
            client_id: 0,
            tx_id: 0,
            amount: Money::try_from(10).expect("10 is representable"),
        })
        .expect("works!");

    processor
        .on_event(Event::Withdrawal {
            client_id: 0,
            tx_id: 1,
            amount: Money::try_from(8).expect("8 is representable"),
        })
        .expect("works!");

    let account = processor.account(0).expect("account exists");
    println!("Balance: {}", account.total());
}
