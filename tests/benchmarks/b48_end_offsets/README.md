# B48 - End length offsets and rigid zones

End length offsets (given lengths) with rigid-zone factors 0, 0.5, 0.75 and 1: the rigid part is rigid in
bending and shear, axial deformation acts over the whole length, loads on a rigid part go to its joint, and
forces are reported on the clear length.

- **A** Three fixed-fixed 6 m beams with 0.4 m offsets at both ends, factors 1, 0.5 and 0; GRAV 12 kN/m.
- **B** A fixed-fixed beam with 0.5 m rigid zones (factor 1) and a 20 kN load at 0.2 m from end i (PT).
- **C** A portal (columns 0.6 m offset at the top, beam 0.25 m at both ends, factor 0.75): GRAV on the beam,
  LAT 15 kN at the left top.
- **D** The same portal with the beam pinned (M3 released at both ends).

References: hand (closed forms for A and B, statics for C and D), OpenSees (rigid parts as `rigidLink beam` to
a node at factor x offset, A and J scaled to keep the axial and torsional stiffness of the whole length; loads
on rigid parts moved to the joints; the hinge at the clear-length end). D shows the rule for releases with
offsets: the hinge sits at the clear-length end (M3 = 0 at x = 0.25 m), not at the joint and not at the rigid
zone's end.

Reference files: `hand.json` (30 values), `opensees.json` (540 values).
