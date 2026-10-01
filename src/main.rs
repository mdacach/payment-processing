use std::{env, fs::File, io, path::Path};

use anyhow::{Context, Result, bail};
use csv::Trim;
use payment_processing::{ClientId, Event, PaymentProcessor, TransactionAmount, TxId};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(serde::Deserialize)]
struct CsvRow {
    #[serde(rename = "type")]
    kind: String,
    client: ClientId,
    tx: TxId,
    amount: String,
}

#[derive(serde::Serialize)]
struct BalanceRow {
    client: ClientId,
    available: String,
    held: String,
    total: String,
    locked: bool,
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        // Write to stderr in order to not pollute output.
        .with_writer(io::stderr)
        .with_target(false)
        .with_ansi(false)
        .init();

    let mut args = env::args_os();
    let program = args.next().unwrap_or_default();
    let usage = format!(
        "usage: {} <transactions.csv>",
        Path::new(&program).display()
    );
    let input_path = args
        .next()
        .with_context(|| format!("please provide a CSV input file; {usage}"))?;
    if args.next().is_some() {
        bail!("expected exactly one CSV input file; {usage}");
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

    let mut processor = PaymentProcessor::default();
    let mut rows = 0;
    let mut rejected = 0;
    // Each row is processed once at a time, in order to avoid materializing a
    // large data set in-memory all at once.
    for (index, row) in reader.deserialize::<CsvRow>().enumerate() {
        let record_number = index + 2;
        let row = row.with_context(|| format!("{}: record {record_number}", path.display()))?;
        let event = Event::try_from(row)
            .with_context(|| format!("{}: record {record_number}", path.display()))?;
        rows += 1;

        // Errors are reported to stderr, but other transactions continue
        // processing normally.
        if let Err(error) = processor.on_event(event) {
            rejected += 1;
            warn!(record_number, ?event, %error, "transaction rejected");
        }
    }
    info!(path = %path.display(), rows, "processed transactions");

    // But for the output, we can't do the same streaming. A client's balance
    // calculation requires processing all of its transactions, and we can't
    // know when they have ended.
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(io::stdout().lock());
    writer.write_record(["client", "available", "held", "total", "locked"])?;
    let mut accounts = 0;
    for (client_id, account) in processor.accounts() {
        writer.serialize(BalanceRow {
            client: client_id,
            available: format!("{:.4}", account.available()),
            held: format!("{:.4}", account.held()),
            total: format!("{:.4}", account.total()),
            locked: account.is_locked(),
        })?;
        accounts += 1;
    }
    writer.flush()?;

    info!(accounts, rejected, "processing complete");
    Ok(())
}

// The parsing happens in two levels. First, we parse the CSV row and make sure
// that it's structurally valid (done above). Then, we try to convert that row
// into a valid event by using the requirements of the system, such as deposits
// having amounts, or values having up to four decimal places of precision.

impl TryFrom<CsvRow> for Event {
    type Error = anyhow::Error;

    fn try_from(row: CsvRow) -> Result<Self> {
        let CsvRow {
            kind,
            client: client_id,
            tx: tx_id,
            amount,
        } = row;

        match kind.as_str() {
            "deposit" | "withdrawal" => {
                if amount.is_empty() {
                    bail!("missing amount for {kind}");
                }
                let truncated = maybe_truncate(&amount)?;
                let amount: TransactionAmount = truncated
                    .parse()
                    .map_err(|e| anyhow::anyhow!("{e}:{truncated}"))?;

                if kind == "deposit" {
                    Ok(Event::Deposit {
                        client_id,
                        tx_id,
                        amount,
                    })
                } else {
                    Ok(Event::Withdrawal {
                        client_id,
                        tx_id,
                        amount,
                    })
                }
            }
            "dispute" | "resolve" | "chargeback" if !amount.is_empty() => {
                bail!("unexpected amount for {kind}: {amount}")
            }
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
}

// Truncate extra places of precision after the fourth decimal.
fn maybe_truncate(raw: &str) -> Result<String> {
    if let Some((whole, fraction)) = raw.split_once('.') {
        if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
            bail!("invalid amount: {raw}");
        }
        if fraction.len() > 4 {
            return Ok(format!("{whole}.{}", &fraction[..4]));
        }
    }

    Ok(raw.to_owned())
}
