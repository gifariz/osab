//! Fill-reducing order of the joints for the sparse LDLᵀ factorization, and its symbolic analysis.
//!
//! Every joint carries several equations (up to 6), and its equations share one sparsity pattern, so the
//! order and the structure of the factor are worked out on the joint graph, each joint weighted by its
//! number of equations:
//! - approximate minimum degree on the quotient graph (Amestoy, Davis and Duff 1996; Davis, *Direct
//!   Methods for Sparse Linear Systems*, ch. 7), with supervariables, mass elimination and element
//!   absorption; joints of very high degree go last (George and Liu 1989);
//! - the elimination tree (Liu 1986, 1990) and its postorder, which leaves the fill unchanged;
//! - the factor's column structure by merging the children's structures up the tree, and supernodes:
//!   chains of joints whose columns share their structure below the diagonal block (Liu, Ng and Peyton
//!   1993), merged with a small parent when few zeros are added (Ashcraft and Grimes 1989).

use std::cmp::Reverse;
use std::collections::BinaryHeap;

const NONE: usize = usize::MAX;

/// The order in which to eliminate the vertices of a graph (`adj`: neighbours of each vertex, no self
/// loops, both directions listed) whose vertex `i` carries `weight[i]` >= 1 equations.
pub fn minimum_degree(adj: &[Vec<usize>], weight: &[usize]) -> Vec<usize> {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Var,
        Elem,
        Gone,
    }
    let n = adj.len();
    let mut state = vec![State::Var; n];
    // Vertices of very high degree (e.g. the master joint of a rigid floor) are ordered last.
    let dense = 16.max((10.0 * (n as f64).sqrt()) as usize);
    let mut last = Vec::new();
    for i in 0..n {
        if adj[i].len() > dense {
            state[i] = State::Gone;
            last.push(i);
        }
    }
    let mut nv: Vec<usize> = weight.to_vec();
    let mut members: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    let mut a: Vec<Vec<usize>> = adj
        .iter()
        .map(|l| {
            l.iter()
                .copied()
                .filter(|&j| state[j] == State::Var)
                .collect()
        })
        .collect();
    let mut e: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut le: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut deg: Vec<usize> = (0..n).map(|i| a[i].iter().map(|&j| nv[j]).sum()).collect();
    let mut remaining: usize = (0..n)
        .filter(|&i| state[i] == State::Var)
        .map(|i| nv[i])
        .sum();
    let mut heap: BinaryHeap<Reverse<(usize, usize)>> = (0..n)
        .filter(|&i| state[i] == State::Var)
        .map(|i| Reverse((deg[i], i)))
        .collect();
    let mut mark = vec![0usize; n];
    let mut stamp = 0usize;
    let mut w = vec![0isize; n];
    let mut wmark = vec![0usize; n];
    let mut order = Vec::with_capacity(n);

    while let Some(Reverse((d, p))) = heap.pop() {
        if state[p] != State::Var || d != deg[p] {
            continue;
        }
        // The new element p: its variables Lp are p's variables and those of the elements it absorbs.
        stamp += 1;
        mark[p] = stamp;
        let mut lp = Vec::new();
        for &el in &e[p] {
            if state[el] != State::Elem {
                continue;
            }
            for &v in &le[el] {
                if state[v] == State::Var && mark[v] != stamp {
                    mark[v] = stamp;
                    lp.push(v);
                }
            }
            state[el] = State::Gone;
            le[el] = Vec::new();
        }
        for &v in &a[p] {
            if state[v] == State::Var && mark[v] != stamp {
                mark[v] = stamp;
                lp.push(v);
            }
        }
        a[p] = Vec::new();
        e[p] = Vec::new();
        state[p] = State::Elem;
        order.extend_from_slice(&members[p]);
        remaining -= nv[p];
        nv[p] = 0;
        // Lp's variables now reach each other through p.
        for &i in &lp {
            e[i].retain(|&x| state[x] == State::Elem);
            e[i].push(p);
            a[i].retain(|&j| state[j] == State::Var && mark[j] != stamp);
        }
        // w(e) = weight of Le outside Lp, for every other element next to Lp; an element inside Lp is
        // absorbed into p.
        let mut touched = Vec::new();
        for &i in &lp {
            for &el in &e[i] {
                if el == p {
                    continue;
                }
                if wmark[el] != stamp {
                    wmark[el] = stamp;
                    le[el].retain(|&v| state[v] == State::Var);
                    w[el] = le[el].iter().map(|&v| nv[v] as isize).sum();
                    touched.push(el);
                }
                w[el] -= nv[i] as isize;
            }
        }
        for &el in &touched {
            if w[el] == 0 {
                state[el] = State::Gone;
                le[el] = Vec::new();
            }
        }
        // Supervariables: variables of Lp with the same elements and variables are merged.
        let mut keyed: Vec<(u64, usize)> = lp
            .iter()
            .map(|&i| {
                e[i].retain(|&x| state[x] == State::Elem);
                e[i].sort_unstable();
                a[i].sort_unstable();
                let h = e[i]
                    .iter()
                    .chain(&a[i])
                    .fold(0u64, |h, &x| h.wrapping_add(x as u64 + 1));
                (h, i)
            })
            .collect();
        keyed.sort_unstable();
        let mut g = 0;
        while g < keyed.len() {
            let mut end = g + 1;
            while end < keyed.len() && keyed[end].0 == keyed[g].0 {
                end += 1;
            }
            for x in g..end {
                let i = keyed[x].1;
                if state[i] != State::Var {
                    continue;
                }
                for y in x + 1..end {
                    let j = keyed[y].1;
                    if state[j] == State::Var && e[i] == e[j] && a[i] == a[j] {
                        nv[i] += nv[j];
                        nv[j] = 0;
                        let mj = std::mem::take(&mut members[j]);
                        members[i].extend(mj);
                        deg[i] = deg[i].saturating_sub(nv[i]);
                        state[j] = State::Gone;
                        a[j] = Vec::new();
                        e[j] = Vec::new();
                    }
                }
            }
            g = end;
        }
        // Mass elimination: a variable reached only through p goes with p.
        let mut kept = Vec::with_capacity(lp.len());
        for &i in &lp {
            if state[i] != State::Var {
                continue;
            }
            if a[i].is_empty() && e[i].len() == 1 {
                order.extend_from_slice(&members[i]);
                remaining -= nv[i];
                nv[i] = 0;
                state[i] = State::Gone;
                e[i] = Vec::new();
            } else {
                kept.push(i);
            }
        }
        // Approximate external degrees of Lp's variables.
        let lpw: usize = kept.iter().map(|&v| nv[v]).sum();
        for &i in &kept {
            let ext_a: usize = a[i]
                .iter()
                .filter(|&&j| state[j] == State::Var)
                .map(|&j| nv[j])
                .sum();
            let ext_e: isize = e[i]
                .iter()
                .filter(|&&x| x != p && state[x] == State::Elem)
                .map(|&x| w[x])
                .sum();
            let own = lpw - nv[i];
            let bound = ext_a + ext_e.max(0) as usize + own;
            let d = (remaining - nv[i]).min(deg[i] + own).min(bound);
            deg[i] = d;
            heap.push(Reverse((d, i)));
        }
        le[p] = kept;
    }
    order.extend(last);
    order
}

