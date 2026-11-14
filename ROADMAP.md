# Roadmap

OSAB is a linear structural analysis program for buildings: a desktop app (and a browser demo) to model frames,
floors and walls, analyse them and see the results. This page says what it does today and what comes next.
Plans change; nothing here is a promise of dates.

## What OSAB does today
- **Modelling:** frames and trusses (sections from shapes or a steel list, releases, end offsets, insertion points,
  property modifiers), floors and walls (thin and thick shells, membrane floors, openings, warped quads), joint,
  line and area springs, rigid links, rigid and semi-rigid diaphragms, stories, groups, pier and spandrel labels.
- **Meshing:** automatic floor and wall meshes (general or rectangular, finer near openings and corners), stored
  meshes, areas kept as one element, frames joined where they cross and split at area edges.
- **Loads:** load patterns (joint, frame and area loads, self-weight, user wind loads), load cases of patterns and
  load combinations (linear add, envelope, SRSS).
- **Analysis:** linear static, P-Delta (from the mass or iteratively from loads), modal analysis (Eigen or Ritz
  vectors), response spectrum cases (user functions or ASCE 7-16; CQC, SRSS, absolute; directional rules,
  eccentricity).
- **Results:** displacements, reactions, frame, link and shell forces, base reactions, story drifts and forces,
  pier and spandrel forces, centres of mass and rigidity, modal periods, mass ratios and load participation;
  contour and deformed views, tables, export to Excel and CSV.
- **Verification:** every feature is checked on benchmark models against hand calculations and OpenSees
  ([docs/VERIFICATION.md](docs/VERIFICATION.md)).

## Next
- More of the benchmark models checked against OpenSees, through an export of OSAB models to OpenSees.
- Faster and lighter results views for very large models.
- Reading models from common exchange formats.
- A first public release with an installer.

## Later
- Nonlinear static cases, then staged construction.
- Design checks.
- The desktop app for macOS and Linux (the web demo and the command-line tool already run there).

## Not planned for now
Automatic seismic and wind load patterns, general section cuts, automatic load combinations, linear time history.
