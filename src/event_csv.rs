//! CSV input and output for ordered payment events.

use std::{fs::File, io::Write, path::Path};

use anyhow::{Context, Result, bail};
use csv::{StringRecord, Trim};

use crate::{Event, Money};

pub const HEADER: [&str; 4] = ["type", "client", "tx", "amount"];

/// Read and validate every row before returning any events.
pub fn read_events(path: &Path) -> Result<Vec<(usize, Event)>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut reader = csv::ReaderBuilder::new().trim(Trim::All).from_reader(file);

    let headers = reader
        .headers()
        .with_context(|| format!("{}: record 1: reading header", path.display()))?;
    if !headers.iter().eq(HEADER) {
        bail!(
            "{}: record 1: expected header type,client,tx,amount",
            path.display()
        );
    }

    reader
        .records()
        .enumerate()
        .map(|(index, record)| {
            let record_number = index + 2;
            let record =
                record.with_context(|| format!("{}: record {record_number}", path.display()))?;
            let event = parse_event(&record)
                .with_context(|| format!("{}: record {record_number}", path.display()))?;
            Ok((record_number, event))
        })
        .collect()
}

/// Write events in their original order using the CLI's input schema.
pub fn write_events<W: Write>(writer: W, events: &[Event]) -> Result<()> {
    let mut writer = csv::Writer::from_writer(writer);
    writer.write_record(HEADER)?;
    for event in events {
        let (kind, client_id, tx_id, amount) = match *event {
            Event::Deposit {
                client_id,
                tx_id,
                amount,
            } => ("deposit", client_id, tx_id, format!("{amount:.4}")),
            Event::Withdrawal {
                client_id,
                tx_id,
                amount,
            } => ("withdrawal", client_id, tx_id, format!("{amount:.4}")),
            Event::Dispute {
                client_id,
                referred_tx_id,
            } => ("dispute", client_id, referred_tx_id, String::new()),
            Event::Resolve {
                client_id,
                referred_tx_id,
            } => ("resolve", client_id, referred_tx_id, String::new()),
            Event::Chargeback {
                client_id,
                referred_tx_id,
            } => ("chargeback", client_id, referred_tx_id, String::new()),
        };
        writer.write_record([kind, &client_id.to_string(), &tx_id.to_string(), &amount])?;
    }
    writer.flush()?;
    Ok(())
}

fn parse_event(record: &StringRecord) -> Result<Event> {
    let kind = record.get(0).context("missing type")?;
    let client_id = record
        .get(1)
        .context("missing client")?
        .parse()
        .context("invalid client ID")?;
    let tx_id = record
        .get(2)
        .context("missing tx")?
        .parse()
        .context("invalid transaction ID")?;
    let amount = record.get(3).context("missing amount column")?;

    match kind {
        "deposit" => Ok(Event::Deposit {
            client_id,
            tx_id,
            amount: parse_amount(amount)?,
        }),
        "withdrawal" => Ok(Event::Withdrawal {
            client_id,
            tx_id,
            amount: parse_amount(amount)?,
        }),
        "dispute" => Ok(Event::Dispute {
            client_id,
            referred_tx_id: tx_id,
        }),
        "resolve" => Ok(Event::Resolve {
            client_id,
            referred_tx_id: tx_id,
        }),
        "chargeback" => Ok(Event::Chargeback {
            client_id,
            referred_tx_id: tx_id,
        }),
        _ => bail!("unknown transaction type: {kind}"),
    }
}

fn parse_amount(raw: &str) -> Result<Money> {
    let truncated = if let Some((whole, fraction)) = raw.split_once('.') {
        if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
            bail!("invalid amount: {raw}");
        }
        if fraction.len() > 4 {
            format!("{whole}.{}", &fraction[..4])
        } else {
            raw.to_owned()
        }
    } else {
        raw.to_owned()
    };

    truncated
        .parse()
        .with_context(|| format!("invalid amount: {raw}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_event_variants_round_trip_in_order() {
        let events = [
            Event::Deposit {
                client_id: 2,
                tx_id: 7,
                amount: Money::from_mantissa(12_345),
            },
            Event::Withdrawal {
                client_id: 2,
                tx_id: 8,
                amount: Money::from_mantissa(1),
            },
            Event::Dispute {
                client_id: 2,
                referred_tx_id: 7,
            },
            Event::Resolve {
                client_id: 2,
                referred_tx_id: 7,
            },
            Event::Chargeback {
                client_id: 2,
                referred_tx_id: 7,
            },
        ];
        let mut bytes = Vec::new();
        write_events(&mut bytes, &events).unwrap();
        assert_eq!(
            String::from_utf8(bytes.clone()).unwrap(),
            "type,client,tx,amount\n\
             deposit,2,7,1.2345\n\
             withdrawal,2,8,0.0001\n\
             dispute,2,7,\n\
             resolve,2,7,\n\
             chargeback,2,7,\n"
        );

        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), bytes).unwrap();
        let parsed = read_events(file.path()).unwrap();
        assert_eq!(
            parsed
                .into_iter()
                .map(|(_, event)| event)
                .collect::<Vec<_>>(),
            events
        );
    }
}
