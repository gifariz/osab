# B70 - P-Delta from loads: two-story building with a wall

Two stories of 3.5 m and 3 m, 8 x 5 m: C50 columns at the corners and a C80 at (8, 2.5), B60x30 beams round
each floor, a 200 mm Shell-Thin wall (one element) at X = 0 in story 1, rigid diaphragm D1 at both floors;
joint masses 100 t at the corners and 300 t at (8, 5) on each floor (UX, UY); weight 25 kN/m³. Patterns DEAD
(self weight), MG (the joint masses times g as joint loads) and LAT (side loads). P-Delta iterative from
loads: 1.2 DEAD + 0.5 MG. Cases DEAD, MG, LAT; modal case (12 modes, joint masses only); spectrum cases X and
Y (B64's function FUNC, CQC, SRSS); combinations 1.2 DEAD + LAT, DEAD + X, and an envelope.

Checks: P-Delta from the frames and the wall reaching every case: displacements, reactions, frame and wall
forces, periods and mode shapes, spectrum results, combinations, base reactions, story drifts and story
forces.

Reference files: none published; `osab bench` runs the model to check that it solves.
