# B53 - Modal analysis with element mass

Two 3 m stories, 2 x 1 bays of 6 m x 5 m: C50 columns, B30x60 beams and S200 Shell-Thin floors (the default
mesh, 1.25 m), the floors in rigid diaphragm D1 (one constraint per story). One column is split at 1.5 m, with
2 t there (lumped to the levels below and above); 5 t at one roof corner. Mass source: element mass, joint
masses and load case SDL x 1 (1.5 kN/m2 on the floors and 8 kN down at one roof joint), lateral only, lumped
at stories (the defaults otherwise). Modal case: 12 modes. DEAD (self-weight) is a static case.

References: OpenSees (`eigen` on OSAB's mesh with OSAB's lumped masses): periods, participating mass ratios,
mode shapes, and DEAD's displacements and reactions.

Reference files: `opensees.json` (6,132 values).
