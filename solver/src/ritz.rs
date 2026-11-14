//! Load-dependent Ritz vectors (Wilson, Yuan and Dickens 1982; Chopra, *Dynamics of Structures*, Ritz
//! vectors): each starting load p_j gives a stream of vectors, the first from K v = p_j, each next one from
//! K v = M q with the stream's last accepted vector q. The streams take turns, one vector each per cycle,
//! in their given order, until `count` vectors are accepted or every stream stops (its cycle limit, or a
//! candidate with nothing new). Each candidate is made stiffness-orthogonal to the vectors before it (two
//! passes of v -= q (qᵀ K v)); one whose stiffness norm is then at most 1e-6 of its uncleaned norm adds
//! nothing and stops its stream. The modes come from the small problem Qᵀ M Q y = μ Qᵀ K Q y (μ = 1/ω²):
//! a mode with μ > 0 has φ = Q y / √μ at unit modal mass, one with μ = 0 (a load on massless
//! coordinates) is a static mode, φ = Q y at unit stiffness (docs/phases/RITZ_PLAN.md).
//!
//! Every accepted vector keeps K q alongside it (K q = its right-hand side, cleaned the same way), so no
//! product with K is needed.

use crate::eigen::{jacobi, SymSparse};
use crate::sparse::Factor;

/// A candidate is kept when its cleaned stiffness norm is above this fraction of its uncleaned one.
const DEPENDENT: f64 = 1e-6;
/// μ at most this fraction of the largest μ counts as zero: a static mode.
const STATIC: f64 = 1e-12;

/// One starting load on the equations, and the most vectors it may give (0 = no limit).
pub struct Start {
    pub load: Vec<f64>,
    pub cycles: usize,
}

/// Finite modes (largest period first): eigenvalue ω² and the shape at unit modal mass; then static
/// modes, shapes at unit stiffness.
pub struct Modes {
    pub finite: Vec<(f64, Vec<f64>)>,
    pub statics: Vec<Vec<f64>>,
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// The Ritz vectors of `starts` (at most `count`) and the modes they span.
pub fn modes(k: &Factor, m: &SymSparse, starts: &[Start], count: usize) -> Modes {
    let mut q: Vec<Vec<f64>> = Vec::new();
    let mut kq: Vec<Vec<f64>> = Vec::new();
    // Per stream: still going, vectors given, its last vector.
    let mut active = vec![true; starts.len()];
    let mut given = vec![0usize; starts.len()];
    let mut last: Vec<Option<usize>> = vec![None; starts.len()];
    'cycles: while active.iter().any(|a| *a) {
        for (j, st) in starts.iter().enumerate() {
            if q.len() >= count {
                break 'cycles;
            }
            if !active[j] {
                continue;
            }
            if st.cycles > 0 && given[j] >= st.cycles {
                active[j] = false;
                continue;
            }
            let rhs = match last[j] {
                None => st.load.clone(),
                Some(i) => m.mul_many(&q[i], 1),
            };
            let mut v = rhs.clone();
            k.solve(&mut v);
            let mut kv = rhs;
            let full = dot(&v, &kv);
            if !(full > 0.0) {
                active[j] = false;
                continue;
            }
            for _ in 0..2 {
                for (qi, kqi) in q.iter().zip(&kq) {
                    let c = dot(kqi, &v);
                    for (a, b) in v.iter_mut().zip(qi) {
                        *a -= c * b;
                    }
                    for (a, b) in kv.iter_mut().zip(kqi) {
                        *a -= c * b;
                    }
                }
            }
            let left = dot(&v, &kv);
            if !(left > DEPENDENT * DEPENDENT * full) {
                active[j] = false;
                continue;
            }
            let s = 1.0 / left.sqrt();
            v.iter_mut().for_each(|a| *a *= s);
            kv.iter_mut().for_each(|a| *a *= s);
            last[j] = Some(q.len());
            given[j] += 1;
            q.push(v);
            kq.push(kv);
        }
    }

