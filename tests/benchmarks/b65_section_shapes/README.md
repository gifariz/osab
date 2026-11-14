# B65: section shapes

A 6 m cantilever column of every section shape (steel, concrete, composite), fixed at the base: tip forces of
10 kN along X (FX) and Y (FY), and self weight (DEAD). The angle AN and the concrete L CL have a product of
inertia: pushed along X they also move along Y. TE8 is the tee at insertion point 8 (top centre) with the
stiffness transform. Each section is given by its `shape` only; the mesh step works out its numbers its own
from its section of the same kind (composite sections by table edit).

- Hand: statics and the self weight of the shapes with a closed-form area.

No torsion: the torsion constants (within about 1 % of the reference's) do not enter the results; the shear
areas (within 1 %, filled pipes 4 %) change the tip displacements far less than the tolerance.

Reference files: `hand.json` (84 values).
