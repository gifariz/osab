//! Wall-clock times of the analysis phases, for `osab solve --timing`. Nothing is recorded unless
//! [`record`] was called on this thread; the browser build has no clock, so there a lap records nothing.
//!
//! With a memory probe set ([`MEMORY`]; the command line program does so in a build that counts its
//! memory), each lap also records the most memory in use since the lap before and what is in use at
//! its end, and [`take`] adds those as notes: where a run's memory goes, phase by phase.

use std::cell::RefCell;
use std::sync::OnceLock;

/// Gives the bytes in use now and the most in use since it was last called. Unset: no memory is
/// recorded.
pub static MEMORY: OnceLock<fn() -> (usize, usize)> = OnceLock::new();

/// One recorded line: a phase with its total time and how often it ran, or a plain note (a size, a count).
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Phase {
        name: String,
        seconds: f64,
        count: usize,
    },
    Note(String),
}

thread_local! {
    static LOG: RefCell<Option<Vec<Line>>> = const { RefCell::new(None) };
    /// Per phase with a lap: its name, the most bytes in use during any of its laps, the bytes in use
    /// at the end of its last lap.
    static MEMORY_LOG: RefCell<Vec<(String, usize, usize)>> = const { RefCell::new(Vec::new()) };
}

/// Starts recording on this thread (and forgets earlier lines).
pub fn record() {
    LOG.with(|l| *l.borrow_mut() = Some(Vec::new()));
    MEMORY_LOG.with(|m| m.borrow_mut().clear());
    if let Some(probe) = MEMORY.get() {
        probe();
    }
}

/// The lines recorded since [`record`]; recording stops. The memory of the phases, when it was
/// recorded, follows as notes.
pub fn take() -> Vec<Line> {
    let mut lines = LOG.with(|l| l.borrow_mut().take().unwrap_or_default());
    let memory = MEMORY_LOG.with(|m| std::mem::take(&mut *m.borrow_mut()));
    if !memory.is_empty() {
        const GB: f64 = 1024.0 * 1024.0 * 1024.0;
        lines.push(Line::Note(format!(
            "memory in use, GB: {:<28} {:>8} {:>8}",
            "phase", "most", "after"
        )));
        for (name, peak, live) in memory {
            lines.push(Line::Note(format!(
                "  {name:<44} {:>8.2} {:>8.2}",
                peak as f64 / GB,
                live as f64 / GB
            )));
        }
    }
    lines
}

/// Records the memory of the lap that just ended.
#[cfg(not(target_arch = "wasm32"))]
fn memory_lap(name: &str) {
    let Some(probe) = MEMORY.get() else {
        return;
    };
    if !LOG.with(|l| l.borrow().is_some()) {
        return;
    }
    let (live, peak) = probe();
    MEMORY_LOG.with(|m| {
        let mut m = m.borrow_mut();
        match m.iter_mut().find(|(n, _, _)| n == name) {
            Some((_, most, after)) => {
                *most = (*most).max(peak);
                *after = live;
            }
            None => m.push((name.to_string(), peak, live)),
        }
    });
}

/// Adds a note while recording (the text is only made then).
pub fn note(text: impl FnOnce() -> String) {
    LOG.with(|l| {
        if let Some(lines) = l.borrow_mut().as_mut() {
            lines.push(Line::Note(text()));
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn add(name: &str, seconds: f64) {
    LOG.with(|l| {
        if let Some(lines) = l.borrow_mut().as_mut() {
            for line in lines.iter_mut() {
                if let Line::Phase {
                    name: n,
                    seconds: s,
                    count,
                } = line
                {
                    if n == name {
                        *s += seconds;
                        *count += 1;
                        return;
                    }
                }
            }
            lines.push(Line::Phase {
                name: name.to_string(),
                seconds,
                count: 1,
            });
        }
    });
}

/// A stopwatch: each [`Clock::lap`] adds the time since the previous lap (or since `new`) to the phase
/// of that name, so a phase that runs once per load case adds up over the cases.
pub struct Clock {
    #[cfg(not(target_arch = "wasm32"))]
    last: std::time::Instant,
}

/// Records a nested phase until this guard is dropped. Disabled recording does not read the clock.
/// Nested phases overlap their parents and must not be added to them when computing a total.
pub struct Span {
    #[cfg(not(target_arch = "wasm32"))]
    start: Option<std::time::Instant>,
    #[cfg(not(target_arch = "wasm32"))]
    name: &'static str,
}

impl Span {
    pub fn new(name: &'static str) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self {
                start: LOG.with(|l| l.borrow().is_some().then(std::time::Instant::now)),
                name,
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = name;
            Self {}
        }
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(start) = self.start {
            add(self.name, start.elapsed().as_secs_f64());
        }
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock {
    pub fn new() -> Clock {
        Clock {
            #[cfg(not(target_arch = "wasm32"))]
            last: std::time::Instant::now(),
        }
    }

    pub fn lap(&mut self, name: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let now = std::time::Instant::now();
            add(name, (now - self.last).as_secs_f64());
            memory_lap(name);
            self.last = now;
        }
        #[cfg(target_arch = "wasm32")]
        let _ = name;
    }
}

/// The lines as a table: phase, seconds, and how often it ran when more than once.
pub fn report(lines: &[Line]) -> String {
    let mut out = String::new();
    for line in lines {
        match line {
            Line::Phase {
                name,
                seconds,
                count,
            } => {
                let times = if *count > 1 {
                    format!("  ({count} times)")
                } else {
                    String::new()
                };
                out.push_str(&format!("{name:<44} {seconds:>10.3} s{times}\n"));
            }
            Line::Note(text) => out.push_str(&format!("  {text}\n")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laps_add_up_by_name_and_only_while_recording() {
        let mut c = Clock::new();
        c.lap("ignored");
        note(|| "ignored".into());
        record();
        c.lap("a");
        c.lap("b");
        c.lap("a");
        note(|| "n = 3".into());
        let lines = take();
        assert_eq!(lines.len(), 3);
        assert!(matches!(&lines[0], Line::Phase { name, count: 2, .. } if name == "a"));
        assert!(matches!(&lines[1], Line::Phase { name, count: 1, .. } if name == "b"));
        assert_eq!(lines[2], Line::Note("n = 3".into()));
        assert!(take().is_empty());
    }
}
