# B74 - Deck in-plane stiffness: filled, unfilled and solid slab decks

4 x 2 m panels at z = 3 m (one element each), the two joints at the panel's x = x0 fixed, the other two free
in X and Y only, 10 kN at each free joint in X (PX) and in Y (PY); each deck with local axis 1 along X (angle
0) and along Y (angle 90). A deck's in-plane stiffness is the same in every direction:

- **DF** filled deck (0.065 m of concrete above 0.075 m ribs): a Membrane of the 0.065 m slab in the fill's
  concrete.
- **DU** unfilled deck: a Membrane of the steel deck's shear thickness (1 mm).
- **DS** solid slab deck 0.15 m: a Membrane of the slab.

Reference files: none published; `osab bench` runs the model to check that it solves.
