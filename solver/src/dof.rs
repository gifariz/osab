//! Degree-of-freedom numbering. The joints with equations are put in a fill-reducing order
//! ([`crate::sparse::Pattern`]), so the factor of the stiffness stays sparse; each joint's free DOFs
//! get consecutive equation numbers in that order.

use std::sync::Arc;

use crate::sparse::Pattern;

pub const NDF: usize = 6;

pub struct DofMap {
    /// Equation number of DOF `node * 6 + d`, or `None` when restrained.
    pub eq: Vec<Option<usize>>,
    pub n_eq: usize,
    /// Where the stiffness and its factor have entries, in these equation numbers.
    pub pattern: Arc<Pattern>,
}

impl DofMap {
    /// `no_equation[node * 6 + d]` (restrained, or carried by another joint); `connections` = pairs of
    /// joints that share an element.
    pub fn new(n_nodes: usize, no_equation: &[bool], connections: &[[usize; 2]]) -> Self {
        // The graph of the joints with equations, each weighted by its number of equations.
        let free = |n: usize| (0..NDF).filter(move |&d| !no_equation[n * NDF + d]);
        let mut vertex = vec![None; n_nodes];
        let mut nodes = Vec::new();
        let mut weight = Vec::new();
        for n in 0..n_nodes {
            let w = free(n).count();
            if w > 0 {
                vertex[n] = Some(nodes.len());
                nodes.push(n);
                weight.push(w);
            }
        }
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
        for &[a, b] in connections {
            if let (Some(va), Some(vb)) = (vertex[a], vertex[b]) {
                if va != vb {
                    adj[va].push(vb);
                    adj[vb].push(va);
                }
            }
        }
        for list in &mut adj {
            list.sort_unstable();
            list.dedup();
        }
        let pattern = Arc::new(Pattern::new(&adj, &weight));
        let mut eq = vec![None; n_nodes * NDF];
        for (v, &n) in nodes.iter().enumerate() {
            for (i, d) in free(n).enumerate() {
                eq[n * NDF + d] = Some(pattern.first_equation(v) + i);
            }
        }
        DofMap {
            eq,
            n_eq: pattern.len(),
            pattern,
        }
    }

    /// Node and local DOF index for an equation number.
    pub fn locate(&self, equation: usize) -> (usize, usize) {
        let k = self
            .eq
            .iter()
            .position(|e| *e == Some(equation))
            .expect("valid equation");
        (k / NDF, k % NDF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_free_dofs_only() {
        let mut restrained = vec![false; 3 * NDF];
        for d in 0..NDF {
            restrained[d] = true; // node 0 fixed
        }
        let map = DofMap::new(3, &restrained, &[[0, 1], [1, 2]]);
        assert_eq!(map.n_eq, 12);
        assert!(map.eq[..NDF].iter().all(|e| e.is_none()));
        let (node, d) = map.locate(map.eq[2 * NDF + 4].unwrap());
        assert_eq!((node, d), (2, 4));
    }

    #[test]
    fn every_equation_once_and_each_joint_together() {
        // A 5 x 4 grid of joints, a few DOFs restrained, one joint with no equation at all.
        let (nx, ny) = (5, 4);
        let mut connections = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                let n = j * nx + i;
                if i + 1 < nx {
                    connections.push([n, n + 1]);
                }
                if j + 1 < ny {
                    connections.push([n, n + nx]);
                }
            }
        }
        let n_nodes = nx * ny;
        let mut no_equation = vec![false; n_nodes * NDF];
        for d in [0, 2, 4] {
            no_equation[3 * NDF + d] = true;
        }
        for d in 0..NDF {
            no_equation[7 * NDF + d] = true;
        }
        let map = DofMap::new(n_nodes, &no_equation, &connections);
        assert_eq!(map.n_eq, n_nodes * NDF - 3 - NDF);
        let mut seen = vec![false; map.n_eq];
        for n in 0..n_nodes {
            let eqs: Vec<usize> = (0..NDF).filter_map(|d| map.eq[n * NDF + d]).collect();
            for w in eqs.windows(2) {
                assert_eq!(w[1], w[0] + 1);
            }
            for e in eqs {
                assert!(!seen[e]);
                seen[e] = true;
            }
        }
        assert!(seen.iter().all(|s| *s));
    }
}
