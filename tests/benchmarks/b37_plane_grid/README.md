# B37 - Plane grid (slab, X-Y): B32's floor on point supports

B32's 10 x 8 m Shell-Thin floor (t = 0.2 m) with its 0.3 x 0.5 m beams, analysed as a plane grid
(`"dimension": "grid"`: only UZ, RX and RY are active). B32's nine column tops are point supports (pinned).
The mesh step meshes it as B32 (4 x 4 elements per bay, beams split). DEAD: 5 kN/m^2; LIVE: 10 kN/m on beam B1
and 20 kN on a joint inside the floor (it only carries the load, so it is left out of the mesh and its load
goes into the floor element round it); MOM: moments MX 10, MY -15 kN m on the middle support joint.

Hand: reaction totals. The 3D run of the same model is the same (a flat floor's bending does not depend on its
in-plane part): `mesh/tests/plane_grid.rs`. OpenSees: OSAB's mesh as plain elements (ShellDKGQ, whose bending
is DKQ as OSAB's Shell-Thin; elastic beams), with UX, UY, RZ held.

Reference files: `hand.json` (3 values), `opensees.json` (972 values).
