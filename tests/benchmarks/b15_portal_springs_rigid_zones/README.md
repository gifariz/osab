# B15 Portal frame on spring bases with rigid end zones (2D)
Columns 4 m at x = 0 and 6 m; the beam runs from x = 0.2 to 5.8 m and rigid links join it to the column tops
(rigid end zones, like a column half-width). Bases: UZ restrained, springs in UX (30000 kN/m) and RY (25000 kN
m/rad). DEAD: 15 kN/m on the beam; WIND: 5 kN at each column top. This is the model the desktop click test
builds. Hand values from symmetry; OpenSees give the rest.

Reference files: `hand.json` (9 values), `opensees.json` (84 values).
