//! Threads for the heavy parts of the analysis, as an option (`osab solve --threads N`): one thread
//! unless set, and always one in the browser build. The results do not depend on the number of threads:
//! the work is split so that every sum still runs in the same order.

use std::sync::atomic::{AtomicUsize, Ordering};

static THREADS: AtomicUsize = AtomicUsize::new(1);

/// Sets the number of threads for the analyses that follow (0: one per core).
pub fn set_threads(n: usize) {
    let n = if n == 0 {
        std::thread::available_parallelism().map_or(1, |n| n.get())
    } else {
        n
    };
    THREADS.store(n, Ordering::Relaxed);
}

/// The number of threads to use (1 in the browser build).
pub fn threads() -> usize {
    if cfg!(target_arch = "wasm32") {
        1
    } else {
        THREADS.load(Ordering::Relaxed).max(1)
    }
}

/// Rows a thread should have at least: starting one costs about as much as working on a few thousand
/// rows of a block of vectors.
pub const MIN_ROWS: usize = 2048;

/// Runs `work(first, piece)` on `data` cut into pieces of whole rows of `width` values, on up to
/// [`threads`] threads (`first`: the piece's first row). Nothing to do when `width` is 0.
pub fn rows<T: Send>(data: &mut [T], width: usize, work: impl Fn(usize, &mut [T]) + Sync) {
    if width == 0 || data.is_empty() {
        return;
    }
    let n = data.len() / width;
    let t = threads().min(n / MIN_ROWS).max(1);
    if t == 1 {
        return work(0, data);
    }
    let per = n.div_ceil(t);
    std::thread::scope(|s| {
        for (k, piece) in data.chunks_mut(per * width).enumerate() {
            let work = &work;
            s.spawn(move || work(k * per, piece));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_cover_every_row_once() {
        // On one thread here (the setting is global); the pieces are checked through `first`.
        let (n, w) = (3 * MIN_ROWS + 5, 3);
        let mut data = vec![0usize; n * w];
        rows(&mut data, w, |first, piece| {
            for (i, row) in piece.chunks_exact_mut(w).enumerate() {
                row.iter_mut().for_each(|v| *v += first + i + 1);
            }
        });
        assert!(data
            .chunks_exact(w)
            .enumerate()
            .all(|(i, r)| r.iter().all(|&v| v == i + 1)));
    }
}
