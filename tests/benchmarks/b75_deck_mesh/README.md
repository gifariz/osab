# B75 - Deck floors cut at the beams crossing them

Four layouts at z = 3 m on 0.5 x 0.5 m columns (bases fixed) and 0.6 x 0.3 m beams (weightless, massless, no
end offsets), each drawn twice: with the filled deck DF of B73 and with a 0.2 m Membrane slab SM (two-way):

- **SPAN2** one 16 x 4 m area over two bays, a beam across it at x = 8, local axis 1 along X.
- **SKEW** SPAN2 with local axis 1 at 30 degrees.
- **GRID** one 18 x 6 m area over three 6 m bays, secondary beams at y = 2 and 4 in every bay, local axis 1
  along Y.
- **L** an L-shaped area, beams on its edges and one inside at x = 4.

A deck is meshed as a Membrane floor: cut at the beams into panels, one element each. Each panel then sends
its loads one way, along local axis 1, to its edges. Dead = self-weight, SDL = 2 kN/m^2 down.

Reference files: none published; `osab bench` runs the model to check that it solves.