    // Qᵀ K Q (the identity up to round-off) and Qᵀ M Q.
    let n = q.len();
    if n == 0 {
        return Modes {
            finite: Vec::new(),
            statics: Vec::new(),
        };
    }
    let mut kr = vec![0.0; n * n];
    let mut mr = vec![0.0; n * n];
    let mq: Vec<Vec<f64>> = q.iter().map(|v| m.mul_many(v, 1)).collect();
    for a in 0..n {
        for b in 0..n {
            kr[a * n + b] = 0.5 * (dot(&q[a], &kq[b]) + dot(&q[b], &kq[a]));
            mr[a * n + b] = 0.5 * (dot(&q[a], &mq[b]) + dot(&q[b], &mq[a]));
        }
    }
    // Kr = L Lᵀ; A = L⁻¹ Mr L⁻ᵀ; its eigenvectors z give y = L⁻ᵀ z, with yᵀ Kr y = 1.
    let mut l = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..=i {
            let s = kr[i * n + j] - (0..j).map(|p| l[i * n + p] * l[j * n + p]).sum::<f64>();
            l[i * n + j] = if i == j {
                s.max(0.0).sqrt()
            } else {
                s / l[j * n + j]
            };
        }
    }
    // B = L⁻¹ Mr (forward substitution per column), then A = L⁻¹ Bᵀ.
    let lower = |b: &[f64]| -> Vec<f64> {
        let mut x = b.to_vec();
        for col in 0..n {
            for i in 0..n {
                let s = x[i * n + col] - (0..i).map(|p| l[i * n + p] * x[p * n + col]).sum::<f64>();
                x[i * n + col] = s / l[i * n + i];
            }
        }
        x
    };
    let b = lower(&mr);
    let bt: Vec<f64> = (0..n * n).map(|t| b[(t % n) * n + t / n]).collect();
    let mut a = lower(&bt);
    for i in 0..n {
        for j in 0..i {
            let s = 0.5 * (a[i * n + j] + a[j * n + i]);
            a[i * n + j] = s;
            a[j * n + i] = s;
        }
    }
    let (mu, z) = jacobi(&a, n);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&x, &y| mu[y].total_cmp(&mu[x]));
    let top = mu.iter().fold(0.0f64, |t, v| t.max(*v));
    let mut out = Modes {
        finite: Vec::new(),
        statics: Vec::new(),
    };
    for &e in &order {
        // y = L⁻ᵀ z (back substitution).
        let mut y: Vec<f64> = (0..n).map(|i| z[i * n + e]).collect();
        for i in (0..n).rev() {
            let s = y[i] - (i + 1..n).map(|p| l[p * n + i] * y[p]).sum::<f64>();
            y[i] = s / l[i * n + i];
        }
        let mut phi = vec![0.0; q[0].len()];
        for (c, v) in y.iter().zip(&q) {
            for (p, x) in phi.iter_mut().zip(v) {
                *p += c * x;
            }
        }
        if top > 0.0 && mu[e] > STATIC * top {
            let s = 1.0 / mu[e].sqrt();
            phi.iter_mut().for_each(|p| *p *= s);
            out.finite.push((1.0 / mu[e], phi));
        } else {
            out.statics.push(phi);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sparse::{Pattern, SymMatrix};
    use std::sync::Arc;

    /// K, M diagonal (four independent oscillators in X, the probe `spring_starts`).
    fn oscillators() -> (Factor, SymSparse, Vec<f64>) {
        let k = [8.0, 75.0, 405.0, 1372.0];
        let m = [2.0, 3.0, 5.0, 7.0];
        let adj: Vec<Vec<usize>> = vec![Vec::new(); 4];
        let pattern = Arc::new(Pattern::new(&adj, &[1; 4]));
        let mut s = SymMatrix::new(pattern);
        for (i, v) in k.iter().enumerate() {
            s.add(i, i, *v);
        }
        let (f, _) = s.factorize().unwrap();
        let mm = SymSparse {
            n: 4,
            entries: m.iter().enumerate().map(|(i, v)| (i, i, *v)).collect(),
        };
        (f, mm, m.to_vec())
    }

    #[test]
    fn one_vector_gives_the_rayleigh_quotient_and_four_the_exact_modes() {
        let (k, m, mass) = oscillators();
        let starts = [Start {
            load: mass.clone(),
            cycles: 0,
        }];
        let one = modes(&k, &m, &starts, 1);
        // λ = Σ m_i/ω_i² / Σ m_i/ω_i⁴ with ω² = 4, 25, 81, 196 (the study's hand quotient).
        let w2 = [4.0, 25.0, 81.0, 196.0];
        let lambda = mass.iter().zip(w2).map(|(m, w)| m / w).sum::<f64>()
            / mass.iter().zip(w2).map(|(m, w)| m / (w * w)).sum::<f64>();
        assert!((one.finite[0].0 - lambda).abs() < 1e-12 * lambda);
        assert!((one.finite[0].0 - 5.487372756498879).abs() < 1e-12);
        let all = modes(&k, &m, &starts, 12);
        let got: Vec<f64> = all.finite.iter().map(|f| f.0).collect();
        assert_eq!(got.len(), 4);
        for (g, w) in got.iter().zip(w2) {
            assert!((g - w).abs() < 1e-9 * w, "{got:?}");
        }
        assert!(all.statics.is_empty());
    }
}
