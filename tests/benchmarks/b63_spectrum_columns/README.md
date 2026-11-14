# B63 - Response spectrum: one-mode columns

Eight cantilever columns 3 m high (C50 with shear deformation, no weight), 5 m apart, each with its own joint
mass in UX at the top, so each is a one-mode system; the masses make the periods 0.05, 0.2, 0.5, 0.55, 1.2,
1.6, 2.4 and 6 s. Mass source: joint masses only.

Spectrum FUNC: 0.4 at 0.1 s, 1.0 from 0.3 to 0.8 s, 0.5 at 1.6 s, 0.25 at 3.2 s (in g), damping 0.05; FUNC2
and FUNC3 the same table at damping 0.02 and 0.10; ASCE_D and ASCE_C the ASCE 7-16 design spectrum (Ss 0.8, S1
0.35, TL 8 s, site class D; Ss 1.6, S1 0.7, TL 4 s, site class C). Cases along X: CQC, SRSS and absolute sum
at 5 % damping; CQC at damping 0, 0.02 and 0.10; FUNC2 at 5 %, FUNC3 at 2 %; FUNC at 2 g; each ASCE function
at 1 g.

Checks: the spectrum value between the table's points (linear in the period), before the first and after the
last point (constant), on a point; the damping correction; each column's tip displacement and base reactions
(one mode each, so the same for every rule); the base shear of the case, where the two close modes (0.5 and
0.55 s) tell CQC from SRSS. References: hand and OpenSees (`responseSpectrumAnalysis` mode by mode, combined
by `checks/opensees_spectrum.py`; the cases whose damping differs from their function's are left out: OpenSees
has no damping correction).

Reference files: `hand.json` (283 values), `opensees.json` (2,284 values).
