//! What the machine Melyxar runs on is spending, read from the files the
//! kernel keeps rather than from any tool.
//!
//! Every reading is split in two: a function that reads a file, and a function
//! that makes sense of its text. The second is where every mistake would be,
//! and it is tested on text written out by hand, so the tests never depend on
//! the machine they run on.
//!
//! In a container, `/proc` already speaks for the container and not for the
//! host: the processor share, the memory and the network are the ones given
//! to Melyxar, which is exactly what is wanted. Whatever cannot be read is
//! answered with nothing, never with a zero that would read as a measurement.

#![forbid(unsafe_code)]

pub mod card;

use std::collections::BTreeMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Where the kernel keeps what this module reads. A field rather than a
/// constant, so the tests can point it at a folder of their own.
#[derive(Debug, Clone)]
pub struct Sources {
    pub proc: PathBuf,
    pub sys: PathBuf,
}

impl Default for Sources {
    fn default() -> Self {
        Self {
            proc: PathBuf::from("/proc"),
            sys: PathBuf::from("/sys"),
        }
    }
}

// ---------------------------------------------------------------------------
// The processor
// ---------------------------------------------------------------------------

/// How long every processor of the machine has spent, busy and in all, since
/// it started. A share is the difference between two of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProcessorTimes {
    pub busy: u64,
    pub total: u64,
}

/// Reads the first line of `/proc/stat`, the one that adds every processor
/// up. Waiting on a disk is idle time, not work: counted as busy, a machine
/// scanning a sleeping disk would look flat out while doing nothing.
pub fn processor_times(stat: &str) -> Option<ProcessorTimes> {
    let line = stat.lines().find(|line| line.starts_with("cpu "))?;
    let fields: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .map(|field| field.parse().ok())
        .collect::<Option<_>>()?;
    // user nice system idle iowait irq softirq steal, then the guest times,
    // which are already counted inside user and nice.
    let counted = fields.iter().take(8).sum::<u64>();
    let idle = fields.get(3).copied()? + fields.get(4).copied().unwrap_or(0);
    Some(ProcessorTimes {
        busy: counted.saturating_sub(idle),
        total: counted,
    })
}

/// The share of the processors that was busy between two readings, from
/// nought to one. Nothing when no time passed between them.
pub fn busy_share(before: ProcessorTimes, after: ProcessorTimes) -> Option<f64> {
    let total = after.total.checked_sub(before.total)?;
    if total == 0 {
        return None;
    }
    let busy = after.busy.saturating_sub(before.busy);
    Some((busy as f64 / total as f64).clamp(0.0, 1.0))
}

/// How many processors the machine gives this server, one per `cpuN` line.
pub fn processor_count(stat: &str) -> usize {
    stat.lines()
        .filter(|line| {
            line.strip_prefix("cpu")
                .and_then(|rest| rest.chars().next())
                .is_some_and(|first| first.is_ascii_digit())
        })
        .count()
}

/// The name the processor gives itself, as `/proc/cpuinfo` has it.
pub fn processor_name(cpuinfo: &str) -> Option<String> {
    cpuinfo
        .lines()
        .find(|line| line.starts_with("model name"))
        .and_then(|line| line.split_once(':'))
        .map(|(_, name)| name.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|name| !name.is_empty())
}

// ---------------------------------------------------------------------------
// Memory and load
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Memory {
    pub total_bytes: u64,
    /// What a program could still be given without anything being pushed out:
    /// the kernel's own answer, which counts the room its caches would give
    /// back. Free memory alone would read as a full machine within an hour of
    /// starting, because a kernel keeps every free page busy caching files.
    pub available_bytes: u64,
}

impl Memory {
    pub fn used_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.available_bytes)
    }
}

pub fn memory(meminfo: &str) -> Option<Memory> {
    let kilobytes = |name: &str| -> Option<u64> {
        meminfo
            .lines()
            .find(|line| line.split(':').next() == Some(name))?
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()
    };
    Some(Memory {
        total_bytes: kilobytes("MemTotal")? * 1024,
        available_bytes: kilobytes("MemAvailable")? * 1024,
    })
}

/// How many things were waiting to run, averaged over the last minute.
pub fn load(loadavg: &str) -> Option<f64> {
    loadavg.split_whitespace().next()?.parse().ok()
}

