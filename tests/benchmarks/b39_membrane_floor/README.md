# B39 - Membrane floor: load transfer to beams, columns and a wall

An L-shaped 0.15 m Membrane floor (10 x 4 m plus 5 x 4 m) at z = 3.5 m drawn as ONE floor object, with 0.3 x
0.5 m beams on its edges except along the 5 m Shell-Thin wall (0.25 m, base fixed) under its edge y = 0, and
five 0.4 x 0.4 m columns (bases fixed). The mesh step cuts the floor at its beams and walls and, at the inside
corner, along local axis 1, into two panels of one element each; the wall's mesh joints and the inside corner
are tied to the panel edges by edge constraints. Vertical load goes to the panel edges by 45-degree tributary
areas: trapezoidal loads on the beams, the wall's share to its top joints, and the cut line's share half to
each of its two ends. DEAD: 5 kN/m^2 on the floor; WIND: 20 kN in X and 15 kN in Y at two corners (diaphragm
action); TRAP: a trapezoidal load on beam B2 from 4 kN/m at 0.5 m to 10 kN/m at 3.5 m.

Hand: base reaction totals.

Reference files: `hand.json` (4 values).