/// The symbolic factorization on the joint graph: the final order (a postorder of the elimination tree
/// with each supernode's joints together), the supernodes as ranges of positions, and the positions
/// below each supernode's diagonal block that its columns reach.
#[derive(Debug, Clone)]
pub struct Symbolic {
    /// Vertex at each position (the elimination order).
    pub order: Vec<usize>,
    /// Supernode `s` holds the positions `start[s]..start[s + 1]`.
    pub start: Vec<usize>,
    /// Positions below supernode `s`'s diagonal block that its columns reach, ascending.
    pub rows: Vec<Vec<usize>>,
    /// Parent supernode in the elimination tree (`None` at a root).
    pub parent: Vec<Option<usize>>,
}

/// When a child supernode may be merged into its parent: always while the merged block has at most
/// `small` columns (equations), otherwise while its zeros stay below `fraction` of its entries.
#[derive(Debug, Clone, Copy)]
pub struct Relax {
    pub small: usize,
    pub fraction: f64,
}

impl Relax {
    pub const NONE: Relax = Relax {
        small: 0,
        fraction: 0.0,
    };

    fn allows(&self, k: usize, zeros: usize, entries: usize) -> bool {
        k <= self.small || (zeros as f64) <= self.fraction * entries as f64
    }
}

/// A supernode while they are merged: its vertices in order (as positions of the first postorder), the
/// weight of its columns and of the rows below them, and the zeros merging has put into its block.
#[derive(Clone, Default)]
struct Group {
    cols: Vec<usize>,
    k: usize,
    m: usize,
    zeros: usize,
}

