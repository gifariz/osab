# B50 - Rigid diaphragm: one story, unequal columns, eccentric loads

Four fixed-base columns of 3.5 m with different sections (one turned 90 degrees) whose tops are joined only by
rigid diaphragm D1 (no beams, no slab; the model has no stories, so the tops' height is the level). LX: 100 kN
in X at one corner; LY: 60 kN in Y at another and a 40 kN m torque at a third. The diaphragm makes the tops
move together in UX, UY and RZ (two shifts and a twist) while each keeps its own UZ, RX and RY, so every
column is a cantilever.

References: hand (the 3 x 3 stiffness of the diaphragm's three DOFs), OpenSees (`rigidDiaphragm 3`).

Reference files: `hand.json` (80 values), `opensees.json` (240 values).
