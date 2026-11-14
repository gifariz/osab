# B55 - P, V2, V3 and T releases with end offsets

Seven portals (6 m beam, 4 m columns, fixed bases), one per release set on the beam: P, V2, V3 or T at end i;
T and M3 at i with M2 at j; P and T at j; V2 at i with M3 at j. Beam offsets 0.25 m (i) and 0.5 m (j) with
rigid-zone factor 0.5; column tops offset 0.6 m with factor 0.75. Cases: GRAV 12 kN/m on the beam, LAT 15 kN
along X at the left top, OUT 8 kN along Y on the beam at 2 m.

The rule for M2/M3 (B48 D, B54) holds per bending plane: the whole offset at a released end is rigid in that
release's plane only (V2 or M3: the 1-2 plane; V3 or M2: the 1-3 plane), and P and T releases leave the
rigid-zone factor as it is; releases act at the clear-length ends. References: hand (total reactions).

Reference files: `hand.json` (21 values).
