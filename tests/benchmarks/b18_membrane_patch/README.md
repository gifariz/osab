# B18 - Membrane patch test

Five distorted Membrane areas (t = 0.2 m, concrete E = 25e6, nu = 0.2) fill a 2.4 x 1.2 m rectangle
(MacNeal-Harder style inside joints). Joint 1 is fixed; every joint has uz, rx, ry restrained.

- TENS: edge force F11 = 10 kN/m on the two short edges, as joint forces (0.6 m share each end).
- SHEAR: edge shear F12 = 5 kN/m on all four edges.

Hand values are exact (constant strain). OpenSees is not used: the in-plane rigid rotation is held only by the
drilling DOF of joint 1, which OpenSees's ShellDKGQ does not hold the same way.

Reference files: `hand.json` (174 values).
