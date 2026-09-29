use std::{env, fs::File, io, path::Path};

use anyhow::{Context, Result, bail};
use csv::{StringRecord, Trim};
use payment_processing::{Event, Money, PaymentProcessor};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    // Keep account CSV on stdout and filter stderr logs with RUST_LOG.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .with_target(false)
        .with_ansi(false)
        .without_time()
        .init();

    let mut args = env::args_os();
    let program = args.next().unwrap_or_default();
    let input_path = args.next().with_context(|| {
        format!(
            "usage: {} <transactions.csv>",
            Path::new(&program).display()
        )
    })?;
    if args.next().is_some() {
        bail!(
            "usage: {} <transactions.csv>",
            Path::new(&program).display()
        );
    }

    run(Path::new(&input_path))
}

fn run(path: &Path) -> Result<()> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut reader = csv::ReaderBuilder::new().trim(Trim::All).from_reader(file);

    let headers = reader
        .headers()
        .with_context(|| format!("{}: record 1: reading header", path.display()))?;
    if !headers.iter().eq(["type", "client", "tx", "amount"]) {
        bail!(
            "{}: record 1: expected header type,client,tx,amount",
            path.display()
        );
    }

    // The events are materialized all at once. Streaming could be an improvement,
    // but is out of scope for now.
    let events: Vec<(usize, Event)> = reader
        .records()
        .enumerate()
        .map(|(index, record)| {
            let record_number = index + 2;
            let event = record
                .with_context(|| format!("{}: record {record_number}", path.display()))
                .and_then(|record| {
                    parse_event(&record)
                        .with_context(|| format!("{}: record {record_number}", path.display()))
                })?;
            Ok((record_number, event))
        })
        .collect::<Result<_>>()?;

    info!(path = %path.display(), rows = events.len(), "parsed transactions");

    let mut processor = PaymentProcessor::default();
    let mut rejected = 0;
    for (record_number, event) in events {
        // Errors while processing the transactions are reported through warning
        // logs, but processing other transactions continues normally.
        if let Err(error) = processor.on_event(event) {
            rejected += 1;
            warn!(record_number, ?event, %error, "transaction rejected");
        }
    }

    // Output is written as CSV according to the README's format.
    let mut writer = csv::Writer::from_writer(io::stdout().lock());
    writer.write_record(["client", "available", "held", "total", "locked"])?;
    let mut accounts = 0;
    for (client_id, account) in processor.accounts() {
        writer.write_record([
            client_id.to_string(),
            format!("{:.4}", account.available()),
            format!("{:.4}", account.held()),
            format!("{:.4}", account.total()),
            account.is_locked().to_string(),
        ])?;
        accounts += 1;
    }
    writer.flush()?;

    info!(accounts, rejected, "processing complete");
    Ok(())
}

// Parses an event that can be processed by the system from a CSV record.
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

// TODO: probably reject negative amounts? here or in the system?
// Parses a fixed-point amount with up to four decimal places.
//
// Inputs with more than four decimal places of precision are truncated.
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
