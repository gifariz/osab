# B62 - Insertion points: offset cantilevers and T-beam floors

Concrete (E = 25e6 kN/m2, nu = 0.2), beams 0.3 wide x 0.6 deep (no shear deformation), all at cardinal point 8
(top centre: the joints lie on the beam's top face, its centroid e = 0.3 m below).

- **K1, K2**: cantilevers 4 m, fixed at the root; K1 with the stiffness transform, K2 without. KZ: 10 kN down
  at the tip; KX: 20 kN along the beam at the tip. Hand (exact): the centroid is an ordinary cantilever and
  the joints sit on rigid arms e above it, so for K1 the tip load KZ also moves the joint out by the rotation
  times e, and KX bends the beam by P e (tip down P e L^2/2EI, joint along the beam P L/EA + P e^2 L/EI, no
  moment at the support joint, M3 = -P e along the beam). K2 behaves as at the centroid.
- **T1, T2**: T-beam floors. A simply supported beam 8 m (8 frame objects; pinned at x = 0, a roller along X
  at x = 8, both ends held against twisting) under a Shell-Thin floor 3 m wide and 0.15 thick (8 x 4 areas of
  1 x 0.75 m) at **Top**. T1 with the transform on beam and floor: the floor's top face lies on the joints
  (its mid-plane 75 mm below them, flush with the beam's top) and the beam's centroid 0.3 m below them, tied
  by rigid arms, so they act as one T-beam. T2 without (the drawing defaults): beam and floor bend each about
  their own axis. Q: 5 kN/m2 on the floors; P: 50 kN at mid-span on each beam. Hand: support reactions
  (statics).

T-section estimate (the whole 3 m flange working, no shear lag; the flange's 0.15 x 0.3 m over the web counted
twice, as the model does): I = 0.01275 m4 against 0.00540 m4 for the beam alone; mid-span deflection under Q
2.51 mm (beam alone 5.93 mm), under P 1.67 mm (beam alone 3.95 mm). T1 gives 2.65 mm and 1.77 mm (OSAB alike,
unchanged with a mesh four times finer): 5-6 % more than the estimate, as the 1.5 m flange outstands do not
work in full over an 8 m span (shear lag). T2 gives 5.13 mm and 3.42 mm: beam and floor each about its own
axis (I = 0.0054 + 3 x 0.15^3/12).

Reference files: `hand.json` (36 values).
