use std::{cell::RefCell, collections::BTreeSet, io::Write, path::PathBuf, rc::Rc};

use super::*;
use anyhow::{Context, Result, ensure};
use hegel::{
    TestCase, generators as gs,
    stateful::{Invariant, Rule, StateMachine},
};

#[derive(Clone, Copy)]
struct Profile {
    name: &'static str,
    steps: i64,
    client_min: ClientId,
    client_max: ClientId,
    deposit_min: i64,
    deposit_max: i64,
    withdrawal_min: i64,
    withdrawal_max: i64,
    // deposit, withdrawal, eligible dispute, random dispute, resolve, chargeback
    weights: [f64; 6],
    min_rows: usize,
    max_rows: usize,
    min_clients: usize,
    max_clients: usize,
    // deposit, withdrawal, dispute, resolve, chargeback
    min_event_counts: [usize; 5],
    require_rejection: bool,
}

const DEFAULT_PROFILE: Profile = Profile {
    name: "property-test",
    steps: 1000,
    client_min: 0,
    client_max: 10,
    deposit_min: 1,
    deposit_max: 10_000_000_000,
    withdrawal_min: 1,
    withdrawal_max: 5_000_000_000,
    weights: [10.0, 6.0, 3.0, 1.0, 2.0, 2.0],
    min_rows: 0,
    max_rows: 1000,
    min_clients: 0,
    max_clients: 11,
    min_event_counts: [0; 5],
    require_rejection: false,
};

fn export_profile(name: &str) -> Option<Profile> {
    Some(match name {
        "small-deposits" => Profile {
            name: "small-deposits",
            steps: 24,
            client_min: 1,
            client_max: 1,
            weights: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            min_rows: 10,
            max_rows: 30,
            min_clients: 1,
            max_clients: 1,
            min_event_counts: [10, 0, 0, 0, 0],
            ..DEFAULT_PROFILE
        },
        "mixed-short" => Profile {
            name: "mixed-short",
            steps: 40,
            client_min: 1,
            client_max: 3,
            weights: [10.0, 6.0, 0.0, 0.0, 0.0, 0.0],
            min_rows: 20,
            max_rows: 50,
            min_clients: 2,
            max_clients: 3,
            min_event_counts: [1, 1, 0, 0, 0],
            ..DEFAULT_PROFILE
        },
        "withdrawal-heavy" => Profile {
            name: "withdrawal-heavy",
            steps: 80,
            client_min: 1,
            client_max: 3,
            weights: [4.0, 12.0, 0.0, 0.0, 0.0, 0.0],
            min_rows: 50,
            max_rows: 100,
            min_clients: 2,
            max_clients: 3,
            min_event_counts: [1, 20, 0, 0, 0],
            require_rejection: true,
            ..DEFAULT_PROFILE
        },
        "dispute-lifecycle" => Profile {
            name: "dispute-lifecycle",
            steps: 80,
            client_min: 1,
            client_max: 3,
            weights: [8.0, 4.0, 5.0, 1.0, 4.0, 4.0],
            min_rows: 50,
            max_rows: 100,
            min_clients: 2,
            max_clients: 3,
            min_event_counts: [1, 1, 1, 1, 1],
            ..DEFAULT_PROFILE
        },
        "tiny-amounts" => Profile {
            name: "tiny-amounts",
            steps: 50,
            client_min: 1,
            client_max: 2,
            deposit_min: 1,
            deposit_max: 100,
            withdrawal_min: 1,
            withdrawal_max: 50,
            weights: [10.0, 6.0, 0.0, 0.0, 0.0, 0.0],
            min_rows: 30,
            max_rows: 60,
            min_clients: 2,
            max_clients: 2,
            min_event_counts: [1, 1, 0, 0, 0],
            ..DEFAULT_PROFILE
        },
        "many-clients-long" => Profile {
            name: "many-clients-long",
            steps: 800,
            client_min: 1,
            client_max: 10,
            weights: [10.0, 6.0, 3.0, 1.0, 2.0, 0.1],
            min_rows: 500,
            max_rows: 1000,
            min_clients: 8,
            max_clients: 10,
            min_event_counts: [1, 1, 1, 1, 1],
            ..DEFAULT_PROFILE
        },
        _ => return None,
    })
}

