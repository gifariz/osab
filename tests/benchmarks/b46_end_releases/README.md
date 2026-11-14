# B46 - Frame end releases

End releases (P V2 V3 T M2 M3 at end i and end j), steel, Euler-Bernoulli sections.

- **A** Two-bay frame in the X-Z plane (6 m bays, 4 m columns, fixed bases): beams B1, B2 pinned at both ends
  (M2, M3 released), a pinned brace D1 from the left base to the middle top. GRAV: 15 kN/m on the beams; LAT:
  20 kN in X at the left top.
- **B** Gerber beam at y = 10 m: B3 (fixed at joint 7) has M3 released at its end j, the hinge, and B4 rests
  on a roller at joint 9. GRAV: 30 kN at 3 m along B4.
- **C** Two beams between fixed ends at y = 20 m, a 6 kN m torque about X at the middle joint (LAT); B5 has T
  released at end i, so all of the torque goes through B6.

References: hand (statics of the released parts), OpenSees (elasticBeamColumn's moment releases; the torsion
release by a second node tied with equalDOF).

Reference files: `hand.json` (28 values), `opensees.json` (462 values).
