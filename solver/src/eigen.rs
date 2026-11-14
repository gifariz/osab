//! The lowest eigenpairs of K φ = λ M φ (K positive definite, M positive semi-definite) by subspace
//! iteration (Bathe, *Finite Element Procedures*, section 11.6): q vectors are improved with K⁻¹ M on the
//! sparse LDLᵀ factor of K (all q in one pass) and re-orthogonalized by a q x q eigenproblem until the
//! lowest p eigenvalues settle; then a Sturm sequence check (the number of negative pivots of K - σM, just
//! above the p-th eigenvalue) makes sure none was missed. Single-threaded, so it runs in the browser too.

use crate::parallel;
use crate::sparse::Factor;

/// A symmetric sparse matrix as (row, column, value) entries, both triangles listed.
#[derive(Debug, Clone, Default)]
pub struct SymSparse {
    pub n: usize,
    pub entries: Vec<(usize, usize, f64)>,
}

impl SymSparse {
    /// M X for `q` vectors stored equation by equation (`x[i * q + a]`); with threads, by pieces of rows
    /// when the entries come row by row (each row adds its entries in the same order).
    pub fn mul_many(&self, x: &[f64], q: usize) -> Vec<f64> {
        let mut y = vec![0.0; self.n * q];
        let by_rows = self.entries.is_sorted_by_key(|e| e.0);
        let work = |first: usize, piece: &mut [f64]| {
            let entries = if by_rows {
                let last = first + piece.len() / q;
                let lo = self.entries.partition_point(|e| e.0 < first);
                let hi = self.entries.partition_point(|e| e.0 < last);
                &self.entries[lo..hi]
            } else {
                &self.entries[..]
            };
            for &(i, j, v) in entries {
                let yi = &mut piece[(i - first) * q..(i - first + 1) * q];
                for (y, xj) in yi.iter_mut().zip(&x[j * q..(j + 1) * q]) {
                    *y += v * xj;
                }
            }
        };
        if by_rows {
            parallel::rows(&mut y, q, work);
        } else if q > 0 {
            work(0, &mut y);
        }
        y
    }

    pub fn diagonal(&self) -> Vec<f64> {
        let mut d = vec![0.0; self.n];
        for &(i, j, v) in &self.entries {
            if i == j {
                d[i] += v;
            }
        }
        d
    }
}

/// Eigenvalues (ascending) and M-orthonormal eigenvectors (φᵀ M φ = 1).
#[derive(Debug, Clone)]
pub struct Eigen {
    pub values: Vec<f64>,
    pub vectors: Vec<Vec<f64>>,
    /// Whether the count of eigenvalues below the last one confirmed that none is missing (false: the
    /// count could not be taken, every shift tried lay on an eigenvalue).
    pub checked: bool,
    /// Set when the eigenvalues stopped getting more exact before the tolerance was reached (a
    /// stiffness that has lost many digits): the relative change they still had between iterations.
    pub accuracy: Option<f64>,
}

/// Relative change of the p eigenvalues between iterations at which they count as converged.
const TOL: f64 = 1e-12;
const MAX_ITERATIONS: usize = 500;
/// Shifts tried for the count of eigenvalues below the last one (each four times further above it).
const SHIFTS: i32 = 5;
/// The iterations have stalled when the last `STALL` brought no change of the eigenvalues below half
/// the lowest before them: round-off in the solves, not the iteration, now sets the change. (A run
/// that reaches the tolerance within `MAX_ITERATIONS` shrinks its change by more than that in any
/// `STALL` iterations, so it is never taken for stalled.)
const STALL: usize = 30;
/// Stalled modes are still given (with a warning) when their eigenvalues change by no more than this
/// between iterations; above it the eigen solution has failed.
const USABLE: f64 = 1e-4;

/// Whether the iterations have stalled, given the change after each so far; then the largest change
/// of the last `STALL`.
fn stalled(changes: &[f64]) -> Option<f64> {
    let n = changes.len();
    if n <= STALL {
        return None;
    }
    let lowest = |v: &[f64]| v.iter().cloned().fold(f64::INFINITY, f64::min);
    let (before, last) = changes.split_at(n - STALL);
    (lowest(last) >= 0.5 * lowest(before)).then(|| last.iter().cloned().fold(0.0, f64::max))
}