#[derive(Clone, Copy)]
struct TraceEntry {
    event: Event,
    accepted: bool,
}

// TODO: think about an oracle to test against.
// TODO: come up with more invariants.
struct PaymentModel {
    profile: Profile,
    processor: PaymentProcessor,
    disputable_deposits: hegel::stateful::Pool<(ClientId, TxId)>,
    currently_disputed_deposits: hegel::stateful::Pool<(ClientId, TxId)>,
    used_tx_ids: BTreeSet<TxId>,
    trace: Option<Rc<RefCell<Vec<TraceEntry>>>>,

    previous_state: Option<ModelSnapshot>,
}

struct ModelSnapshot {
    processor: PaymentProcessor,
}

impl From<&PaymentModel> for ModelSnapshot {
    fn from(value: &PaymentModel) -> Self {
        Self {
            processor: value.processor.clone(),
        }
    }
}

impl PaymentModel {
    fn apply_event(&mut self, event: Event) -> anyhow::Result<()> {
        if let Some(trace) = &self.trace {
            trace.borrow_mut().push(TraceEntry {
                event,
                accepted: false,
            });
        }
        let result = self.processor.on_event(event);
        if let Some(trace) = &self.trace {
            trace
                .borrow_mut()
                .last_mut()
                .expect("just appended")
                .accepted = result.is_ok();
        }
        result
    }

    fn deposit(&mut self, tc: TestCase) {
        let tx_id = self.draw_unused_tx_id(&tc);
        let client_id = self.draw_client_id(&tc);
        // Considering the fixed-point underlying type, this range represents
        // 0.0001 (the smallest allowed value) to 1_000_000.0000 (a very big
        // deposit!).
        let amount = Money::from_mantissa(
            tc.draw(
                gs::integers::<i64>()
                    .min_value(self.profile.deposit_min)
                    .max_value(self.profile.deposit_max),
            ),
        );

        tc.note(&format!("deposit for client {client_id}, amount {amount}"));
        let deposit = Event::Deposit {
            client_id,
            tx_id,
            amount,
        };
        let result = self.apply_event(deposit);
        tc.note(&format!("result: {result:?}"));

        tc.note(&format!(
            "updated account: {:?}",
            self.processor.account(client_id)
        ));

        // Mark this deposit as eligible to be disputed later.
        if result.is_ok() {
            self.disputable_deposits.add((client_id, tx_id));
        }
    }

    fn withdrawal(&mut self, tc: TestCase) {
        let tx_id = self.draw_unused_tx_id(&tc);
        let client_id = self.draw_client_id(&tc);
        // Considering the fixed-point underlying type, this range represents
        // 0.0001 (the smallest allowed value) to 500_000.0000 (a very big
        // withdrawal!).
        let amount = Money::from_mantissa(
            tc.draw(
                gs::integers::<i64>()
                    .min_value(self.profile.withdrawal_min)
                    .max_value(self.profile.withdrawal_max),
            ),
        );

        tc.note(&format!(
            "withdrawal for client {client_id}, amount {amount}"
        ));
        let withdrawal = Event::Withdrawal {
            client_id,
            tx_id,
            amount,
        };
        let result = self.apply_event(withdrawal);
        tc.note(&format!("result: {result:?}"));

        tc.note(&format!(
            "updated account: {:?}",
            self.processor.account(client_id)
        ));
    }

    fn dispute_eligible_deposit(&mut self, tc: TestCase) {
        // TODO: also generate disputes that refer a non-deposit or a deposit
        //       that is already being disputed.

        // A dispute for a non-disputable transaction is still interesting input,
        // but for now let's simply avoid those.

        // Destructuring it immediately makes Hegel not annotate the draw by its name.
        let disputed_deposit = tc.draw(self.disputable_deposits.values_consumed());
        let (client_id, tx_id) = disputed_deposit;

        tc.note(&format!(
            "dispute for client {client_id}, deposit_id {tx_id}"
        ));

        let dispute = Event::Dispute {
            client_id,
            referred_tx_id: tx_id,
        };
        let result = self.apply_event(dispute);
        tc.note(&format!("result: {result:?}"));

        if result.is_ok() {
            self.currently_disputed_deposits.add((client_id, tx_id));
        } else {
            self.disputable_deposits.add((client_id, tx_id));
        }

        tc.note(&format!(
            "updated account: {:?}",
            self.processor.account(client_id)
        ));
    }

