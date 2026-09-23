//! What the machine is spending, measured on a steady beat for the
//! administration's curves.
//!
//! One reading every two seconds, kept in memory for the last ten minutes:
//! that is what the live figures and the shortest curve are drawn from, and it
//! never touches the database. Once a minute, the average of the minute is
//! written down; once an hour, the minutes older than a week are folded into
//! hours and the hours older than a year are forgotten.
//!
//! The reading itself is a handful of small files the kernel writes, read on a
//! thread made for waiting rather than on the ones that answer pages.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use melyxar_core::time::Timestamp;
use melyxar_database::measures::{Measure, Span};
use melyxar_system::card::Handles;
use melyxar_system::{Disk, Memory, ProcessorTimes, Sources, Traffic};
use serde::Serialize;

use crate::{AppState, Result};

/// How often the machine is read.
const READ_EVERY: std::time::Duration = std::time::Duration::from_secs(2);

/// How much of the beat is kept in memory: ten minutes of it.
const KEPT_IN_MEMORY: usize = 300;

/// How long minutes are kept before they are folded into hours.
const MINUTES_KEPT_FOR: time::Duration = time::Duration::days(7);

/// How long hours are kept.
const HOURS_KEPT_FOR: time::Duration = time::Duration::days(365);

/// What the machine is, read once: it does not change while the server runs.
#[derive(Debug, Clone, Serialize)]
pub struct Machine {
    pub processor: Option<String>,
    pub threads: usize,
}

/// What the machine spent over one stretch of time.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Point {
    #[serde(with = "time::serde::rfc3339")]
    pub at: Timestamp,
    /// The share of the processors in use, from nought to one.
    pub processor: Option<f64>,
    pub memory_used: u64,
    pub memory_total: u64,
    pub load: Option<f64>,
    /// Bytes a second.
    pub received: f64,
    pub sent: f64,
    /// The share of the graphics card in use, when the server has one.
    pub card: Option<f64>,
    /// Degrees, when the machine lets them be read.
    pub temperature: Option<f64>,
}

/// What the measuring holds between two readings, for the pages to ask.
pub struct Measuring {
    machine: Machine,
    recent: Mutex<VecDeque<Point>>,
    disks: Mutex<Vec<Disk>>,
    /// Whether the database refused the last minute it was handed: the one
    /// write this server makes every minute, whatever else it is doing, and
    /// so the one that says whether it still can.
    writes_refused: AtomicBool,
    /// The folders of the libraries that were not there at the last look,
    /// by the label somebody gave them.
    missing_folders: Mutex<Vec<String>>,
}

impl Measuring {
    pub(crate) fn new() -> Self {
        let sources = Sources::default();
        let read = |name: &str| std::fs::read_to_string(sources.proc.join(name)).unwrap_or_default();
        Self {
            machine: Machine {
                processor: melyxar_system::processor_name(&read("cpuinfo")),
                threads: melyxar_system::processor_count(&read("stat")),
            },
            recent: Mutex::new(VecDeque::with_capacity(KEPT_IN_MEMORY)),
            disks: Mutex::new(Vec::new()),
            writes_refused: AtomicBool::new(false),
            missing_folders: Mutex::new(Vec::new()),
        }
    }

    /// Whether the database refused the last minute of measures.
    pub(crate) fn writes_refused(&self) -> bool {
        self.writes_refused.load(Ordering::Relaxed)
    }

    /// The folders of the libraries not found at the last look.
    pub(crate) fn missing_folders(&self) -> Vec<String> {
        self.missing_folders
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .clone()
    }

    /// The disks as they were at the last look.
    pub(crate) fn disks(&self) -> Vec<Disk> {
        self.disks.lock().unwrap_or_else(|held| held.into_inner()).clone()
    }

    fn remember(&self, point: Point) {
        let mut recent = self.recent.lock().unwrap_or_else(|held| held.into_inner());
        if recent.len() == KEPT_IN_MEMORY {
            recent.pop_front();
        }
        recent.push_back(point);
    }
}

/// Everything one reading of the machine gathers, before any of it is turned
/// into a share or a rate.
struct Counters {
    taken: Instant,
    processor: Option<ProcessorTimes>,
    memory: Option<Memory>,
    load: Option<f64>,
    traffic: Traffic,
    card: Handles,
    temperature: Option<f64>,
}

