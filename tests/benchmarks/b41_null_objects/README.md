# B41 - Null objects and Include in mesh (Rectangular mesh)

An 8 x 6 m 0.2 m Shell-Thin slab F1 at z = 3 m on five 0.5 x 0.5 m columns (bases fixed; four corners and one
at (4, 3)), Rectangular mesh 1.0 m, with null objects (no section: no stiffness, no elements): null frame N1
across the slab at x = 3.3 m and N2 inside it (y = 4.4 m) carrying line loads (partitions), and null area NA1
carrying a load (a storage zone). The slab is meshed along null frames (N1's and N2's end j lines are kept
like a column's, their end i gets local lines); null areas do not cut the mesh, and their loads, like the null
frames', go to the slab elements under them (shape functions). Joint p1 has only a joint load: (*Include in
mesh* = No, the default for a free joint) it is left out of the mesh and its load goes to the joints of the
element round it. Joint p2 has *Include in mesh* on, so its lines are kept like a column's. Beam S1 (a
stiffener, 0.4 x 0.25 m) has *Include in mesh* off: the slab is not meshed along it, and it is tied to the
slab where it crosses element edges (edge constraints). DEAD: 5 kN/m^2 on F1; SUPER: 6 kN/m on N1, 4 kN/m on
N2, 3 kN/m^2 on NA1, 12 kN at p1, 8 kN at p2; LAT: joint loads at two column tops. Rules:
docs/phases/PHASE3B_PLAN.md, probe findings 2h.

Hand: reaction totals.

Reference files: `hand.json` (4 values).