    // TODO: would be nice to add documentation.
    fn dispute_random(&mut self, tc: TestCase) {
        tc.event("dispute random");
        let client_id = self.draw_client_id(&tc);
        let random_tx_id = loop {
            let candidate = tc.draw(gs::integers::<TxId>());
            if !self.used_tx_ids.contains(&candidate) {
                break candidate;
            }
        };

        tc.note(&format!(
            "dispute for client {client_id}, random_tx_id {random_tx_id}; tx_id was not generated"
        ));

        let dispute = Event::Dispute {
            client_id,
            referred_tx_id: random_tx_id,
        };
        let result = self.apply_event(dispute);
        tc.note(&format!("result: {result:?}"));

        // TODO: maybe simply let dispute generate random numbers anyway?
        //       unclear whether trying to control the weights is helpful.
        tc.note(&format!(
            "updated account: {:?}",
            self.processor.account(client_id)
        ));
    }

    fn resolve(&mut self, tc: TestCase) {
        // TODO: also generate resolves that refer a non-deposit or a deposit
        //       that is not being disputed.

        // Destructuring it immediately makes Hegel not annotate the draw by its name.
        let resolved_deposit = tc.draw(self.currently_disputed_deposits.values_consumed());
        let (client_id, tx_id) = resolved_deposit;

        tc.note(&format!(
            "resolve for client {client_id}, deposit_id {tx_id}"
        ));

        let resolve = Event::Resolve {
            client_id,
            referred_tx_id: tx_id,
        };
        let result = self.apply_event(resolve);
        tc.note(&format!("result: {result:?}"));
        if result.is_err() {
            self.currently_disputed_deposits.add((client_id, tx_id));
        }

        tc.note(&format!(
            "updated account: {:?}",
            self.processor.account(client_id)
        ));
    }

    fn chargeback(&mut self, tc: TestCase) {
        // TODO: also generate chargebacks that refer a non-deposit or a deposit
        //       that is not being disputed.

        // Destructuring it immediately makes Hegel not annotate the draw by its name.
        let chargedback_deposit = tc.draw(self.currently_disputed_deposits.values_consumed());
        let (client_id, tx_id) = chargedback_deposit;

        tc.note(&format!(
            "chargeback for client {client_id}, deposit_id {tx_id}"
        ));

        let chargeback = Event::Chargeback {
            client_id,
            referred_tx_id: tx_id,
        };
        let result = self.apply_event(chargeback);
        tc.note(&format!("result: {result:?}"));
        if result.is_err() {
            self.currently_disputed_deposits.add((client_id, tx_id));
        }

        // TODO: there should be a more ergonomic way of printing something after every rule.
        tc.note(&format!(
            "updated account: {:?}",
            self.processor.account(client_id)
        ));
    }

    // TODO: need to review all of these invariants. which is good, because they
    //       will fail soon (update: they indeed are failing!)

    // TODO: review this reasoning below. for now, simply commenting out and moving on
    //       to work on different failures.
    // "total is non negative" is not necessarily true, as it depends on how we
    // deal with disputes with insufficient funds. Suppose the following
    // scenario:
    //
    // 1. deposit 500
    // 2. withdrawal 500
    // 3. dispute-deposit
    //
    // Allowing the dispute means allowing a negative available balance. If that
    // dispute is then charged back, the account total becomes negative.
    //
    // I see two ways of dealing with this:
    // A. do not allow disputes if there aren't enough available funds.
    // B. do not allow chargebacks if there aren't enough available funds.
    //
    // And I don't know which one to pick yet. Will need to think more about it. But anyway,
    // this invariant is commented out for the time being.
    // #[invariant(always_run)]
    // fn total_is_non_negative(&self, _: TestCase) {
    //     for account in self.processor.accounts.values() {
    //         assert!(account.total >= 0); // TODO: might change if we allow negative totals.
    //     }
    // }

