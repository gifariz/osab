//! Sparse symmetric matrices on a joint graph and their LDLᵀ factorization, supernodal and
//! multifrontal (Duff and Reid 1983; Liu 1992, *The multifrontal method for sparse matrix solution*;
//! Davis 2006, *Direct Methods for Sparse Linear Systems*, ch. 4 and 9).
//!
//! The equations are numbered in a fill-reducing order of the joints ([`crate::order`]), each joint's
//! equations together. Every supernode (a chain of joints whose factor columns share one structure)
//! gathers its matrix columns and its children's update matrices into a dense frontal matrix, eliminates
//! its own columns there and passes the rest (the Schur complement) to its parent. There is no pivoting,
//! as in the stiffness method's classic LDLᵀ: a pivot that vanishes points at the equation where the
//! structure is unstable, and the signs of the pivots count the eigenvalues below a shift (Sturm check).

use std::ops::Range;
use std::sync::Arc;

use crate::order::{self, Relax, Symbolic};
use crate::parallel;

/// A pivot below this fraction of the original diagonal means the structure is unstable
/// (a mechanism) at that equation: about 12 of the 16 significant digits have cancelled.
/// Above it the matrix is accepted; small ratios are reported through [`Conditioning`] so the
/// caller can warn (e.g. a very stiff member next to a very soft one).
const PIVOT_TOL: f64 = 1e-12;

/// The stiffness put in place of a pivot that fails, as a fraction of the equation's original
/// diagonal (the stiffness its own joint's elements give that direction), when the factorization is
/// asked to go on: small enough that a load on the direction shows as an absurd displacement, large
/// enough that round-off left on it moves nothing visibly.
const MEND: f64 = 1e-8;

/// An equation whose pivot failed and was replaced (see [`SymMatrix::factorize_mended`]): the
/// structure has no stiffness in that direction. `ratio` is the pivot found over the original
/// diagonal (zero or less when nothing was left of it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mended {
    pub equation: usize,
    pub ratio: f64,
}

impl Mended {
    /// About how many of the 16 significant digits had cancelled in the pivot.
    pub fn digits_lost(&self) -> f64 {
        if self.ratio > 0.0 && self.ratio.is_finite() {
            (-self.ratio.log10()).clamp(0.0, 16.0)
        } else {
            16.0
        }
    }
}

/// How small supernodes are merged with their parent (larger dense blocks, a few more zeros).
const RELAX: Relax = Relax {
    small: 32,
    fraction: 0.1,
};

/// Smallest pivot ratio d_j / K_jj seen while factorizing, and its equation.
/// `-log10(worst_ratio)` is roughly the number of digits lost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conditioning {
    pub worst_ratio: f64,
    pub worst_equation: usize,
}

/// Where the matrix and its factor have entries, and the equation numbers.
#[derive(Debug)]
pub struct Pattern {
    n: usize,
    /// First equation of each vertex of the graph it was made from.
    vertex_equation: Vec<usize>,
    /// Lower triangle by columns: column j has rows `row[col[j]..col[j + 1]]`, ascending, from j.
    col: Vec<usize>,
    row: Vec<usize>,
    /// Supernode s: the equations `first[s]..first[s + 1]`, and `below[s]` (ascending) under its
    /// diagonal block. Supernodes are in postorder (children before parents, subtrees together).
    first: Vec<usize>,
    below: Vec<Vec<usize>>,
    children: Vec<usize>,
    /// Parent supernode (`None` at a root); the subtree of `s` is the supernodes `subtree[s]..=s`.
    parent: Vec<Option<usize>>,
    subtree: Vec<usize>,
    /// Start of each supernode's block, (k + m) x k column-major, in the factor's storage.
    offset: Vec<usize>,
}

/// Subtrees of the elimination tree that threads work on at the same time, as ranges of supernodes
/// (ascending), and each thread's share of them. The supernodes outside them (the top of the tree) come
/// after them, in order.
#[derive(Default)]
struct Split {
    tasks: Vec<Range<usize>>,
    shares: Vec<Vec<usize>>,
}

impl Pattern {
    /// The pattern of a matrix over a graph whose vertex `v` carries `weight[v]` >= 1 equations and is
    /// coupled to its neighbours `adj[v]`; the equations are numbered in a fill-reducing order.
    pub fn new(adj: &[Vec<usize>], weight: &[usize]) -> Pattern {
        let order = order::minimum_degree(adj, weight);
        let sym = Symbolic::new(adj, weight, &order, RELAX);
        Pattern::from_symbolic(adj, weight, &sym)
    }

    fn from_symbolic(adj: &[Vec<usize>], weight: &[usize], sym: &Symbolic) -> Pattern {
        let nv = adj.len();
        let mut vertex_equation = vec![0; nv];
        let mut pos = vec![0; nv];
        let mut n = 0;
        for (k, &v) in sym.order.iter().enumerate() {
            vertex_equation[v] = n;
            pos[v] = k;
            n += weight[v];
        }
        let eqs = |v: usize| vertex_equation[v]..vertex_equation[v] + weight[v];
        let mut col = Vec::with_capacity(n + 1);
        let mut row = Vec::new();
        col.push(0);
        for &v in &sym.order {
            let mut later: Vec<usize> = adj[v]
                .iter()
                .copied()
                .filter(|&u| pos[u] > pos[v])
                .collect();
            later.sort_unstable_by_key(|&u| pos[u]);
            later.dedup();
            for j in eqs(v) {
                row.extend(j..eqs(v).end);
                for &u in &later {
                    row.extend(eqs(u));
                }
                col.push(row.len());
            }
        }
        let ns = sym.rows.len();
        let first: Vec<usize> = (0..=ns)
            .map(|s| {
                if s < ns {
                    vertex_equation[sym.order[sym.start[s]]]
                } else {
                    n
                }
            })
            .collect();
        let below: Vec<Vec<usize>> = sym
            .rows
            .iter()
            .map(|r| r.iter().flat_map(|&k| eqs(sym.order[k])).collect())
            .collect();
        let mut children = vec![0; ns];
        for p in sym.parent.iter().flatten() {
            children[*p] += 1;
        }
        let parent = sym.parent.clone();
        let mut subtree: Vec<usize> = (0..ns).collect();
        for s in 0..ns {
            if let Some(q) = parent[s] {
                subtree[q] = subtree[q].min(subtree[s]);
            }
        }
        let mut offset = Vec::with_capacity(ns + 1);
        let mut total = 0;
        for s in 0..ns {
            offset.push(total);
            let k = first[s + 1] - first[s];
            total += (k + below[s].len()) * k;
        }
        offset.push(total);
        Pattern {
            n,
            vertex_equation,
            col,
            row,
            first,
            below,
            children,
            parent,
            subtree,
            offset,
        }
    }

