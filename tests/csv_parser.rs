use std::{io::Write, process::Command};

fn run_csv(input: &str) -> std::process::Output {
    run_csv_with_log_level(input, None)
}

fn run_csv_with_log_level(input: &str, log_level: Option<&str>) -> std::process::Output {
    let mut file = tempfile::NamedTempFile::new().expect("create temporary CSV file");
    file.write_all(input.as_bytes()).expect("write CSV input");

    let mut command = Command::new(env!("CARGO_BIN_EXE_payment-processing"));
    command.arg(file.path());
    match log_level {
        Some(level) => {
            command.env("RUST_LOG", level);
        }
        None => {
            command.env_remove("RUST_LOG");
        }
    }
    command.output().expect("run payment processor")
}

#[test]
fn sample_balances_and_rejected_withdrawal() {
    let input = include_str!("../samples/transactions.csv");
    let output = run_csv_with_log_level(input, Some("info"));
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "client,available,held,total,locked\n\
         1,1.5000,0.0000,1.5000,false\n\
         2,2.0000,0.0000,2.0000,false\n"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("transaction rejected"), "{stderr}");
    assert!(stderr.contains("insufficient funds"), "{stderr}");
    assert!(stderr.contains("record_number=6"), "{stderr}");
    assert!(stderr.contains("rows=5"), "{stderr}");
    assert!(stderr.contains("processing complete"), "{stderr}");
}

#[test]
fn logs_are_disabled_by_default() {
    let output = run_csv(include_str!("../samples/transactions.csv"));
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[test]
fn dispute_resolve_and_chargeback() {
    let output = run_csv(
        "type,client,tx,amount\n\
         deposit,1,1,5\n\
         dispute,1,1,\n\
         resolve,1,1,\n\
         deposit,2,2,3\n\
         dispute,2,2,\n\
         chargeback,2,2,\n",
    );
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "client,available,held,total,locked\n\
         1,5.0000,0.0000,5.0000,false\n\
         2,0.0000,0.0000,0.0000,true\n"
    );
}

#[test]
fn whitespace_and_excess_precision_are_handled_without_rounding() {
    let output = run_csv(
        "type, client, tx, amount\n\
         deposit, 1, 1, 1.99999\n",
    );
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "client,available,held,total,locked\n1,1.9999,0.0000,1.9999,false\n"
    );
}

#[test]
fn malformed_input_fails_before_output() {
    let output = run_csv("type,client,tx,amount\ndeposit,1,1,1\ndeposit,1,2,1.2345oops\n");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("record 3"), "{stderr}");
    assert!(stderr.contains("invalid amount"), "{stderr}");

    let bad_header = run_csv("kind,client,tx,amount\ndeposit,1,1,1\n");
    assert!(!bad_header.status.success());
    assert!(bad_header.stdout.is_empty());
    assert!(
        String::from_utf8(bad_header.stderr)
            .unwrap()
            .contains("record 1")
    );
}

#[test]
fn rejected_transaction_does_not_stop_later_rows() {
    let output = run_csv_with_log_level(
        "type,client,tx,amount\n\
         deposit,1,1,2\n\
         withdrawal,1,2,3\n\
         deposit,1,3,1\n",
        Some("info"),
    );
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "client,available,held,total,locked\n1,3.0000,0.0000,3.0000,false\n"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("record_number=3"), "{stderr}");
    assert!(stderr.contains("rejected=1"), "{stderr}");
}

#[test]
fn dispute_requires_empty_amount() {
    let output = run_csv("type,client,tx,amount\ndeposit,1,1,1\ndispute,1,1,2\n");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("record 3"), "{stderr}");
    assert!(stderr.contains("unexpected amount for dispute"), "{stderr}");
}

#[test]
fn deposits_and_withdrawals_require_amounts() {
    for kind in ["deposit", "withdrawal"] {
        let output = run_csv(&format!("type,client,tx,amount\n{kind},1,1,\n"));
        assert!(!output.status.success(), "{kind}");
        assert!(output.stdout.is_empty(), "{kind}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("record 2"), "{stderr}");
        assert!(
            stderr.contains(&format!("missing amount for {kind}")),
            "{stderr}"
        );
    }
}
