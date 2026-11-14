# B58 - User wind loads on a rigid diaphragm

B50's four unequal columns (one turned 90 degrees), now two stories of 3.5 m, joined at each level only by
rigid diaphragm D1 (no beams, no slab). Stories Base, Story1, Story2.

Patterns: Dead (self-weight), WX and WY: Wind patterns with user wind loads (story forces from, e.g., a wind
tunnel): WX 40 kN in X at (2, 1.5) with 10 kN m at Story1 and 60 kN at (5, 4) at Story2; WY 30 kN in Y at (10,
9), a point off the floor, at Story1, and 50 kN at (1, 5) with -20 kN m at Story2. The loads act through the
diaphragm (as a force at that point of a rigid floor); no joint is added.

Cases: Dead, WX, WY, C1 = 1.2 Dead + 1.6 WX - 0.8 WY; combination ENV = envelope of WX and WY.

References: hand (the 6 x 6 stiffness of the two levels' UX, UY, RZ).

Reference files: `hand.json` (156 values).
