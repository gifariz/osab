# B56 - One-way Membrane floors on beams, walls and a free edge

Six 8 x 4 m Membrane floors (0.2 m) at z = 3 m with *Use special one-way load distribution* (`"one_way":
true`): each spans along its local axis 1 as strips, half of each strip's load to each end
(docs/elements/ONE_WAY_LOAD.md). Beams 0.3 x 0.5 m, columns 0.4 x 0.4 m and Shell-Thin walls 0.25 m (meshed
1.25 m, bases fixed) hold the edges:

- **A** beams on all four edges, local axis 1 along X: the strips go to the 4 m beams.
- **B** as A with local axes at 30 degrees: every beam gets part, as l / 2 x |sin a|.
- **W** walls under the 4 m edges, beams on the 8 m edges, axis 1 along X: the strips end on the walls.
- **V** as W with local axes at 90 degrees: the strips go to the beams; the walls get nothing along them.
- **S** as W with local axes at 30 degrees: walls and beams share.
- **K** beams on three edges, none on y = 4, axis 1 along Y: the strips end at the free edge, whose load goes
  to its two end joints.

DEAD: 5 kN/m^2 on every floor; SW: self-weight at 24 kN/m^3 (the floors' weight goes the same way). Hand: the
base reaction total.

Reference files: `hand.json` (1 values).
