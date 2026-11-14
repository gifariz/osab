# B45 - Area property modifiers

Shell property modifiers (f11 f22 f12 membrane, m11 m22 m12 bending, v13 v23 transverse shear) on area
sections and on area objects (multiplied), on every shell type. Concrete, meshing off (every area is one
element, *No Auto Mesh* / walls 1 x 1).

- **A** Membrane patch (B18's five distorted quads): section f11 = f22 = 0.7, every area f22 0.5 and f12 0.25.
  TENS F11 = 10, TENS2 F22 = 8, SHEAR F12 = 5 kN/m as edge loads.
- **B** Two Shell-Thin plate patches (B19's): section m11 = m22 = m12 = 0.5, every area m11 0.5 and m12 0.7.
  BEND: constant M11 = 10 kN m/m; TWIST: a 4 kN corner load (pure twist).
- **C** Shell-Thick cantilever plate 4 x 2 m, t = 0.4 m, section m 0.7, v13 0.5, v23 0.8 (the first row of
  elements also v23 0.8 on the area): TIPC 10 kN down at each tip joint, TWISTC +5 / -5 kN at the tip corners.
- **D** A 6 x 6 m slab (3 x 3 Shell-Thin, bending 0.25) on beams (I 0.35) and four columns (I 0.7): GRAVD 8
  kN/m^2.
- **E** A 6 x 3 m Shell-Thin wall (3 x 2 elements), section f11 = f22 = 0.7, bottom row f12 0.5 on the areas:
  WALLX 20 kN in plane, WALLY 3 kN out of plane at each top joint.

References: hand (exact patch states, statics). OpenSees has no per-term shell modifiers (its elastic plate
section has only one factor on all of the bending), so there is no OpenSees file here; frame modifiers are
checked against OpenSees in B44.

Reference files: `hand.json` (517 values).
