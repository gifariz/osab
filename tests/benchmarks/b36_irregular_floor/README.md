# B36 - Irregular floor with grading (General mesh)

A Shell-Thin floor (t = 0.2 m) at z = 3 m with two skew edges meeting at a re-entrant corner (4, 3.5) and a 1
x 0.8 m opening near that corner. Simply supported along the edges y = 0 and x = 0 ("add restraints on edges")
and at the re-entrant corner; the corners (9, 3) and (3.5, 8) are free. General mesh, maximum size 1.0 m,
grading on (the default): elements are half size at the re-entrant corner and at the opening's corners and
grow back to the full size over two elements. DEAD: 6 kN/m^2; PT: 20 kN down at joint p1 (6, 1.5), which is
included in the mesh.

Hand: reaction totals. OpenSees: OSAB's mesh as plain ShellDKGQ / ShellDKGT elements (no in-plane load, so
only the plate bending acts, DKQ / DKT as in OSAB): every value.

Reference files: `hand.json` (2 values), `opensees.json` (1,004 values).
