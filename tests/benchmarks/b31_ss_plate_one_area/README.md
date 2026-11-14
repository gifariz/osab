# B31 - Simply supported plate drawn as one floor (auto-mesh)

B20's 4 x 4 m Shell-Thin plate (t = 0.2 m, 10 kN/m^2 down), but drawn as ONE floor object and meshed by the
mesh step with a maximum size of 0.5 m: an 8 x 8 grid, B20's mesh. The four corners are restrained (ux, uy,
uz) and "add restraints on edges" passes that to every mesh joint on the edges, so the edges are simply
supported. In-plane restraints do not change bending here (no in-plane load).

Hand: total reaction 160 kN. OpenSees: B20's values (same mesh, ShellDKGQ), relabelled by coordinates.

Reference files: `hand.json` (1 values), `opensees.json` (437 values).