    /// The tree split for `threads` threads by each supernode's work `cost(s)`: a subtree with more than
    /// a quarter of a thread's share is split into its children's subtrees (its root goes to the top);
    /// the subtrees go, largest first, to the thread with the least work so far.
    fn split(&self, threads: usize, cost: impl Fn(usize) -> f64) -> Split {
        let ns = self.below.len();
        let mut total = vec![0.0; ns];
        let mut kids: Vec<Vec<usize>> = vec![Vec::new(); ns];
        let mut roots = Vec::new();
        for s in 0..ns {
            total[s] += cost(s);
            match self.parent[s] {
                Some(q) => {
                    total[q] += total[s];
                    kids[q].push(s);
                }
                None => roots.push(s),
            }
        }
        let limit = roots.iter().map(|&r| total[r]).sum::<f64>() / (4 * threads) as f64;
        let mut tasks = Vec::new();
        let mut open = roots;
        while let Some(s) = open.pop() {
            if total[s] > limit && !kids[s].is_empty() {
                open.extend(&kids[s]);
            } else {
                tasks.push(self.subtree[s]..s + 1);
            }
        }
        tasks.sort_by_key(|t| t.start);
        let work = |t: usize| total[tasks[t].end - 1];
        let mut order: Vec<usize> = (0..tasks.len()).collect();
        order.sort_by(|&a, &b| work(b).total_cmp(&work(a)));
        let mut load = vec![0.0f64; threads];
        let mut shares = vec![Vec::new(); threads];
        for t in order {
            let i = (0..threads)
                .min_by(|&a, &b| load[a].total_cmp(&load[b]))
                .unwrap_or(0);
            load[i] += work(t);
            shares[i].push(t);
        }
        Split { tasks, shares }
    }

    /// Number of equations.
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// First equation of vertex `v` (its equations follow in a row).
    pub fn first_equation(&self, v: usize) -> usize {
        self.vertex_equation[v]
    }

    /// Entries stored in the factor (zeros added by merging supernodes included).
    pub fn factor_entries(&self) -> usize {
        *self.offset.last().unwrap_or(&0)
    }

    /// The most memory the factorization's work takes at one time besides the factor, in bytes: the
    /// front being eliminated and the update matrices waiting for their parents. Exact for one thread
    /// (it walks the supernodes as the factorization does); for more threads, the fronts the threads
    /// keep for their subtrees are added (an estimate from above).
    pub fn work_bytes(&self, threads: usize) -> usize {
        let size = |s: usize| {
            let k = self.first[s + 1] - self.first[s];
            (k, self.below[s].len())
        };
        let mut waiting: Vec<usize> = Vec::new();
        let (mut held, mut front, mut most) = (0usize, 0usize, 0usize);
        for s in 0..self.below.len() {
            let (k, m) = size(s);
            // The front's storage is kept between supernodes, so it is the largest so far.
            front = front.max((k + m) * (k + m));
            most = most.max(held + front);
            for _ in 0..self.children[s] {
                held -= waiting.pop().expect("a child's update matrix");
            }
            if m > 0 {
                waiting.push(m * m);
                held += m * m;
                most = most.max(held + front);
            }
        }
        // With threads, each thread working on subtrees keeps a front of its own, as large as the
        // largest in its subtrees.
        let mut others = 0;
        if threads > 1 {
            let split = self.split(threads, |s| {
                let (k, m) = size(s);
                (k * (k + m) * (k + m)) as f64
            });
            let widest = |task: &Range<usize>| {
                task.clone()
                    .map(|s| {
                        let (k, m) = size(s);
                        (k + m) * (k + m)
                    })
                    .max()
                    .unwrap_or(0)
            };
            others = split
                .shares
                .iter()
                .map(|share| {
                    share
                        .iter()
                        .map(|&t| widest(&split.tasks[t]))
                        .max()
                        .unwrap_or(0)
                })
                .sum();
        }
        8 * (most + others)
    }

    /// Number of supernodes and the order of the largest frontal matrix.
    pub fn supernodes(&self) -> (usize, usize) {
        let big = (0..self.below.len())
            .map(|s| self.first[s + 1] - self.first[s] + self.below[s].len())
            .max()
            .unwrap_or(0);
        (self.below.len(), big)
    }

    /// Position of entry (i, j) in the lower-triangle storage.
    fn find(&self, i: usize, j: usize) -> Option<usize> {
        let (r, c) = if i >= j { (i, j) } else { (j, i) };
        let rows = &self.row[self.col[c]..self.col[c + 1]];
        rows.binary_search(&r).ok().map(|k| self.col[c] + k)
    }
}

/// A symmetric matrix on a [`Pattern`] (the lower triangle is stored).
#[derive(Debug, Clone)]
pub struct SymMatrix {
    pattern: Arc<Pattern>,
    val: Vec<f64>,
}

impl SymMatrix {
    pub fn new(pattern: Arc<Pattern>) -> SymMatrix {
        let val = vec![0.0; pattern.row.len()];
        SymMatrix { pattern, val }
    }

    pub fn pattern(&self) -> &Arc<Pattern> {
        &self.pattern
    }

    /// Whether entry (i, j) can hold a value.
    pub fn contains(&self, i: usize, j: usize) -> bool {
        i < self.pattern.n && j < self.pattern.n && self.pattern.find(i, j).is_some()
    }

    /// Adds `v` to entry (i, j) and so to (j, i): add each pair of distinct equations once.
    pub fn add(&mut self, i: usize, j: usize, v: f64) {
        let k = self
            .pattern
            .find(i, j)
            .expect("entry inside the matrix pattern");
        self.val[k] += v;
    }

    pub fn diagonal(&self) -> Vec<f64> {
        let p = &self.pattern;
        (0..p.n).map(|j| self.val[p.col[j]]).collect()
    }

    /// Takes the equations marked in `held` out of the matrix: their rows and columns become zero with
    /// one on the diagonal, the matrix of the structure with those directions restrained.
    pub fn isolate(&mut self, held: &[bool]) {
        let p = &self.pattern;
        for j in 0..p.n {
            for idx in p.col[j]..p.col[j + 1] {
                let i = p.row[idx];
                if held[i] || held[j] {
                    self.val[idx] = if i == j { 1.0 } else { 0.0 };
                }
            }
        }
    }

    /// Factorizes K = L D Lᵀ for a K that should be positive definite. On failure returns the equation
    /// whose pivot vanished (or turned negative): the structure is unstable there.
    pub fn factorize(&self) -> Result<(Factor, Conditioning), usize> {
        self.factorize_on(parallel::threads())
    }

    fn factorize_on(&self, threads: usize) -> Result<(Factor, Conditioning), usize> {
        let diag0 = self.diagonal();
        let (l, pivots) = numeric(self, Check::Definite, false, &diag0, threads)?;
        let (worst_ratio, worst_equation) = pivots.worst.unwrap_or((1.0, 0));
        Ok((
            Factor {
                pattern: self.pattern.clone(),
                l,
                diag0,
            },
            Conditioning {
                worst_ratio,
                worst_equation,
            },
        ))
    }

    /// [`SymMatrix::factorize`] that goes on past a pivot that vanished or turned negative: the
    /// equation gets a tiny stiffness of its own (`MEND` times its original diagonal) and is cut from
    /// the equations after it, so the rest of the structure is solved as if it were not there. Returns
    /// those equations in elimination order; the conditioning leaves them out.
    pub fn factorize_mended(&self) -> (Factor, Conditioning, Vec<Mended>) {
        let diag0 = self.diagonal();
        let (l, pivots) = numeric(self, Check::Definite, true, &diag0, parallel::threads())
            .expect("no pivot fails when they are mended");
        let (worst_ratio, worst_equation) = pivots.worst.unwrap_or((1.0, 0));
        (
            Factor {
                pattern: self.pattern.clone(),
                l,
                diag0,
            },
            Conditioning {
                worst_ratio,
                worst_equation,
            },
            pivots.mended,
        )
    }

