# B38 - Overlapping floors: a drop panel and a slab band (Rectangular mesh)

An 8 x 6 m 0.2 m Shell-Thin slab F1 at z = 3 m with a 2 x 2 m drop panel P1 (0.35 m, slab type Drop) over an
interior column and a 1 m wide slab band S1 (0.45 m) across the floor, on five 0.5 x 0.5 m columns (bases
fixed). Where objects overlap only one gets elements: Drop over Slab, else the smaller area, so P1 keeps its
whole square, S1 everything but its overlap with P1, F1 the rest; the meshes share their joints. Every
object's load acts on its whole area. Rectangular mesh, 1.0 m. DEAD: 5 kN/m^2 on F1; EXTRA: 2 kN/m^2 on P1 and
3 kN/m^2 on S1.

Hand: reaction totals.

Reference files: `hand.json` (2 values).
