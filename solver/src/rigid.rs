//! Rigid bodies: joints joined by rigid links move as one rigid body (master/slave constraint).
//!
//! One joint of each body is its master; every other joint (a slave) follows it exactly:
//! u_slave = u_master + θ_master × r and θ_slave = θ_master, with r = x_slave − x_master.
//! The analysis keeps equations for masters only and carries members, springs and loads at slaves over
//! to the master with the 6 × 6 matrix of [`transform`]. No stiff numbers are involved, so the
//! constraint is exact (like a link with all DOFs fixed).

use std::collections::{HashMap, VecDeque};

use crate::axes::{self, Vec3};
use crate::model::{coords, Id, Model};
use crate::Error;

pub struct Bodies {
    /// Each node's carrier: the master of its rigid body, or the node itself.
    pub carrier: Vec<usize>,
    /// Offset of each node from its carrier (zero for carriers).
    pub offset: Vec<Vec3>,
    /// `(slave, link, parent)` for every slave, breadth-first from its master: `link` (an index into
    /// `model.links`) joins the slave to `parent`, which is the master or an earlier slave.
    pub tree: Vec<(usize, usize, usize)>,
}

impl Bodies {
    /// Builds the rigid bodies of a model whose links refer to existing joints. Fails when links form
    /// a closed loop or when more than one joint of a body has restraints.
    pub fn new(model: &Model) -> Result<Bodies, Error> {
        let n = model.nodes.len();
        let index: HashMap<&Id, usize> = model
            .nodes
            .iter()
            .enumerate()
            .map(|(i, nd)| (&nd.id, i))
            .collect();
        let mut root: Vec<usize> = (0..n).collect();
        fn find(root: &mut [usize], mut x: usize) -> usize {
            while root[x] != x {
                root[x] = root[root[x]];
                x = root[x];
            }
            x
        }
        let mut adj: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
        for (k, l) in model.links.iter().enumerate() {
            let (Some(&a), Some(&b)) = (index.get(&l.nodes[0]), index.get(&l.nodes[1])) else {
                return Err(Error::Invalid(format!("link {}: unknown node", l.id)));
            };
            let (ra, rb) = (find(&mut root, a), find(&mut root, b));
            if ra == rb {
                return Err(Error::Invalid(format!(
                    "rigid link {} closes a loop: joints {} and {} are already joined rigidly by other \
                     links (remove one link of the loop; the body stays rigid)",
                    l.id, l.nodes[0], l.nodes[1]
                )));
            }
            root[ra] = rb;
            adj[a].push((b, k));
            adj[b].push((a, k));
        }

        let mut restrained = vec![false; n];
        for s in &model.supports {
            if let (Some(&i), Ok(r)) = (index.get(&s.node), s.restraints()) {
                restrained[i] |= r.iter().any(|x| *x);
            }
        }
        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in (0..n).filter(|&i| !adj[i].is_empty()) {
            groups.entry(find(&mut root, i)).or_default().push(i);
        }
        let mut masters: Vec<usize> = Vec::new();
        for members in groups.values() {
            let held: Vec<usize> = members.iter().copied().filter(|&i| restrained[i]).collect();
            if held.len() > 1 {
                return Err(Error::Invalid(format!(
                    "joints {} and {} are joined by rigid links and both have restraints; \
                     a rigid body can have restraints at one joint only",
                    model.nodes[held[0]].id, model.nodes[held[1]].id
                )));
            }
            // The restrained joint is the master, so its restraints act directly on the body.
            masters.push(held.first().copied().unwrap_or(members[0]));
        }
        masters.sort_unstable();

        let mut carrier: Vec<usize> = (0..n).collect();
        let mut offset = vec![[0.0; 3]; n];
        let mut tree = Vec::new();
        let mut seen = vec![false; n];
        for m in masters {
            seen[m] = true;
            let xm = coords(&model.nodes[m]);
            let mut queue = VecDeque::from([m]);
            while let Some(v) = queue.pop_front() {
                for &(w, k) in &adj[v] {
                    if !seen[w] {
                        seen[w] = true;
                        carrier[w] = m;
                        offset[w] = axes::sub(coords(&model.nodes[w]), xm);
                        tree.push((w, k, v));
                        queue.push_back(w);
                    }
                }
            }
        }
        Ok(Bodies {
            carrier,
            offset,
            tree,
        })
    }

    pub fn is_slave(&self, node: usize) -> bool {
        self.carrier[node] != node
    }
}

pub type Mat6 = [[f64; 6]; 6];