    /// Factorizes K = L D Lᵀ for a K that may have negative pivots (a stiffness with P-Delta of a
    /// structure that buckles under its P-Delta load). Fails only on a pivot that is nearly zero. Also
    /// returns the number of negative pivots and the equation of the first (elimination order).
    pub fn factorize_indefinite(&self) -> Result<Indefinite, usize> {
        self.factorize_indefinite_on(false).map(|r| r.0)
    }

    /// [`SymMatrix::factorize_indefinite`] that goes on past a pivot that is nearly zero, as
    /// [`SymMatrix::factorize_mended`] does; also returns those equations.
    pub fn factorize_indefinite_mended(&self) -> (Indefinite, Vec<Mended>) {
        self.factorize_indefinite_on(true)
            .expect("no pivot fails when they are mended")
    }

    fn factorize_indefinite_on(&self, mend: bool) -> Result<(Indefinite, Vec<Mended>), usize> {
        let diag0 = self.diagonal();
        let (l, pivots) = numeric(self, Check::Indefinite, mend, &diag0, parallel::threads())?;
        let (worst_ratio, worst_equation) = pivots.worst.unwrap_or((1.0, 0));
        Ok((
            (
                Factor {
                    pattern: self.pattern.clone(),
                    l,
                    diag0,
                },
                Conditioning {
                    worst_ratio,
                    worst_equation,
                },
                pivots.negative,
                pivots.first_negative,
            ),
            pivots.mended,
        ))
    }

    /// The number of negative pivots of L D Lᵀ for a matrix that need not be positive definite
    /// (K - σM for the Sturm check): the number of eigenvalues below σ. Fails (with the equation) on a
    /// pivot that is nearly zero, i.e. σ at an eigenvalue.
    pub fn negative_pivots(&self) -> Result<usize, usize> {
        let diag0 = self.diagonal();
        let (_, pivots) = numeric(self, Check::Inertia, false, &diag0, parallel::threads())?;
        Ok(pivots.negative)
    }
}

/// What [`SymMatrix::factorize_indefinite`] gives: the factor, its conditioning, the number of negative
/// pivots and the equation of the first.
pub type Indefinite = (Factor, Conditioning, usize, Option<usize>);

/// How the pivots are checked: K positive definite (each pivot above `PIVOT_TOL` of its diagonal), or
/// any K - σM (no pivot nearly zero; the negative ones are counted), or a stiffness that may have
/// negative pivots (counted, the first one noted, the smallest size ratio kept for the conditioning).
#[derive(Clone, Copy)]
enum Check {
    Definite,
    Inertia,
    Indefinite,
}

/// What the pivots of a factorization (or of a part of it) showed, in elimination order: the number of
/// negative pivots, and the smallest pivot ratio d_j / K_jj below 1 (the first of equals) with its
/// equation.
#[derive(Default)]
struct Pivots {
    negative: usize,
    worst: Option<(f64, usize)>,
    first_negative: Option<usize>,
    /// The pivots that failed and were replaced.
    mended: Vec<Mended>,
}

impl Pivots {
    /// Takes pivot `d` of equation `eq` (diagonal `k_jj` before factorizing): the pivot to go on
    /// with, or none when it fails. With `mend`, a pivot that fails is replaced (the caller then cuts
    /// the equation from the ones after it).
    fn take(&mut self, check: Check, mend: bool, eq: usize, d: f64, k_jj: f64) -> Option<f64> {
        let failed = match check {
            Check::Definite => !(d > PIVOT_TOL * k_jj.abs()) || !d.is_finite(),
            Check::Inertia | Check::Indefinite => {
                d.abs() <= PIVOT_TOL * k_jj.abs() || !d.is_finite()
            }
        };
        if failed {
            if !mend {
                return None;
            }
            // An equation with no diagonal of its own gets the share of a unit stiffness.
            let own = if k_jj.abs() > 0.0 && k_jj.is_finite() {
                k_jj.abs()
            } else {
                1.0
            };
            self.mended.push(Mended {
                equation: eq,
                ratio: d / k_jj.abs(),
            });
            return Some(MEND * own);
        }
        match check {
            Check::Definite => self.note_ratio(d / k_jj, eq),
            Check::Inertia => {
                if d < 0.0 {
                    self.negative += 1;
                }
            }
            Check::Indefinite => {
                if d < 0.0 {
                    self.negative += 1;
                    self.first_negative.get_or_insert(eq);
                }
                self.note_ratio(d.abs() / k_jj.abs(), eq);
            }
        }
        Some(d)
    }

    fn note_ratio(&mut self, ratio: f64, eq: usize) {
        if ratio < self.worst.map_or(1.0, |w| w.0) {
            self.worst = Some((ratio, eq));
        }
    }

    /// Adds the pivots that came after these.
    fn then(&mut self, later: &Pivots) {
        self.negative += later.negative;
        if self.first_negative.is_none() {
            self.first_negative = later.first_negative;
        }
        if let Some((ratio, eq)) = later.worst {
            self.note_ratio(ratio, eq);
        }
        self.mended.extend(&later.mended);
    }
}

/// Work arrays of one thread of the factorization.
struct Workspace {
    /// Place of each equation in the frontal matrix being assembled.
    local: Vec<usize>,
    front: Vec<f64>,
    work: dense::Work,
}

impl Workspace {
    fn new(n: usize) -> Workspace {
        Workspace {
            local: vec![0; n],
            front: Vec::new(),
            work: dense::Work::default(),
        }
    }
}

/// Disjoint pieces `ranges` (ascending, not overlapping) of `all`.
fn carve<T>(all: &mut [T], ranges: impl Iterator<Item = Range<usize>>) -> Vec<&mut [T]> {
    let mut out = Vec::new();
    let mut rest = all;
    let mut at = 0;
    for r in ranges {
        let (_, tail) = std::mem::take(&mut rest).split_at_mut(r.start - at);
        let (piece, tail) = tail.split_at_mut(r.end - r.start);
        out.push(piece);
        rest = tail;
        at = r.end;
    }
    out
}