fn read_the_machine(sources: &Sources) -> Counters {
    let read = |name: &str| std::fs::read_to_string(sources.proc.join(name)).unwrap_or_default();
    Counters {
        taken: Instant::now(),
        processor: melyxar_system::processor_times(&read("stat")),
        memory: melyxar_system::memory(&read("meminfo")),
        load: melyxar_system::load(&read("loadavg")),
        traffic: melyxar_system::traffic(&read("net/dev")),
        card: melyxar_system::card::handles(sources),
        temperature: melyxar_system::processor_temperature(sources),
    }
}

/// One point, from two readings a beat apart.
fn point_between(before: &Counters, after: &Counters, has_a_card: bool, at: Timestamp) -> Point {
    let seconds = after.taken.duration_since(before.taken).as_secs_f64();
    let (received, sent) =
        melyxar_system::traffic_rate(before.traffic, after.traffic, seconds).unwrap_or_default();
    let memory = after.memory.unwrap_or(Memory {
        total_bytes: 0,
        available_bytes: 0,
    });
    Point {
        at,
        processor: before
            .processor
            .zip(after.processor)
            .and_then(|(was, is)| melyxar_system::busy_share(was, is)),
        memory_used: memory.used_bytes(),
        memory_total: memory.total_bytes,
        load: after.load,
        received,
        sent,
        card: has_a_card.then(|| melyxar_system::card::busy_share(&before.card, &after.card, seconds)),
        temperature: after.temperature,
    }
}

/// The average of a stretch of points, set at the instant it starts. Every
/// figure is averaged over the points that have it: a temperature read on
/// half of them is the average of that half, not of it and a row of zeroes.
fn average(points: &[Point], at: Timestamp) -> Option<Point> {
    fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
        let (sum, count) = values.fold((0.0, 0usize), |(sum, count), value| (sum + value, count + 1));
        (count > 0).then(|| sum / count as f64)
    }
    if points.is_empty() {
        return None;
    }
    Some(Point {
        at,
        processor: mean(points.iter().filter_map(|point| point.processor)),
        memory_used: mean(points.iter().map(|point| point.memory_used as f64)).unwrap_or(0.0) as u64,
        memory_total: points.iter().map(|point| point.memory_total).max().unwrap_or(0),
        load: mean(points.iter().filter_map(|point| point.load)),
        received: mean(points.iter().map(|point| point.received)).unwrap_or(0.0),
        sent: mean(points.iter().map(|point| point.sent)).unwrap_or(0.0),
        card: mean(points.iter().filter_map(|point| point.card)),
        temperature: mean(points.iter().filter_map(|point| point.temperature)),
    })
}

/// An instant with its seconds and what is below them taken off.
fn start_of_the_minute(at: Timestamp) -> Timestamp {
    at.replace_second(0)
        .and_then(|at| at.replace_nanosecond(0))
        .unwrap_or(at)
}

fn start_of_the_hour(at: Timestamp) -> Timestamp {
    start_of_the_minute(at).replace_minute(0).unwrap_or(at)
}

fn as_measure(point: &Point) -> Measure {
    Measure {
        at: point.at,
        processor: point.processor,
        memory_used: i64::try_from(point.memory_used).unwrap_or(i64::MAX),
        memory_total: i64::try_from(point.memory_total).unwrap_or(i64::MAX),
        load: point.load,
        received: point.received,
        sent: point.sent,
        card: point.card,
        temperature: point.temperature,
    }
}

fn as_point(measure: Measure) -> Point {
    Point {
        at: measure.at,
        processor: measure.processor,
        memory_used: u64::try_from(measure.memory_used).unwrap_or(0),
        memory_total: u64::try_from(measure.memory_total).unwrap_or(0),
        load: measure.load,
        received: measure.received,
        sent: measure.sent,
        card: measure.card,
        temperature: measure.temperature,
    }
}

/// The folders of the libraries, by label, and whether each is still there.
///
/// Asked of what the kernel already holds about the folder rather than of its
/// contents: a folder whose disk went away is gone from the tree, and finding
/// that out reads nothing from any disk, so a sleeping one is left asleep.
fn folders_still_there(roots: &[(String, PathBuf)]) -> Vec<String> {
    roots
        .iter()
        .filter(|(_, path)| {
            path != std::path::Path::new(melyxar_database::synthetic::BENCH_ROOT)
                && !std::fs::metadata(path).is_ok_and(|metadata| metadata.is_dir())
        })
        .map(|(label, _)| label.clone())
        .collect()
}

