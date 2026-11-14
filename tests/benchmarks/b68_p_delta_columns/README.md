# B68 - P-Delta from loads: columns, a leaning column, a beam-column

Separate structures 10 m apart, E 25e6, no weight; columns 0.3 x 0.6 (the 0.6 depth along X), 3 m, no shear
deformation unless said. P-Delta iterative from loads: pattern PD x 1. Cases PD (axial loads), LAT (side
loads) and PD + LAT; joint masses P / g where the axial loads act, modal case (its periods with P-Delta).

- K1 cantilever, 3000 kN on top, 10 kN X and 5 kN Y; K2 the same with shear deformation (As 5/6 A); K4 as four
  frame objects; KT in tension (+3000); KU uniform side loads 5 and 2 kN/m; KW the axial load spread along it
  (-1000 kN/m); KPT an axial point load -3000 at 1 m; KO1 end offsets 0.5 m with rigid-zone factor 1, KO0 with
  factor 0; KI its element 0.2 m off the joints (insertion point, transformed).
- LEAN: a cantilever (500 kN) tied by a 4 m beam to a pinned truss column carrying 1500 kN (its top held in Y,
  where the tie alone is too soft).
- LNK: a frame 0 - 1.5 m topped by a rigid link to 3 m, loads at 3 m.
- BC: a simply supported beam 6 m along X with 1500 kN compression, 10 kN/m down and 2 kN/m sideways.
- KR: a column released (M2, M3) at its top, tied by a beam to a fixed column.

Checks: the consistent geometric stiffness of one element (cubic shapes; Timoshenko shapes with shear), the
mean axial force of a member loaded along its length, rigid arms as bars, releases condensed with the
geometric stiffness, a truss as a string, rigid links without P-Delta, frame forces at stations (the P-Delta
moment along the member), reactions, periods. References: hand.

Reference files: `hand.json` (48 values).
