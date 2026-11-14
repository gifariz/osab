# B54 - Small building

Phase 4's closing check: three stories (3.5, 3 and 3 m) of 2 x 2 bays (6 m x 5 m) with the defaults left on -
self-weight, automatic end length offsets (rigid-zone factor 0), meshed floors and walls (1.25 m) - and the
features most real models use:

- C50 columns and B30x60 beams on every grid line; a 250 mm Shell-Thin wall bay at X = 12 between Y = 0 and 5
  (no columns there), its bottom restrained;
- S200 Shell-Thin floors in rigid diaphragm D1 (one constraint per story);
- the beams along Y = 10 pinned (M2 and M3 released at both ends: the hinges sit at the clear-length ends);
- cracked-section modifiers: columns I 0.7, beams I 0.35, floors bending 0.25, the wall membrane 0.7;
- cases DEAD (self-weight), SDL 1.5 kN/m2, LIVE 2.5 kN/m2 (roof 1.0), EQX and EQY story forces at two joints
  each; combinations C1 = 1.2 D + 1.2 SDL + 1.6 L and C2 = 1.2 D + 1.2 SDL + L + EQX;
- mass source: element mass, joint masses, SDL x 1 and LIVE x 0.25 (lateral, lumped at stories); modal case
  with 12 modes.

References: hand (base reactions by statics).

Reference files: `hand.json` (8 values).
