//! A check before factorizing that the analysis will fit in memory (docs/phases/MEMORY_DESIGN.md,
//! Part C). The factor's size is known exactly once the equations are ordered; the rest is estimated
//! from the model's sizes. When the total is more than the free memory less a reserve, the analysis
//! stops with a message instead of filling the memory until the computer stops answering.
//!
//! The free memory is read from the system, or given ([`set_available`]: the command line's
//! `--memory`, and tests). [`set_force`] turns the stop off (`--force`).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

use crate::Error;

const GB: f64 = 1024.0 * 1024.0 * 1024.0;
const NOT_GIVEN: u64 = u64::MAX;

static GIVEN: AtomicU64 = AtomicU64::new(NOT_GIVEN);
static FORCE: AtomicBool = AtomicBool::new(false);

/// Gives the free memory in bytes instead of asking the system (`None`: ask the system again).
pub fn set_available(bytes: Option<u64>) {
    GIVEN.store(bytes.unwrap_or(NOT_GIVEN), Relaxed);
}

/// With `true` the analysis goes on even when the estimate says it will not fit.
pub fn set_force(on: bool) {
    FORCE.store(on, Relaxed);
}

/// The free memory in bytes: as given, else the system's free physical memory (not known on every
/// system: then nothing is checked).
pub fn available() -> Option<u64> {
    match GIVEN.load(Relaxed) {
        NOT_GIVEN => system_free(),
        bytes => Some(bytes),
    }
}

/// What may be used of the free memory: all but a tenth of it, and at least 1 GB left.
pub fn usable(free: u64) -> u64 {
    free.saturating_sub((free / 10).max(1 << 30))
}

#[cfg(windows)]
fn system_free() -> Option<u64> {
    #[repr(C)]
    struct MemoryStatus {
        length: u32,
        memory_load: u32,
        total_physical: u64,
        available_physical: u64,
        total_page_file: u64,
        available_page_file: u64,
        total_virtual: u64,
        available_virtual: u64,
        available_extended_virtual: u64,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
    }
    let mut status = MemoryStatus {
        length: std::mem::size_of::<MemoryStatus>() as u32,
        memory_load: 0,
        total_physical: 0,
        available_physical: 0,
        total_page_file: 0,
        available_page_file: 0,
        total_virtual: 0,
        available_virtual: 0,
        available_extended_virtual: 0,
    };
    // The system fills the structure whose length is set; it reads nothing else.
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    (ok != 0).then_some(status.available_physical)
}

#[cfg(target_os = "linux")]
fn system_free() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = text.lines().find(|l| l.starts_with("MemAvailable:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn system_free() -> Option<u64> {
    None
}

/// The model's sizes the estimate is made from.
pub struct Sizes {
    pub equations: usize,
    pub joints: usize,
    pub frames: usize,
    pub shells: usize,
    /// Shell corners in all.
    pub corners: usize,
    /// Stations along a frame.
    pub stations: usize,
    /// Entries of the factor, and the bytes its fronts and update matrices take at most.
    pub factor_entries: usize,
    pub work: usize,
    /// Cases solved together.
    pub batch: usize,
    /// Modes asked for (0: no modal case).
    pub modes: usize,
    /// Levels of running maximum and minimum of the combinations with two values, in all.
    pub running_levels: usize,
}

/// The memory an analysis still needs when its equations are ordered, by part, in bytes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Need {
    pub factor: u64,
    pub work: u64,
    pub cases: u64,
    pub modal: u64,
    pub combinations: u64,
}

impl Need {
    pub fn of(s: &Sizes) -> Need {
        // Values in one case's results: displacements, frame stations, shell corner forces.
        let values = 6 * s.joints + 9 * s.stations * s.frames + 8 * s.corners;
        // A case's loads and right-hand side; one case's results as structures (about ten times
        // their numbers).
        let loaded = 8 * s.equations + 48 * s.joints + 96 * s.frames + 216 * s.shells;
        let vectors = if s.modes == 0 {
            0
        } else {
            (2 * s.modes).min(s.modes + 8)
        };
        Need {
            factor: 8 * s.factor_entries as u64,
            work: s.work as u64,
            cases: (s.batch * loaded + 80 * values) as u64,
            // The iteration's blocks of vectors, and the shapes kept.
            modal: (8 * s.equations * 7 * vectors + 80 * s.modes * s.joints) as u64,
            combinations: (16 * values * s.running_levels) as u64,
        }
    }

    pub fn total(&self) -> u64 {
        self.factor + self.work + self.cases + self.modal + self.combinations
    }

    /// One line for the timing report.
    pub fn describe(&self, free: Option<u64>) -> String {
        let gb = |b: u64| b as f64 / GB;
        format!(
            "memory still needed, GB: factor {:.2}, factorizing {:.2}, cases {:.2}, modal {:.2}, \
             combinations {:.2}; {:.2} in all, {} free",
            gb(self.factor),
            gb(self.work),
            gb(self.cases),
            gb(self.modal),
            gb(self.combinations),
            gb(self.total()),
            free.map_or("unknown".to_string(), |f| format!("{:.2}", gb(f)))
        )
    }

    /// Stops when the need is more than may be used of the free memory (unless forced, or the free
    /// memory is not known).
    pub fn check(&self, equations: usize) -> Result<(), Error> {
        let Some(free) = available() else {
            return Ok(());
        };
        if FORCE.load(Relaxed) || self.total() <= usable(free) {
            return Ok(());
        }
        Err(Error::TooBigForMemory {
            needed: self.total() as f64 / GB,
            free: free as f64 / GB,
            factor: self.factor as f64 / GB,
            combinations: self.combinations as f64 / GB,
            equations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserve_is_a_tenth_and_at_least_one_gb() {
        assert_eq!(usable(32 << 30), (32u64 << 30) - (32u64 << 30) / 10);
        assert_eq!(usable(4 << 30), 3 << 30);
        assert_eq!(usable(1 << 29), 0);
    }

    #[test]
    fn need_adds_its_parts() {
        let s = Sizes {
            equations: 600_000,
            joints: 100_000,
            frames: 10_000,
            shells: 100_000,
            corners: 400_000,
            stations: 11,
            factor_entries: 600_000_000,
            work: 2 << 30,
            batch: 32,
            modes: 12,
            running_levels: 150,
        };
        let n = Need::of(&s);
        assert_eq!(n.factor, 4_800_000_000);
        assert_eq!(
            n.total(),
            n.factor + n.work + n.cases + n.modal + n.combinations
        );
        // 150 running levels of a result of 4.79 million values: 11.5 GB.
        assert_eq!(n.combinations, 16 * 4_790_000 * 150);
        assert!(n.describe(Some(17 << 30)).contains("17.00 free"));
    }
}
