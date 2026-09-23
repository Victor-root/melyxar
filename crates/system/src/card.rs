//! How busy the graphics card is, from what its driver says about each
//! program that has it open.
//!
//! The usual tools read the card's own counters, which asks for rights an
//! unprivileged container does not have. The driver also keeps a tally for
//! every open handle on the card, in the handle's `fdinfo`, and a program may
//! read the tallies of its own processes. This server's conversions are its
//! own processes, and the container runs nothing else on the card, so their
//! tallies added up are the card's work.
//!
//! Two ways of counting are written there, depending on the driver: time spent
//! on each engine in nanoseconds (`drm-engine-render: 9288 ns`), or cycles
//! spent against the cycles that went by (`drm-cycles-rcs` and
//! `drm-total-cycles-rcs`). Both are read.

use std::collections::BTreeMap;
use std::path::Path;

use crate::Sources;

/// One engine of the card, as one handle has used it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Engine {
    /// Nanoseconds or cycles spent on it.
    pub busy: u64,
    /// The cycles that went by meanwhile, for a driver counting in cycles;
    /// nothing for one counting in time.
    pub elapsed: Option<u64>,
    /// How many engines of this kind the card has. Two video engines at work
    /// for a second each is one second of the pair, not two.
    pub capacity: u64,
}

/// One handle on the card: who it is, and what it spent on each engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    /// The card and the handle's number on it. Several open files of one
    /// process can share a handle, and it is counted once.
    pub id: String,
    pub engines: BTreeMap<String, Engine>,
}

/// Makes sense of one `fdinfo`. Nothing when it is not the file of a card, or
/// its driver keeps no tally.
pub fn client(fdinfo: &str) -> Option<Client> {
    let fields: BTreeMap<&str, &str> = fdinfo
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim(), value.trim()))
        .collect();
    let number = fields.get("drm-client-id")?;
    let card = fields.get("drm-pdev").copied().unwrap_or("");

    let number_in = |value: &str| -> Option<u64> { value.split_whitespace().next()?.parse().ok() };
    let capacity_of = |engine: &str| -> u64 {
        fields
            .get(format!("drm-engine-capacity-{engine}").as_str())
            .and_then(|value| number_in(value))
            .filter(|capacity| *capacity > 0)
            .unwrap_or(1)
    };

    let mut engines = BTreeMap::new();
    for (key, value) in &fields {
        if let Some(engine) = key.strip_prefix("drm-engine-") {
            if engine.starts_with("capacity-") {
                continue;
            }
            let Some(busy) = number_in(value) else { continue };
            engines.insert(
                engine.to_string(),
                Engine {
                    busy,
                    elapsed: None,
                    capacity: capacity_of(engine),
                },
            );
        } else if let Some(engine) = key.strip_prefix("drm-cycles-") {
            let Some(busy) = number_in(value) else { continue };
            let elapsed = fields
                .get(format!("drm-total-cycles-{engine}").as_str())
                .and_then(|total| number_in(total));
            engines.insert(
                engine.to_string(),
                Engine {
                    busy,
                    elapsed,
                    capacity: capacity_of(engine),
                },
            );
        }
    }

    (!engines.is_empty()).then(|| Client {
        id: format!("{card}/{number}"),
        engines,
    })
}

/// Every handle on a card this server can see, each once.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Handles(pub BTreeMap<String, Client>);

/// Walks the processes this server may look into for open files on a graphics
/// card, and reads each one's tally. A process that went away or cannot be
/// read is passed over: the next reading has it or does not.
pub fn handles(sources: &Sources) -> Handles {
    let mut found = BTreeMap::new();
    let Ok(processes) = std::fs::read_dir(&sources.proc) else {
        return Handles(found);
    };
    for process in processes.filter_map(Result::ok) {
        let name = process.file_name();
        if !name.to_string_lossy().bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let Ok(files) = std::fs::read_dir(process.path().join("fd")) else {
            continue;
        };
        for file in files.filter_map(Result::ok) {
            if !opens_a_card(&file.path()) {
                continue;
            }
            let fdinfo = process.path().join("fdinfo").join(file.file_name());
            if let Some(client) = std::fs::read_to_string(fdinfo).ok().as_deref().and_then(client) {
                found.entry(client.id.clone()).or_insert(client);
            }
        }
    }
    Handles(found)
}

/// Whether an open file is a graphics card. Asked of the link before the
/// tally is read, so the few hundred sockets and files of the server itself
/// cost one look each and nothing more.
fn opens_a_card(file: &Path) -> bool {
    std::fs::read_link(file).is_ok_and(|target| target.starts_with("/dev/dri/"))
}

