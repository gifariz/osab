# B25 - Wall with a beam framing into it in its plane

3 x 3 m Shell-Thin wall (t = 0.25 m, 3 x 3 areas) in the X-Z plane, base fixed. A 0.3 x 0.6 m beam runs from
the wall's top corner (3, 0, 3) to a 0.4 x 0.4 m column at x = 8 (base fixed). The beam's in-plane end moment
goes into the wall through the drilling DOF.

- LAT: 50 kN in X at the column top.
- GRAV: 20 kN/m down on the beam.

Hand: base reaction totals.

Where a frame meets a wall element, the solver adds a rotational spring along the element's edge nearest the
horizontal (`solver/src/elements/wall_joint.rs`).

Reference files: `hand.json` (4 values).