/// Looks at every disk worth watching, and at whether the folders of the
/// libraries are still there: every folder a library looks in, and every
/// folder the server writes to.
async fn look_at_the_disks(state: &AppState) {
    let roots: Vec<(String, PathBuf)> = match state.database().roots_with_access().await {
        Ok(roots) => roots
            .into_iter()
            .map(|entry| (entry.root.label, entry.root.path))
            .collect(),
        Err(error) => {
            tracing::warn!(%error, "the folders of the libraries could not be read for their disks");
            Vec::new()
        }
    };
    let directories = &state.config().directories;
    let mut folders: Vec<PathBuf> = roots.iter().map(|(_, path)| path.clone()).collect();
    folders.extend([
        directories.data.clone(),
        directories.cache.clone(),
        directories.transcodes.clone(),
    ]);
    let looked = tokio::task::spawn_blocking(move || {
        (melyxar_system::disks(&folders), folders_still_there(&roots))
    })
    .await;
    if let Ok((disks, missing)) = looked {
        let measuring = state.measuring();
        *measuring.disks.lock().unwrap_or_else(|held| held.into_inner()) = disks;
        *measuring.missing_folders.lock().unwrap_or_else(|held| held.into_inner()) = missing;
    }
}

/// Keeps measuring for as long as the server runs.
pub fn keep_measuring(state: &AppState) -> tokio::task::JoinHandle<()> {
    let state = state.clone();
    tokio::spawn(async move {
        let sources = Sources::default();
        let has_a_card = state
            .capabilities()
            .and_then(melyxar_ffmpeg::Capabilities::card)
            .is_some();
        let read = {
            let sources = sources.clone();
            move || {
                let sources = sources.clone();
                tokio::task::spawn_blocking(move || read_the_machine(&sources))
            }
        };

        let Ok(mut before) = read().await else {
            return;
        };
        look_at_the_disks(&state).await;
        let mut this_minute = start_of_the_minute(melyxar_core::time::now());
        let mut this_hour = start_of_the_hour(this_minute);
        let mut minute: Vec<Point> = Vec::new();

        loop {
            tokio::time::sleep(READ_EVERY).await;
            let Ok(after) = read().await else {
                continue;
            };
            let now = melyxar_core::time::now();
            let point = point_between(&before, &after, has_a_card, now);
            before = after;
            state.measuring().remember(point.clone());

            let started = start_of_the_minute(now);
            if started != this_minute {
                if let Some(kept) = average(&minute, this_minute) {
                    let written = state.database().keep_measure(Span::Minute, &as_measure(&kept)).await;
                    if let Err(error) = &written {
                        tracing::warn!(%error, "a minute of measures could not be written down");
                    }
                    state
                        .measuring()
                        .writes_refused
                        .store(written.is_err(), Ordering::Relaxed);
                }
                minute.clear();
                this_minute = started;
                look_at_the_disks(&state).await;
            }
            minute.push(point);

            let hour = start_of_the_hour(now);
            if hour != this_hour {
                this_hour = hour;
                if let Err(error) = state
                    .database()
                    .fold_measures(start_of_the_hour(now - MINUTES_KEPT_FOR), now - HOURS_KEPT_FOR)
                    .await
                {
                    tracing::warn!(%error, "old measures could not be folded into hours");
                }
            }
        }
    })
}

/// What the live figures are drawn from.
#[derive(Debug, Clone, Serialize)]
pub struct Live {
    pub machine: Machine,
    /// The last ten minutes, one point every two seconds, oldest first.
    pub recent: Vec<Point>,
    pub disks: Vec<DiskView>,
}

/// One disk as a page shows it.
#[derive(Debug, Clone, Serialize)]
pub struct DiskView {
    pub folders: Vec<String>,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

pub fn live(state: &AppState) -> Live {
    let measuring = state.measuring();
    Live {
        machine: measuring.machine.clone(),
        recent: measuring
            .recent
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .iter()
            .cloned()
            .collect(),
        disks: measuring
            .disks
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .iter()
            .map(|disk| DiskView {
                folders: disk
                    .folders
                    .iter()
                    .map(|folder| folder.display().to_string())
                    .collect(),
                total_bytes: disk.total_bytes,
                available_bytes: disk.available_bytes,
            })
            .collect(),
    }
}

/// How far back a curve looks, and how long each of its points is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Over {
    Hour,
    Day,
    Week,
    Month,
    Year,
}