/// The share of the busiest engine between two readings, from nought to one.
///
/// The busiest engine rather than an average: a conversion that keeps the
/// video engine full while the others sit idle is a card that cannot take
/// another conversion, and an average of a quarter would say it could.
/// Handles that were not there at both readings say nothing about the time
/// between them and are left out.
pub fn busy_share(before: &Handles, after: &Handles, seconds: f64) -> f64 {
    if seconds <= 0.0 {
        return 0.0;
    }
    let mut by_engine: BTreeMap<&str, f64> = BTreeMap::new();
    for (id, now) in &after.0 {
        let Some(then) = before.0.get(id) else { continue };
        for (name, engine) in &now.engines {
            let Some(earlier) = then.engines.get(name) else { continue };
            let busy = engine.busy.saturating_sub(earlier.busy) as f64;
            let share = match (engine.elapsed, earlier.elapsed) {
                (Some(elapsed), Some(was)) => {
                    let went_by = elapsed.saturating_sub(was) as f64;
                    if went_by == 0.0 {
                        continue;
                    }
                    busy / went_by
                }
                _ => busy / (seconds * 1e9),
            } / engine.capacity as f64;
            *by_engine.entry(name.as_str()).or_default() += share;
        }
    }
    by_engine
        .into_values()
        .fold(0.0_f64, f64::max)
        .clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A_HANDLE_IN_TIME: &str = "pos:\t0\nflags:\t02100002\n\
        drm-driver:\ti915\ndrm-client-id:\t7\ndrm-pdev:\t0000:03:00.0\n\
        drm-engine-render:\t1000000000 ns\ndrm-engine-video:\t500000000 ns\n\
        drm-engine-capacity-video:\t2\n";

    const A_HANDLE_IN_CYCLES: &str = "drm-driver:\txe\ndrm-client-id:\t42\n\
        drm-pdev:\t0000:03:00.0\ndrm-cycles-vcs:\t300\ndrm-total-cycles-vcs:\t1000\n";

    #[test]
    fn a_tally_in_time_is_read_with_how_many_engines_share_it() {
        let client = client(A_HANDLE_IN_TIME).expect("a card");
        assert_eq!(client.id, "0000:03:00.0/7");
        assert_eq!(
            client.engines["video"],
            Engine {
                busy: 500_000_000,
                elapsed: None,
                capacity: 2
            }
        );
        assert_eq!(client.engines["render"].capacity, 1);
        assert!(!client.engines.contains_key("capacity-video"));
    }

    #[test]
    fn a_tally_in_cycles_is_read_with_the_cycles_that_went_by() {
        let client = client(A_HANDLE_IN_CYCLES).expect("a card");
        assert_eq!(
            client.engines["vcs"],
            Engine {
                busy: 300,
                elapsed: Some(1000),
                capacity: 1
            }
        );
    }

    #[test]
    fn a_file_that_is_not_a_card_has_no_tally() {
        assert_eq!(client("pos:\t0\nflags:\t02\nmnt_id:\t26\n"), None);
    }

    fn handles_of(clients: &[Client]) -> Handles {
        Handles(
            clients
                .iter()
                .map(|client| (client.id.clone(), client.clone()))
                .collect(),
        )
    }

    fn with_busy(mut client: Client, engine: &str, busy: u64, elapsed: Option<u64>) -> Client {
        let entry = client.engines.get_mut(engine).expect("engine");
        entry.busy = busy;
        entry.elapsed = elapsed;
        client
    }

    #[test]
    fn the_busiest_engine_is_the_card_s_share() {
        let first = client(A_HANDLE_IN_TIME).expect("a card");
        // Over two seconds: render busy a quarter of the time, the pair of
        // video engines busy two seconds between them, which is half of them.
        let later = with_busy(
            with_busy(first.clone(), "render", 1_500_000_000, None),
            "video",
            2_500_000_000,
            None,
        );
        let share = busy_share(&handles_of(&[first]), &handles_of(&[later]), 2.0);
        assert!((share - 0.5).abs() < 1e-9, "{share}");
    }

    #[test]
    fn cycles_are_measured_against_the_cycles_that_went_by() {
        let first = client(A_HANDLE_IN_CYCLES).expect("a card");
        let later = with_busy(first.clone(), "vcs", 1100, Some(2000));
        let share = busy_share(&handles_of(&[first]), &handles_of(&[later]), 2.0);
        assert!((share - 0.8).abs() < 1e-9, "{share}");
    }

    #[test]
    fn a_handle_seen_only_once_says_nothing_and_an_idle_card_is_idle() {
        let only_now = client(A_HANDLE_IN_TIME).expect("a card");
        assert_eq!(busy_share(&Handles::default(), &handles_of(&[only_now]), 2.0), 0.0);
        assert_eq!(busy_share(&Handles::default(), &Handles::default(), 2.0), 0.0);
    }
}
