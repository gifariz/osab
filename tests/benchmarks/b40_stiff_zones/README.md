# B40 - Stiff zones over columns (Stiff slab type, Rectangular mesh)

An 8 x 6 m 0.2 m Shell-Thin slab F1 at z = 3 m on five 0.5 x 0.5 m columns (bases fixed), with a 2 x 2 m drop
panel P1 (0.35 m, Drop) over the interior column and 0.5 x 0.5 m Stiff areas (0.2 m, slab type Stiff: bending
100 times stiffer, docs/elements/STIFF_AREA.md) over three columns: K1 inside the drop panel, K2 and K3 at
slab corners reaching past the slab edges (like automatic rigid zones). Where objects overlap only one gets
elements: the object inside another (K1 in P1), else Stiff over Drop over Slab (K2, K3 over F1), else the
smaller one. Every object's load acts on its whole area. Rectangular mesh, 1.0 m: the Stiff areas' edges are
off the 1 m grid, so their lines only cut the cells next to them (local lines, joints tied by edge
constraints; docs/phases/PHASE3B_PLAN.md, probe findings 2g). DEAD: 5 kN/m^2 on F1; EXTRA: 2 kN/m^2 on P1, 4
kN/m^2 on K1 and K2; LAT: joint loads at two column tops.

Hand: reaction totals.

Reference files: `hand.json` (4 values).