    fn available_is_total_minus_held(&self, _: TestCase) {
        for account in self.processor.accounts.values() {
            assert_eq!(account.available, account.total - account.held);
        }
    }

    fn held_is_total_minus_available(&self, _: TestCase) {
        for account in self.processor.accounts.values() {
            assert_eq!(account.held, account.total - account.available);
        }
    }

    fn total_is_available_plus_held(&self, _: TestCase) {
        for account in self.processor.accounts.values() {
            assert_eq!(account.total, account.available + account.held);
        }
    }

    fn locked_accounts_are_not_mutable(&self, _: TestCase) {
        let Some(previous) = &self.previous_state else {
            return;
        };

        let current_accounts = &self.processor.accounts;

        for (id, previous_account) in &previous.processor.accounts {
            if previous_account.is_locked {
                let current_account = current_accounts
                    .get(&id)
                    .unwrap_or_else(|| panic!("previously locked account {id} is missing!"));

                // TODO: could use an Eq implementation here, but a bit weird, maybe.
                assert_eq!(previous_account.available, current_account.available);
                assert_eq!(previous_account.held, current_account.held);
                assert_eq!(previous_account.total, current_account.total);
                assert_eq!(previous_account.is_locked, current_account.is_locked);
            }
        }
    }

    // Transition checks require knowledge of the previous state (to compare
    // with the current one). This is a hacky way of persisting that state:
    // because an always_run invariant runs after every step, and because it
    // allows for mutable state, we can make it save the state for the future.
    // But this isn't ideal — if we removed the always_run annotation, this
    // invariant would only run after _some_ steps, and the state tracking would
    // be inconsistent.
    // TODO: investigate a better way of doing this.
    fn save_snapshot(&mut self, _: TestCase) {
        self.previous_state = Some(ModelSnapshot::from(&*self));
    }

    fn draw_unused_tx_id(&mut self, tc: &TestCase) -> TxId {
        // TODO: an observant reader might see that used tx ids are never updated,
        //       which defeats the purpose of this function. I'm letting it go because
        //       I want to see whether the proptests will catch that failure eventually
        //       (when two transactions with the same id cause havoc).

        // This is a naive way of drawing an available transaction id,
        // but it shall suffice for now.
        loop {
            let candidate_tx_id = tc.draw(gs::integers::<TxId>());

            if self.used_tx_ids.contains(&candidate_tx_id) {
                tc.note(&format!(
                    "transaction ID {candidate_tx_id} is already used; retrying"
                ));
                continue;
            }

            tc.note(&format!("selected transaction ID {candidate_tx_id}"));
            self.used_tx_ids.insert(candidate_tx_id);
            return candidate_tx_id;
        }
    }

    fn draw_client_id(&mut self, tc: &TestCase) -> ClientId {
        tc.draw_named(
            "client_id",
            gs::integers::<ClientId>()
                .min_value(self.profile.client_min)
                .max_value(self.profile.client_max),
        )
    }
}

impl StateMachine for PaymentModel {
    fn rules(&self) -> Vec<Rule<Self>> {
        let candidates: [(&str, fn(&mut Self, TestCase)); 6] = [
            ("deposit", Self::deposit),
            ("withdrawal", Self::withdrawal),
            ("dispute_eligible_deposit", Self::dispute_eligible_deposit),
            ("dispute_random", Self::dispute_random),
            ("resolve", Self::resolve),
            ("chargeback", Self::chargeback),
        ];
        candidates
            .into_iter()
            .zip(self.profile.weights)
            .filter(|(_, weight)| *weight > 0.0)
            .map(|((name, apply), weight)| Rule::new(name, weight, apply))
            .collect()
    }

    fn invariants(&self) -> Vec<Invariant<Self>> {
        vec![
            Invariant::new_always_run("available_is_total_minus_held", |m, tc| {
                m.available_is_total_minus_held(tc)
            }),
            Invariant::new_always_run("held_is_total_minus_available", |m, tc| {
                m.held_is_total_minus_available(tc)
            }),
            Invariant::new_always_run("total_is_available_plus_held", |m, tc| {
                m.total_is_available_plus_held(tc)
            }),
            Invariant::new_always_run("locked_accounts_are_not_mutable", |m, tc| {
                m.locked_accounts_are_not_mutable(tc)
            }),
            Invariant::new_always_run("save_snapshot", |m, tc| m.save_snapshot(tc)),
        ]
    }
}

