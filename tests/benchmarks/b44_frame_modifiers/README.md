# B44 - Frame property modifiers

Frame property modifiers (A, As2, As3, J, I22, I33) on sections and on frame objects; a frame uses its
section's modifiers times its own.

- **C1**: a 3.5 m concrete cantilever (0.4 x 0.6 m section with shear areas), cracked-section modifiers on its
  section (A 0.8, As2 = As3 0.6, J 0.25, I22 = I33 0.7, mass and weight 0.9) and its own I22 0.5, J 0.4. TIP:
  8 kN in X, -5 kN in Y, 120 kN down and a 3 kN m torque at the top.
- **Portal** at y = 8 m: two 4 m columns (0.5 x 0.5, section I22 = I33 0.7, C3 also its own 0.5) and a 6 m
  beam (0.3 x 0.6, section I 0.35 and J 0.1, its own A 0.5), fixed bases. GRAV: 20 kN/m on the beam. LAT: 15
  kN in X at the left top, 6 kN in Y at the right top.

References: hand (closed form for C1, statics for the portal), OpenSees (elasticBeamColumn /
ElasticTimoshenkoBeam with the modified section values).

Reference files: `hand.json` (21 values), `opensees.json` (306 values).
