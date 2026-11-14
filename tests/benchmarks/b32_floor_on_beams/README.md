# B32 - Floor on beams and columns, one floor object (auto-mesh)

A 10 x 8 m Shell-Thin floor (t = 0.2 m) at z = 3.5 m drawn as ONE floor object over 2 x 2 bays, with 0.3 x 0.5
m beams on the grid lines and nine 0.4 x 0.4 m columns (bases fixed). The mesh step cuts the floor at the
beams, gives each 5 x 4 m bay a 4 x 4 grid (1.25 x 1.0 m) and splits the beams at the mesh joints. DEAD: 5
kN/m^2 down; WIND: 20 kN in X and 10 kN in Y at a corner.

Hand: base reaction totals.

Reference files: `hand.json` (3 values), `opensees.json` (180 values).
