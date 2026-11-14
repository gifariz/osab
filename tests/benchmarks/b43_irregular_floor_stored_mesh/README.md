# B43 - Irregular floor on a stored paved mesh

B36's floor, supports and loads, with a paved mesh (General, maximum size 0.5 m: 346 elements, 56 of them
triangles, paved round the edges) stored on the floor (`user_mesh`). The opening lies inside the stored mesh,
which already leaves it out; the floor's load over the opening is not carried.

Hand: reaction totals. OpenSees: the same mesh as plain ShellDKGQ / ShellDKGT elements.

Reference files: `hand.json` (2 values), `opensees.json` (3,802 values).
