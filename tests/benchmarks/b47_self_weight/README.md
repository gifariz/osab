# B47 - Self-weight

Self-weight: material weight per volume (24 kN/m^3 here) times the load pattern's multiplier, on frames (x A x
weight modifiers; the A modifier changes stiffness only) and areas (x thickness x weight modifiers). Meshing
off, no end offsets.

- C1: a 3 m cantilever column (section weight modifier 0.8, A modifier 0.5).
- Portal at y = 6 m: two 4 m columns and a 6 m beam with its own weight modifier 0.5.
- T1: a 4 m truss bar between two pins (half its weight to each pin).
- A 6 x 6 m Shell-Thin slab (3 x 3 areas, 0.2 m, the middle one at weight 0.5) on four columns.
- A 6 x 3 m Shell-Thin wall (0.25 m, 3 x 2 areas), fixed base.

Cases: DEAD (multiplier 1), HALF (multiplier 0.5 and a 4 kN load in X at C1's top); ULS = 1.2 DEAD + 1.6 HALF.
References: hand (statics), OpenSees (frame parts only, the weight written out as frame and joint loads).

Reference files: `hand.json` (24 values), `opensees.json` (244 values).
