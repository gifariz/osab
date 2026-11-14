# B21 - Simply supported thick square plate (Shell-Thick)

B20's 4 x 4 m plate, 8 x 8 areas, but t = 0.8 m (span / thickness = 5, so the plate's shear deformation
matters) and Shell-Thick. Cases: Q = uniform 10 kN/m^2 down; P = 100 kN down at the centre joint. Edges: uz =
0 (rotations free); in-plane rigid motion held at two corners.

Hand: reaction totals. The rectangle recovery correction passes the existing per-value benchmark rule; worst
moment error is 7.47e-7 of the peak and 0.0277 times the allowed tolerance. This is approximate recovery, not
the exact general Shell-Thick formulation; see the review for held-out failures and shape limits. No OpenSees
twin: its thick plates (ShellMITC4, ShellNLDKGQ...) are different elements.

Reference files: `hand.json` (2 values).
