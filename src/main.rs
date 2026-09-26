use crate::payment_processor::PaymentProcessor;

mod event;
mod payment_processor;

fn main() {
    let mut processor = PaymentProcessor::default();

    processor
        .on_event(event::Event::Deposit {
            client_id: 0,
            tx_id: 0,
            amount: 10,
        })
        .expect("works!");

    processor
        .on_event(event::Event::Withdrawal {
            client_id: 0,
            tx_id: 0,
            amount: 8,
        })
        .expect("works!");

    dbg!(processor.account(0));
}
