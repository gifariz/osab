# Sources of OSAB's modeling choices

Where each modeling rule in OSAB comes from, and which test checks it. Code comments describe the rules
in OSAB's own words and point here for their origin.

## How references are used
- OSAB's solver and mesher are written from textbooks, published papers and published manuals.
- Other programs are used as black boxes only: models are built and run through their public interface, and
  their results are compared with OSAB's. No other program's code is copied, decompiled or disassembled.
  OpenSees is run as a black box only (its license does not allow its code in OSAB).
- Where no publication gives a rule, OSAB sets its own rule and checks it with small models (one element, one
  joint, one load).

Source kinds below: **Textbook / paper**, **Manual** (the *CSI Analysis Reference Manual*), **Common
convention** (shared by the usual building analysis programs), **OSAB's own rule**.


## Conventions
| Choice | Source | Checked by |
|---|---|---|
| Frame end order: end i at the lower Z, then lower X, then lower Y | Common convention | Every frame benchmark (B01-B17) |
| Frame local axes (1 along the member, 2 up in the vertical plane, vertical members 2 = +X) and forces P V2 V3 T M2 M3 with their signs | Common convention | B01-B17, `solver/src/axes.rs` tests |
| Area local axes (3 normal, 1 horizontal = ref x 3, ref +Z or +Y near vertical) and corner order (first lowest corner, direction kept) | Manual, section 10.6, eq. 10-3 | - |
| Shell forces F11 F22 F12 M11 M22 M12 V13 V23, positive M = tension at the bottom (-3 face) | Manual, section 10.18 | B18-B30 |
| Shell shape warnings (aspect ratio above 4, corner angles outside 45-135 degrees) | Manual, section 10.2.1 | Model check (warnings only) |
| Object types (column within 1 degree of vertical, beam / floor within 20 degrees of horizontal) | Common convention | - |
| Labels by plan position (letter by type; the same label for the same XY, story span and distance to the level on every story; moved objects join the label at their new place) | OSAB's own rule | - |
| Story of an object (a joint: the lowest level at or above it, below the base the base, above the top the top story; frames, links and areas: the story of their highest joint) | OSAB's own rule | - |
| Changing story levels: every joint keeps its distance to its own story's level; frame loads keep their fraction of the length | OSAB's own rule | - |
| Deleting a story deletes its structure, the story above grows to cover it; adding a story moves what is above it up | OSAB's own rule | - |
| Special joints: a joint drawn alone is special; deleting or moving objects deletes joints left with nothing attached that are not special, whatever is assigned to them; turning Special off for a joint with nothing attached deletes it at once | OSAB's own rule | `gui/src/model/model.test.ts` |
| Move: selected joints move and stretch their members (frame point loads keep their relative distance); a moved frame, link or area takes the joints only it uses and gets new joints where a joint is shared with an object that stays; joints landing on joints merge into them | OSAB's own rule | - |
| Replicate (linear, radial, mirror, story; delete original objects; rotate local axes in radial) | OSAB's own rule | - |
| Load combinations Linear Add and Envelope, Max / Min steps | Common convention | B26 |
| App: response spectrum cases in the Load Cases list (one list of names with linear static cases, a type chosen for a new case); the first spectrum case turns on the modal case (12 modes), which cannot then be turned off; an ASCE 7-16 function's points come from the solver (`spectrum` command) | OSAB's decision | `gui/src/model/spectrum.test.ts` |
| SRSS combinations: +R and -R; a part with a maximum and a minimum counts with the larger size; inside a Linear Add or Envelope as +R / -R | Common convention | B26, `solver/tests/combinations.rs` |
| Moments drawn on the tension side; local axis colours | Common convention | - |
| 3D view picking: a joint within 10 px wins, then a member or link within 7 px before an area edge within 7 px, then the nearest face under the mouse; ties go to the first in scene order (results before the model) | OSAB's own choice; since 2026-11-02 searched through screen cells with the same rules (GUI_SPEED_PLAN step 3); since 2026-11-11 each point (coordinates equal to the last bit) kept and projected once (GUI_SPEED_PLAN, *Results memory*) | `gui/src/viewport/three/picking.test.ts` (against a search over every item) |
| 3D view drawing order: see-through faces back to front object by object (the middle of each object's box), ties in scene order; lines in groups of one colour, width and dash, in the order the groups first come; the results drawn after the model, also when only the model's layer is built again | The order the three.js renderer gives separate objects, kept when the view merged its objects into a few buffers (GUI_SPEED_PLAN step 3); the results layer's objects made again after a new model layer (`SceneLayer.renew`, 2026-11-11) | Screenshots before and after (`gui/scripts/view-timing.mjs`), GUI_SPEED_PLAN |
| 3D view depth: lines, joint dots and support symbols drawn before the see-through faces with the depth test on (dots and symbols pulled 8 px towards the camera along the line of sight, so the lines meeting at a joint do not hide them), so solid objects in front hide them and each see-through face in front dims them; area faces 0.4 opaque (0.55 selected) | OSAB's own rule | Looked at in the browser on B51 (plain, extruded, plan); `gui/src/scene/viewOptions.test.ts` (opacity) |
| Shell contours coloured per pixel from the element's own field: bilinear over a quad, linear over a triangle, on the colour ramp blue, cyan, green, yellow, red | OSAB's own choice (the field of the element's shape functions) | `gui/src/viewport/three/materials.test.ts`, `gui/src/scene/scene.test.ts` |
| More than 500 texts in the view: hidden while it moves, shown again when it rests | OSAB's own choice (ROADMAP item 12) | - |

## Elements
| Choice | Source | Checked by |
|---|---|---|
| 3D frame (12 x 12, with shear deformation) and truss | Textbook: McGuire, Gallagher and Ziemian, *Matrix Structural Analysis* | B01-B17 |
| End releases by static condensation | Textbook (static condensation) | B46 |
| End length offsets and rigid zones; automatic offsets from connectivity | OSAB's own rule | B48, B49, B55 |
| A frame split by the mesh step with a bending release at an end: the release stays on the first (last) piece and the end offset on that piece reaches at most 99 % of it (the hinge 1 % of the piece before the joint; the offset on the pieces past it unchanged); the object's results start at its own offsets (a station added there when it falls inside a piece's flexible part). Rigid zone factor 0 only measured | OSAB's own rule | - |
| Rigid links and diaphragms as exact master/slave constraints | Textbook (constraint equations) | B15, B50, B51 |
| Shell-Thin plate bending: DKQ on quads, DKT on triangles | Paper: Batoz and Tahar (1982); Batoz, Bathe and Ho (1980) | B18-B20, B27-B30 |
| Membrane: bilinear quad with an Allman-type drilling field, two condensed bubbles, one-point penalty | OSAB's own rule | - |
| Membrane elements keep a vanishing plate stiffness | OSAB's own rule | - |
| Shell-Thick: bending part in series with a shear part | OSAB's own rule | SHELL_THICK.md, B21 |
| Area loads as consistent corner forces q ∫N dA | Textbook (consistent loads) | B18-B24 |
| Warped quads (corners off one plane): the flat element on the mean plane (through the corner centroid, normal from the lines joining opposite mid-sides), tied to each joint by a rigid arm along the normal to the corner's projection; area loads and self-weight go to the joints as the flat element's shares with no moment from the arms; mass the same shares; not meshed (one element); twist warning above 30°, poor above 45° | OSAB's own rule | `solver/src/area.rs` `shape_checks` |
| Stiff slab type: plate bending x 100 | OSAB's own rule | STIFF_AREA.md, B40 |
| Property modifiers on frames and shells (section x object) | OSAB's own rule | B44, B45 |

## How elements are joined
| Choice | Source | Checked by |
|---|---|---|
| Edge constraints as three hidden beams per tied joint ("flexible interpolation"); torsion constant 16.65 W t³ with E | Manual, section 10.3 (idea) | B35 |
| Frames crossing frames: both are split where they pass within 1 mm of each other (whatever their lengths, in the model's length unit), at the point on the frame that comes first in the model; 1.5 mm apart they are not joined. A frame passing within 1 mm of an area's edge (in the area's plane or not: a column through a floor's edge, a beam across a wall's end) is split there at the point on the frame, and a meshed area gets a mesh joint there; the edge tie then connects it when it lies within 0.1 mm of the edge. Null frames and null areas split nothing; a frame piercing a floor inside, or lying inside a one-element floor, is not split. Also in older files with the model's mesh `"off"`, read as every area set to no mesh | OSAB's own rule | - |
| A joint is tied to an area edge when it lies less than 0.1 mm off it (and that far from its ends), whatever the edge's length, in the model's length unit | OSAB's own rule | - |
| A DOF without stiffness is restrained automatically (and a load on it refused): its stiffness diagonal is at most 1e-12 of the largest stiffness one element gives its joint in the same kind of direction (the three translations, or the three rotations). The model's largest diagonal is not the measure: a very short, stiff piece elsewhere must not make a connected joint count as free | OSAB's own choice (round-off of a rotated element is far below 1e-12 of its own stiffness; a real-model re-check showed the earlier whole-model measure refusing connected wall joints, REAL_PROJECT_GAPS.md, *Re-check 2026-11*) | - |
| An unstable structure runs with a warning: a pivot of the factorization that is zero, negative or below 1e-12 of the equation's own diagonal is replaced by 1e-8 of that diagonal (the stiffness the joint's own elements give the direction, not the model's largest) and the equation is cut from the ones after it, so the rest is solved as if the unstable part were not there; each such joint and direction is listed with the digits lost. `"on_instability": "stop"` keeps the refusal | Common practice of building analysis programs (a warning with lost digits, not a refusal); the two fractions are OSAB's own choice | `solver/tests/instability.rs` (a floating beam, a mechanism, a loose beam next to a frame whose results stay the frame's own) |
| Modal case of an unstable structure: the directions given a tiny stiffness are held (zero in every mode; their mass takes no part; the count of eigenvalues is taken on the stiffness without them), so they make no modes with huge periods. The count is taken 1e-6 above the last eigenvalue asked for, and up to four times further (x4 each) when the shift lies on an eigenvalue; when none works the modes are given with a warning. Iterations that have stalled (the last 30 brought no change of the eigenvalues below half the lowest before them: round-off of an ill-conditioned stiffness sets the change) end at once: modes whose eigenvalues still change by 1e-4 or less are given with a warning that says so, otherwise the modal case fails. A modal case that fails is named in `failed` and the other cases are analysed without it | OSAB's own choices | `solver/tests/modal_sturdy.rs` (a loose beam beside a frame: the frame's periods unchanged; equal modes split by the number asked; a modal failure with static cases, a spectrum case and combinations), `eigen.rs` unit test of the stall rule (with a real model's history) |
| A check before factorizing that the analysis fits in memory: the factor's size (exact once the equations are ordered), the factorization's fronts and waiting update matrices, the cases loaded together, the modal vectors and the running results of combinations with a maximum and a minimum, against the free memory less a tenth (at least 1 GB); more than that stops the analysis with a message | OSAB's own choice (MEMORY_DESIGN.md) | `solver/tests/memory_estimate.rs`, `solver/src/memory.rs` and `sparse.rs` unit tests |
| Frame-to-wall joint spring k = E t d³ / (4 L) along the edge nearest the horizontal | OSAB's own rule | B25 |

## Meshing
| Choice | Source | Checked by |
|---|---|---|
| Default maximum size 1.25 m; General floor mesh; walls cut along openings only | OSAB's own rule | B31-B34 |
| Span rule n = ceil(length / size) equal pieces; equal grid on four-sided regions | OSAB's own rule | B31, B32 |
| Rectangular floor mesh with localized meshing | OSAB's own rule | B33, B40-B42 |
| Overlapping areas: opening, then inside object, then Stiff > Drop > Slab, then the smaller area; loads add | OSAB's own rule | `mesh/tests/overlaps.rs`, B38, B40 |
| Membrane floors: panels at beams and walls, 45-degree tributary areas, trapezoidal beam loads | OSAB's own rule | B39 |
| One-way load distribution on Membrane floors | OSAB's own rule | ONE_WAY_LOAD.md, B56 |
| Area section property type (slab or wall) is a label only: any type on any area, floor or wall by the slope alone (19 degrees a floor, 20 a wall), the same results for either type; edits keep the section | OSAB's own rule | `gui/src/model/area.test.ts`, `solver/tests/modifiers.rs` |
| Material types (steel, concrete, other) and strengths (Fy, Fu, f'c): labels, no result depends on them; a file without types: steel from E = 150 GPa, concrete 10 to 60 GPa, else other | OSAB's own convention (common material groups) | `gui/src/model/materials.test.ts`, `gui/e2e/materials.spec.ts` |
| Deck sections: always a one-way Membrane along local axis 1; weight per area = concrete per area (slab depth + rib depth x mean rib width / spacing; a solid slab its depth; none unfilled) x the fill's unit weight + the deck's unit weight; in-plane stiffness a Membrane of the slab depth in the fill's concrete (filled, solid) or of the shear thickness in the deck's steel (unfilled); the slab type plays no part; meshed as a Membrane floor (cut at crossing beams and walls into panels, one element each) | OSAB's own rule | B73, B74, B75, `solver/tests/deck.rs`, `gui/src/model/deck.test.ts` |
| Null objects and *Include in mesh* | OSAB's own rule | `mesh/tests/nulls.rs`, B41 |
| Grading near openings and re-entrant corners (irregular General regions) | OSAB's own choice | B36 (convergence) |
| Stored meshes | OSAB's own implementation of a common feature | B43 |
| Stored meshes: no joint needed at a corner an opening covers; an opening over stored areas only (inside one, or across two floors) is already their hole and is dropped | OSAB's own rule | `mesh/tests/stored.rs` |
| An area set to *no auto mesh* with more than 4 corners is meshed with the model's mesh settings (as if it had no mesh option), with a warning in the mesh notes and in Check Model | OSAB's own rule | `mesh/tests/no_mesh_polygon.rs`, `gui/src/model/area.test.ts` |
| Areas that are not meshed (stored elements, *no mesh* areas) and overlap: both analysed in full there (stiffness, self-weight and the loads of the object over them, on each), with a note | OSAB's own rule | `mesh/tests/stored.rs` |
| Triangle mesher: the boundary edges recovered while the enclosing triangle is still there, its triangles removed after (the border of a triangulation can miss an edge between boundary points nearly in line) | Paper: Sloan 1993 | `mesh/src/tri.rs` test (a 26 m long sliver), the tower model |

## Building features
| Choice | Source | Checked by |
|---|---|---|
| Example models open with the defaults of drawn objects (automatic end offsets, drawn insertion points, the semi-rigid D1 in 3D) where the file gives none, so they match a model drawn today | OSAB's own choice | `gui/src/model/examples.test.ts` (each object against the same object drawn) |
| Self-weight multiplier per load case; DEAD x 1 and unit weights in a new model | OSAB's own rule | B47 |
| Load patterns (type, self-weight multiplier, loads) and linear static load cases of patterns times scale factors; mass from patterns | OSAB's own rule | B57, `solver/tests/load_patterns.rs`, `mesh/tests/load_patterns.rs` |
| User wind loads (Fx, Fy, Mz at a point per story and diaphragm) on a rigid diaphragm: through the constraint as at the master, Mz + (x - x_m) Fy - (y - y_m) Fx | Textbook (statics of a rigid body) | B58, `solver/tests/user_wind.rs` |
| In the app: adding a load pattern adds a case of the same name (as the reference program); renaming a pattern renames that case and deleting it deletes that case (OSAB's own choice) | OSAB's own rule | `gui/src/model/model.test.ts` |
| User wind loads on a semi-rigid diaphragm: into the floor element under the point by its shape functions (the default) | OSAB's own rule | `mesh/tests/user_wind.rs`, B41, B59 |
| User wind loads on a semi-rigid diaphragm spread over its joints as a rigid floor's inertia forces (and as the fallback for a point on no floor element) | Textbook (rigid-body inertia forces); OSAB's own choice as an option | B59 |
| User wind loads on a semi-rigid diaphragm tied to the diaphragm's joint object nearest its centre of mass | OSAB's own rule | B59 (WSN, every value) |
| User wind loads at the diaphragm's centre of mass (the default point) | OSAB's own choice (wind tunnel story resultants act there) | B59 |
| A load combination with no load cases is left out with a warning (and every combination using it), not a refusal | OSAB's own choice (real models carry such combinations) | `solver/tests/combinations.rs` |
| A user wind row at a story where its diaphragm has no joints carries no load and is listed once in a warning (not a refusal) | OSAB's own choice (real converted models carry such rows; refusing stopped the whole run) | `solver/tests/user_wind.rs` |
| Centres of mass and rigidity per story and diaphragm (masses after lumping, x by the Y masses, cumulative per diaphragm from the story up, rigidity by a force on one diaphragm with the others free; semi-rigid: centre of mass only) | OSAB's own rule | B59 |
| A new model's diaphragm D1 is semi-rigid; new floors with a section and new joints at a story level above the base get the first diaphragm | OSAB's own choice | `gui/src/model/diaphragms.test.ts` |
| A joint drawn on its own (Joint tool, Joint by Coordinates) is included in the mesh | OSAB's own choice | `gui/src/model/diaphragms.test.ts` |
| Rigid diaphragms per story; a joint between levels joins the story above | OSAB's own rule | B50, B51 |
| Insertion points of frames: cardinal points 1-11 on the section's box (bottom = -2 face, left = -3 side), mirrors, joint offsets in local or global axes; with the stiffness transform, rigid arms from the joints to the element at the centroid (forces on the element's axis, loads on that axis, releases at its ends); without it, no effect on the analysis; automatic end offsets unchanged | OSAB's own rule | `solver/src/arms.rs` tests, B62 |
| Insertion points of shells: Top / Middle / Bottom, the mid-plane half the thickness from the joints along local 3; with the transform, rigid arms couple membrane and bending | OSAB's own rule | `mesh/tests/insertion.rs`, B62 |
| Mass of members with insertion points: stays at the joints (half of a frame's to each end, a shell's to its corners, no rotational part) whatever the cardinal point or the stiffness transform; only the stiffness moves | OSAB's own rule | - |
| Line and area springs: lumped at the joints of each analysis element in its local axes, translations (and R1 about a frame's axis) with no coupling; a frame end gets k L/2, a shell corner k ∫N dA; at the joints, not at an insertion point; in the joints' reactions | OSAB's own rule | `mesh/tests/springs.rs` |
| Objects drawn by hand get beams 8, columns 5, braces 10, floors Top, walls Middle, all without the transform (the app's draw defaults); objects made through the API get 10 / Middle | OSAB's own rule | - |
| Display options (View > Set Display Options): the items per object kind, *Objects present in view*, special effects; defaults (no object texts or assignment symbols, areas filled, supports on; the grid off); *Invisible* joints (no dots, still picked, their assignments and loads drawn); several texts on one object stacked; release dots, offset lines, joint-mass arrows with values, *Modified* for objects with their own modifiers, diaphragm extent as dashed lines from the outer joints to the centre | OSAB's own rule | `gui/src/scene/viewOptions.test.ts`, `gui/e2e/display.spec.ts` |
| Hiding and object types are two filters (drawn only when the type is ticked and the object is not hidden); objects of a type not drawn cannot be selected | OSAB's own rule | `gui/e2e/display.spec.ts` |
| Mass source, lumping (half of a frame's mass to each end, lateral mass split between story levels) | OSAB's own rule | B52-B54 |
| Modal analysis by subspace iteration with a Sturm sequence check | Textbook: Bathe, *Finite Element Procedures*, section 11.6 | B52, B53 |
| Participating mass ratios; mode shapes at unit modal mass | Common definition | B52, B53 |
| Ritz vectors: starting loads (an acceleration's p = M r, a pattern's assembled loads) take turns, one vector per cycle in their order; first K v = p, then K v = M q from the start's last vector; each made stiffness-orthogonal to the ones before (two passes); a start stops when what is left has at most 1e-6 of the candidate's stiffness norm, or at its cycle limit (the static solve is cycle 1); modes from Qᵀ M Q y = μ Qᵀ K Q y, μ = 1/ω² | OSAB's own rule | B71, B72, `solver/src/ritz.rs` test |
| Ritz rotational acceleration starts RX, RY, RZ: the load M r with r the rigid-body rotation about the centre of all the mass (the point the rotational mass ratios use); load participation lists UX, UY, UZ and the starting patterns only | OSAB's own rule | - |
| Ritz static modes: μ = 0 (a load on joints without mass), at most 1e-12 of the largest μ; shape at unit stiffness, period 0, after the finite modes, left out of spectrum cases | OSAB's own rule | - |
| Modal load participation ratios: static % = Σ (φᵀ p)² / (φᵀ K φ) over pᵀ K⁻¹ p; dynamic % = Σ (φᵀ p_m)² over p_mᵀ M⁺ p_m, p_m the load's part on M's range, finite modes; for UX, UY, UZ and each starting pattern | OSAB's own rule | - |
| A Ritz case uses the same stiffness as every case (with P-Delta, the P-Delta stiffness); minimum modes and target participation are not options (the target does not stop generation) | OSAB's own rule | - |
| A rotation whose free mass is only round-off (all the mass at one height, none rotational) has no mass ratio (0) | OSAB's own choice (the ratio is 0 / 0 there) | B63 |
| Every mode asked for (as many as the mass's rank): one subspace pass from a unit vector at every DOF with mass, exact | Textbook: Bathe, section 11.6 (the Ritz pairs of a subspace holding every mode are exact) | B63 (periods 0.05 to 6 s) |
| Response spectrum: modal amplitude Γ Sa / ω² with Γ = φᵀ M r at unit modal mass; every result value per mode, combined on its own | Textbook: Chopra, *Dynamics of Structures*, ch. 13 | B63, B64 |
| Spectrum function: linear in the period between points, constant before the first and after the last | OSAB's own rule | `solver/src/spectrum.rs` tests, B63 |
| Damping correction of a spectrum: (2.31 - 0.41 ln 100ξ) / (2.31 - 0.41 ln 100ξf) at every period, none at ξ = 0 | OSAB's own rule | `solver/src/spectrum.rs` tests, B63 |
| Modal combinations CQC (equal modal damping), SRSS, absolute sum | Papers: Der Kiureghian 1981; Wilson, Der Kiureghian and Bayo 1981 (CQC); textbook: Chopra (SRSS, sum) | B63, B64 |
| Directional combinations SRSS, absolute with a factor (0.3: the 100/30 rule), CQC3 | Common convention (SRSS, 100/30) | B64 |
| Diaphragm eccentricity: story forces the sizes of the differences of the case's combined story shears (the story forces at each story's bottom cut), the larger of ratio x By x FX and ratio x Bx x FY per diaphragm times its share of the story's whole mass, as joint moments by mass (none outside the diaphragms), the static response added as a size | OSAB's own rule | B64 |
| Story forces leave out what passes through a rigid link crossing the cut | OSAB's own rule | - |
| The absolute directional combination's factor when none is given: 1 (the plain sum) | OSAB's own rule | - |
| Base reactions: the full sum of the support reactions, also of the force a rigid link brings to a support (the reference program leaves that force out of its totals, not of its joint reactions) | OSAB's own choice (the totals balance the loads) | B63 |
| Spectrum cases in combinations as +R and -R | OSAB's own rule | B64 |
| ASCE 7-16 design spectrum: SDS, SD1 (11.4.4-11.4.5), the curve of 11.4.6, Fa and Fv from Tables 11.4-1 and 11.4-2 with straight lines between columns; kept as points at 0, T0, TS and fixed periods above TS + 0.1 s, straight lines between | OSAB's own rule | B63 |
| Story results: joint drift against the joint at the same plan position on the level below over the story height; story drift the largest by size; story forces at a cut below each level and above the level below, about (0, 0) at the level's height, an element's own loads inside it | OSAB's own rule | B64 |
| A joint at a level with no joint under it on the level below: its drift against the lower end of a frame going down from it, wherever that end is, over the height between them; without such a frame, no drift. Story forces: an element counts only in the story holding its highest joint, and only at a cut whose level has one of its joints | OSAB's own rule | - |
| Story results of a frame the mesh step splits go by the object, not its pieces. Story forces: only in the story of the object's highest joint, at a cut whose level has one of the object's joints, the force taken from the piece the cut crosses. Joint drift: a frame going down from a joint means the object, so its lower end is the object's, and a joint partway along an object has no such frame | OSAB's own rule | - |
| P-Delta options: none, non-iterative from the mass, iterative from loads (load patterns times factors); every linear case (static, modal, so spectrum, combinations) on the P-Delta stiffness; no tolerance setting (the reference program's has no effect) | OSAB's own rule | B68, B69, B70 |
| Frame geometric stiffness: the consistent one, pd ∫ N'ᵀ N' dx with the element's own bending shape functions (cubic; the Timoshenko element's with a shear area), both planes, one element per frame | Textbooks: Przemieniecki, *Theory of Matrix Structural Analysis*, ch. 15 | `elements::frame` tests, B68 |
| A member's P-Delta axial force: the mean of its axial force over the length, rigid zones included | OSAB's own rule | `pdelta` tests, B68 |
| Rigid end zones: the flexible part's geometric stiffness through the rigid arms, each arm a bar carrying the axial force (pd a on its turn); flexible offsets (factor 0) as the whole length | OSAB's own rule | `elements::frame` tests, B68 |
| End releases: K + KG condensed together; a truss gets the string pd / L sideways | OSAB's own rule | B68 |
| A section with a product of inertia under P-Delta: the same geometric stiffness in its principal axes; the moments' P-Delta part turned back with the others. Pier and spandrel forces with P-Delta: the members' forces at the cut with their geometric stiffness | OSAB's own rule | - |
| Rigid links carry no P-Delta | OSAB's own rule | B68 (LNK) |
| The P-Delta load's iterations: constant-stiffness iterations on the stiffness without P-Delta, at most 10, ending as soon as the unbalance has grown twice in a row (the first comparison not counted); then Newton from no displacement again, the first iteration on that stiffness and each later one on K + KG at the last iterate, 40 at most, given up earlier when four Newton iterations in a row bring no new lowest unbalance (OSAB's own choice: a P-Delta load that does not converge then fails after a few factorizations, not forty); stop when the unbalance's size is below 1e-4 of the size of the joint loads and the support reactions' forces; the stiffness at the last iterate for every case | OSAB's own rule | `pdelta` tests, `mesh/tests/benchmarks.rs` (`p_delta_iteration_counts_match_the_logs`), B68, B70 |
| Shell geometric stiffness: ∫ ∇Nᵀ S ∇N dA of the membrane forces on the three translations, bilinear N, S at the quad's 2 x 2 points, nothing on the rotations; a triangle: linear N, its mean forces, three quarters of the result | Textbook: Cook, Malkus and Plesha, *Concepts and Applications of FEA* (initial-stress stiffness) | B70 |
| Frame forces with P-Delta: end forces (Ke + KG) u; moments along the member the usual statics plus pd (v(x) - v at end i), v the element's own shape (on the flexible part; the arms as bars) | OSAB's own rule | `elements::frame` tests, B68 |
| That shape on a member with a bending release: a released end's turn from the plain bending stiffness, without the geometric stiffness, shear deformation or the member's loads, and plain cubics between the hinges (both planes, even with a release in one). A truss stays straight. Geometric stiffness of a member released at its clear-length ends: per piece, split only at an end with a bending release | OSAB's own rule | - |
| Centres of rigidity with P-Delta: from loads, on the stiffness without the geometric stiffness; from the mass, with its springs | OSAB's own rule | - |
| Deflection along a member with P-Delta: the curvature of its moments with their P-Delta part; what is left at end j (the element's own error) spread linearly | OSAB's own choice (the reference results compared have no deflection between joints) | `solver/tests/station_displacements.rs` (against a finely divided column) |
| Mass method: a spring -m g / z in UX and UY from each joint with mass to the ground, z its height above the base level (the lowest story level; without stories the lowest joint); none at or below it; the springs count in the base reactions and the equilibrium check, not as joint reactions | OSAB's own rule | B69 |
| A structure that buckles under its P-Delta load (negative pivots): a note, results given anyway; negative eigenvalues reported with negative periods and frequencies | OSAB's own rule | COLS_M |
| Story forces with P-Delta: a frame's internal forces at the cut (its P-Delta moment included); a shell's forces from its joints above a story's top cut and below its bottom cut | OSAB's own rule | B70 |
| Equation ordering: approximate minimum degree on the joint graph (supervariables, mass elimination, element absorption, very dense joints last) | Papers: Amestoy, Davis and Duff 1996; George and Liu 1989; textbook: Davis 2006, ch. 7 | `solver/src/order.rs` tests, all benchmarks |
| Elimination tree, postorder, supernodes (merged with a small parent when few zeros are added) | Papers: Liu 1986, 1990; Liu, Ng and Peyton 1993; Ashcraft and Grimes 1989; textbook: Davis 2006, ch. 4 | `solver/src/order.rs` tests, all benchmarks |
| Mesh step searches (areas that may overlap, areas over a point, segments that may cross, elements under a load; the lines, joints and shared points that reach an area, attached joints inside a face, frames that may cross, points a frame passes through) through a uniform grid of bounding boxes grown by more than the checks' tolerance; the same exact checks on the candidates, in the same order | Common method (spatial grid) | `mesh/src/boxes.rs` tests; both real project models' analysis models unchanged byte for byte; all benchmarks unchanged |
| Section shapes: A, I22, I33, I23 and the centroid as exact integrals of the outline (circles exact); shear areas As = I² / ∫ Q²/b over 50 strips with each strip's averaged width (circles exactly; pipes A·3(1 + m²) / 4(1 + m + m²); parts more than 6 strips apart: the whole area); J by Saint-Venant warping on quadratic triangles, circles as 24-sided polygons, a steel pipe the polar moment of the 24-sided ring | Textbook (shear flow, Saint-Venant torsion, finite elements) | - |
| Composite sections: transformed to the steel (Ec/Es; J from both materials bonded with Gc/Gs); an encased circle's concrete a 24-sided polygon, a filled pipe's circles 32-sided; self weight each material's area times its weight | OSAB's own rule | - |
| Steel section list: AISC Shapes Database v16.0, A, Ix, Iy, J as tabulated (double angles 2 J of the angle); shear areas per type from the dimensions; angles' I23 from the outline with the root fillet (kdes - t); centroids from x and y | OSAB's own rule | `mesh/src/aisc.rs` tests |
| Product of inertia I23: the member bends about its principal axes (tan 2θ = 2 I23 / (I33 - I22)), shear areas turned as flexibilities without the coupling term; results in local 2-3. The shear centre offset changes nothing (members twist about the centroid) | Textbook (unsymmetric bending) | - |
| Pier and spandrel labels: a pier label counts on walls with \|n.z\| < 0.001 and frames with ends within 0.001 m in plan, a spandrel label on such walls and frames with ends within 0.001 m in height; others are kept and left out | OSAB's own rule | - |
| Pier stories: an object at the story holding its highest joint; the cuts at the lowest and highest labelled joint | OSAB's own rule | - |
| Spandrel stories: a wall at the level nearest its middle height (the base too; halfway goes up), a frame at its own story and starting a spandrel at its nearest level only above the base; multi-story: one at the highest such level (the base counts as the first story) | OSAB's own rule | - |
| Spandrel validity: every corner within 0.001 m of the first wall's plane (of the vertical plane through the first frame); joints above the highest level give no force, all above it no spandrel | OSAB's own rule | - |
| Pier and spandrel forces: the joint forces on the labelled elements on the material side of each cut (frame end forces, shell K u less the element's own loads, an area's edge-constraint beams and frame-joint springs with it, each with its own side), moved to the cut's centroid: F = s Σ f, M = s Σ (m + (x - c) × f), s = +1 top / right, -1 bottom / left; P V2 V3 = F along 1 2 3, T = M along 1, M2 = -M along 2, M3 = M along 3 | OSAB's own rule | B67 (hand: statics), `solver/tests/piers.rs` |
| Cut centroids: wall strips (length × thickness at their middle) and frames (section area at the joint) by area, L and C corners not subtracted; a cut touching only corners at one plan point: (x, y, 0) | OSAB's own rule | - |
| Pier axis angle: walls in model order, each from the first corner of its lowest edge; grouped greedily with the first group whose first wall has \|n · n_g\| > 0.99 and whose first corner is that one's or lies along its plane (\|unit(v - v_g) × n_g\| > 0.99); the longest group at the bottom cut (else the top) gives axis 2 = its first wall's horizontal direction in (-90, 90]; without walls there the first frame's local 2 in (-180, 180]; judged in the model's own units | OSAB's own rule | - |
| Spandrel axes: the longest plan distance between two corners or frame ends (the first of equal ones) gives the cuts' direction; axis 1 from the left centroid to the right one, horizontal; 2 = +Z | OSAB's own rule | - |
| Pier and spandrel forces of a spectrum case: each mode's signed forces combined like any other value; of combinations: combined like any other value | OSAB's own rule | `gui/src/ipc/wasm.test.ts` |
| A stored mesh's area loads go to its elements as their own loads (global X, Y, Z parts) when its elements cover it (less openings) and no other area overlaps it, so pier, spandrel and story forces leave them out like any element load; otherwise they are shared out as before | OSAB's own choice (the same loads as before, given to the elements) | all stored-mesh benchmarks unchanged |
| Cases set not to run (`not_run`): no results for them, nor for a combination with a part that did not run (nested too); the modal case runs anyway while a spectrum case runs; a later run keeps finished cases; deleting the modal case's results deletes the spectrum cases' | OSAB's own rule | - |
| Nearest joint at a story level (mass lumping) through square plan cells, searched ring by ring outward until no farther cell can hold a nearer joint; the same distance and the same tie rule (first in node order) | Common method (spatial grid, nearest neighbour) | `solver/src/mass.rs` test `plan_grid_finds_the_nearest_joint_like_checking_every_joint`; mass benchmarks unchanged |
| Edge recovery of the triangle mesher by flips: the edge map kept up to date at each flip instead of rebuilt; of the edges crossing a missing edge, the first flippable one in edge order | Textbook (edge flips: de Berg et al., Computational Geometry, ch. 9) | `mesh/src/tri.rs` tests; both real project models' meshes unchanged byte for byte |
| Threads (an option): subtrees of the elimination tree factorized and solved at the same time, the top of the tree after them in order; large dense updates split by tile columns; every sum in the same order as on one thread | Papers: Liu 1990; Geist and Ng 1989 | `solver/src/sparse.rs` test `threads_give_the_same_numbers` (bit for bit); the real project model and all benchmarks unchanged with threads |
| Sparse LDLᵀ without pivoting, supernodal and multifrontal (blocked dense kernels); several right-hand sides in one solve; the Sturm count from the pivots' signs | Papers: Duff and Reid 1983; Liu 1992; textbooks: Davis 2006, ch. 4 and 9; Bathe, ch. 8 | `solver/src/sparse.rs` tests, all benchmarks |

## References
- Przemieniecki, J. S. (1968). *Theory of Matrix Structural Analysis*. McGraw-Hill. Geometric stiffness of
  a beam (consistent, cubic shapes): chapter 15.
- Cook, R. D., Malkus, D. S., Plesha, M. E. and Witt, R. J. (2002). *Concepts and Applications of Finite
  Element Analysis*, 4th ed. Wiley. Initial-stress (geometric) stiffness of plane and plate elements: the
  chapter on buckling.
- McGuire, W., Gallagher, R. H. and Ziemian, R. D. (2000). *Matrix Structural Analysis*, 2nd ed. Wiley.
- Bathe, K.-J. (2014). *Finite Element Procedures*, 2nd ed. Prentice Hall / K. J. Bathe. Subspace
  iteration and the Sturm sequence check: section 11.6. LDLᵀ factorization of the stiffness:
  chapter 8.
- Batoz, J.-L., Bathe, K.-J. and Ho, L.-W. (1980). A study of three-node triangular plate bending
  elements. *International Journal for Numerical Methods in Engineering* 15(12), 1771-1812. (DKT)
- Batoz, J.-L. and Tahar, M. B. (1982). Evaluation of a new quadrilateral thin plate bending element.
  *International Journal for Numerical Methods in Engineering* 18(11), 1655-1677. (DKQ)
- Allman, D. J. (1984). A compatible triangular element including vertex rotations for plane elasticity
  analysis. *Computers & Structures* 19(1-2), 1-8. (drilling field, triangles)
- Allman, D. J. (1988). A quadrilateral finite element including vertex rotations for plane elasticity
  analysis. *International Journal for Numerical Methods in Engineering* 26(3), 717-730. (drilling
  field, quads)
- Hughes, T. J. R. and Brezzi, F. (1989). On drilling degrees of freedom. *Computer Methods in Applied
  Mechanics and Engineering* 72(1), 105-121. (drilling penalty)
- Chopra, A. K. (2017). *Dynamics of Structures*, 5th ed. Pearson. (response spectrum analysis, ch. 13)
- Der Kiureghian, A. (1981). A response spectrum method for random vibration analysis of MDF systems.
  *Earthquake Engineering and Structural Dynamics* 9(5), 419-435. (CQC)
- Wilson, E. L., Der Kiureghian, A. and Bayo, E. P. (1981). A replacement for the SRSS method in seismic
  analysis. *Earthquake Engineering and Structural Dynamics* 9(2), 187-192. (CQC)
- Menun, C. and Der Kiureghian, A. (1998). A replacement for the 30 %, 40 % and SRSS rules for
  multicomponent seismic analysis. *Earthquake Spectra* 14(1), 153-163. (CQC3)
- Newmark, N. M. and Hall, W. J. (1982). *Earthquake Spectra and Design*. EERI. (spectral amplification
  factors and their damping dependence)
- ASCE/SEI 7-16 (2017). *Minimum Design Loads and Associated Criteria for Buildings and Other
  Structures*. American Society of Civil Engineers. (design spectrum, section 11.4)
- Davis, T. A. (2006). *Direct Methods for Sparse Linear Systems*. SIAM. (elimination tree and sparse
  Cholesky: ch. 4; minimum degree ordering: ch. 7; supernodal and multifrontal methods: ch. 9)
- Amestoy, P. R., Davis, T. A. and Duff, I. S. (1996). An approximate minimum degree ordering
  algorithm. *SIAM Journal on Matrix Analysis and Applications* 17(4), 886-905.
- George, A. and Liu, J. W. H. (1989). The evolution of the minimum degree ordering algorithm. *SIAM
  Review* 31(1), 1-19.
- Liu, J. W. H. (1986). A compact row storage scheme for Cholesky factors using elimination trees.
  *ACM Transactions on Mathematical Software* 12(2), 127-148. (the elimination tree with path
  compression)
- Liu, J. W. H. (1990). The role of elimination trees in sparse factorization. *SIAM Journal on Matrix
  Analysis and Applications* 11(1), 134-172.
- Geist, G. A. and Ng, E. (1989). Task scheduling for parallel sparse Cholesky factorization.
  *International Journal of Parallel Programming* 18(4), 291-314.
- Liu, J. W. H., Ng, E. G. and Peyton, B. W. (1993). On finding supernodes for sparse matrix
  computations. *SIAM Journal on Matrix Analysis and Applications* 14(1), 242-252.
- Ashcraft, C. and Grimes, R. (1989). The influence of relaxed supernode partitions on the multifrontal
  method. *ACM Transactions on Mathematical Software* 15(4), 291-309.
- Duff, I. S. and Reid, J. K. (1983). The multifrontal solution of indefinite sparse symmetric linear
  equations. *ACM Transactions on Mathematical Software* 9(3), 302-325.
- Liu, J. W. H. (1992). The multifrontal method for sparse matrix solution: theory and practice. *SIAM
  Review* 34(1), 82-109.
- de Berg, M., Cheong, O., van Kreveld, M. and Overmars, M. (2008). *Computational Geometry: Algorithms
  and Applications*, 3rd ed. Springer. (Delaunay triangulations and edge flips: ch. 9)
- Sloan, S. W. (1993). A fast algorithm for generating constrained Delaunay triangulations. *Computers &
  Structures* 47(3), 441-450.
- *CSI Analysis Reference Manual* (Computers and Structures, Inc.): sections
  10.2.1, 10.3, 10.6 and 10.18 as cited above.
- AISC Shapes Database v16.0. American Institute of Steel Construction, Chicago (aisc.org): the steel
  section list (`mesh/src/aisc16.json`).
