use std::{env, io, path::Path};

use anyhow::{Context, Result, bail};
use payment_processing::{PaymentProcessor, event_csv};
use tracing::{info, warn};

fn main() -> Result<()> {
    // TODO: audit this configuration.
    tracing_subscriber::fmt()
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
    let events = event_csv::read_events(path)?;

    info!(path = %path.display(), rows = events.len(), "parsed transactions");

    let mut processor = PaymentProcessor::default();
    let mut rejected = 0;
    for (record_number, event) in events {
        if let Err(error) = processor.on_event(event) {
            rejected += 1;
            warn!(record_number, ?event, %error, "transaction rejected");
        }
    }

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
