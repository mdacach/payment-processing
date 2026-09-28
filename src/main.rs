use crate::payment_processor::PaymentProcessor;

mod event;
mod payment_processor;
mod types;

fn main() {
    let mut processor = PaymentProcessor::default();

    processor
        .on_event(event::Event::Deposit {
            client_id: 0,
            tx_id: 0,
            amount: types::Money::try_from(10).expect("10 is representable"),
        })
        .expect("works!");

    processor
        .on_event(event::Event::Withdrawal {
            client_id: 0,
            tx_id: 0,
            amount: types::Money::try_from(8).expect("8 is representable"),
        })
        .expect("works!");

    dbg!(processor.account(0));
}