// ---------------------------------------------------------------------------
// The network
// ---------------------------------------------------------------------------

/// Bytes through every interface since it came up, the machine talking to
/// itself left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Traffic {
    pub received: u64,
    pub sent: u64,
}

pub fn traffic(net_dev: &str) -> Traffic {
    net_dev
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.trim() != "lo")
        .filter_map(|(_, counters)| {
            let fields: Vec<u64> = counters
                .split_whitespace()
                .map(|field| field.parse().ok())
                .collect::<Option<_>>()?;
            Some((*fields.first()?, *fields.get(8)?))
        })
        .fold(Traffic::default(), |sum, (received, sent)| Traffic {
            received: sum.received + received,
            sent: sum.sent + sent,
        })
}

/// Bytes a second between two readings, in each direction. A counter that
/// went down is an interface that came back up, and says nothing.
pub fn traffic_rate(before: Traffic, after: Traffic, seconds: f64) -> Option<(f64, f64)> {
    if seconds <= 0.0 {
        return None;
    }
    let received = after.received.checked_sub(before.received)?;
    let sent = after.sent.checked_sub(before.sent)?;
    Some((received as f64 / seconds, sent as f64 / seconds))
}

// ---------------------------------------------------------------------------
// Disks
// ---------------------------------------------------------------------------

/// One disk, and how full it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Disk {
    /// The folders asked about that live on it, in the order they were asked
    /// about: a disk has no name a person would know, the folders on it do.
    pub folders: Vec<PathBuf>,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

/// How full the disks under these folders are, each disk once however many of
/// the folders it holds. A folder that cannot be reached is left out rather
/// than counted as an empty disk.
pub fn disks(folders: &[PathBuf]) -> Vec<Disk> {
    let mut by_device: BTreeMap<u64, Disk> = BTreeMap::new();
    let mut order = Vec::new();
    for folder in folders {
        let Ok(metadata) = std::fs::metadata(folder) else {
            continue;
        };
        let device = metadata.dev();
        if let Some(disk) = by_device.get_mut(&device) {
            disk.folders.push(folder.clone());
            continue;
        }
        let Ok(space) = rustix::fs::statvfs(folder.as_path()) else {
            continue;
        };
        order.push(device);
        by_device.insert(
            device,
            Disk {
                folders: vec![folder.clone()],
                total_bytes: space.f_blocks * space.f_frsize,
                available_bytes: space.f_bavail * space.f_frsize,
            },
        );
    }
    order
        .into_iter()
        .filter_map(|device| by_device.remove(&device))
        .collect()
}

// ---------------------------------------------------------------------------
// Temperature
// ---------------------------------------------------------------------------

/// The drivers that report the processor's own temperature, the most precise
/// first. Anything else under `hwmon` is a disk, a board or a graphics card.
const PROCESSOR_SENSORS: &[&str] = &["k10temp", "zenpower", "coretemp", "cpu_thermal"];

/// The processor's temperature in degrees, when the machine lets it be read.
/// A container often does not, and then this says nothing.
pub fn processor_temperature(sources: &Sources) -> Option<f64> {
    let hwmon = sources.sys.join("class/hwmon");
    let mut sensors: Vec<(usize, PathBuf)> = std::fs::read_dir(&hwmon)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = std::fs::read_to_string(entry.path().join("name")).ok()?;
            let rank = PROCESSOR_SENSORS.iter().position(|known| *known == name.trim())?;
            Some((rank, entry.path()))
        })
        .collect();
    sensors.sort();
    sensors
        .into_iter()
        .find_map(|(_, sensor)| read_trimmed(&sensor.join("temp1_input")))
        .and_then(|text| degrees(&text))
}

/// A temperature as the kernel writes it, in thousandths of a degree.
pub fn degrees(millidegrees: &str) -> Option<f64> {
    millidegrees
        .trim()
        .parse::<i64>()
        .ok()
        .map(|value| value as f64 / 1000.0)
}