impl Over {
    /// A curve of somewhere between sixty and a hundred and seventy points,
    /// whatever its length: enough to see a shape, few enough to send.
    fn reach_and_stretch(self) -> (time::Duration, i64) {
        match self {
            Self::Hour => (time::Duration::hours(1), 60),
            Self::Day => (time::Duration::days(1), 600),
            Self::Week => (time::Duration::days(7), 3600),
            Self::Month => (time::Duration::days(30), 6 * 3600),
            Self::Year => (time::Duration::days(365), 3 * 24 * 3600),
        }
    }
}

pub async fn history(state: &AppState, over: Over) -> Result<Vec<Point>> {
    let (reach, stretch) = over.reach_and_stretch();
    let since = melyxar_core::time::now() - reach;
    Ok(state
        .database()
        .measures_since(since, stretch)
        .await?
        .into_iter()
        .map(as_point)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn a_point(processor: Option<f64>, temperature: Option<f64>) -> Point {
        Point {
            at: datetime!(2026-09-23 14:05:12 UTC),
            processor,
            memory_used: 1000,
            memory_total: 4000,
            load: Some(1.0),
            received: 10.0,
            sent: 2.0,
            card: None,
            temperature,
        }
    }

    #[test]
    fn a_minute_averages_each_figure_over_the_points_that_have_it() {
        let at = datetime!(2026-09-23 14:05 UTC);
        let kept = average(
            &[a_point(Some(0.2), Some(40.0)), a_point(Some(0.4), None), a_point(None, None)],
            at,
        )
        .expect("an average");
        assert_eq!(kept.at, at);
        assert!((kept.processor.expect("a share") - 0.3).abs() < 1e-9);
        assert_eq!(kept.temperature, Some(40.0), "not dragged down by the points without one");
        assert_eq!(kept.card, None);
        assert_eq!(average(&[], at), None);
    }

    #[test]
    fn a_folder_of_a_library_that_went_away_is_named() {
        let here = tempfile::tempdir().expect("folder");
        let roots = vec![
            ("still here".to_string(), here.path().to_path_buf()),
            ("gone".to_string(), here.path().join("unplugged")),
            (
                "invented".to_string(),
                PathBuf::from(melyxar_database::synthetic::BENCH_ROOT),
            ),
        ];
        assert_eq!(folders_still_there(&roots), vec!["gone".to_string()]);
    }

    #[test]
    fn instants_are_cut_to_their_minute_and_their_hour() {
        let at = datetime!(2026-09-23 14:05:12.345 UTC);
        assert_eq!(start_of_the_minute(at), datetime!(2026-09-23 14:05 UTC));
        assert_eq!(start_of_the_hour(at), datetime!(2026-09-23 14:00 UTC));
    }

    #[test]
    fn a_point_turns_counters_into_shares_and_rates() {
        let start = Instant::now();
        let before = Counters {
            taken: start,
            processor: Some(ProcessorTimes { busy: 100, total: 1000 }),
            memory: None,
            load: None,
            traffic: Traffic { received: 0, sent: 0 },
            card: Handles::default(),
            temperature: None,
        };
        let after = Counters {
            taken: start + std::time::Duration::from_secs(2),
            processor: Some(ProcessorTimes { busy: 300, total: 1800 }),
            memory: Some(Memory { total_bytes: 4000, available_bytes: 1000 }),
            load: Some(0.5),
            traffic: Traffic { received: 4000, sent: 200 },
            card: Handles::default(),
            temperature: Some(41.0),
        };
        let at = datetime!(2026-09-23 14:05 UTC);

        let point = point_between(&before, &after, true, at);
        assert_eq!(point.processor, Some(0.25));
        assert_eq!(point.memory_used, 3000);
        assert_eq!((point.received, point.sent), (2000.0, 100.0));
        assert_eq!(point.card, Some(0.0), "a card doing nothing is idle, not unknown");

        let without = point_between(&before, &after, false, at);
        assert_eq!(without.card, None, "a server with no card says nothing about one");
    }
}
