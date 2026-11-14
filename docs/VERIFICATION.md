# Verification

OSAB is checked on 76 benchmark models (`tests/benchmarks/`) against:

- **Hand calculations** and closed-form results (textbooks, statics, patch tests);
- **OpenSees**, an open-source finite element program, run as a black box.

`osab bench` compares every stored value: 3,224 hand and 26,072 OpenSees values. A value
passes when it is within 0.1 % of the reference, plus a small absolute floor for values near zero. Most
agree far closer than that, often to round-off. Every model is also checked against a commercial building
analysis program; those results are not published.

Where each modeling rule comes from: [SOURCES.md](SOURCES.md).

## Benchmarks
Values compared per reference ("-": no reference of that kind for this model). Some OpenSees values are
extrapolated from a mesh-convergence study.

| # | Model | Hand | OpenSees |
|---|---|---:|---:|
| B01 | Cantilever with tip load (2D) | 7 | 15 |
| B02 | Simply supported beam, uniform load (2D) | 9 | 24 |
| B03 | Fixed-fixed beam, uniform load (2D) | 9 | 27 |
| B04 | Propped cantilever, point load inside the member (2D) | 7 | 16 |
| B05 | Portal frame (2D) | - | 72 |
| B06 | Pratt truss (2D) | 15 | 53 |
| B07 | 3D cantilever: biaxial bending + torsion | 15 | 30 |
| B08 | Inclined member with rotated local axes (3D) | 17 | 30 |
| B09 | Deep cantilever with shear deformation (2D) | 6 | 15 |
| B10 | 3D one-bay space frame | - | 336 |
| B11 | 3D tripod truss | 15 | 39 |
| B12 | Rotated column with local member loads and shear deformation (3D) | 13 | 30 |
| B13 | Members given against the end order (3D) | - | 30 |
| B14 | Column on six base springs (3D) | 30 | 30 |
| B15 | Portal frame on spring bases with rigid end zones (2D) | 9 | 84 |
| B16 | Column with a rigid L-shaped bracket (3D) | 60 | 42 |
| B17 | Two columns tied at the top by a rigid link (3D) | 122 | 122 |
| B18 | Membrane patch test | 174 | - |
| B19 | Plate patch test (Shell-Thin) | 251 | 182 |
| B20 | Simply supported square plate (Shell-Thin) | - | 440 |
| B21 | Simply supported thick square plate (Shell-Thick) | 2 | - |
| B22 | Walls in pure in-plane bending | 6 | - |
| B23 | U-shaped core wall | 6 | - |
| B24 | Slab on four columns (click-test model) | 4 | - |
| B25 | Wall with a beam framing into it in its plane | 4 | - |
| B26 | Load combinations (Linear Add, Envelope, SRSS) | 9 | - |
| B27 | Membrane patch test with triangles | 228 | - |
| B28 | Plate patch test with triangles (Shell-Thin) | 351 | 182 |
| B29 | Simply supported square plate with triangles (Shell-Thin) | 1 | 440 |
| B30 | Slab with quads and triangles and a triangular gable wall, on four columns | 5 | - |
| B31 | Simply supported plate drawn as one floor (auto-mesh) | 1 | 437 |
| B32 | Floor on beams and columns, one floor object (auto-mesh) | 3 | 180 |
| B33 | L-shaped floor with an opening (Rectangular mesh) | 1 | 97 |
| B34 | Two-story wall core with a door, floors meeting the walls (auto-mesh) | 3 | - |
| B35 | Edge constraints (floors) | 3 | - |
| B36 | Irregular floor with grading (General mesh) | 2 | 1,004 |
| B37 | Plane grid (slab, X-Y): B32's floor on point supports | 3 | 972 |
| B38 | Overlapping floors: a drop panel and a slab band (Rectangular mesh) | 2 | - |
| B39 | Membrane floor: load transfer to beams, columns and a wall | 4 | - |
| B40 | Stiff zones over columns (Stiff slab type, Rectangular mesh) | 4 | - |
| B41 | Null objects and Include in mesh (Rectangular mesh) | 4 | - |
| B42 | L-shaped floor with an opening on beams and columns (built by clicking) | 2 | - |
| B43 | Irregular floor on the reference program's mesh (stored mesh) | 2 | 3,802 |
| B44 | Frame property modifiers | 21 | 306 |
| B45 | Area property modifiers | 517 | - |
| B46 | Frame end releases | 28 | 462 |
| B47 | Self-weight | 24 | 244 |
| B48 | End length offsets and rigid zones | 30 | 540 |
| B49 | Automatic end offsets | 4 | 720 |
| B50 | Rigid diaphragm: one story, unequal columns, eccentric loads | 80 | 240 |
| B51 | Rigid diaphragms on meshed floors: three stories | 4 | 5,412 |
| B52 | Modal analysis, hand cases | 159 | 1,001 |
| B53 | Modal analysis with element mass | - | 6,132 |
| B54 | Small building | 8 | - |
| B55 | P, V2, V3 and T releases with end offsets | 21 | - |
| B56 | One-way Membrane floors on beams, walls and a free edge | 1 | - |
| B57 | Load patterns and load cases of patterns times scale factors | 50 | - |
| B58 | User wind loads on a rigid diaphragm: two stories, unequal columns, points off centre and off the floor | 156 | - |
| B59 | Diaphragm centres of mass and rigidity; user wind loads at the centre of mass and on a semi-rigid floor | 56 | - |
| B60 | Beam on elastic foundation: a line spring on a free beam of 96 pieces, uniform and point loads | 5 | - |
| B61 | Mat on Winkler springs: an area spring on 64 areas, uniform pressure and column loads | 6 | - |
| B62 | Insertion points: cantilevers at cardinal point 8 with and without the transform, T-beam floors (beams at 8 under floors at Top) | 36 | - |
| B63 | Response spectrum: one-mode columns, spectrum values, damping, CQC / SRSS / absolute, ASCE 7-16 | 283 | 2,284 |
| B64 | Response spectrum: two-story eccentric building with a wall; directional rules, eccentricity, combinations, story drifts and forces | - | - |
| B65 | Section shapes: a 6 m cantilever of every steel, concrete and composite shape (sections given by their shape) | 84 | - |
| B66 | Steel section list: cantilevers of AISC shapes (W, C, WT, L, 2L, HSS, pipe, HP) by name | 40 | - |
| B67 | Pier and spandrel forces: a wall with a door and a header (split wall, unequal cuts, a spandrel) | 84 | - |
| B68 | P-Delta from loads: columns (shear deformation, end offsets, axial loads along them, an insertion point), a leaning truss column, a rigid link, a beam-column, a released column | 48 | - |
| B69 | P-Delta from the mass: columns, masses at two heights and between them, a support above the base, element mass | 40 | - |
| B70 | P-Delta from loads: two-story building with a wall (self weight and scale factors in the P-Delta load); periods, spectrum, combinations, story drifts and forces | - | - |
| B71 | Ritz vectors, full basis: four point masses on springs, starts UX, UY, UZ (the exact oscillator modes) | 24 | - |
| B72 | Ritz vectors, partial cycle: the same, 5 vectors (the order of the starts shows) | - | - |
| B73 | Deck floors on beams: a filled deck and a solid slab deck (weight from the deck's concrete, one-way to the end beams) | 4 | - |
| B74 | Deck in-plane stiffness: filled, unfilled and solid slab decks, local axis 1 along X and Y | - | - |
| B75 | Deck meshing: decks and Membrane slabs cut at the beams crossing them (two spans, skewed axes, secondary beams, an L) | - | - |
| B76 | L-shaped floor example: B42 with the sections' depth and width and automatic end offsets, as drawn in the app (the example model; the click test draws it) | 2 | - |
| | **Total** | **3,224** | **26,072** |

Each benchmark folder has a README with the model, the load cases and how its references were made.

## Not included
- Partial fixity springs at frame ends (full releases are included).
- Ritz vectors (Eigen vectors are included). Response spectrum analysis is included (since 2026-10-30):
  user and ASCE 7-16 functions, CQC / SRSS / absolute, SRSS / absolute / CQC3, constant damping with its
  correction, eccentricity, story drifts and forces (B63, B64). So is P-Delta (since 2026-10-30): none,
  from the mass, iterative from loads, frames and shells, for every case (B68-B70); nonlinear static
  cases (P-Delta or large displacements per case) are not.
- Compression-only and tension-only line and area springs, area springs from a soil profile; spring links
  between two joints, partly rigid links.
- Load combination types Absolute Add and Range Add (Linear Add, Envelope and SRSS are included).

OSAB is a tool for engineers: the engineer using it is responsible for the model, for checking its
results and for the design decisions made with them.
