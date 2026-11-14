# B26 - Load combinations

6 x 6 m Shell-Thin slab (t = 0.2 m, 2 x 2 areas) at z = 3.5 m on four 0.4 x 0.4 m columns (C1..C4, bases
fixed). Load cases DEAD (8 kN/m^2 on the slab), LIVE (3 kN/m^2 on F1 and F2, and 4 kN in X inside column C1 at
1.5 m) and WIND (10 kN in X and -6 kN in Y at the top corner). Combinations of the three types OSAB has: ULS1
= 1.2 DEAD + 1.6 LIVE, ULS2 = 1.2 DEAD + LIVE + WIND, ULS3 = 0.9 DEAD - WIND (Linear Add), ENV = envelope of
ULS1..3 and ENVL = ENV - 0.5 LIVE (a Linear Add of an envelope); SRSS1 = SRSS of DEAD, 0.5 LIVE and -WIND,
SRSS2 = SRSS of ENV and 0.5 WIND, SRSSL = -0.5 SRSS1 + DEAD, SRSSE = envelope of SRSS1 and ULS1, SRSSN = SRSS
of SRSS1 and 2 LIVE. Combinations with a maximum and a minimum have reference values per step (max, min).

Hand: base reaction totals of ULS1..3 (statics).

Reference files: `hand.json` (9 values).
