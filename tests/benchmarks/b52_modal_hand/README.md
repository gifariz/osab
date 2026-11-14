# B52 - Modal analysis, hand cases

Three independent structures in one model (joint masses only; the material has no weight), modal case with 12
modes asked for (11 exist: one per mass DOF that can move):

- **a** A 4 m cantilever column (0.3 x 0.5) with 10 t at the top in X and Y: T = 2 pi sqrt(m h^3 / 3EI).
- **b** A three-story shear building: one column line with its floors held in rotation, 20, 20 and 12 t: the 3
  x 3 eigenproblem in X and in Y.
- **c** One story of four unequal cantilever columns joined by rigid diaphragm D1, with 15, 15 and 8 t at
  three tops and 30 t m2 about Z: the 3 x 3 eigenproblem in UX, UY and RZ.

Periods, participating mass ratios (over the free mass, rotations about the centre of all the mass) and mode
shapes at unit modal mass. References: hand, OpenSees (`eigen`, the same masses).

Reference files: `hand.json` (159 values), `opensees.json` (1,001 values).