#[hegel::test(report_multiple_failures = true, test_cases = 5000, suppress_health_check = [hegel::HealthCheck::TooSlow])]
fn state_machine_run(tc: TestCase) {
    let processor = PaymentProcessor::default();
    let model = PaymentModel {
        profile: DEFAULT_PROFILE,
        processor,
        disputable_deposits: hegel::stateful::pool(&tc),
        currently_disputed_deposits: hegel::stateful::pool(&tc),
        used_tx_ids: Default::default(),
        trace: None,
        previous_state: Default::default(),
    };
    hegel::stateful::machine(model)
        .steps(DEFAULT_PROFILE.steps)
        .run(tc)
}

struct TraceStats {
    clients: usize,
    counts: [usize; 5],
    rejected: usize,
}

fn validate_trace(profile: Profile, trace: &[TraceEntry]) -> Result<TraceStats> {
    ensure!(
        (profile.min_rows..=profile.max_rows).contains(&trace.len()),
        "{}: expected {}..={} rows, got {}",
        profile.name,
        profile.min_rows,
        profile.max_rows,
        trace.len()
    );

    let mut clients = BTreeSet::new();
    let mut counts = [0; 5];
    let mut rejected = 0;
    for entry in trace {
        let client_id = *entry.event.client_id();
        ensure!(
            (profile.client_min..=profile.client_max).contains(&client_id),
            "{}: client {client_id} outside profile range",
            profile.name
        );
        clients.insert(client_id);
        if !entry.accepted {
            rejected += 1;
        }
        let category = match entry.event {
            Event::Deposit { amount, .. } => {
                ensure!(
                    (profile.deposit_min..=profile.deposit_max).contains(&amount.mantissa()),
                    "{}: deposit amount outside profile range",
                    profile.name
                );
                0
            }
            Event::Withdrawal { amount, .. } => {
                ensure!(
                    (profile.withdrawal_min..=profile.withdrawal_max).contains(&amount.mantissa()),
                    "{}: withdrawal amount outside profile range",
                    profile.name
                );
                1
            }
            Event::Dispute { .. } => 2,
            Event::Resolve { .. } => 3,
            Event::Chargeback { .. } => 4,
        };
        counts[category] += 1;
    }

    ensure!(
        (profile.min_clients..=profile.max_clients).contains(&clients.len()),
        "{}: expected {}..={} clients, got {}",
        profile.name,
        profile.min_clients,
        profile.max_clients,
        clients.len()
    );
    for (kind, (&actual, &minimum)) in ["deposit", "withdrawal", "dispute", "resolve", "chargeback"]
        .into_iter()
        .zip(counts.iter().zip(&profile.min_event_counts))
    {
        ensure!(
            actual >= minimum,
            "{}: expected at least {minimum} {kind} events, got {actual}",
            profile.name
        );
    }
    ensure!(
        !profile.require_rejection || rejected > 0,
        "{}: expected at least one rejected event",
        profile.name
    );
    Ok(TraceStats {
        clients: clients.len(),
        counts,
        rejected,
    })
}