/// Runs `work(state, task, piece)` for every subtree of `split` on its share's thread (`state` made once
/// per thread by `init`), `piece` being the subtree's part `span(task)` of `data`; the results by
/// subtree.
fn on_subtrees<D: Send, S, T: Send>(
    split: &Split,
    data: &mut [D],
    span: impl Fn(&Range<usize>) -> Range<usize> + Sync,
    init: impl Fn() -> S + Sync,
    work: impl Fn(&mut S, Range<usize>, &mut [D]) -> T + Sync,
) -> Vec<T> {
    let mut pieces: Vec<Option<&mut [D]>> = carve(data, split.tasks.iter().map(&span))
        .into_iter()
        .map(Some)
        .collect();
    let mut out: Vec<Option<T>> = split.tasks.iter().map(|_| None).collect();
    std::thread::scope(|sc| {
        let handles: Vec<_> = split
            .shares
            .iter()
            .map(|share| {
                let mine: Vec<(usize, &mut [D])> = share
                    .iter()
                    .map(|&t| (t, pieces[t].take().expect("one piece per subtree")))
                    .collect();
                let (init, work) = (&init, &work);
                sc.spawn(move || {
                    let mut state = init();
                    mine.into_iter()
                        .map(|(t, piece)| (t, work(&mut state, split.tasks[t].clone(), piece)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in handles {
            for (t, r) in h.join().expect("a solver thread") {
                out[t] = Some(r);
            }
        }
    });
    out.into_iter()
        .map(|r| r.expect("every subtree worked on"))
        .collect()
}

/// A factorized matrix L D Lᵀ.
#[derive(Debug)]
pub struct Factor {
    pattern: Arc<Pattern>,
    /// Supernode blocks, (k + m) x k column-major: unit L below the diagonal, D on it.
    l: Vec<f64>,
    diag0: Vec<f64>,
}

impl Factor {
    pub fn len(&self) -> usize {
        self.pattern.n
    }

    pub fn is_empty(&self) -> bool {
        self.pattern.n == 0
    }

    /// The diagonal of the matrix before it was factorized.
    pub fn original_diagonal(&self) -> &[f64] {
        &self.diag0
    }

    /// Solves K x = b in place.
    pub fn solve(&self, b: &mut [f64]) {
        self.solve_many(b, 1);
    }

    /// Solves K X = B in place for `nrhs` right-hand sides stored equation by equation
    /// (`b[eq * nrhs + r]`). With threads ([`crate::parallel`]) subtrees of the elimination tree are
    /// solved at the same time; every value comes out the same.
    pub fn solve_many(&self, b: &mut [f64], nrhs: usize) {
        self.solve_on(b, nrhs, parallel::threads());
    }

    fn solve_on(&self, b: &mut [f64], nrhs: usize, threads: usize) {
        let p = &*self.pattern;
        let (n, ns) = (p.n, p.below.len());
        assert_eq!(b.len(), n * nrhs);
        let split = if threads > 1 {
            p.split(threads, |s| {
                let k = p.first[s + 1] - p.first[s];
                (k * (k + p.below[s].len())) as f64
            })
        } else {
            Split::default()
        };
        let mut in_task = vec![false; ns];
        for t in &split.tasks {
            in_task[t.clone()].fill(true);
        }
        let rows = |t: &Range<usize>| p.first[t.start] * nrhs..p.first[t.end] * nrhs;
        // L D y = b: the subtrees at the same time (what they take off rows above them is kept), then the
        // top of the tree in order, each subtree's part taken off at its place.
        let spills = on_subtrees(&split, b, rows, Vec::new, |tmp, task, part| {
            let (base, limit) = (p.first[task.start], p.first[task.end]);
            let mut spill = Spill::default();
            for s in task {
                self.forward(s, part, base, limit, nrhs, tmp, &mut spill);
            }
            spill
        });
        // The top of the tree for the right-hand sides `cols` (`b` holding just those, `width` of them):
        // L D y = b in order, each subtree's part taken off at its place, then Lᵀ x = z in reverse order.
        let top = |b: &mut [f64], width: usize, cols: Range<usize>| {
            let mut tmp = Vec::new();
            let mut none = Spill::default();
            let mut tasks = split.tasks.iter().zip(&spills).peekable();
            let mut s = 0;
            while s < ns {
                if let Some((task, spill)) = tasks.next_if(|(t, _)| t.start == s) {
                    for (i, &row) in spill.rows.iter().enumerate() {
                        let part = &spill.vals[i * nrhs + cols.start..i * nrhs + cols.end];
                        for (v, d) in b[row * width..(row + 1) * width].iter_mut().zip(part) {
                            *v -= d;
                        }
                    }
                    s = task.end;
                } else {
                    self.forward(s, b, 0, n, width, &mut tmp, &mut none);
                    s += 1;
                }
            }
            for s in (0..ns).rev().filter(|&s| !in_task[s]) {
                self.backward(s, b, 0, n, &[], width, &mut tmp);
            }
        };
        // With threads, groups of whole tiles of right-hand sides go through the top at the same time.
        let groups = threads.min(nrhs.div_ceil(dense::W));
        if groups <= 1 {
            top(b, nrhs, 0..nrhs);
        } else {
            let width = nrhs.div_ceil(groups).next_multiple_of(dense::W);
            let ranges: Vec<Range<usize>> = (0..nrhs)
                .step_by(width)
                .map(|r0| r0..(r0 + width).min(nrhs))
                .collect();
            let parts: Vec<Vec<f64>> = std::thread::scope(|sc| {
                let (b, top) = (&*b, &top);
                let handles: Vec<_> = ranges
                    .iter()
                    .map(|r| {
                        sc.spawn(move || {
                            let w = r.len();
                            let mut part = vec![0.0; n * w];
                            for (row, out) in b.chunks_exact(nrhs).zip(part.chunks_exact_mut(w)) {
                                out.copy_from_slice(&row[r.clone()]);
                            }
                            top(&mut part, w, r.clone());
                            part
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|h| h.join().expect("a solver thread"))
                    .collect()
            });
            for (r, part) in ranges.iter().zip(&parts) {
                for (row, q) in b.chunks_exact_mut(nrhs).zip(part.chunks_exact(r.len())) {
                    row[r.clone()].copy_from_slice(q);
                }
            }
        }
        // The subtrees at the same time (the rows above them read from a copy).
        if !split.tasks.is_empty() {
            let above = b.to_vec();
            on_subtrees(&split, b, rows, Vec::new, |tmp, task, part| {
                let (base, limit) = (p.first[task.start], p.first[task.end]);
                for s in task.rev() {
                    self.backward(s, part, base, limit, &above, nrhs, tmp);
                }
            });
        }
    }

    /// L D y = b at supernode `s`: its rows of `b` (rows of `nrhs` values from equation `base` on) become
    /// y scaled by D, and the rows below lose the supernode's part: in place below equation `limit`, else
    /// kept in `spill`.
    #[allow(clippy::too_many_arguments)]
    fn forward(
        &self,
        s: usize,
        b: &mut [f64],
        base: usize,
        limit: usize,
        nrhs: usize,
        tmp: &mut Vec<f64>,
        spill: &mut Spill,
    ) {
        let (f0, k, below, blk) = self.block(s);
        let m = below.len();
        let nf = k + m;
        let x0 = (f0 - base) * nrhs;
        for j in 0..k {
            for i in j + 1..k {
                let lij = blk[i + j * nf];
                if lij != 0.0 {
                    for r in 0..nrhs {
                        b[x0 + i * nrhs + r] -= lij * b[x0 + j * nrhs + r];
                    }
                }
            }
        }
        if m > 0 {
            tmp.clear();
            tmp.resize(m * nrhs, 0.0);
            dense::below_times(&blk[k..], nf, m, k, &b[x0..x0 + k * nrhs], nrhs, tmp);
            for (i, &row) in below.iter().enumerate() {
                let part = &tmp[i * nrhs..(i + 1) * nrhs];
                if row < limit {
                    let at = (row - base) * nrhs;
                    for (v, d) in b[at..at + nrhs].iter_mut().zip(part) {
                        *v -= d;
                    }
                } else {
                    spill.rows.push(row);
                    spill.vals.extend_from_slice(part);
                }
            }
        }
        // The rows are final now (only the supernode's descendants change them): D z = y.
        for t in 0..k {
            let d = blk[t + t * nf];
            for v in &mut b[x0 + t * nrhs..x0 + (t + 1) * nrhs] {
                *v /= d;
            }
        }
    }

    /// Lᵀ x = z at supernode `s`: its rows of `b` (from equation `base` on) become x; the rows below it
    /// come from `b` below equation `limit`, from `above` (all rows) at and past it.
    #[allow(clippy::too_many_arguments)]
    fn backward(
        &self,
        s: usize,
        b: &mut [f64],
        base: usize,
        limit: usize,
        above: &[f64],
        nrhs: usize,
        tmp: &mut Vec<f64>,
    ) {
        let (f0, k, below, blk) = self.block(s);
        let m = below.len();
        let nf = k + m;
        let x0 = (f0 - base) * nrhs;
        if m > 0 {
            tmp.clear();
            for &row in below {
                if row < limit {
                    tmp.extend_from_slice(&b[(row - base) * nrhs..(row - base + 1) * nrhs]);
                } else {
                    tmp.extend_from_slice(&above[row * nrhs..(row + 1) * nrhs]);
                }
            }
            dense::below_transposed(&blk[k..], nf, m, k, tmp, nrhs, &mut b[x0..x0 + k * nrhs]);
        }
        for j in (0..k).rev() {
            for i in j + 1..k {
                let lij = blk[i + j * nf];
                if lij != 0.0 {
                    for r in 0..nrhs {
                        b[x0 + j * nrhs + r] -= lij * b[x0 + i * nrhs + r];
                    }
                }
            }
        }
    }

    /// First equation, number of columns, rows below and the block of supernode `s`.
    /// Holds equations that the factorization mended (each stands alone in the factor, see
    /// [`SymMatrix::factorize_mended`]): every solve gives zero there until [`Factor::release`], the
    /// solution of the structure with those directions restrained. Returns what `release` needs.
    pub fn hold(&mut self, equations: &[usize]) -> Vec<(usize, f64)> {
        equations
            .iter()
            .map(|&eq| {
                let at = self.pivot_at(eq);
                // A solve divides by the pivot.
                (at, std::mem::replace(&mut self.l[at], f64::INFINITY))
            })
            .collect()
    }

    /// Frees the factor's storage. Nothing may be solved with it afterwards: for a factor that is
    /// about to be replaced, so that the old and the new one are not both in memory.
    pub fn free(&mut self) {
        self.l = Vec::new();
    }

    /// Gives held equations their pivots back.
    pub fn release(&mut self, held: Vec<(usize, f64)>) {
        for (at, d) in held {
            self.l[at] = d;
        }
    }

    /// Where the pivot of an equation is stored.
    fn pivot_at(&self, eq: usize) -> usize {
        let p = &*self.pattern;
        let s = p.first.partition_point(|&f| f <= eq) - 1;
        let (k, m) = (p.first[s + 1] - p.first[s], p.below[s].len());
        let t = eq - p.first[s];
        p.offset[s] + t + t * (k + m)
    }

    fn block(&self, s: usize) -> (usize, usize, &[usize], &[f64]) {
        let p = &*self.pattern;
        let f0 = p.first[s];
        let k = p.first[s + 1] - f0;
        (f0, k, &p.below[s], &self.l[p.offset[s]..p.offset[s + 1]])
    }
}

/// What a part of the forward solve takes off rows outside it: the rows, and their values one after
/// another, in the order they came.
#[derive(Default)]
struct Spill {
    rows: Vec<usize>,
    vals: Vec<f64>,
}

/// An update matrix waiting for its parent (m x m column-major, lower triangle), with the supernode
/// that made it.
type Update = (Vec<f64>, usize);

/// The multifrontal LDLᵀ factorization of `a` on `threads` threads: the supernode blocks and what the
/// pivots showed; on a failed pivot its equation (the first in elimination order).
fn numeric(
    a: &SymMatrix,
    check: Check,
    mend: bool,
    diag0: &[f64],
    threads: usize,
) -> Result<(Vec<f64>, Pivots), usize> {
    let p = &*a.pattern;
    let ns = p.below.len();
    // Counting the negative pivots needs the fronts only: no factor is stored (or allocated) for it.
    let store = !matches!(check, Check::Inertia);
    let mut l = if store {
        vec![0.0; p.factor_entries()]
    } else {
        Vec::new()
    };
    // With threads: subtrees at the same time (by their dense work), each with its own stack, then the
    // top of the tree in order, each subtree's update matrix and pivots taken at its place.
    let split = if threads > 1 {
        p.split(threads, |s| {
            let k = (p.first[s + 1] - p.first[s]) as f64;
            let nf = k + p.below[s].len() as f64;
            k * nf * nf
        })
    } else {
        Split::default()
    };
    let blocks = |t: &Range<usize>| {
        if store {
            p.offset[t.start]..p.offset[t.end]
        } else {
            0..0
        }
    };
    let init = || Workspace::new(p.n);
    let done = on_subtrees(
        &split,
        &mut l,
        blocks,
        init,
        |ws, task, part| -> Result<_, usize> {
            let base = p.offset[task.start];
            let mut stack: Vec<Update> = Vec::new();
            let mut pivots = Pivots::default();
            for s in task {
                let block: &mut [f64] = if store {
                    &mut part[p.offset[s] - base..p.offset[s + 1] - base]
                } else {
                    &mut []
                };
                front(
                    a,
                    s,
                    check,
                    mend,
                    diag0,
                    block,
                    &mut stack,
                    ws,
                    &mut pivots,
                    1,
                )?;
            }
            Ok((stack, pivots))
        },
    );
    let mut ws = Workspace::new(p.n);
    let mut stack: Vec<Update> = Vec::new();
    let mut pivots = Pivots::default();
    let mut tasks = split.tasks.iter().zip(done).peekable();
    let mut s = 0;
    while s < ns {
        if let Some((task, result)) = tasks.next_if(|(t, _)| t.start == s) {
            let (rest, part) = result?;
            pivots.then(&part);
            stack.extend(rest);
            s = task.end;
        } else {
            let block: &mut [f64] = if store {
                &mut l[p.offset[s]..p.offset[s + 1]]
            } else {
                &mut []
            };
            front(
                a,
                s,
                check,
                mend,
                diag0,
                block,
                &mut stack,
                &mut ws,
                &mut pivots,
                threads,
            )?;
            s += 1;
        }
    }
    Ok((l, pivots))
}

/// Supernode `s` of the factorization: gathers its matrix columns and its children's update matrices
/// (from the top of `stack`) into its frontal matrix, eliminates its columns into `block` (the dense
/// update on `threads` threads) and pushes its own update matrix. On a failed pivot returns its
/// equation.
#[allow(clippy::too_many_arguments)]
fn front(
    a: &SymMatrix,
    s: usize,
    check: Check,
    mend: bool,
    diag0: &[f64],
    block: &mut [f64],
    stack: &mut Vec<Update>,
    ws: &mut Workspace,
    pivots: &mut Pivots,
    threads: usize,
) -> Result<(), usize> {
    let p = &*a.pattern;
    let (f0, f1) = (p.first[s], p.first[s + 1]);
    let k = f1 - f0;
    let below = &p.below[s];
    let m = below.len();
    let nf = k + m;
    let (local, front) = (&mut ws.local, &mut ws.front);
    for t in 0..k {
        local[f0 + t] = t;
    }
    for (i, &r) in below.iter().enumerate() {
        local[r] = k + i;
    }
    front.clear();
    front.resize(nf * nf, 0.0);
    for j in f0..f1 {
        let cj = (j - f0) * nf;
        for idx in p.col[j]..p.col[j + 1] {
            front[local[p.row[idx]] + cj] += a.val[idx];
        }
    }
    for _ in 0..p.children[s] {
        let (u, c) = stack.pop().expect("a child's update matrix");
        let rows = &p.below[c];
        let mc = rows.len();
        let at: Vec<usize> = rows.iter().map(|&r| local[r]).collect();
        for cj in 0..mc {
            let fj = at[cj] * nf;
            let src = &u[cj * mc..(cj + 1) * mc];
            for ci in cj..mc {
                front[at[ci] + fj] += src[ci];
            }
        }
    }
    dense::ldl_partial(front, nf, k, &mut ws.work, threads, &mut |t, d| {
        pivots.take(check, mend, f0 + t, d, diag0[f0 + t])
    })
    .map_err(|t| f0 + t)?;
    // An empty `block`: the factor is not kept (the pivots are counted only).
    if !block.is_empty() {
        block.copy_from_slice(&front[..nf * k]);
    }
    if m > 0 {
        let mut u = vec![0.0; m * m];
        for j in 0..m {
            let from = (k + j) * nf + k;
            u[j * m + j..(j + 1) * m].copy_from_slice(&front[from + j..from + m]);
        }
        stack.push((u, s));
    }
    Ok(())
}

/// Dense kernels on column-major blocks.
mod dense {
    /// Panel width of the blocked elimination.
    const NB: usize = 32;
    /// Rows and columns of the register tile of the update kernel.
    const R: usize = 4;

    /// Right-hand sides taken together by the solve kernels.
    pub const W: usize = 4;

    /// Forward step under a supernode's diagonal block: t[i][r] = Σ_j l[i + j·ld] x[j][r] for i < m,
    /// j < k, r < nrhs (`l` from the first row under the block, column-major with leading dimension
    /// `ld`; x is k x nrhs and t m x nrhs, by rows). Every value is summed over j in order from zero, in
    /// tiles of R rows and W right-hand sides.
    pub fn below_times(
        l: &[f64],
        ld: usize,
        m: usize,
        k: usize,
        x: &[f64],
        nrhs: usize,
        t: &mut [f64],
    ) {
        let wide = nrhs / W * W;
        let mut i0 = 0;
        while i0 + R <= m {
            for r0 in (0..wide).step_by(W) {
                let mut acc = [[0.0f64; W]; R];
                for j in 0..k {
                    let lj: &[f64; R] = l[i0 + j * ld..i0 + j * ld + R].try_into().unwrap();
                    let xj: &[f64; W] = x[j * nrhs + r0..j * nrhs + r0 + W].try_into().unwrap();
                    for a in 0..R {
                        for c in 0..W {
                            acc[a][c] += lj[a] * xj[c];
                        }
                    }
                }
                for (a, row) in acc.iter().enumerate() {
                    t[(i0 + a) * nrhs + r0..(i0 + a) * nrhs + r0 + W].copy_from_slice(row);
                }
            }
            for r in wide..nrhs {
                let mut acc = [0.0f64; R];
                for j in 0..k {
                    let xjr = x[j * nrhs + r];
                    for (a, s) in acc.iter_mut().enumerate() {
                        *s += l[i0 + a + j * ld] * xjr;
                    }
                }
                for (a, s) in acc.iter().enumerate() {
                    t[(i0 + a) * nrhs + r] = *s;
                }
            }
            i0 += R;
        }
        for i in i0..m {
            for r in 0..nrhs {
                let mut s = 0.0;
                for j in 0..k {
                    s += l[i + j * ld] * x[j * nrhs + r];
                }
                t[i * nrhs + r] = s;
            }
        }
    }

    /// Back step under a supernode's diagonal block: x[j][r] -= Σ_i l[i + j·ld] t[i][r] for j < k,
    /// i < m, r < nrhs (layouts as in [`below_times`]). Every value is updated over i in order, in tiles
    /// of R columns and W right-hand sides.
    pub fn below_transposed(
        l: &[f64],
        ld: usize,
        m: usize,
        k: usize,
        t: &[f64],
        nrhs: usize,
        x: &mut [f64],
    ) {
        let wide = nrhs / W * W;
        let mut j0 = 0;
        while j0 + R <= k {
            let cols: [&[f64]; R] = std::array::from_fn(|c| &l[(j0 + c) * ld..(j0 + c) * ld + m]);
            for r0 in (0..wide).step_by(W) {
                let mut acc: [[f64; W]; R] = std::array::from_fn(|c| {
                    x[(j0 + c) * nrhs + r0..(j0 + c) * nrhs + r0 + W]
                        .try_into()
                        .unwrap()
                });
                for i in 0..m {
                    let ti: &[f64; W] = t[i * nrhs + r0..i * nrhs + r0 + W].try_into().unwrap();
                    for c in 0..R {
                        let lc = cols[c][i];
                        for r in 0..W {
                            acc[c][r] -= lc * ti[r];
                        }
                    }
                }
                for (c, row) in acc.iter().enumerate() {
                    x[(j0 + c) * nrhs + r0..(j0 + c) * nrhs + r0 + W].copy_from_slice(row);
                }
            }
            for r in wide..nrhs {
                let mut acc: [f64; R] = std::array::from_fn(|c| x[(j0 + c) * nrhs + r]);
                for i in 0..m {
                    let ti = t[i * nrhs + r];
                    for (c, s) in acc.iter_mut().enumerate() {
                        *s -= cols[c][i] * ti;
                    }
                }
                for (c, s) in acc.iter().enumerate() {
                    x[(j0 + c) * nrhs + r] = *s;
                }
            }
            j0 += R;
        }
        for j in j0..k {
            let col = &l[j * ld..j * ld + m];
            for r in 0..nrhs {
                let mut s = x[j * nrhs + r];
                for (i, lij) in col.iter().enumerate() {
                    s -= lij * t[i * nrhs + r];
                }
                x[j * nrhs + r] = s;
            }
        }
    }

    /// Packed panels for the update kernel, kept between calls.
    #[derive(Default)]
    pub struct Work {
        a: Vec<f64>,
        w: Vec<f64>,
    }

    /// Eliminates the first `k` columns of the symmetric `nf` x `nf` matrix `f` (column-major, lower
    /// triangle used): those columns become L (unit, below the diagonal) and D (on it), the trailing
    /// block becomes the Schur complement (large updates on `threads` threads). `pivot(t, d)` checks
    /// each pivot and gives the one to use; none stops at column t. A pivot it changes stands alone:
    /// the column under it is set to zero, so that equation is cut from the ones after it.
    pub fn ldl_partial(
        f: &mut [f64],
        nf: usize,
        k: usize,
        work: &mut Work,
        threads: usize,
        pivot: &mut dyn FnMut(usize, f64) -> Option<f64>,
    ) -> Result<(), usize> {
        let mut b0 = 0;
        while b0 < k {
            let b1 = (b0 + NB).min(k);
            // The panel, a column at a time: scale column j, update the panel's later columns.
            for j in b0..b1 {
                let found = f[j + j * nf];
                let Some(d) = pivot(j, found) else {
                    return Err(j);
                };
                let (left, right) = f.split_at_mut((j + 1) * nf);
                let colj = &mut left[j * nf..];
                if d != found {
                    colj[j] = d;
                    colj[j + 1..nf].fill(0.0);
                }
                for c in j + 1..b1 {
                    let fac = colj[c] / d;
                    if fac == 0.0 {
                        continue;
                    }
                    let colc = &mut right[(c - j - 1) * nf..(c - j) * nf];
                    for i in c..nf {
                        colc[i] -= colj[i] * fac;
                    }
                }
                for v in &mut colj[j + 1..nf] {
                    *v /= d;
                }
            }
            // The trailing block (columns and rows from b1): F -= L_panel D L_panelᵀ, lower triangle.
            if b1 < nf {
                update(f, nf, b0, b1, work, threads);
            }
            b0 = b1;
        }
        Ok(())
    }

    /// About the multiply-adds one thread does in the time it takes to start a thread.
    const START_COST: usize = 300_000;

    /// F[i][c] -= sum over the panel columns p of L[i][p] d_p L[c][p], for b1 <= c <= i < nf, in tiles
    /// of R x R on packed copies of the panel (rows in groups of R, zeros past the end). On several
    /// threads each takes a range of tile columns (about the same number of tiles each).
    fn update(f: &mut [f64], nf: usize, b0: usize, b1: usize, work: &mut Work, threads: usize) {
        let nb = b1 - b0;
        let nt = nf - b1;
        let groups = nt.div_ceil(R);
        work.a.clear();
        work.a.resize(groups * nb * R, 0.0);
        work.w.clear();
        work.w.resize(groups * nb * R, 0.0);
        for p in 0..nb {
            let col = (b0 + p) * nf;
            let d = f[b0 + p + col];
            for i in 0..nt {
                let v = f[b1 + i + col];
                let at = ((i / R) * nb + p) * R + i % R;
                work.a[at] = v;
                work.w[at] = v * d;
            }
        }
        let (a, w) = (&work.a[..], &work.w[..]);
        // Tile column gc on `cols`, the columns b1 + gc R .. of f.
        let tile_column = |gc: usize, cols: &mut [f64]| {
            let w = &w[gc * nb * R..(gc + 1) * nb * R];
            for gi in gc..groups {
                let a = &a[gi * nb * R..(gi + 1) * nb * R];
                let mut acc = [[0.0f64; R]; R];
                for (ap, wp) in a.as_chunks::<R>().0.iter().zip(w.as_chunks::<R>().0) {
                    for cc in 0..R {
                        for r in 0..R {
                            acc[cc][r] += ap[r] * wp[cc];
                        }
                    }
                }
                let (i0, c0) = (b1 + gi * R, b1 + gc * R);
                for cc in 0..R.min(nf - c0) {
                    let col = cc * nf;
                    for r in 0..R.min(nf - i0) {
                        cols[i0 + r + col] -= acc[cc][r];
                    }
                }
            }
        };
        let trailing = &mut f[b1 * nf..];
        // Threads cost time to start: about sqrt(work / START_COST) of them gives the shortest time.
        let worth = ((nt * nt * nb / 2 / START_COST) as f64).sqrt() as usize;
        let threads = threads.min(worth).min(groups);
        if threads <= 1 {
            for (gc, cols) in trailing.chunks_mut(R * nf).enumerate() {
                tile_column(gc, cols);
            }
            return;
        }
        // Tile column gc has groups - gc tiles: cut where the running count passes each thread's share.
        let tiles = groups * (groups + 1) / 2;
        let mut cuts = vec![0];
        let mut count = 0;
        for gc in 0..groups {
            count += groups - gc;
            if count * threads >= tiles * cuts.len() && cuts.len() < threads {
                cuts.push(gc + 1);
            }
        }
        cuts.push(groups);
        cuts.dedup();
        let pieces = super::carve(
            trailing,
            cuts.windows(2)
                .map(|c| c[0] * R * nf..(c[1] * R * nf).min(nt * nf)),
        );
        std::thread::scope(|sc| {
            for (g0, piece) in cuts.iter().zip(pieces) {
                let tile_column = &tile_column;
                sc.spawn(move || {
                    for (g, cols) in piece.chunks_mut(R * nf).enumerate() {
                        tile_column(g0 + g, cols);
                    }
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A random-looking symmetric positive definite matrix on a graph of `n` vertices with `w`
    /// equations each (a ring with chords), as dense rows.
    fn test_matrix(n: usize, w: usize, seed: u64) -> (Vec<Vec<usize>>, Vec<Vec<f64>>) {
        let mut adj = vec![Vec::new(); n];
        let mut s = seed;
        let mut rnd = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s % 10_000) as f64 / 10_000.0 - 0.5
        };
        for v in 0..n {
            for u in [(v + 1) % n, (v * 7 + 3) % n] {
                if u != v {
                    adj[v].push(u);
                    adj[u].push(v);
                }
            }
        }
        for l in &mut adj {
            l.sort_unstable();
            l.dedup();
        }
        let ne = n * w;
        let mut a = vec![vec![0.0; ne]; ne];
        for v in 0..n {
            for &u in adj[v].iter().filter(|&&u| u > v) {
                for p in 0..w {
                    for q in 0..w {
                        let x = rnd();
                        a[v * w + p][u * w + q] = x;
                        a[u * w + q][v * w + p] = x;
                    }
                }
            }
            for p in 0..w {
                for q in 0..=p {
                    let x = rnd();
                    a[v * w + p][v * w + q] = x;
                    a[v * w + q][v * w + p] = x;
                }
            }
        }
        // Diagonal dominance makes it positive definite.
        for i in 0..ne {
            let s: f64 = a[i].iter().map(|x| x.abs()).sum();
            a[i][i] = s + 1.0;
        }
        (adj, a)
    }

    fn assemble(pattern: &Arc<Pattern>, a: &[Vec<f64>], n: usize, w: usize) -> SymMatrix {
        let mut m = SymMatrix::new(pattern.clone());
        let eq = |i: usize| pattern.first_equation(i / w) + i % w;
        for i in 0..n * w {
            for j in 0..=i {
                if a[i][j] != 0.0 {
                    m.add(eq(i), eq(j), a[i][j]);
                }
            }
        }
        m
    }

    #[test]
    fn solves_like_the_dense_matrix() {
        for (n, w) in [(1, 1), (2, 3), (30, 6), (200, 3)] {
            let (adj, a) = test_matrix(n, w, 7 + n as u64);
            let pattern = Arc::new(Pattern::new(&adj, &vec![w; n]));
            let m = assemble(&pattern, &a, n, w);
            let (f, cond) = m.factorize().expect("positive definite");
            assert!(cond.worst_ratio > 0.0 && cond.worst_ratio <= 1.0);
            // Two right-hand sides at once and one alone give the same x, and A x = b.
            let ne = n * w;
            let x_true: Vec<f64> = (0..ne).map(|i| 1.0 + 0.25 * (i % 7) as f64).collect();
            let eq = |i: usize| pattern.first_equation(i / w) + i % w;
            let mut b = vec![0.0; ne];
            for i in 0..ne {
                b[eq(i)] = (0..ne).map(|j| a[i][j] * x_true[j]).sum();
            }
            let mut two: Vec<f64> = b.iter().flat_map(|&v| [v, 2.0 * v]).collect();
            f.solve(&mut b);
            f.solve_many(&mut two, 2);
            for i in 0..ne {
                let x = b[eq(i)];
                assert!((x - x_true[i]).abs() < 1e-12, "{x} vs {}", x_true[i]);
                assert!((two[2 * eq(i)] - x).abs() < 1e-14);
                assert!((two[2 * eq(i) + 1] - 2.0 * x).abs() < 1e-13);
            }
            // Nine right-hand sides at once (tiles of four and one left over) give exactly what each
            // gives alone: the sums run in the same order.
            let nine: Vec<f64> = (0..ne * 9)
                .map(|t| ((t * 37 + 11) % 101) as f64 / 50.0 - 1.0)
                .collect();
            let mut together = nine.clone();
            f.solve_many(&mut together, 9);
            for r in 0..9 {
                let mut alone: Vec<f64> = (0..ne).map(|e| nine[e * 9 + r]).collect();
                f.solve(&mut alone);
                for e in 0..ne {
                    assert_eq!(together[e * 9 + r], alone[e]);
                }
            }
        }
    }

    #[test]
    fn counts_negative_pivots_and_finds_mechanisms() {
        // A - σI with σ between eigenvalues: the count is that of the dense matrix's eigenvalues below σ.
        let (n, w) = (12, 2);
        let (adj, a) = test_matrix(n, w, 3);
        let pattern = Arc::new(Pattern::new(&adj, &vec![w; n]));
        let ne = n * w;
        let (mu, _) = crate::eigen::jacobi(&a.iter().flatten().copied().collect::<Vec<_>>(), ne);
        let mut mu = mu;
        mu.sort_by(f64::total_cmp);
        let sigma = 0.5 * (mu[9] + mu[10]);
        let mut shifted = a.clone();
        for i in 0..ne {
            shifted[i][i] -= sigma;
        }
        let m = assemble(&pattern, &shifted, n, w);
        assert_eq!(m.negative_pivots(), Ok(10));
        assert!(m.factorize().is_err());
        // Two springs in series with no support: the rigid-body mode stops the factorization.
        let adj = vec![vec![1], vec![0, 2], vec![1]];
        let pattern = Arc::new(Pattern::new(&adj, &[1, 1, 1]));
        let mut k = SymMatrix::new(pattern.clone());
        let e = |v: usize| pattern.first_equation(v);
        for (i, j, v) in [
            (0, 0, 1.0),
            (0, 1, -1.0),
            (1, 1, 2.0),
            (1, 2, -1.0),
            (2, 2, 1.0),
        ] {
            k.add(e(i), e(j), v);
        }
        assert!(k.factorize().is_err());
    }

    #[test]
    fn accepts_stiff_member_on_soft_support_and_reports_it() {
        // Soft spring (1) to ground, then a very stiff bar (1e11): stable, but 11 digits cancel.
        let (ks, kb) = (1.0, 1e11);
        let adj = vec![vec![1], vec![0]];
        let pattern = Arc::new(Pattern::new(&adj, &[1, 1]));
        let e = |v: usize| pattern.first_equation(v);
        let mut k = SymMatrix::new(pattern.clone());
        k.add(e(0), e(0), ks + kb);
        k.add(e(0), e(1), -kb);
        k.add(e(1), e(1), kb);
        let (f, cond) = k.factorize().expect("stable structure");
        assert!(cond.worst_ratio < 1e-10 && cond.worst_ratio > 1e-12);
        let mut x = vec![0.0; 2];
        x[e(1)] = 1.0;
        f.solve(&mut x);
        assert!(
            (x[0] - 1.0).abs() < 1e-4 && (x[1] - 1.0).abs() < 1e-4,
            "{x:?}"
        );
    }

    #[test]
    fn dense_kernel_matches_plain_elimination() {
        // A front of 70 with 40 columns to eliminate: more than one panel and partial tiles.
        let (nf, k) = (70, 40);
        let mut f = vec![0.0; nf * nf];
        for j in 0..nf {
            for i in j..nf {
                let v = if i == j {
                    nf as f64 + 1.0
                } else {
                    ((i * 31 + j * 17) % 13) as f64 / 13.0 - 0.5
                };
                f[i + j * nf] = v;
            }
        }
        let mut plain = f.clone();
        for j in 0..k {
            let d = plain[j + j * nf];
            for c in j + 1..nf {
                let fac = plain[c + j * nf] / d;
                for i in c..nf {
                    plain[i + c * nf] -= plain[i + j * nf] * fac;
                }
            }
            for i in j + 1..nf {
                plain[i + j * nf] /= d;
            }
        }
        dense::ldl_partial(
            &mut f,
            nf,
            k,
            &mut dense::Work::default(),
            1,
            &mut |_, d| Some(d),
        )
        .unwrap();
        for j in 0..nf {
            for i in j..nf {
                let (x, y) = (f[i + j * nf], plain[i + j * nf]);
                assert!(
                    (x - y).abs() < 1e-12 * (1.0 + y.abs()),
                    "({i},{j}): {x} vs {y}"
                );
            }
        }
    }

    #[test]
    fn threads_give_the_same_numbers() {
        // Large enough that the tree splits into subtrees and the top's dense updates are split too.
        let (n, w) = (300, 6);
        let (adj, a) = test_matrix(n, w, 11);
        let pattern = Arc::new(Pattern::new(&adj, &vec![w; n]));
        let m = assemble(&pattern, &a, n, w);
        let split = pattern.split(4, |_| 1.0);
        let in_tasks: usize = split.tasks.iter().map(|t| t.len()).sum();
        assert!(split.tasks.len() > 4 && in_tasks < pattern.below.len());
        let (one, c1) = m.factorize_on(1).unwrap();
        let (four, c4) = m.factorize_on(4).unwrap();
        assert_eq!(one.l, four.l);
        assert_eq!(c1, c4);
        let diag0 = m.diagonal();
        let negative = |threads| {
            numeric(&m, Check::Inertia, false, &diag0, threads)
                .unwrap()
                .1
                .negative
        };
        assert_eq!(negative(1), negative(4));
        for nrhs in [1, 9] {
            let b: Vec<f64> = (0..n * w * nrhs)
                .map(|t| ((t * 37 + 11) % 101) as f64 / 50.0 - 1.0)
                .collect();
            let (mut x1, mut x4) = (b.clone(), b);
            one.solve_on(&mut x1, nrhs, 1);
            one.solve_on(&mut x4, nrhs, 4);
            assert_eq!(x1, x4);
        }
        // A front's update split by tile columns gives the same numbers as on one thread.
        let (nf, k) = (700, 70);
        let mut f = vec![0.0; nf * nf];
        for j in 0..nf {
            for i in j..nf {
                f[i + j * nf] = if i == j {
                    nf as f64
                } else {
                    ((i * 13 + j * 7) % 11) as f64 / 11.0 - 0.5
                };
            }
        }
        let mut g = f.clone();
        let mut work = dense::Work::default();
        dense::ldl_partial(&mut f, nf, k, &mut work, 1, &mut |_, d| Some(d)).unwrap();
        dense::ldl_partial(&mut g, nf, k, &mut work, 4, &mut |_, d| Some(d)).unwrap();
        assert_eq!(f, g);
    }

    #[test]
    fn work_estimate_covers_the_fronts_and_the_waiting_updates() {
        let (n, w) = (300, 6);
        let (adj, _) = test_matrix(n, w, 11);
        let p = Pattern::new(&adj, &vec![w; n]);
        let (_, largest) = p.supernodes();
        let one = p.work_bytes(1);
        // At least the largest front; at most that front and every update matrix at once.
        let updates: usize = p.below.iter().map(|b| b.len() * b.len()).sum();
        assert!(one >= 8 * largest * largest);
        assert!(one <= 8 * (largest * largest + updates));
        // The update matrices under the root's children wait together with a front.
        assert!(one > 8 * largest * largest);
        // More threads: fronts of subtrees worked on at the same time are added.
        assert!(p.work_bytes(4) > one);
        assert_eq!(p.work_bytes(0), one);
    }
}
