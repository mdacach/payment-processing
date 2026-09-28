use crate::payment_processor::PaymentProcessor;
use crate::types::Money;

mod event;
mod payment_processor;
mod types;

fn main() {
    let mut processor = PaymentProcessor::default();

    processor
        .on_event(event::Event::Deposit {
            client_id: 0,
            tx_id: 0,
            amount: "10".parse::<Money>().expect("valid amount"),
        })
        .expect("works!");

    processor
        .on_event(event::Event::Withdrawal {
            client_id: 0,
            tx_id: 0,
            amount: "8".parse::<Money>().expect("valid amount"),
        })
        .expect("works!");

    dbg!(processor.account(0));
}
