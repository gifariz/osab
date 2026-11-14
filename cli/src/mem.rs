//! Counts the program's memory, for `osab solve --timing` in a build with the `mem-profile` feature
//! (`cargo run --release -p osab-cli --features mem-profile -- solve ...`): the bytes allocated and not
//! yet freed, and their largest value since it was last asked for. The timing report then shows, for
//! each phase, the most memory in use during it and what is still held after it. A normal build does
//! not have this allocator: counting costs a little on every allocation.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static HIGHEST: AtomicUsize = AtomicUsize::new(0);

/// The system allocator with a count of the bytes in use.
pub struct Counting;

fn took(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Relaxed) + bytes;
    PEAK.fetch_max(live, Relaxed);
    HIGHEST.fetch_max(live, Relaxed);
}

// The counts are only added to and read; every allocation is passed on to the system allocator
// unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            took(layout.size());
        }
        p
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            took(layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) };
        LIVE.fetch_sub(layout.size(), Relaxed);
    }

    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, layout, new_size) };
        if !q.is_null() {
            LIVE.fetch_sub(layout.size(), Relaxed);
            took(new_size);
        }
        q
    }
}

/// Bytes in use now, and the most in use since the last call.
pub fn probe() -> (usize, usize) {
    let live = LIVE.load(Relaxed);
    (live, PEAK.swap(live, Relaxed).max(live))
}

/// The most bytes in use at any time.
pub fn highest() -> usize {
    HIGHEST.load(Relaxed)
}