fn write_fixture(
    path: &std::path::Path,
    profile: Profile,
    seed: u64,
    trace: &[TraceEntry],
    stats: &TraceStats,
) -> Result<()> {
    let parent = path.parent().context("output path has no parent")?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    let events: Vec<Event> = trace.iter().map(|entry| entry.event).collect();
    let mut csv_temp = tempfile::NamedTempFile::new_in(parent)?;
    crate::event_csv::write_events(&mut csv_temp, &events)?;
    csv_temp.flush()?;

    let manifest_path = path.with_extension("manifest");
    let mut manifest_temp = tempfile::NamedTempFile::new_in(parent)?;
    writeln!(manifest_temp, "format_version=1")?;
    writeln!(manifest_temp, "hegeltest_version=0.47.4")?;
    writeln!(manifest_temp, "profile={}", profile.name)?;
    writeln!(manifest_temp, "seed={seed}")?;
    writeln!(manifest_temp, "steps={}", profile.steps)?;
    writeln!(
        manifest_temp,
        "client_range={}..={}",
        profile.client_min, profile.client_max
    )?;
    writeln!(
        manifest_temp,
        "deposit_mantissa_range={}..={}",
        profile.deposit_min, profile.deposit_max
    )?;
    writeln!(
        manifest_temp,
        "withdrawal_mantissa_range={}..={}",
        profile.withdrawal_min, profile.withdrawal_max
    )?;
    writeln!(manifest_temp, "rule_weights={:?}", profile.weights)?;
    writeln!(
        manifest_temp,
        "required_row_range={}..={}",
        profile.min_rows, profile.max_rows
    )?;
    writeln!(
        manifest_temp,
        "required_client_range={}..={}",
        profile.min_clients, profile.max_clients
    )?;
    writeln!(
        manifest_temp,
        "minimum_event_counts={:?}",
        profile.min_event_counts
    )?;
    writeln!(
        manifest_temp,
        "require_rejection={}",
        profile.require_rejection
    )?;
    writeln!(manifest_temp, "rows={}", trace.len())?;
    writeln!(manifest_temp, "clients={}", stats.clients)?;
    writeln!(manifest_temp, "event_counts={:?}", stats.counts)?;
    writeln!(manifest_temp, "rejected={}", stats.rejected)?;
    manifest_temp.flush()?;

    csv_temp
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("persisting {}", path.display()))?;
    if let Err(error) = manifest_temp.persist(&manifest_path) {
        let _ = std::fs::remove_file(path);
        return Err(error.error).with_context(|| format!("persisting {}", manifest_path.display()));
    }
    Ok(())
}

#[test]
#[ignore = "run with PAYMENT_EXPORT_PROFILE, PAYMENT_EXPORT_OUTPUT, and PAYMENT_EXPORT_SEED"]
fn export_generated_case() -> Result<()> {
    let profile_name =
        std::env::var("PAYMENT_EXPORT_PROFILE").context("PAYMENT_EXPORT_PROFILE is required")?;
    let profile = export_profile(&profile_name)
        .with_context(|| format!("unknown profile: {profile_name}"))?;
    let output = PathBuf::from(
        std::env::var("PAYMENT_EXPORT_OUTPUT").context("PAYMENT_EXPORT_OUTPUT is required")?,
    );
    let seed: u64 = std::env::var("PAYMENT_EXPORT_SEED")
        .context("PAYMENT_EXPORT_SEED is required")?
        .parse()
        .context("PAYMENT_EXPORT_SEED must be a u64")?;

    let trace = Rc::new(RefCell::new(Vec::new()));
    let captured = trace.clone();
    hegel::Hegel::new(move |tc| {
        captured.borrow_mut().clear();
        let model = PaymentModel {
            profile,
            processor: PaymentProcessor::default(),
            disputable_deposits: hegel::stateful::pool(&tc),
            currently_disputed_deposits: hegel::stateful::pool(&tc),
            used_tx_ids: Default::default(),
            trace: Some(captured.clone()),
            previous_state: None,
        };
        hegel::stateful::machine(model).steps(profile.steps).run(tc);
    })
    .settings(
        hegel::Settings::new()
            .test_cases(1)
            .seed(Some(seed))
            .database(None)
            .phases([hegel::Phase::Generate])
            .suppress_health_check([hegel::HealthCheck::TooSlow]),
    )
    .run();

    let trace = trace.borrow();
    let stats = validate_trace(profile, &trace)?;
    write_fixture(&output, profile, seed, &trace, &stats)?;
    let parsed = crate::event_csv::read_events(&output)?;
    ensure!(
        parsed
            .into_iter()
            .map(|(_, event)| event)
            .eq(trace.iter().map(|entry| entry.event)),
        "exported CSV does not match captured trace"
    );
    eprintln!(
        "exported {}: {} rows, {} clients, {} rejected",
        profile.name,
        trace.len(),
        stats.clients,
        stats.rejected
    );
    Ok(())
}