impl Symbolic {
    /// `order` is any elimination order of the graph (e.g. [`minimum_degree`]); it becomes a postorder
    /// of its elimination tree with each supernode's joints together, which keeps the fill but for the
    /// zeros that merging small supernodes adds (`relax`).
    pub fn new(adj: &[Vec<usize>], weight: &[usize], order: &[usize], relax: Relax) -> Symbolic {
        let n = adj.len();
        let mut pos = vec![NONE; n];
        for (k, &v) in order.iter().enumerate() {
            pos[v] = k;
        }
        let tree = etree(adj, order, &pos);
        let post = postorder(&tree);
        let mut renumber = vec![0; n];
        for (k, &old) in post.iter().enumerate() {
            renumber[old] = k;
        }
        let order: Vec<usize> = post.iter().map(|&k| order[k]).collect();
        for (k, &v) in order.iter().enumerate() {
            pos[v] = k;
        }
        let mut parent = vec![NONE; n];
        for (old, &p) in tree.iter().enumerate() {
            if p != NONE {
                parent[renumber[old]] = renumber[p];
            }
        }
        let mut children = vec![0usize; n];
        for &p in &parent {
            if p != NONE {
                children[p] += 1;
            }
        }
        // Column structures (positions below each joint): the graph's own, merged with the children's,
        // whose first entry is the parent itself; only their lengths are kept. A joint continues the
        // supernode of the joint before it when it is that joint's parent with no other child and their
        // structures match.
        let wt = |k: usize| weight[order[k]];
        let mut pending: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut cols: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut len = vec![0usize; n];
        let mut wlen = vec![0usize; n];
        let mut first = Vec::new();
        for k in 0..n {
            let mut s: Vec<usize> = adj[order[k]]
                .iter()
                .map(|&v| pos[v])
                .filter(|&i| i > k)
                .collect();
            s.sort_unstable();
            s.dedup();
            for c in std::mem::take(&mut pending[k]) {
                let cs = std::mem::take(&mut cols[c]);
                s = merge(&s, &cs[1..]);
            }
            len[k] = s.len();
            wlen[k] = s.iter().map(|&i| wt(i)).sum();
            let chained =
                k > 0 && parent[k - 1] == k && children[k] == 1 && len[k - 1] == len[k] + 1;
            if !chained {
                first.push(k);
            }
            if parent[k] != NONE {
                pending[parent[k]].push(k);
            }
            cols[k] = s;
        }
        drop(cols);
        // Fundamental supernodes and their tree.
        let ns = first.len();
        let mut group_of = vec![0usize; n];
        let mut groups: Vec<Group> = (0..ns)
            .map(|s| {
                let end = if s + 1 < ns { first[s + 1] } else { n };
                for k in first[s]..end {
                    group_of[k] = s;
                }
                Group {
                    cols: (first[s]..end).collect(),
                    k: (first[s]..end).map(wt).sum(),
                    m: wlen[end - 1],
                    zeros: 0,
                }
            })
            .collect();
        let mut sparent: Vec<usize> = (0..ns)
            .map(|s| {
                let last = first.get(s + 1).copied().unwrap_or(n) - 1;
                if parent[last] == NONE {
                    NONE
                } else {
                    group_of[parent[last]]
                }
            })
            .collect();
        // Small supernodes merge into their parent (children first): the child's columns go in front
        // of the parent's and take the parent's rows, which adds zeros to the child's columns.
        let mut kids: Vec<Vec<usize>> = vec![Vec::new(); ns];
        for s in 0..ns {
            if sparent[s] != NONE {
                kids[sparent[s]].push(s);
            }
        }
        let mut alive = vec![true; ns];
        for p in 0..ns {
            let mut merged_cols: Vec<usize> = Vec::new();
            let mut i = 0;
            while i < kids[p].len() {
                let c = kids[p][i];
                i += 1;
                let (gc, gp) = (&groups[c], &groups[p]);
                let added = gc.k * (gp.k + gp.m).saturating_sub(gc.m);
                let k = gc.k + gp.k;
                let entries = k * (k + 1) / 2 + k * gp.m;
                let zeros = gc.zeros + gp.zeros + added;
                if !relax.allows(k, zeros, entries) {
                    continue;
                }
                let gc = std::mem::take(&mut groups[c]);
                alive[c] = false;
                merged_cols.extend(gc.cols);
                groups[p].k = k;
                groups[p].zeros = zeros;
                // The merged child's children become the parent's (and may merge in their turn).
                let grand = std::mem::take(&mut kids[c]);
                for &g in &grand {
                    sparent[g] = p;
                }
                kids[p].extend(grand);
            }
            if !merged_cols.is_empty() {
                merged_cols.append(&mut groups[p].cols);
                groups[p].cols = merged_cols;
            }
        }
        // The final order: a postorder of the merged tree, each supernode's columns after its subtrees.
        let live: Vec<usize> = (0..ns).filter(|&s| alive[s]).collect();
        let mut index = vec![NONE; ns];
        for (i, &s) in live.iter().enumerate() {
            index[s] = i;
        }
        let tparent: Vec<usize> = live
            .iter()
            .map(|&s| {
                if sparent[s] == NONE {
                    NONE
                } else {
                    index[sparent[s]]
                }
            })
            .collect();
        let spost = postorder(&tparent);
        let mut final_order = Vec::with_capacity(n);
        let mut start = Vec::with_capacity(live.len() + 1);
        let mut snew = vec![0usize; live.len()];
        for (i, &t) in spost.iter().enumerate() {
            snew[t] = i;
            start.push(final_order.len());
            final_order.extend(groups[live[t]].cols.iter().map(|&k| order[k]));
        }
        start.push(n);
        let order = final_order;
        for (k, &v) in order.iter().enumerate() {
            pos[v] = k;
        }
        let sparent: Vec<Option<usize>> = spost
            .iter()
            .map(|&t| (tparent[t] != NONE).then(|| snew[tparent[t]]))
            .collect();
        // Rows below each supernode: its joints' neighbours beyond it and its children's rows beyond it.
        let nsup = start.len() - 1;
        let mut rows: Vec<Vec<usize>> = vec![Vec::new(); nsup];
        let mut pending: Vec<Vec<usize>> = vec![Vec::new(); nsup];
        for s in 0..nsup {
            let last = start[s + 1] - 1;
            let mut r: Vec<usize> = (start[s]..=last)
                .flat_map(|k| adj[order[k]].iter().map(|&v| pos[v]))
                .filter(|&i| i > last)
                .collect();
            r.sort_unstable();
            r.dedup();
            for c in std::mem::take(&mut pending[s]) {
                let beyond: Vec<usize> = rows[c].iter().copied().filter(|&i| i > last).collect();
                r = merge(&r, &beyond);
            }
            if let Some(p) = sparent[s] {
                pending[p].push(s);
            }
            rows[s] = r;
        }
        Symbolic {
            order,
            start,
            rows,
            parent: sparent,
        }
    }

