# B49 - Automatic end offsets

*Automatic from Connectivity* end offsets on a two-story 3D frame (6 m x 5 m, 3.5 m stories): columns 0.6 x
0.4 m (one turned 90 degrees, one 30 degrees) and 0.5 x 0.5 m below, 0.4 x 0.4 m above; beams 0.6 x 0.3 m
along X, 0.45 x 0.25 m along Y on story 1, 0.5 x 0.3 m on the roof with rigid-zone factor 0.75 (story 1: the
default 0). Beams get the column below their ends, columns the deepest beam at their top, nothing at their
bottom.

Cases: DEAD (self-weight; beams weigh over their clear length), LIVE (10 kN/m on all story-1 beams), LAT (20
kN in X and 10 kN in Y at each roof corner). References: hand (statics with the automatic lengths), OpenSees
(the same lengths, rigid parts as rigid links).

Reference files: `hand.json` (4 values), `opensees.json` (720 values).
