# B73 - Deck floors on beams: a filled deck and a solid slab deck

Two 8 x 4 m floors at z = 3 m, each on four 0.5 x 0.5 m columns (bases fixed) and four 0.6 x 0.3 m beams
(weightless, massless, no end offsets), local axis 1 along X, on one rigid diaphragm. Concrete 4000Psi (23.563
kN/m^3), deck steel A992Fy50.

- **DF** filled deck: 0.065 m of concrete above 0.075 m ribs, 0.15 m wide at the top and 0.12 m at the bottom,
  0.3 m apart, deck 0.1 kN/m^2: 0.09875 m of concrete per area, 2.4268 kN/m^2.
- **DS** solid slab deck 0.15 m: 3.5345 kN/m^2.

A deck is always a one-way Membrane: Dead (self-weight) and SDL (2 kN/m^2 down) go along local axis 1 to the 4
m beams; the 8 m beams get nothing. Hand: each bay's base reactions carry its load.

Reference files: `hand.json` (4 values).
