# B64 - Response spectrum: two-story eccentric building

Two stories of 3.5 and 3 m, 8 x 5 m: C50 columns at the corners and a C80 column at (8, 2.5), B60x30 beams
round each floor, a 200 mm Shell-Thin wall (one element) between the columns at X = 0 in story 1, all story
joints in rigid diaphragm D1; 10 t (UX, UY) at every story corner and 20 t more at (8, 5), so the modes couple
X, Y and torsion. Material weight 25 kN/m3 (DEAD); mass source joint masses only. The probe model B1 without
its massless Membrane floors, plus the wall. For story results: the column at (0, 0) split at 1.75 m (a joint
between levels, no drift), a 2 m roof cantilever from (8, 5) (its tip has no joint below, no drift), a static
case LAT (joint loads, a uniform X load along the column at (0, 5) in story 1, a down load at the cantilever
tip).

Spectrum FUNC (as B63) and FUNCH (1.0 from 0.04 to 0.07 s, then 0.02: short periods only). Cases: X, Y, X and
Y by SRSS, by the absolute rule (0.3 and 1.0), by CQC3 with 0.6 g on one of them; X and Y at 30 degrees; SRSS
and absolute modal combinations; FUNCH on Y; eccentricity 0.05 and 0.10 on one and two directions, with the
absolute and CQC3 rules, at 60 degrees, and where X wins on one story and Y on the other (XH42E).
Combinations: 1.2 DEAD + X, DEAD - 0.5 X, X + 0.3 Y, envelope of DEAD and XE, DEAD + LAT, DEAD + X.

No OpenSees twin: its shell membrane is not the wall's (the frames-only probe model MR of
checks/rs_probe3_models.py has one).

Reference files: none published; `osab bench` runs the model to check that it solves.
