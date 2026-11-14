# B69 - P-Delta from the mass: columns, masses at two heights and between them, element mass

Separate structures 10 m apart, E 25e6; columns 0.3 x 0.6 (the 0.6 depth along X). P-Delta non-iterative from
the mass; mass source joint masses and element mass (only KSW's material weighs, 2500 kN/m³). Joint masses P /
g in UX and UY where the PD loads act; cases PD, LAT and PD + LAT; modal case.

- K1, K2 (shear deformation), K4 (four objects): 3 m cantilevers, 3000 kN of mass weight on top, LAT 10 kN X
  and 5 kN Y; KO1 with rigid end offsets 0.5 m; LNK a frame to 1.5 m with a rigid link to 3 m.
- KH: its support at z 1 (the lowest joint is at 0), the mass at 4 m: the string uses 4 m.
- K6: a 6 m column of two pieces, 1000 kN of mass weight at 3 m and at 6 m.
- KMID: a column 0 - 2 - 3 m, the mass at 2 m, LAT at 3 m.
- KSW: no joint mass, the element mass of its weighty material (half at the top joint).
- LEAN: a cantilever (500 kN) and a pinned truss column (1500 kN, its top held in Y) tied by a 4 m beam.

Checks: the strings (each mass joint's own weight over its own height above the base, no axial forces), joint
reactions with the strings' pull, base reactions without it, periods. References: hand.

Reference files: `hand.json` (40 values).