    /// Entries of the factor and multiply-add pairs of the factorization, counting equations (`weight`
    /// of each vertex).
    pub fn cost(&self, weight: &[usize]) -> (usize, f64) {
        let mut entries = 0;
        let mut ops = 0.0;
        for s in 0..self.rows.len() {
            let k: usize = (self.start[s]..self.start[s + 1])
                .map(|i| weight[self.order[i]])
                .sum();
            let m: usize = self.rows[s].iter().map(|&i| weight[self.order[i]]).sum();
            entries += k * (k + 1) / 2 + k * m;
            for t in 0..k {
                let c = (k - 1 - t + m) as f64;
                ops += c * c / 2.0;
            }
        }
        (entries, ops)
    }
}

/// Elimination tree of the graph in the order `order` (`pos` = position of each vertex): the parent
/// position of every position, `NONE` at roots.
fn etree(adj: &[Vec<usize>], order: &[usize], pos: &[usize]) -> Vec<usize> {
    let n = order.len();
    let mut parent = vec![NONE; n];
    let mut anc = vec![NONE; n];
    for k in 0..n {
        for &v in &adj[order[k]] {
            let mut r = pos[v];
            if r >= k {
                continue;
            }
            while anc[r] != NONE && anc[r] != k {
                let t = anc[r];
                anc[r] = k;
                r = t;
            }
            if anc[r] == NONE {
                anc[r] = k;
                parent[r] = k;
            }
        }
    }
    parent
}

