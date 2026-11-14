# B20 - Simply supported square plate (Shell-Thin)

4 x 4 m, t = 0.2 m, concrete, uniform load 10 kN/m^2 down, 8 x 8 areas. Edges: uz = 0 (rotations free);
in-plane rigid motion held at two corners.

No hand file: the Navier solution is only reached as the mesh is refined; that is checked in
solver/tests/shell_convergence.rs (meshes 8, 16, 32 extrapolated).

Reference files: `opensees.json` (440 values).