/// The lowest `p` eigenpairs (fewer when M has fewer than `p` independent mass DOFs). `k` is K already
/// factorized, `k_diagonal` its diagonal before that; `sturm(σ)` returns the number of eigenvalues
/// below σ (negative pivots of K - σM).
pub fn subspace(
    k: &Factor,
    k_diagonal: &[f64],
    m: &SymSparse,
    p: usize,
    sturm: &mut dyn FnMut(f64) -> Result<usize, String>,
) -> Result<Eigen, String> {
    let mut clock = crate::timing::Clock::new();
    let n = m.n;
    let md = m.diagonal();
    let r = rank(m);
    clock.lap("subspace: rank of M");
    if r == 0 {
        return Err("the model has no mass in the DOFs that can move".into());
    }
    let p = p.min(r);
    let mut q = (2 * p).min(p + 8).max(p).min(r);
    // Every mode asked for: one vector per DOF with mass; K⁻¹ M of them spans every mode, so one pass
    // gives the exact pairs (iterating would lose a very stiff mode next to very soft ones).
    let all = q == r;
    let massive: Vec<usize> = (0..n).filter(|&i| md[i] > 0.0).collect();
    if all {
        q = massive.len();
    }
    loop {
        // The q vectors X, equation by equation (`x[i * q + a]`, the layout of the block solve).
        let mut x = vec![0.0; n * q];
        let start = if all {
            massive
                .iter()
                .map(|&i| {
                    let mut v = vec![0.0; n];
                    v[i] = 1.0;
                    v
                })
                .collect()
        } else {
            start_vectors(&md, k_diagonal, q)
        };
        for (a, v) in start.iter().enumerate() {
            for (i, vi) in v.iter().enumerate() {
                x[i * q + a] = *vi;
            }
        }
        clock.lap("subspace: start vectors");
        let mut prev: Vec<f64> = Vec::new();
        let mut values = Vec::new();
        let mut converged = false;
        let mut accuracy = None;
        let mut iterations = 0;
        // The largest relative change of the eigenvalues asked for, iteration by iteration.
        let mut changes: Vec<f64> = Vec::new();
        for _ in 0..MAX_ITERATIONS {
            iterations += 1;
            // X̄ = K⁻¹ M X (all q in one pass); reduced K_r = X̄ᵀ K X̄ = X̄ᵀ (M X), M_r = X̄ᵀ M X̄.
            let y = m.mul_many(&x, q);
            let mut xb = y.clone();
            clock.lap("subspace: M X");
            k.solve_many(&mut xb, q);
            clock.lap("subspace: block solves");
            let mxb = m.mul_many(&xb, q);
            // Each X̄ column scaled to unit M-norm (they differ by orders of magnitude between soft
            // and stiff directions); columns without mass are dropped.
            let mut norms = vec![-0.0; q];
            for (xi, mi) in xb.chunks_exact(q).zip(mxb.chunks_exact(q)) {
                for ((s, x), mx) in norms.iter_mut().zip(xi).zip(mi) {
                    *s += x * mx;
                }
            }
            let keep: Vec<(usize, f64)> = norms
                .iter()
                .enumerate()
                .filter(|(_, norm)| **norm > 0.0 && norm.is_finite())
                .map(|(a, norm)| (a, 1.0 / norm.sqrt()))
                .collect();
            let qn = keep.len();
            let scaled = |v: &[f64]| -> Vec<f64> {
                let mut out = vec![0.0; n * qn];
                parallel::rows(&mut out, qn, |first, piece| {
                    for (row, o) in v[first * q..]
                        .chunks_exact(q)
                        .zip(piece.chunks_exact_mut(qn))
                    {
                        for (ov, &(a, s)) in o.iter_mut().zip(&keep) {
                            *ov = row[a] * s;
                        }
                    }
                });
                out
            };
            let (xs, ys, ms) = (scaled(&xb), scaled(&y), scaled(&mxb));
            clock.lap("subspace: M X̄, norms, scaling");
            let (kr, mr) = gram(&xs, &ys, &ms, qn);
            clock.lap("subspace: K_r and M_r");
            let (lam, vecs) = reduced(&kr, &mr, qn);
            // The new X = X̄ W; directions lost to round-off (vectors made nearly parallel by the low
            // modes) come back as fresh vectors.
            let e = vecs.len();
            let mut w = vec![0.0; qn * e];
            for (c, v) in vecs.iter().enumerate() {
                for (j, vj) in v.iter().enumerate() {
                    w[j * e + c] = *vj;
                }
            }
            if e > 0 {
                parallel::rows(&mut x, q, |first, piece| {
                    for (xi, row) in xs[first * qn..]
                        .chunks_exact(qn)
                        .zip(piece.chunks_exact_mut(q))
                    {
                        let out = &mut row[..e];
                        out.fill(0.0);
                        for (j, xij) in xi.iter().enumerate() {
                            for (o, wjc) in out.iter_mut().zip(&w[j * e..(j + 1) * e]) {
                                *o += wjc * xij;
                            }
                        }
                    }
                });
            }
            for a in e..q {
                let v = random_vector(&md, a as u64 + 7 * q as u64);
                for (i, vi) in v.iter().enumerate() {
                    x[i * q + a] = *vi;
                }
            }
            values = lam;
            clock.lap("subspace: new vectors");
            if values.len() >= p && prev.len() >= p {
                changes.push(
                    (0..p)
                        .map(|i| (values[i] - prev[i]).abs() / values[i].abs())
                        .fold(0.0, f64::max),
                );
            }
            if values.len() >= p
                && (all
                    || prev.len() >= p
                        && (0..p).all(|i| (values[i] - prev[i]).abs() <= TOL * values[i].abs()))
            {
                converged = true;
                break;
            }
            // No longer getting more exact: good enough to give with a warning, or failed.
            if let Some(level) = stalled(&changes) {
                accuracy = Some(level);
                converged = level <= USABLE;
                break;
            }
            prev = values.clone();
        }
        crate::timing::note(|| format!("subspace: {q} vectors, {iterations} iterations"));
        let lowest = changes.iter().cloned().fold(f64::INFINITY, f64::min);
        // (None in one pass, when every mode is asked for.)
        crate::timing::note(|| {
            let show = |v: &[f64]| {
                v.iter()
                    .map(|c| format!("{c:.1e}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let n = changes.len();
            if n == 0 {
                return "subspace: every mode in one pass".to_string();
            }
            format!(
                "subspace: change of the eigenvalues per iteration: first {} ... last {}; lowest {lowest:.1e}",
                show(&changes[..n.min(6)]),
                show(&changes[n.saturating_sub(6)..])
            )
        });
        if !converged {
            return Err(format!(
                "the eigen solution did not converge in {iterations} iterations (the eigenvalues \
                 still change by {:.1e} from one iteration to the next; the least they changed was \
                 {lowest:.1e}, and {TOL:.0e} is asked)",
                changes.last().copied().unwrap_or(f64::NAN)
            ));
        }
        // Sturm check just above the p-th eigenvalue: nothing below it may be missing. A shift that
        // lies on an eigenvalue (the count fails) is moved up a little, a few times; when none works
        // the modes are given without the check.
        let mut count = None;
        for step in 0..SHIFTS {
            let sigma = values[p - 1] * (1.0 + 1e-6 * 4f64.powi(step));
            if let Ok(below) = sturm(sigma) {
                count = Some((below, sigma));
                break;
            }
        }
        let (below, sigma) = count.unwrap_or((0, 0.0));
        let found = values.iter().filter(|v| **v < sigma).count();
        if below <= found {
            values.truncate(p);
            let vectors = (0..p)
                .map(|a| (0..n).map(|i| x[i * q + a]).collect())
                .collect();
            return Ok(Eigen {
                values,
                vectors,
                checked: count.is_some(),
                accuracy,
            });
        }
        let more = (q + p).min(r);
        if all || more == q {
            return Err(format!(
                "the Sturm check finds {below} eigenvalues below {sigma:.6e}, the solution {found}"
            ));
        }
        q = more;
    }
}

/// K_r = Xᵀ Y and M_r = Xᵀ Z for n x `qn` blocks stored by rows, each entry summed over the rows in order
/// (on one thread: reading the rows is the cost, more threads only read them more often).
fn gram(xs: &[f64], ys: &[f64], zs: &[f64], qn: usize) -> (Vec<f64>, Vec<f64>) {
    let mut kr = vec![0.0; qn * qn];
    let mut mr = vec![0.0; qn * qn];
    for ((xi, yi), zi) in xs
        .chunks_exact(qn.max(1))
        .zip(ys.chunks_exact(qn.max(1)))
        .zip(zs.chunks_exact(qn.max(1)))
    {
        for (a, xa) in xi.iter().enumerate() {
            let row = a * qn + a..(a + 1) * qn;
            for (((kv, mv), y), z) in kr[row.clone()]
                .iter_mut()
                .zip(&mut mr[row])
                .zip(&yi[a..])
                .zip(&zi[a..])
            {
                *kv += xa * y;
                *mv += xa * z;
            }
        }
    }
    for a in 0..qn {
        for b in a + 1..qn {
            kr[b * qn + a] = kr[a * qn + b];
            mr[b * qn + a] = mr[a * qn + b];
        }
    }
    (kr, mr)
}

/// Bathe's starting vectors: the mass diagonal, unit vectors at the DOFs with the largest m_ii / k_ii,
/// and one pseudo-random vector over the DOFs with mass.
fn start_vectors(md: &[f64], kd: &[f64], q: usize) -> Vec<Vec<f64>> {
    let n = md.len();
    let mut x = vec![md.to_vec()];
    let mut order: Vec<usize> = (0..n).filter(|&i| md[i] > 0.0).collect();
    order.sort_by(|&a, &b| (md[b] / kd[b]).total_cmp(&(md[a] / kd[a])));
    let units = if q >= 3 { q - 2 } else { q - 1 };
    for &i in order.iter().take(units) {
        let mut v = vec![0.0; n];
        v[i] = 1.0;
        x.push(v);
    }
    if x.len() < q {
        x.push(random_vector(md, 0));
    }
    x
}

/// A pseudo-random vector over the DOFs with mass (`seed` picks one of a sequence).
fn random_vector(md: &[f64], seed: u64) -> Vec<f64> {
    let mut s: u64 = 0x2545_f491_4f6c_dd1d ^ seed.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    md.iter()
        .map(|&m| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            if m > 0.0 {
                (s % 10_000) as f64 / 10_000.0 + 0.5
            } else {
                0.0
            }
        })
        .collect()
}

/// The rank of M: the sum over its independent blocks (a lumped mass couples only the DOFs of one
/// joint and of the joints tied to it), each by its eigenvalues.
fn rank(m: &SymSparse) -> usize {
    let mut root: Vec<usize> = (0..m.n).collect();
    fn find(root: &mut [usize], mut x: usize) -> usize {
        while root[x] != x {
            root[x] = root[root[x]];
            x = root[x];
        }
        x
    }
    for &(i, j, _) in &m.entries {
        let (a, b) = (find(&mut root, i), find(&mut root, j));
        root[a] = b;
    }
    // Each block's DOFs (in the order they first appear), every DOF's place in its block, and the
    // blocks as dense matrices.
    let mut block_of: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut dofs: Vec<Vec<usize>> = Vec::new();
    let mut place = vec![usize::MAX; m.n];
    let mut block = Vec::with_capacity(m.entries.len());
    for &(i, _, _) in &m.entries {
        let r = find(&mut root, i);
        let b = *block_of.entry(r).or_insert_with(|| {
            dofs.push(Vec::new());
            dofs.len() - 1
        });
        if place[i] == usize::MAX {
            place[i] = dofs[b].len();
            dofs[b].push(i);
        }
        block.push(b);
    }
    let mut mats: Vec<Vec<f64>> = dofs.iter().map(|d| vec![0.0; d.len() * d.len()]).collect();
    for (&(i, j, v), &b) in m.entries.iter().zip(&block) {
        let k = dofs[b].len();
        mats[b][place[i] * k + place[j]] += v;
    }
    let mut total = 0;
    for (a, d) in mats.iter().zip(&dofs) {
        let (mu, _) = jacobi(a, d.len());
        let top = mu.iter().fold(0.0f64, |x, y| x.max(*y));
        total += mu.iter().filter(|&&v| v > 1e-12 * top).count();
    }
    total
}

/// The q x q problem K_r w = λ M_r w (row-major), on the part of the space with mass: eigenvalues
/// ascending and their w, M_r-orthonormal. Directions without mass (eigenvalues of M_r below 1e-12 of
/// the largest) are dropped.
fn reduced(kr: &[f64], mr: &[f64], q: usize) -> (Vec<f64>, Vec<Vec<f64>>) {
    let (mu, v) = jacobi(mr, q);
    let top = mu.iter().fold(0.0f64, |a, b| a.max(*b));
    let keep: Vec<usize> = (0..q).filter(|&j| mu[j] > 1e-12 * top).collect();
    let e = keep.len();
    // T = V_keep diag(1/√μ), so Tᵀ M_r T = I.
    let t: Vec<Vec<f64>> = keep
        .iter()
        .map(|&j| (0..q).map(|i| v[i * q + j] / mu[j].sqrt()).collect())
        .collect();
    let mut c = vec![0.0; e * e];
    for a in 0..e {
        for b in 0..e {
            let mut s = 0.0;
            for i in 0..q {
                for j in 0..q {
                    s += t[a][i] * kr[i * q + j] * t[b][j];
                }
            }
            c[a * e + b] = s;
        }
    }
    let (lam, w) = jacobi(&c, e);
    let mut order: Vec<usize> = (0..e).collect();
    order.sort_by(|&a, &b| lam[a].total_cmp(&lam[b]));
    let values = order.iter().map(|&k| lam[k]).collect();
    let vectors = order
        .iter()
        .map(|&k| {
            (0..q)
                .map(|i| (0..e).map(|a| t[a][i] * w[a * e + k]).sum())
                .collect()
        })
        .collect();
    (values, vectors)
}

/// Eigenvalues and eigenvectors (columns of the row-major result) of a symmetric matrix, by cyclic
/// Jacobi rotations.
pub fn jacobi(a: &[f64], n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut a = a.to_vec();
    let mut v = vec![0.0; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }
    let norm: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    for _ in 0..100 {
        let off: f64 = (0..n)
            .flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j)))
            .map(|(i, j)| a[i * n + j] * a[i * n + j])
            .sum::<f64>()
            .sqrt();
        if off <= 1e-15 * norm || norm == 0.0 {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = a[p * n + q];
                if apq.abs() <= 1e-300 {
                    continue;
                }
                let theta = (a[q * n + q] - a[p * n + p]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let (akp, akq) = (a[k * n + p], a[k * n + q]);
                    a[k * n + p] = c * akp - s * akq;
                    a[k * n + q] = s * akp + c * akq;
                }
                for k in 0..n {
                    let (apk, aqk) = (a[p * n + k], a[q * n + k]);
                    a[p * n + k] = c * apk - s * aqk;
                    a[q * n + k] = s * apk + c * aqk;
                }
                for k in 0..n {
                    let (vkp, vkq) = (v[k * n + p], v[k * n + q]);
                    v[k * n + p] = c * vkp - s * vkq;
                    v[k * n + q] = s * vkp + c * vkq;
                }
            }
        }
    }
    ((0..n).map(|i| a[i * n + i]).collect(), v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sparse::{Pattern, SymMatrix};
    use std::sync::Arc;

    #[test]
    fn iterations_stall_when_the_change_stops_falling() {
        // Falling by a tenth each iteration: never stalled, however long.
        let falling: Vec<f64> = (0..100).map(|i| 0.9f64.powi(i)).collect();
        assert!((1..=falling.len()).all(|n| stalled(&falling[..n]).is_none()));
        // The slowest run that still reaches 1e-12 in 500 iterations is not stalled either.
        let slow: Vec<f64> = (0..500).map(|i| 0.946f64.powi(i)).collect();
        assert!((1..=slow.len()).all(|n| stalled(&slow[..n]).is_none()));
        // Six good iterations, then round-off holds the change near 2e-6 (a real model's history).
        let mut noisy = vec![3.4e1, 3.5e-1, 2.0e-3, 1.4e-4, 1.3e-5, 1.6e-6];
        noisy.extend((0..40).map(|i| [1.3e-6, 3.0e-6, 2.1e-6, 9.0e-7][i % 4]));
        let at = (1..=noisy.len()).find(|&n| stalled(&noisy[..n]).is_some());
        assert_eq!(at, Some(6 + STALL));
        let level = stalled(&noisy[..at.unwrap()]).unwrap();
        assert!(level <= USABLE && (level - 3.0e-6).abs() < 1e-12);
        // Stalled far from converged: over the usable level.
        let bad: Vec<f64> = (0..STALL + 5)
            .map(|i| 0.02 + 0.001 * (i % 3) as f64)
            .collect();
        assert!(stalled(&bad).is_some_and(|level| level > USABLE));
    }

    #[test]
    fn jacobi_diagonalizes() {
        let a = [4.0, 1.0, 2.0, 1.0, 3.0, 0.5, 2.0, 0.5, 5.0];
        let (l, v) = jacobi(&a, 3);
        for k in 0..3 {
            for i in 0..3 {
                let av: f64 = (0..3).map(|j| a[i * 3 + j] * v[j * 3 + k]).sum();
                assert!((av - l[k] * v[i * 3 + k]).abs() < 1e-12);
            }
        }
    }

    /// A chain of springs k with masses m (fixed at one end): λ_j = 4 k/m sin²((2j-1)π / (2(2n+1))).
    #[test]
    fn spring_chain() {
        let (n, kk, mm) = (30, 1000.0, 2.0);
        let adj: Vec<Vec<usize>> = (0..n)
            .map(|j: usize| {
                [j.checked_sub(1), (j + 1 < n).then_some(j + 1)]
                    .into_iter()
                    .flatten()
                    .collect()
            })
            .collect();
        let pattern = Arc::new(Pattern::new(&adj, &vec![1; n]));
        let e = |j: usize| pattern.first_equation(j);
        let mut stiffness = SymMatrix::new(pattern.clone());
        for j in 0..n {
            stiffness.add(e(j), e(j), if j + 1 < n { 2.0 * kk } else { kk });
            if j + 1 < n {
                stiffness.add(e(j), e(j + 1), -kk);
            }
        }
        let (k, _) = stiffness.factorize().unwrap();
        let kd: Vec<f64> = k.original_diagonal().to_vec();
        // The same mass at every joint, so it does not depend on the equation order.
        let m = SymSparse {
            n,
            entries: (0..n).map(|i| (i, i, mm)).collect(),
        };
        let mut sturm = |s: f64| {
            let mut shifted = stiffness.clone();
            for i in 0..n {
                shifted.add(i, i, -s * mm);
            }
            shifted.negative_pivots().map_err(|e| format!("{e}"))
        };
        let e = subspace(&k, &kd, &m, 6, &mut sturm).unwrap();
        for (j, l) in e.values.iter().enumerate() {
            let want = 4.0 * kk / mm
                * (((2 * j + 1) as f64 * std::f64::consts::PI) / (2.0 * (2 * n + 1) as f64))
                    .sin()
                    .powi(2);
            assert!((l - want).abs() < 1e-10 * want, "mode {j}: {l} vs {want}");
            let g: f64 = e.vectors[j].iter().map(|x| mm * x * x).sum();
            assert!((g - 1.0).abs() < 1e-10);
        }
    }
}
