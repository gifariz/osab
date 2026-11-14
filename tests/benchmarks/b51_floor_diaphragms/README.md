# B51 - Rigid diaphragms on meshed floors: three stories

Three 3 m stories, 2 x 1 bays of 6 m x 5 m: C50 columns, B30x60 beams on every grid line and S200 Shell-Thin
floors (the default mesh, 1.25 m), each floor area in diaphragm D1, so every mesh joint of a story's two
floors joins one constraint (one per story: the model lists its stories). Cases: DEAD (self-weight), LIVE (2.5
kN/m2 on the floors), EQX (10, 20, 30 kN in X at one corner of stories 1-3) and EQY (8, 16, 24 kN in Y at the
opposite corner): eccentric, so the floors twist.

References: hand (statics of the base reactions), OpenSees (OSAB's mesh, `rigidDiaphragm 3` per story).

Reference files: `hand.json` (4 values), `opensees.json` (5,412 values).