pub(crate) fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = "cpu  4705 150 1120 16250 520 0 25 0 0 0\n\
                        cpu0 2350 75 560 8125 260 0 12 0 0 0\n\
                        cpu1 2355 75 560 8125 260 0 13 0 0 0\n\
                        intr 12345\n";

    #[test]
    fn waiting_on_a_disk_is_idle_time_and_not_work() {
        let times = processor_times(STAT).expect("read");
        assert_eq!(times.total, 4705 + 150 + 1120 + 16250 + 520 + 25);
        assert_eq!(times.busy, 4705 + 150 + 1120 + 25);
    }

    #[test]
    fn a_share_is_the_difference_between_two_readings() {
        let before = ProcessorTimes { busy: 100, total: 1000 };
        let after = ProcessorTimes { busy: 150, total: 1200 };
        assert_eq!(busy_share(before, after), Some(0.25));
        assert_eq!(busy_share(after, after), None, "no time passed");
    }

    #[test]
    fn processors_are_counted_and_named() {
        assert_eq!(processor_count(STAT), 2);
        let cpuinfo = "processor\t: 0\nmodel name\t: An   Invented Processor 8 Core\nflags : x\n";
        assert_eq!(
            processor_name(cpuinfo).as_deref(),
            Some("An Invented Processor 8 Core")
        );
        assert_eq!(processor_name("processor : 0\n"), None);
    }

    #[test]
    fn memory_in_use_leaves_out_what_the_caches_would_give_back() {
        let meminfo = "MemTotal:       16384000 kB\nMemFree:          512000 kB\n\
                       MemAvailable:   12288000 kB\nBuffers: 1 kB\n";
        let memory = memory(meminfo).expect("read");
        assert_eq!(memory.total_bytes, 16_384_000 * 1024);
        assert_eq!(memory.used_bytes(), 4_096_000 * 1024);
    }

    #[test]
    fn the_load_is_the_last_minute_of_it() {
        assert_eq!(load("0.76 0.52 0.40 2/310 4410\n"), Some(0.76));
        assert_eq!(load(""), None);
    }

    #[test]
    fn traffic_adds_every_interface_but_the_machine_talking_to_itself() {
        let net_dev = "Inter-|   Receive |  Transmit\n \
                       face |bytes packets errs drop fifo frame compressed multicast|bytes packets\n    \
                       lo: 9000 10 0 0 0 0 0 0 9000 10 0 0 0 0 0 0\n  \
                       eth0: 1000 5 0 0 0 0 0 0 400 3 0 0 0 0 0 0\n  \
                       eth1: 24 1 0 0 0 0 0 0 6 1 0 0 0 0 0 0\n";
        assert_eq!(
            traffic(net_dev),
            Traffic {
                received: 1024,
                sent: 406
            }
        );
    }

    #[test]
    fn a_rate_is_bytes_a_second_and_a_counter_that_went_back_says_nothing() {
        let before = Traffic { received: 1000, sent: 100 };
        let after = Traffic { received: 5000, sent: 300 };
        assert_eq!(traffic_rate(before, after, 2.0), Some((2000.0, 100.0)));
        assert_eq!(traffic_rate(after, before, 2.0), None);
    }

    #[test]
    fn two_folders_on_one_disk_make_one_disk() {
        let here = tempfile::tempdir().expect("folder");
        let one = here.path().join("one");
        let two = here.path().join("two");
        std::fs::create_dir(&one).expect("one");
        std::fs::create_dir(&two).expect("two");

        let found = disks(&[one.clone(), two.clone(), here.path().join("not there")]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].folders, vec![one, two]);
        assert!(found[0].total_bytes >= found[0].available_bytes);
    }

    #[test]
    fn the_processor_sensor_is_found_among_the_others() {
        let sys = tempfile::tempdir().expect("folder");
        for (sensor, name, temperature) in [
            ("hwmon0", "nvme", "38850"),
            ("hwmon1", "k10temp", "42125"),
        ] {
            let folder = sys.path().join("class/hwmon").join(sensor);
            std::fs::create_dir_all(&folder).expect("sensor");
            std::fs::write(folder.join("name"), format!("{name}\n")).expect("name");
            std::fs::write(folder.join("temp1_input"), format!("{temperature}\n")).expect("temp");
        }
        let sources = Sources {
            proc: PathBuf::from("/nowhere"),
            sys: sys.path().to_path_buf(),
        };
        assert_eq!(processor_temperature(&sources), Some(42.125));
    }

    #[test]
    fn a_machine_that_shows_no_sensor_has_no_temperature() {
        let sources = Sources {
            proc: PathBuf::from("/nowhere"),
            sys: PathBuf::from("/nowhere"),
        };
        assert_eq!(processor_temperature(&sources), None);
    }
}
