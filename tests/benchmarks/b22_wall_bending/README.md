# B22 - Walls in pure in-plane bending

Two 6 x 1.2 m walls (t = 0.3 m), 6 x 2 areas each, in X-Z planes: Membrane at y = 0, Shell-Thin at y = 3. End
moments M = 30.0 kN m as linear edge stresses (joint forces -25.0, 0, +25.0 kN at the right end, opposite at
the left). Held at the middle joint of the left end (all six DOFs); the Membrane wall also has uy, rx, rz
restrained at every joint.

The closed form (I = 0.0432 m^4) is checked by mesh refinement in solver/tests/shell_convergence.rs.

Reference files: `hand.json` (6 values).