/// How every joint's displacements follow the independent ones: u_node = Σ T u_carrier over its parts.
/// A free joint is its own carrier; a rigid-body slave has one part (its master, [`transform`]); a
/// diaphragm slave two: itself for UZ, RX, RY and its master for UX, UY, RZ ([`in_plane`]).
pub struct Ties {
    /// Parts of each joint; `None` = the joint itself.
    pub parts: Vec<Option<Vec<(usize, Mat6)>>>,
    /// Per DOF (node * 6 + d): whether it is independent (gets an equation unless restrained).
    pub own: Vec<bool>,
}

impl Ties {
    /// Ties of rigid-link bodies and of diaphragm levels (the master first; none in a plane grid).
    pub fn new(
        n: usize,
        bodies: &Bodies,
        levels: &[crate::diaphragm::Level],
        x: impl Fn(usize) -> Vec3,
    ) -> Ties {
        let mut parts: Vec<Option<Vec<(usize, Mat6)>>> = vec![None; n];
        let mut own = vec![true; n * 6];
        for s in (0..n).filter(|&s| bodies.is_slave(s)) {
            parts[s] = Some(vec![(bodies.carrier[s], transform(bodies.offset[s]))]);
            own[s * 6..s * 6 + 6].fill(false);
        }
        for l in levels {
            let m = l.joints[0];
            for &s in &l.joints[1..] {
                let mut keep = [[0.0; 6]; 6];
                for d in [2, 3, 4] {
                    keep[d][d] = 1.0;
                    own[s * 6 + d] = true;
                }
                for d in [0, 1, 5] {
                    own[s * 6 + d] = false;
                }
                parts[s] = Some(vec![(s, keep), (m, in_plane(axes::sub(x(s), x(m))))]);
            }
        }
        Ties { parts, own }
    }

    /// A stiffness matrix over `nodes` (6 DOFs each, row-major) carried over to their carriers: Bᵀ K B.
    /// Returns the carriers (each once) and the matrix over them.
    pub fn carry(&self, nodes: &[usize], k: Vec<f64>) -> (Vec<usize>, Vec<f64>) {
        if nodes.iter().all(|&n| self.parts[n].is_none()) {
            return (nodes.to_vec(), k);
        }
        let mut carriers: Vec<usize> = Vec::new();
        let mut slot = |c: usize| match carriers.iter().position(|&x| x == c) {
            Some(i) => i,
            None => {
                carriers.push(c);
                carriers.len() - 1
            }
        };
        let identity = transform([0.0; 3]);
        let maps: Vec<Vec<(usize, Mat6)>> = nodes
            .iter()
            .map(|&n| match &self.parts[n] {
                None => vec![(slot(n), identity)],
                Some(p) => p.iter().map(|&(c, t)| (slot(c), t)).collect(),
            })
            .collect();
        let (s, m) = (6 * nodes.len(), 6 * carriers.len());
        // K B
        let mut kb = vec![0.0; s * m];
        for (bn, map) in maps.iter().enumerate() {
            for (ci, t) in map {
                for i in 0..s {
                    let row = &k[i * s + 6 * bn..i * s + 6 * bn + 6];
                    for c in 0..6 {
                        kb[i * m + 6 * ci + c] += (0..6).map(|r| row[r] * t[r][c]).sum::<f64>();
                    }
                }
            }
        }
        // Bᵀ (K B)
        let mut out = vec![0.0; m * m];
        for (bn, map) in maps.iter().enumerate() {
            for (ci, t) in map {
                for c in 0..6 {
                    for j in 0..m {
                        out[(6 * ci + c) * m + j] += (0..6)
                            .map(|r| t[r][c] * kb[(6 * bn + r) * m + j])
                            .sum::<f64>();
                    }
                }
            }
        }
        (carriers, out)
    }

    /// Forces at a node, as they act on its carriers (Tᵀ f for each part).
    pub fn carried(&self, n: usize, f: &[f64]) -> Vec<(usize, [f64; 6])> {
        match &self.parts[n] {
            None => vec![(n, f.try_into().expect("6 values"))],
            Some(p) => p
                .iter()
                .map(|(c, t)| (*c, apply_transposed(t, f)))
                .collect(),
        }
    }

    /// Fills in the displacements of the tied nodes from their carriers'.
    pub fn place(&self, u: &mut [f64]) {
        for (n, p) in self.parts.iter().enumerate() {
            let Some(p) = p else { continue };
            let mut un = [0.0; 6];
            for (c, t) in p {
                let v = apply(t, &u[c * 6..c * 6 + 6]);
                for d in 0..6 {
                    un[d] += v[d];
                }
            }
            u[n * 6..n * 6 + 6].copy_from_slice(&un);
        }
    }
}