/// Postorder of a forest given by parents (`NONE` at roots): children in ascending order.
fn postorder(parent: &[usize]) -> Vec<usize> {
    let n = parent.len();
    let mut head = vec![NONE; n];
    let mut next = vec![NONE; n];
    for k in (0..n).rev() {
        if parent[k] != NONE {
            next[k] = head[parent[k]];
            head[parent[k]] = k;
        }
    }
    let mut out = Vec::with_capacity(n);
    let mut stack = Vec::new();
    for root in (0..n).filter(|&k| parent[k] == NONE) {
        stack.push(root);
        while let Some(&top) = stack.last() {
            let c = head[top];
            if c == NONE {
                stack.pop();
                out.push(top);
            } else {
                head[top] = next[c];
                stack.push(c);
            }
        }
    }
    out
}

/// Union of two ascending lists.
fn merge(a: &[usize], b: &[usize]) -> Vec<usize> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => {
                out.push(a[i]);
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                out.push(b[j]);
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                out.push(a[i]);
                i += 1;
                j += 1;
            }
        }
    }
    out.extend_from_slice(&a[i..]);
    out.extend_from_slice(&b[j..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grid of `nx` x `ny` vertices joined like 4-node elements (each to its 8 neighbours).
    fn grid(nx: usize, ny: usize) -> Vec<Vec<usize>> {
        let id = |i: usize, j: usize| i * ny + j;
        let mut adj = vec![Vec::new(); nx * ny];
        for i in 0..nx {
            for j in 0..ny {
                for (di, dj) in [
                    (-1i64, -1i64),
                    (-1, 0),
                    (-1, 1),
                    (0, -1),
                    (0, 1),
                    (1, -1),
                    (1, 0),
                    (1, 1),
                ] {
                    let (a, b) = (i as i64 + di, j as i64 + dj);
                    if a >= 0 && b >= 0 && (a as usize) < nx && (b as usize) < ny {
                        adj[id(i, j)].push(id(a as usize, b as usize));
                    }
                }
            }
        }
        adj
    }

    /// The structure below each column of the factor, by eliminating one vertex at a time (dense sets).
    fn brute_structure(adj: &[Vec<usize>], order: &[usize]) -> Vec<Vec<usize>> {
        let n = adj.len();
        let mut pos = vec![0; n];
        for (k, &v) in order.iter().enumerate() {
            pos[v] = k;
        }
        let mut set: Vec<std::collections::BTreeSet<usize>> = (0..n)
            .map(|k| {
                adj[order[k]]
                    .iter()
                    .map(|&v| pos[v])
                    .filter(|&i| i > k)
                    .collect()
            })
            .collect();
        for k in 0..n {
            let s: Vec<usize> = set[k].iter().copied().collect();
            if let Some(&p) = s.first() {
                for &i in &s[1..] {
                    set[p].insert(i);
                }
            }
        }
        set.into_iter().map(|s| s.into_iter().collect()).collect()
    }

    #[test]
    fn minimum_degree_is_a_permutation_with_less_fill_than_the_natural_order() {
        let adj = grid(40, 30);
        let w = vec![6; adj.len()];
        let order = minimum_degree(&adj, &w);
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..adj.len()).collect::<Vec<_>>());
        let natural: Vec<usize> = (0..adj.len()).collect();
        let fill = |o: &[usize]| brute_structure(&adj, o).iter().map(Vec::len).sum::<usize>();
        let (md, nat) = (fill(&order), fill(&natural));
        assert!(md * 10 < nat * 7, "minimum degree {md}, natural {nat}");
        // `cost` counts the same entries (with the diagonal) when no supernodes are merged.
        let one = vec![1; adj.len()];
        let sym = Symbolic::new(&adj, &one, &order, Relax::NONE);
        assert_eq!(sym.cost(&one).0, md + adj.len());
        // On a larger grid the gain grows (banded fill grows faster than minimum degree's).
        let adj = grid(150, 150);
        let one = vec![1; adj.len()];
        let natural: Vec<usize> = (0..adj.len()).collect();
        let md = Symbolic::new(&adj, &one, &minimum_degree(&adj, &one), Relax::NONE).cost(&one);
        let nat = Symbolic::new(&adj, &one, &natural, Relax::NONE).cost(&one);
        assert!(md.0 * 3 < nat.0 && md.1 * 4.0 < nat.1, "{md:?} vs {nat:?}");
    }

    #[test]
    fn supernodes_hold_every_column_structure() {
        // Each column's structure (by brute force, in the final order) lies inside its supernode's
        // columns after it and the supernode's rows; with no merging it is exactly that.
        for relax in [
            Relax::NONE,
            Relax {
                small: 12,
                fraction: 0.2,
            },
        ] {
            let mut adj = grid(9, 7);
            // A vertex joined to many others (a rigid floor's master joint) and an isolated one.
            let hub = adj.len();
            adj.push((0..hub).step_by(3).collect());
            for v in (0..hub).step_by(3) {
                adj[v].push(hub);
            }
            adj.push(Vec::new());
            let w: Vec<usize> = (0..adj.len()).map(|i| 1 + i % 6).collect();
            let order = minimum_degree(&adj, &w);
            let sym = Symbolic::new(&adj, &w, &order, relax);
            let truth = brute_structure(&adj, &sym.order);
            let n = adj.len();
            assert_eq!(*sym.start.last().unwrap(), n);
            for s in 0..sym.rows.len() {
                let (a, b) = (sym.start[s], sym.start[s + 1]);
                for k in a..b {
                    let mut claimed: Vec<usize> = (k + 1..b).collect();
                    claimed.extend(&sym.rows[s]);
                    assert!(truth[k].iter().all(|i| claimed.contains(i)), "column {k}");
                    if relax.small == 0 {
                        assert_eq!(truth[k], claimed, "column {k}");
                    }
                }
                // The parent supernode holds the first row.
                if let Some(&r) = sym.rows[s].first() {
                    let p = sym.parent[s].expect("a parent");
                    assert!(sym.start[p] <= r && r < sym.start[p + 1]);
                }
            }
        }
    }
}