/// The in-plane part of a rigid diaphragm, for a joint at offset `r` from its master: UX, UY and RZ follow
/// the master's (u = u_m - θz r_y, v = v_m + θz r_x); UZ, RX, RY rows are zero.
pub fn in_plane(r: Vec3) -> Mat6 {
    let mut t = [[0.0; 6]; 6];
    t[0][0] = 1.0;
    t[0][5] = -r[1];
    t[1][1] = 1.0;
    t[1][5] = r[0];
    t[5][5] = 1.0;
    t
}

/// 6 × 6 matrix T with u_node = T u_carrier, for a node at offset `r` from its carrier:
/// translations get θ × r added, rotations are equal.
pub fn transform(r: Vec3) -> [[f64; 6]; 6] {
    let mut t = [[0.0; 6]; 6];
    for (d, row) in t.iter_mut().enumerate() {
        row[d] = 1.0;
    }
    // θ × r = (θy rz − θz ry, θz rx − θx rz, θx ry − θy rx)
    t[0][4] = r[2];
    t[0][5] = -r[1];
    t[1][3] = -r[2];
    t[1][5] = r[0];
    t[2][3] = r[1];
    t[2][4] = -r[0];
    t
}

/// T u: carrier displacements -> node displacements.
pub fn apply(t: &[[f64; 6]; 6], u: &[f64]) -> [f64; 6] {
    let mut out = [0.0; 6];
    for (r, o) in out.iter_mut().enumerate() {
        *o = (0..6).map(|c| t[r][c] * u[c]).sum();
    }
    out
}

/// Tᵀ f: forces at the node -> the same forces acting on the carrier (moved to its position).
pub fn apply_transposed(t: &[[f64; 6]; 6], f: &[f64]) -> [f64; 6] {
    let mut out = [0.0; 6];
    for (c, o) in out.iter_mut().enumerate() {
        *o = (0..6).map(|r| t[r][c] * f[r]).sum();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_moves_with_the_rotation() {
        // Rotation about Z at the master moves a slave at +X along +Y.
        let t = transform([2.0, 0.0, 0.0]);
        let u = apply(&t, &[0.0, 0.0, 0.0, 0.0, 0.0, 0.1]);
        assert_eq!(u, [0.0, 0.2, 0.0, 0.0, 0.0, 0.1]);
        // A force at the slave gives the master the same force plus r x F.
        let f = apply_transposed(&t, &[0.0, 0.0, -5.0, 0.0, 0.0, 0.0]);
        assert_eq!(f, [0.0, 0.0, -5.0, 0.0, 10.0, 0.0]);
    }

    #[test]
    fn carry_moves_stiffness_to_the_carriers() {
        let free = Ties {
            parts: vec![None, None],
            own: vec![true; 12],
        };
        let k: Vec<f64> = (0..144).map(|v| v as f64).collect();
        assert_eq!(free.carry(&[0, 1], k.clone()), (vec![0, 1], k));
        // Spring of stiffness 3 in UY at a rigid-body slave 2 m along X: master gets k (UY, RZ) coupling.
        let mut d = vec![0.0; 36];
        d[6 + 1] = 3.0;
        let body = Ties {
            parts: vec![None, Some(vec![(0, transform([2.0, 0.0, 0.0]))])],
            own: [vec![true; 6], vec![false; 6]].concat(),
        };
        let (nodes, kc) = body.carry(&[1], d.clone());
        assert_eq!(nodes, vec![0]);
        assert_eq!((kc[6 + 1], kc[6 + 5], kc[5 * 6 + 5]), (3.0, 6.0, 12.0));
        // The same spring at a diaphragm slave: UY and RZ go to the master, UZ, RX, RY stay its own.
        let mut keep = [[0.0; 6]; 6];
        for k in [2, 3, 4] {
            keep[k][k] = 1.0;
        }
        let dia = Ties {
            parts: vec![None, Some(vec![(1, keep), (0, in_plane([2.0, 0.0, 0.0]))])],
            own: vec![true; 12],
        };
        let (nodes, kc) = dia.carry(&[1], d);
        assert_eq!(nodes, vec![1, 0]);
        assert_eq!(
            (kc[7 * 12 + 7], kc[7 * 12 + 11], kc[11 * 12 + 11]),
            (3.0, 6.0, 12.0)
        );
        assert!(kc[..72].iter().all(|v| *v == 0.0));
    }
}
