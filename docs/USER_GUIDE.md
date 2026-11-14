# OSAB user guide: model a portal frame, a slab and a wall

OSAB works like the usual building analysis programs: draw on a grid, assign properties, run, look at results.
Z is up. Values are shown and typed in the model's display units (see *Units* below); the examples here
use SI. Sections 1-5 model a portal frame (5b adds load combinations); section 6 adds slabs and walls.
The app speaks English and Indonesian (**Options > Preferences > Language**) and has light and dark
themes (**Options > Preferences > Theme**: light by default, or dark, or *Same as Windows*).
**File > Open Example…** opens three finished models (a portal frame, a slab on columns, an L-shaped
floor on beams) with verified results ([VERIFICATION.md](VERIFICATION.md)); press F5 to run one. They
open with their objects named and labelled as objects you draw are (see *Names and labels* in section 3), and with
what drawn objects get: automatic end offsets, the drawn insertion points and, in 3D, the semi-rigid diaphragm D1
on floors and story joints.

**Help in windows:** a window with explanations has a **?** beside its **×**. It shows or hides the explanations
of every window (the choice is kept); notes about the model or the selection always show. Short labels such as
f11 or I33 show their full name when the pointer rests on them.

**Web demo:** the same app runs in a browser at https://gifariz.com/osab/. Everything stays in
your browser. **Save** downloads the model as an `.osab` file and **Open** loads one from your computer.
The web demo opens with the L-shaped floor example. Models of any size open and can be edited and saved
(files up to 200 MB); analysis runs on models up to 8,000 analysis joints (joints plus mesh joints). A run
that takes longer than half a second shows **Running… · Stop** in the status bar. **Help > Web Demo…** shows
the welcome with the examples.

## 1. New model
**File > New Model**: X grid spacings `6`, Y grid spacings empty (one frame line), story heights `4`, OK.
Models are always 3D. Click **Elev X-Z** on the toolbar to see the frame line with grid lines A and B and
levels Base and Story1.

**Stories: Define > Story Data.** A table of the stories, top down: each story's name and height, and
the base with its elevation (the elevations follow). **+** adds a story above a row (as high as that row;
3 m above the base), **✕** deletes one. On **OK** the structure moves with its stories, as one undo step:
- every object belongs to a story: a joint to the lowest level at or above it (below the base to the
  base, above the top level to the top story); a frame or area to the story of its highest joint (a
  column to the level at its top);
- every joint keeps its distance to its own story's level, so columns and walls stretch with their
  story's height, a joint between levels keeps its distance below the level above, and everything above
  a story that grows moves up; a new story moves what is above it up by its height;
- loads along frames keep their place as a fraction of the length (a load at mid-height stays there);
- deleting a story deletes its structure (its columns, walls, braces, the beams and floors at its level;
  the dialog says how many objects first); the story above grows to cover it, so the levels above stay,
  and its columns reach down to the story below. User wind loads on a deleted story go; renamed stories
  keep their wind loads;
- an edit that would make a story lower than a joint between its levels is refused;
- a stored mesh moves with its area, or is removed (with a note) when the area changes shape.
**Edit > Edit Grid** sets the X and Y grid lines only.
A new model already has steel and concrete, a 300 x 300 frame section `FSEC1`, 200 mm area sections
`Slab1` (a slab) and `Wall1` (a wall), both Shell-Thin concrete, a filled deck `Deck1`, and the load patterns `Dead` and `Live` with a
load case each.

**Define > Area Sections** asks, in order: name, shell type, **property type** (Slab, Deck or Wall), the slab type
(Slab, Drop, Stiff; a wall has only "Wall") or deck type, material(s) and dimensions. The property type sorts the
lists (Assign > Area Section and the draw panel show slab sections, then decks, then walls). Slab and wall sections
analyse alike. Any section may go on any area: an area is a floor or a wall by its slope. Check
Model notes a floor with a wall section or a wall with a slab or deck section, in case it was a slip.

A **Deck** section is always a Membrane floor spanning one way along its local axis 1 (the ribs' direction), so
its weight and loads go to the beams or walls at the ends of that span. Its type is Filled (concrete on a steel deck),
Unfilled (the steel deck alone) or Solid slab; give the slab material (the concrete) and the deck material (the
steel), the slab depth above the ribs, the rib depth, the rib widths at top and bottom, the rib spacing, the deck's
shear thickness and its unit weight. The dialog shows the weight per area: the concrete per area times the slab
material's unit weight, plus the deck's unit weight. In its plane a filled deck is as stiff as its slab above the
ribs, a solid slab as its slab, an unfilled deck as a steel sheet of the shear thickness. Like any Membrane floor,
a deck is cut at the beams and walls that cross it into panels, each spanning between its own beams.
**Tables > Deck Sections** lists every deck with its weight. Plan views show the span direction of every one-way
floor (decks and one-way Membranes): a line along local axis 1 with a slash at each end. Files saved before
property types existed open with each section typed by its areas: Wall if only walls use it, else Slab.

For plank or deck floors, give a **Membrane** area section **Use special one-way load distribution**
(**Define > Area Sections**; the box shows for Membrane only): the floor's loads then span
along its local axis 1 (**Assign > Area Section / Local Axes** turns it) to the beams or walls at each end, instead
of to all sides.

For a frame that only works in its own plane, use **Analyze > Analysis Options > Plane frame in X-Z**
(only UX, UZ and RY are active). Then all joints must lie at the
same Y and loads must act in the X-Z plane. For a floor alone (a slab with its beams) choose
**Plane grid in X-Y** (only UZ, RX and RY): all joints at one Z, horizontal areas, vertical loads and
moments about X and Y; put point supports where the columns are. The status bar shows which option is on.

## Units
Every model is stored in kN, m and °C (mass in t), whatever you choose to see. The box at the right of
the status bar (also **Options > Preferences** and **File > New Model**) picks one of three display
systems; each shows every quantity in its usual unit:

| Quantity | SI | MKS | US |
|---|---|---|---|
| Coordinates, lengths, story heights | m | m | ft |
| Section sizes, thickness | mm | mm | in |
| Section area A, As2, As3 | cm² | mm² | in² |
| Section I22, I33, J | cm⁴ | mm⁴ | in⁴ |
| Displacements | mm | mm | in |
| Forces / moments | kN / kN·m | tonf / tonf·m | kip / kip·ft |
| Line loads, shell F and V | kN/m | tonf/m | kip/ft |
| Area loads | kN/m² | kgf/m² | lb/ft² |
| Shell moments | kN·m/m | tonf·m/m | kip·ft/ft |
| E, G | MPa | kgf/mm² | lb/in² |
| Weight per volume | kN/m³ | kgf/m³ | lb/ft³ |
| Joint masses / mass moments of inertia | kg / t·m² | kg / tonf·m·s² | lb·s²/ft / kip·ft·s² |
| Springs | kN/m, kN·m/rad | tonf/mm, tonf·mm/rad | kip/in, kip·in/rad |

The choice is saved in the model file. Changing it never changes the model and keeps the results (it is
not an undo step). Tables, labels, diagrams and the contour legend show their units. The drawing snap
step (Options > Preferences) is always in mm.

## 2. Define
- **Define > Materials**: the material type (Steel, Concrete or Other), E and Poisson's ratio (steel: E = 200000 MPa,
  ν = 0.3), the unit weight, and the strengths of its type (steel Fy and Fu, concrete f'c, none for Other). The type
  and strengths stay with the model (for design and exchanging models) and change no result; new deck sections take
  the first steel material as their deck material and the first concrete one as their slab material.
  **Tables > Materials** lists them.
- **Define > Frame Sections**: *Add New*, give a name, then type the properties (General) or pick a
  shape and its dimensions: rectangle / plate, I (unequal flanges too), channel, tee, angle, double angle,
  double channel, box / tube, pipe, circle / rod, concrete tee and L (tapered), concrete pipe, cross,
  trapezoid, concrete-filled steel tube and pipe, and an I-section encased in a concrete rectangle or
  circle. The dialog draws the shape (the dot is its centroid) and shows A, I33, I22, I23, J and the shear
  areas as the analysis uses them. Angles and L sections have a product of inertia I23: pushed along one
  leg they also move along the other. A composite section counts its concrete with Ec/Es (Gc/Gs for J)
  and gives its frames the steel material; its self weight counts each material with its own weight.
  *From the steel list (AISC)* picks a section of the AISC Shapes Database v16.0 (W, M, S, HP, C, MC, WT,
  MT, ST, L, 2L, HSS, pipe): choose the type, or type part of a name in *Search*, then the section. Its
  A, I and J are the tabulated values (shapes are drawn without fillets).
  I33 is the major axis (bending in the plane for a beam).
- **Define > Load Patterns**: *Add New*, name `WIND`, type *Wind*, *Add* (a load case `WIND` comes with it).
- **Define > Pier Labels / Spandrel Labels** (also Assign > Pier Label / Spandrel Label): *Add New*, a
  name, *Add* (a spandrel label may be *multi-story*: one spandrel over every story). Select walls and
  frames and press *Give to selected frames and areas*; *Take label from selected* removes it. See
  *Pier and spandrel forces* in section 6.
- **Define > Groups** (also Select > Select by Group): named sets of objects. *Add New*, a name, *Add*; then
  select objects and press *Add Selected Objects* (only what is selected goes in: a frame's joints do not
  come with it). For a group's objects: *Select*, *Add to Selection*, *Deselect*, *Show Only*, *Hide*,
  *Show*. *All* is every object. Deleting objects takes them out of their groups.

## 3. Draw
- **Frame** tool (toolbar): pick the section on the left, then click grid points joint to joint.
  Each click adds a member from the previous point. Right-click or Esc ends the chain.
  End i follows the usual rule (lower Z, then lower X, then lower Y), whatever order you click in.
- **Joint** tool: click to add joints (joints made by drawing members and areas too).
- **Names and labels.** Every object has a *name*, which never changes: joints 1, 2, 3…, frames M1,
  M2…, areas A1, A2…, rigid links K1, K2…. It also has a *label* by its plan position, the same on every
  story: J for joints, C = column, B = beam, D = brace by direction (null frames too), F = floor, W =
  wall by slope (openings and null areas too), L for links. The column at one grid point is C1 on every
  story; a beam between the same two points is the same label whichever way it was drawn; the two
  diagonals of an X brace get two labels; a column spanning two stories, or a joint between levels, gets
  its own. A new object under one on another story takes its label; an object moved onto another's
  position takes that label; a new position gets the next number. Labels follow the type (a beam tilted
  into a brace becomes a D). Labels are saved in the model file. The views show labels (View > Show Labels; names, sections and more in View > Set Display Options),
  Properties shows the name, the label and the story.
- **One story / All stories** (status bar, next to the units): with *All stories*, clicking an object (or
  a window selection) also selects the objects with the same label on every story, e.g. the column at one
  grid point on all floors.
- In **plan and elevation views** you can click anywhere: the point snaps to grid intersections and
  joints when the mouse is close, otherwise it is rounded to the snap step (0.1 mm by default; change it
  in **Options > Preferences**). The status bar shows the coordinates before you click.
- In the **3D view** points snap to grid intersections and joints only.
- **Draw > Joint by Coordinates** adds a joint at typed coordinates.
- **Special joints.** Deleting frames, links or areas also deletes the joints they leave with nothing
  attached, with any restraints, springs, masses and loads on them. A *special* joint stays.
  Joints drawn on their own (Joint tool, Joint by Coordinates) are special; **Assign > Special Joint**
  turns it on or off for selected joints, and Properties shows it. Turning it off for a joint with
  nothing attached deletes that joint (Ctrl+Z brings it back).
- **Edit > Move** (Ctrl+M) moves the selection by dX, dY, dZ, typed or taken from two points clicked in
  the view (**Pick in View**: the dialog hides; click the first point, then the second, snapping to
  joints and grid points; Esc goes back). Selected joints move and stretch the members attached to them.
  A selected frame, link or area takes along the joints only moving objects use; a joint it shares with
  an object that stays is left there, and the moved object gets a joint at the new place. Joints that
  land on a joint join it. Point and distributed loads on stretched members keep their place as a
  fraction of the length.
- **Edit > Replicate** (Ctrl+R) copies the selection, on four tabs: **Linear** (dX, dY, dZ, typed or two
  picked points, and the number of copies), **Radial** (about a vertical axis through a typed or picked
  point, by an angle counterclockwise seen from above, with the number of copies; *Rotate local axes
  where applicable* turns the local axes of columns and floors with them), **Mirror** (about the vertical
  plane through two points) and **Story** (to the ticked stories: each joint keeps its distance to its
  story's level, so a column reaches the level of a taller story). Copies use joints already at their
  corners; a copy of an object already there is left out (the message says how many). Ticks choose what
  goes along: restraints, springs and masses of selected joints, loads (joint loads of selected joints,
  frame and area loads), diaphragms; **Delete original objects** (off by default) deletes the selection
  afterwards. The copies are selected, ready for another Move or Replicate.

## 4. Assign
Select first, then assign:
- **Select** tool: click objects to add them to the selection (click again to remove), or drag a box.
  Left-to-right selects what is fully inside; right-to-left also selects what the box touches.
  Esc clears the selection.
- **Assign > Joint Restraints**: select the two base joints, click *Fixed*, OK.
- **Assign > Frame Loads**: select the beam, load pattern Dead, direction *Gravity*, w = 15 (positive is
  down), OK.
- **Assign > Joint Loads**: select the top left joint, load pattern WIND, FX = 10, OK.
No loads are drawn at first. **Display > Show Loads…** picks a load pattern whose loads are then drawn: joint and
frame loads as arrows, area loads as their value written on the area (e.g. *-6 kN/m² Z*; no arrows)
(*None* draws none), and **Display > Hide Loads** hides them again. In the Model explorer,
double-click a load pattern (or right-click > *Show Loads*) to show its loads; right-click it again for
*Hide Loads*. Assigning loads shows the pattern they were assigned to.

**Load patterns and load cases**: loads always belong to a **load pattern** (**Define > Load Patterns**:
a name, a type such as Dead, Super Dead, Live, Reducible Live, Roof Live, Snow, Seismic, Wind,
Temperature, Hydrostatic, Construction, Notional or Other, and a self-weight multiplier). The analysis
runs **load cases** (**Define > Load Cases**): each is a list of patterns times scale factors, e.g.
`1.2 Dead + 1.6 Live`; a pattern may appear twice, a case with no pattern gives zero. Adding a pattern
also adds a case of the same name; renaming the pattern renames that case too; deleting the pattern
deletes it (a pattern another case still uses cannot be deleted). The type does not change the
analysis (it is kept for later design features). Results, tables and combinations are per load case.
Files saved before load patterns open with one pattern per case and give the same results.
Example: tests/benchmarks/b57_load_patterns.

**User wind loads** (story forces, e.g. from a wind tunnel): in **Define > Load Patterns** a pattern of
type *Wind* has a table of rows story, diaphragm, Fx, Fy, Mz and the point X, Y (*Add Row*, or paste rows
copied from a spreadsheet into *Paste rows* and *Add pasted rows*), then *Apply wind loads*. *Load
point*: *Centre of mass* (the default; X and Y are greyed out and not used: the load acts at the
diaphragm's centre of mass at that story) or *Specified coordinates (X, Y)*. On a rigid diaphragm the load
acts through the diaphragm at that point (the point may lie outside the floor). *On semi-rigid
diaphragms*: *Into the floor at the point* (the default; where the point is on no floor element of that
diaphragm, it is spread by mass instead, with a note), *Spread over the diaphragm's joints by mass* (as a
rigid floor's inertia forces: the totals and the torsion stay the same) or *Through the joint nearest the
centre of mass*. No joints are added, so moving or
deleting a load leaves nothing behind. The loads are drawn as arrows at their points when the pattern
is shown. Examples: tests/benchmarks/b58_user_wind, b59_diaphragm_centres.

**Centres of mass and rigidity**: after a run of a model with diaphragms, **Display > Tables > Centers of
Mass and Rigidity** lists per story and diaphragm the masses, the centre of mass, the cumulative mass and
centre (this story and every story above) and the centre of rigidity (where a force on that diaphragm alone
moves it without turning it). A semi-rigid diaphragm shows its centre of mass only. Plan views at a story
draw the centres as points labelled *CM D1* and *CR D1*.

**Self-weight**: **Define > Materials** has the *Weight per unit volume*, **Define > Load
Patterns** a *Self weight multiplier* per pattern: 1 adds the structure's own weight to that pattern, 0
leaves it out (a case gets its patterns' multipliers times their factors). A new model starts with Dead
at multiplier 1, concrete 23.5631 and steel 76.9729 kN/m³. To
compare with a model analysed elsewhere without self-weight, set the multiplier to 0. Weight modifiers (Property
modifiers, *Weight*) scale it per section or object. Example: tests/benchmarks/b47_self_weight.

**End releases** (pins): select members, **Assign > Frame Releases**, tick the end forces that
are zero at the start (end i) and end (end j); *Pinned* ticks M2 and M3 at both ends (a simply supported
beam, a pinned brace). With *End releases* ticked in View > Set Display Options, released ends are drawn with a small dot. Sets that would make a member unstable
(e.g. torsion released at both ends) are refused. Example: tests/benchmarks/b46_end_releases.

**End length offsets** (rigid zones at beam-column joints): select members, **Assign > Frame
End Length Offsets**: *Automatic from connectivity* (a beam end reaches to the face of the column below
it, a column top to the soffit of the deepest beam, from the section depth and width), *Define lengths*
(at end i and end j) or *None*, and the *Rigid-zone factor* (0 to 1: that part of the offsets is rigid
in bending and shear). New members get automatic offsets with factor 0. With *End length offsets* ticked in
View > Set Display Options, offsets are drawn as thicker lines; they are listed in the Frames table; frame forces are reported over the clear length, and
releases act at the clear-length ends. To compare with a model analysed elsewhere without offsets, select all and
choose *None*. Examples: tests/benchmarks/b48_end_offsets, b49_auto_offsets.

**Insertion points** (where the section sits on the line of its joints): select members, **Assign >
Frame Insertion Point**: the *cardinal point* (1 bottom left to 9 top right on the box around the
section, bottom and top being its -2 and +2 faces, left and right its -3 and +3 sides; 10 centroid; 11
shear centre), *Mirror about local 2 / 3*, *joint offsets* from each joint to the cardinal point (local
1, 2, 3 or global X, Y, Z) and *Offsets change the stiffness*. Areas: **Assign > Shell Insertion Point**,
*Top*, *Middle* or *Bottom* (the face on the plane of the joints) and the same stiffness option. New
columns are drawn at 5 (middle centre), beams at 8 (top centre), braces at 10, floors at their top and
walls at their middle, all with the stiffness option off: then the insertion point only moves the object
in the extruded view. With it on, the member is analysed at its section's centroid (the shell at its
mid-plane), tied to its joints by rigid arms; its forces are reported there, and loads act there. A
floor at its top over beams at their top, both with the option on, act together as T-beams. Properties,
the Frames and Areas tables and View > Set Display Options (*Insertion point*: "IP 8 T", T = stiffness
transformed) show them. Automatic end offsets do not depend on them, and neither does the mass: it stays
at the joints (half of a member's to each end, a floor's to its corners) wherever the section sits.

**Rigid diaphragms**: a new model has diaphragm D1 (semi-rigid). New floors and new joints at a story
level above the base get D1 (the first diaphragm) automatically. **Define > Diaphragms** adds or deletes
others, and makes one *Semi-rigid* (it then ties nothing: the floor's own in-plane stiffness acts, and the
diaphragm only groups the floor, e.g. for user wind loads). Select joints or floors (areas) and use **Assign > Diaphragm**: at each story the joints of a
diaphragm then move together in their plane (UX, UY and RZ), while each keeps its own vertical movement
and rotations. A floor's diaphragm reaches all its joints, mesh joints too; a joint between two story levels
belongs to the story above. With *Diaphragm extent* ticked in View > Set Display Options, diaphragm joints
are drawn in purple, with dashed lines from the outer joints to the centre and the diaphragm's name there;
the Joints and Areas tables list them. At most one joint per diaphragm and story may be restrained in UX, UY or
RZ. Examples: tests/benchmarks/b50_rigid_diaphragm, b51_floor_diaphragms.

**Modal analysis**: a new model has a modal case (12 modes). **Define > Mass Source and
Modal Case** sets where the mass comes from (the elements' own mass from the material weight, added joint
masses, load patterns times a factor; lateral mass only and lumping at the story levels, the defaults)
and the number of modes, or turns the modal case off. The modal case finds **Eigen vectors** (the
lowest modes) or **Ritz vectors**: these start from loads (ground accelerations UX, UY, UZ, ground rotations RX, RY, RZ about the
centre of mass, or load patterns, in the order listed; *Cycles* limits how many vectors one start gives, 0 = no limit), so
fewer modes reach the mass that a spectrum or the loads excite. A load on joints without mass gives a
*static* mode (no period). *Modal Load Participation Ratios* in Display > Tables says how much of each
acceleration and starting pattern the modes reach (static and dynamic %, aim for close to 100). **Assign > Joint Masses** adds mass to selected
joints (UX UY UZ in mass units, RX RY RZ as mass moments of inertia). After **Run**, **Display > Mode
Shape** shows a mode (pick it in the list with its period; *Animate* makes it swing, the slider scales
it), and **Display > Tables** has *Modal Periods* and *Modal Mass Ratios* (participating mass ratios in
the usual definition, with their running sums). Examples: tests/benchmarks/b52_modal_hand,
b53_modal_building.

**Property modifiers** (cracked sections): factors on section values, 1 = unchanged.
- On a section: **Define > Frame Sections** (or **Area Sections**), the *Property Modifiers…* button opens
  their window; *OK* there, then *Apply* in the section form. E.g. a beam section I33 = I22 = 0.35, a column section 0.7, a slab
  m11 = m22 = m12 = 0.25.
- On objects: select members and use **Assign > Frame Property Modifiers**, or areas and **Assign > Area
  Property Modifiers**. An empty box keeps each object's own value; *Reset to 1* clears them.
- An object is analysed with its section's modifiers **times** its own. A Stiff area section
  always uses fixed values (bending x 100). **Display > Tables > Frame Modifiers / Area
  Modifiers** lists every section and object with a modifier other than 1. Examples:
  tests/benchmarks/b44_frame_modifiers and b45_area_modifiers.

## 4b. Springs and rigid links
- **Assign > Joint Springs**: select joints, type the stiffness in UX, UY, UZ (kN/m) and RX, RY, RZ
  (kN·m/rad), global axes. The dialog shows the first joint's springs; *Replace* sets them,
  *Add* adds to them, *Delete* removes them. A spring can sit on a joint with restraints; on a restrained
  direction it has no effect. Springs are drawn as a small coil beside the joint, and their forces are
  reported with the reactions.
- **Line and area springs** (soil under a grade beam or a mat, soil against a wall): **Define > Line Spring
  Properties** / **Area Spring Properties** make named properties: stiffness per unit length along the
  frame's local 1, 2, 3 and about local 1 (kN/m/m, kN·m/rad/m), or per unit area along the area's local
  1, 2, 3 (kN/m³; U3 of a floor is the subgrade modulus). **Assign > Frame Line Springs** / **Shell Area
  Springs** put one on the selected frames or areas (not on null frames or openings). The analysis puts
  them at the joints of each analysis element: a frame end gets the stiffness times half the element's
  length, an area corner the stiffness times its share of the element (as a uniform area load is shared;
  a meshed floor shares its spring among its mesh joints). So a beam drawn as one long frame has its
  spring at its two ends only: draw it in pieces (or let a meshed floor cut it). The springs' forces are
  in the reactions of those joints. Springs resist tension and compression alike (no lift-off). Properties,
  the tables (*Spring Properties*, the Frames and Areas tables), the Model Explorer and View > Set
  Display Options (*Springs*) show them. Examples: tests/benchmarks/b60_beam_on_elastic_foundation,
  b61_mat_on_winkler_springs.
- **Draw > Rigid Link** (toolbar **Link**): click joint to joint, like the Frame tool. The two joints
  then move as one rigid body, e.g. a rigid zone where a beam meets a column, or a bracket. Links are
  named K1, K2… (labels L) and drawn as thick light lines. They are exact (no "very stiff member"), like a
  link with all six directions *Fixed*.
  - Links may not close a loop (a loop adds nothing; the joints are already rigid together).
  - Only one joint of a rigidly linked group may have restraints.
- Results: links follow the deformed shape, show force diagrams (click one in *Member forces*), and have
  a *Link Forces* table. Example: tests/benchmarks/b15_portal_springs_rigid_zones is a portal frame on
  springs with rigid end zones.

## 5. Run and results
- **F5** (Analyze > Run Analysis). OSAB first checks the model (no supports, joints not connected,
  and so on) and tells you what to fix. The desktop app uses every processor core.
- **An unstable model still runs.** Where the structure has no stiffness (a part that is not held, a
  mechanism), the run goes on with a tiny stiffness there, and the messages warn and list the joints and
  directions. Results at and near those joints are not reliable: a load there shows as an absurdly large
  displacement. (A model file can ask the run to stop instead: `"on_instability": "stop"`.)
- **Results are saved with the model** (desktop app): **Save** after a run also writes a folder
  `<name>.results` next to the model file, and opening the model brings its results back without a new
  run, as long as the model in the file is the one they were made for (any change needs a new run). A
  case's values are read only when you show them, and during a run the cases of a large model go to a
  temporary folder as they are solved, so models with many load cases need little memory.
- **A model too big for the computer's memory** (desktop app): before the heavy part of a run, OSAB estimates
  the memory it needs. If that is more than the free memory (less a reserve), the run stops and says how much
  is needed and how much is free; a coarser mesh, fewer combinations with a maximum and a minimum, or closing
  other programs helps. You can also choose **Run anyway**, but the computer may become very slow until the
  run ends. (Command line: `osab solve --force`; `--memory 12` sets the free memory to assume, in GB.)
- **Analyze > Load Cases to Run** chooses which cases a run includes: untick *Run* for a case to leave it
  out. A case left out has no results, and neither has a combination that uses it (the dialog lists
  them). The modal case still runs while a response spectrum case runs, because the spectrum case needs
  its modes. *Status* shows which cases have results (*Finished*). Click a row to select it (Ctrl+click
  adds a row, Shift+click a range); the buttons switch the selected cases on or off or delete their
  results (deleting the modal case's results also deletes the spectrum cases'; combinations that use a
  deleted case go too). A run keeps the results of finished cases even when they are set not to run.
  **OK** keeps the settings without running, **Run Now** keeps them and runs. Changing these settings
  keeps the results.
- **F6** deformed shape, **F7** member forces, **F8** reactions, **F4** back to the model.
- **F4** (Display > Undeformed Shape, toolbar *Model*) also returns to the default display: no loads,
  labels, extrusion, local axes or analysis mesh. The view, zoom, selection, hidden objects, results and
  the other display options stay.
- In *Member forces* pick the component (M3, V2, P, ...) on the left. Moments are drawn on the
  tension side (untick *Moments on tension side* to flip). Click a member to see its
  shear, moment and deflection along the length.
- **Ctrl+T** tables (Display > Tables) opens *Choose Tables* first: a tree of *Model Definition*
  (properties, connectivity, loads) and *Analysis Results* (joint, element and structure output, modal
  results, analysis mesh). Tick tables or whole groups, pick the load patterns and the load cases /
  combinations to show (Ctrl+click or Shift+click for several; all by default), then **OK**. Only then
  are the tables built and the chosen cases' results read, so a large model opens the dialog at once.
  Result tables list every chosen case, with a *Case* column first.
- In a table, click a column's heading to sort by it (again for descending, a third time for the table's
  order). The row under the headings filters each column: text finds the cells that contain it; for
  numbers `> 5`, `<= -2`, `= 0`, `!= 0` or a range `1..3` (in the units shown). *Copy as CSV* copies the
  rows as shown, *Save as CSV…* saves them as a file. **Export to Excel…** saves every chosen table into
  one Excel workbook, a sheet each (the table's name, the column names, the units, then the rows). Long tables draw only the rows on screen, so scrolling stays quick. *Choose Tables…*
  goes back to the tree; the choice is kept until the app closes.
- Any change to the model clears the results; press F5 again.

## 5b. Load combinations
**Define > Load Combinations**: *Add New*, a name, the **type** and rows of load case
(or another combination) and **scale factor**; *Add Row* for more, ✕ removes a row; *Add* saves.
- *Linear Add*: the scaled results added, e.g. ULS1 = 1.2 DEAD + 1.6 LIVE.
- *Envelope*: the largest and the smallest of the parts, e.g. ENV of ULS1, ULS2, ULS3.
- *SRSS*: the square root of the sum of the squared scaled parts, as a plus and a minus result (the
  sign of a factor makes no difference). A part that has a Max and a Min counts with the larger size.
Results: the *Load case* list on the left (and in the tables) also lists the combinations. A Linear
Add of plain cases gives one result; an Envelope or SRSS (or a Linear Add containing one) gives two, shown as e.g. **ENV (Max)** and **ENV (Min)**
(Max and Min steps). Each value is enveloped on its own, so the Max diagram of M3 and the Max of
V2 may come from different cases. Renaming a load case renames it in the combinations; a load case or
combination that a combination uses cannot be deleted. Example: tests/benchmarks/b26_load_combinations.

## 5c. Response spectrum
A response spectrum case gives the largest response of the structure to a design earthquake, from its
modes (the modal case) and a spectrum: the acceleration a single-mode structure feels against its period.
1. **Define > Response Spectrum Functions**: *Add New*, a name, and the **function type**:
   - *User table*: one row per point, the period in seconds and the value (e.g. in g), with a space, tab
     or comma between (rows copied from a spreadsheet paste straight in). Between points the value is a
     straight line; before the first point and after the last it stays the same.
   - *ASCE 7-16*: Ss, S1 (g), TL (s) and the site class. Fa and Fv come from the code's tables (leave
     them empty) or are typed in (class F and some class E sites need a site study). The dialog shows Fa,
     Fv, SDS and SD1.
   The **damping ratio** is the one the function is given for (usually 0.05). The plot shows the function.
2. **Define > Load Cases**: *Add New*, **Case type** *Response spectrum*, a name, and rows of
   **direction** (X, Y or Z), **function**, **scale factor** and **angle**. The scale factor turns the
   function's values into an acceleration: 9.80665 m/s² for a function in g (in US units 386.09 in/s²).
   The angle turns an X or Y row about Z. Then the case's **damping**, the **modal combination** (CQC, the
   usual; SRSS; Absolute), the **directional combination** (SRSS; Absolute with its factor, e.g. 0.3 for
   the 100/30 rule; CQC3) and the **eccentricity ratio** (e.g. 0.05: the floors' mass moved by 5 % of
   their width, as a torsion on each diaphragm). The first spectrum case turns the modal case on (12
   modes) when it is off; it cannot be turned off while spectrum cases exist.
3. **Run.** A spectrum case's results are **sizes** (all positive): each value is combined over the
   modes on its own, so the values in a row need not happen together. Use spectrum cases in **Load
   Combinations** like other cases: they count as plus and minus (e.g. 1.2 DEAD + EQX gives a Max and a
   Min).

**Display > Tables** after a run: *Base Reactions* (every case: the sum of the support reactions and
spring forces about the origin), and for models with stories *Story Drifts* (the largest of each story
and direction), *Joint Drifts* (each joint at a level over the joint below it, divided by the height
between them) and *Story Forces* (P, VX, VY, T, MX, MY at the top and the bottom of each story). Examples:
tests/benchmarks/b63_spectrum_columns, b64_spectrum_building.

## 5d. P-Delta
P-Delta adds the effect of the gravity loads acting on the deformed structure: columns under compression
get softer sideways. **Define > P-Delta Options** sets it for the whole model:
- *None* (the default).
- *Non-iterative, based on mass*: each joint with mass (the mass source's) pulls sideways by its weight
  over its height above the base. Quick, with no iterations; columns get no axial force from it.
- *Iterative, based on loads*: rows of **load pattern** and **scale factor** make the P-Delta load (e.g.
  DEAD x 1.2 and LIVE x 0.5). It is solved with P-Delta by iteration, and its axial forces in the frames
  and membrane forces in the shells give them their geometric stiffness.

Every case then uses that stiffness: load cases, combinations and the modal case (so spectrum cases too).
After **Run**, the messages in the side panel say how many iterations it took. A structure that buckles
under the P-Delta load still gives results, with a note, and its modes have negative periods: check the
model. Examples: tests/benchmarks/b68_p_delta_columns, b69_p_delta_mass, b70_p_delta_building.

## 6. Slabs and walls (areas)
Areas are flat shells: **floors** (sloping less than 20°, labels F1, F2…) and
**walls** (labels W1, W2…; openings and null areas get F or W by their slope too), with any number of corners (an L-shaped floor is one area). OSAB
**meshes them automatically** when you run: floors are cut at beams and walls and divided into elements
of at most 1.25 m (the defaults), walls likewise; joints line up where floors, walls and beams meet. Frames that cross
(within 1 mm) are joined there, and a frame crossing an area's edge, such as a column passing a floor's edge, is split
and tied to that edge.
**Analyze > Mesh Settings** sets the floor mesh (*General*, the default, or *Rectangular*) and the
maximum sizes, *Finer near openings and re-entrant corners* (General only, on by default: half-size
elements there, in irregular regions only; four-sided regions keep an equal grid). **Assign > Area Mesh Options** gives
selected areas their own size or no mesh (one element, then it needs 3 or 4 corners; make finer meshes with **Edit > Divide
Areas**; an area with more than 4 corners cannot be one, so it is meshed with the model's settings and Check Model and the
run messages warn), and the "add restraints on edges" option. Frames are joined where they cross and split at area edges
whatever the areas' mesh options. Files from older versions with no auto mesh for the whole model open with every area
set to no mesh. **View > Show
Analysis Mesh** draws the elements, also before a run. **Edit > Store Mesh** keeps the current mesh of
the selected areas in the file (a user mesh, drawn on the areas): it then stays as it is whatever
the mesh settings, and neighbouring areas line up with it. **Edit > Remove Stored Mesh** lets the mesher
mesh them again. If you move an area's corner afterwards, Check Model asks you to store it again.

Example (the benchmark tests/benchmarks/b24_slab_on_columns): a 6 x 6 m slab on four columns.
1. **File > New Model**: X and Y spacings `4*1.5`, story height `3.5`.
2. **Define > Frame Sections**: add `COL`, Rectangle 400 x 400 (mm). **Define > Area Sections** shows
   `Slab1` (Shell-Thin, 0.2 m); add your own there: type, material and thickness.
   - *Shell-Thin*: in-plane and bending (thin plate, no shear deformation), for slabs and walls.
   - *Membrane*: in-plane only (e.g. a wall that only takes in-plane forces).
   - *Shell-Thick*: like Shell-Thin plus shear deformation of the plate (thick slabs, mats). It
     is verified (deflections, moments and shears) on every tested shape:
     rectangles, parallelograms, tapered and general quads and triangles, also on distorted floor
     meshes (see Shell-Thick quad law).
3. Columns: **Elev X-Z**, **Frame** tool with section `COL` and material `Concrete`, click base to top;
   **PgUp** moves to the next grid line.
4. Slab: **Plan**, then the **Area** tool (Draw > Area) and click the corners in order around the area,
   then press **Enter**, right-click or click the first corner again (three corners make a triangle).
   In the draw panel's section list, **None** draws null areas ("None": no stiffness; loads on them go
   to the floor under them) and **Opening** draws openings, which cut a hole in the floor or wall they
   lie in.
   Floors always get local axis 3 up; walls keep the order you clicked.
5. Mesh: B24's reference values were computed without auto mesh, so for this example select everything
   (Ctrl+A) and choose **Assign > Area Mesh Options > No mesh** (and **Edit > Divide Areas** for finer areas: e.g. 2 x 2, a triangle
   becomes n1 x n1 triangles; joints are shared, section, local axes and loads kept). With auto-meshing
   (the default) the 6 x 6 m slab would be divided by the mesher instead.
6. Loads: select the areas (Ctrl+A selects everything in the view), **Assign > Area Loads**, direction
   *Gravity*, q = 8 kN/m² (positive is down). Other directions: global X, Y, Z or the area's local 1, 2, 3.
7. **Assign > Area Section / Local Axes** changes the section (or None), makes areas openings or back,
   turns local axes 1 and 2 about 3 (angle), or flips local 3 (*Reverse*). **Assign > Floor Meshing
   Option** is *Include in mesh* for joints (a joint with only a point load is left out of the
   mesh by default, its load going into the floor; a joint drawn on its own with the Joint tool or
   *Joint by Coordinates* is included) and frames (*Exclude* for a stiffener the floor should
   not be meshed along).
8. Supports at the column bases, **F5**.

**Local axes**: 3 is the normal; 1 is horizontal (+X for a floor with 3 up), 2 = 3 x 1.
**View > Show Local Axes** draws them (1 red, 2 white, 3 cyan) on members and areas.

**Results:**
- **Display > Shell Forces** (toolbar *Shells*): contours of F11, F22, F12 (membrane forces, kN/m),
  M11, M22, M12 (moments, kN·m/m), V13, V23 (shears, kN/m) and the principal values FMax, FMin, MMax,
  MMin, VMax. Values are per unit length in each area's local axes; positive M11 and M22 give tension at
  the bottom (the -3 face), so sagging in a floor with 3 up is positive. *Averaged at joints* smooths the
  contours; off, each element shows its own corner values (not averaged). Hover an area to
  see its corner values in the status bar (for a meshed area: the largest and smallest over its elements).
- **F6** shows the deformed areas (their mesh elements); **Ctrl+T** has the *Areas*, *Area Sections*,
  *Area Loads* and *Area Forces* tables (per element `F1-1`… and its area), and for a meshed model the
  *Mesh Joints* (`~1`…) and *Mesh Elements* tables.

**View > Extrude** (toolbar *Extrude*) draws members with their section and areas with their
thickness. Sections made with the Rectangle or I-section calculator keep their shape; general sections
are drawn as the rectangle with the same A and I33.

**Check Model** (Analyze) warns about areas that are not flat (errors; a 4-corner area may be warped: it is analysed as one element on its mean plane, not meshed) or not convex (errors), badly shaped
areas (a corner angle outside 45-135° or sides more than 4:1) and beams running along divided area edges
(connected only at their ends, unless the model is auto-meshed). Joints lying on an area edge are tied to
it (edge constraints), so they need nothing. For openings and overlapping areas it follows the
mesher's rules; a **note** says which area gets elements where two overlap (notes do not hold up F5).

**Pier and spandrel forces** (shear walls). A **pier** is a vertical wall strip, with the columns at
its ends if you like; a **spandrel** is a coupling beam or the wall over a door. Give them labels
(Define > Pier Labels, Spandrel Labels; section 2):
- A pier label counts on vertical walls and vertical frames; the objects with one label make one pier per
  story (the story of each object's highest joint).
- A spandrel label counts on vertical walls and horizontal frames; a wall belongs to the story level
  nearest its middle height. All of one spandrel's walls and frames must lie in one vertical plane.
- A label on any other object (a floor, a brace) is kept but does not count; the run's notes name it.
After **Run**, **Display > Tables > Piers and Spandrels**: *Pier Forces* (P, V2, V3, T, M2, M3 at each
pier's top and bottom), *Spandrel Forces* (at each spandrel's left and right end), and the section
properties (each pier's axis angle, the centroids of the cuts). The forces are what the labelled walls
and frames carry across each cut, about the cut's centroid: a pier's axis 1 is up and 2 runs along the
wall (along its longest walls at the bottom); a spandrel's 1 runs from its left end to its right, 2 is up.
P > 0 is tension. Combinations and spectrum cases have them too (a spectrum case gives sizes). To see the
labels on the objects, tick *Pier labels* / *Spandrel labels* in View > Set Display Options. Example:
tests/benchmarks/b67_pier_spandrel.

## 6b. Mesh a floor with an opening
Example (the benchmark tests/benchmarks/b42_l_floor_beams, also built by the click test
`gui/e2e/floor.spec.ts`): an L-shaped floor with a 1 x 2 m opening on beams and columns.
1. **File > New Model**: X and Y spacings `2*4`, story height `3`. **Define > Frame Sections**: `COL`
   (400 x 400 mm) and `BEAM` (500 deep, 300 wide).
2. Columns in the elevations (**Elev X-Z**, **Frame** tool, PgUp for the next grid line), beams in the
   **Plan** at the story round the L and on the inner grid lines.
3. The floor as **one** area: **Area** tool, click the six corners of the L in order and click the first
   corner again (or press Enter).
4. The opening: its corners off the grid are typed with **Draw > Joint by Coordinates** (1,5 / 2,5 /
   2,7 / 1,7 at z = 3), then the **Area** tool with the section list on **Opening** clicks them and Enter closes it.
5. **Analyze > Mesh Settings**: General or Rectangular and the maximum size (B42 uses Rectangular,
   1 m). **View > Show Analysis Mesh** shows the elements before you run: the floor is cut at the beams,
   and the opening and the beams get mesh lines.
6. Loads (**Assign > Area Loads** on the floor, **Assign > Frame Loads** on a beam), fixed supports at
   the column bases, **F5**. Shell forces are shown on the mesh elements (Display > Shell Forces).

To compare with another program: use the same mesh settings there. On four-sided regions and with the Rectangular
mesh the meshes are the same, so the numbers are (B32, B33, B42). On irregular regions of a General mesh
other programs may pave, and OSAB uses its own graded mesh; both tend to the same values as the mesh gets finer
(B36). For another program's exact numbers there, store its mesh on the area (B43; `areas[].user_mesh`).

## Model Explorer and hiding
The left panel shows the model explorer (**Model**) above **Properties**; drag the line between them up or down,
and its right edge to make the panel wider or narrower (double-click either for the default; both are kept on
this computer). The button at the top right (⧉) shows them as two tabs instead, and back (☰). **Model** is the explorer tree, drawn with tree
lines: materials, frame and area sections, load patterns, the object types with how many objects each has
(joints, frames as columns / beams / braces / null frames, areas as floors / walls / openings / null
areas, links, supports, springs; objects are not listed one by one), the stories (top down) and, after a
run, the results. Every branch starts closed: click its + box to open it. Load cases and combinations are in the Define menu, the stories in Define > Story Data, the grid in Edit > Edit Grid, the
analysis options in Analyze. The type follows from the direction
(within 1° of vertical a column, within 20° of horizontal a beam or floor, else a brace or wall; both
angles in **Options > Tolerances**), so it changes when you move a joint; the object keeps its name.
**Select > Object Type** selects (or deselects) every object of the ticked types, and **View > Set Display
Options** (*Objects present in view*) hides types from the view. **Properties** shows the
drawing settings, the selection and the results options (as tabs, it opens by itself when you draw or run).
With one object selected it shows three groups, each closed or opened by a click on its name (a closed group
stays closed for the next object): **Geometry** (name, label, story, coordinates or ends, length or area),
**Assignments** (section, material, type, axis angle, modifiers, releases, end offsets, insertion point, springs,
masses, diaphragm, pier and spandrel, mesh options…) and **Loads** (one row per load pattern with loads).
- **Edit in place:** a value with one choice is a drop-down (section, material, type, restraints, diaphragm,
  springs, pier, spandrel, *In mesh*…) and a number is typed (a joint's X, Y, Z, the axis angle; Enter or leaving
  the field applies it, Esc puts it back). Each change is one undo step.
- **Several values** (modifiers, releases, end offsets, insertion point, springs, masses, mesh) show a summary
  (*Default*, *Specified*, *None*, *Automatic*); click the row to see each value, and its **…** to open the window
  that edits it, for this object. The **…** of *Loads* opens the loads window.
- Short labels show their full name when the pointer rests on them (*End offsets*: End length offsets). The
  modifiers listed are the object's own (its section's modifiers are in Define > Frame Sections / Area Sections).
- The **?** at the top of Properties shows the side panel's explanations (the same switch as in windows); how to
  draw with the current tool is always in the status bar.
- **Double-click** a material or section to edit it, a load pattern to show its loads, a type (Columns,
  Joints, ...) to select its objects, a story to edit the stories (Story Data), a results case to show it.
- **Right-click** for more: Copy, Assign to Selected Members, Select / Deselect / Show / Hide / Show Only
  Objects Using It, Delete for materials and sections; Select, Deselect, Show, Hide, Show Only These for
  objects.
- The **dot** before a type hides or shows its objects.
- **Hide**, **Show Only** and **Show All** are also on the toolbar and in the View menu. Hidden objects are
  not drawn, picked or snapped to, and their results are not shown. The status bar counts them. Hiding is
  not saved in the file.

## Display options
**View > Set Display Options** (Ctrl+W; toolbar *Display*; in the web demo the menu or toolbar, as the
browser keeps Ctrl+W for closing the tab) chooses what the view shows. **OK** applies and closes, **Apply**
keeps the dialog open, **Default Options** sets everything back. The options are saved with the model file.
- **General**: *Colors by* sections (the default), object types or materials. *Objects present in view*:
  joints, columns, beams, braces, null frames, floors, walls, openings, null areas and links; *Invisible*
  under joints leaves out the joint dots (a selected joint or one under the mouse still shows its dot), while
  joints can still be clicked and their supports, springs, masses, loads and texts are still drawn; an unticked
  type is not drawn or picked, and Show All or Show in the Model Explorer does not bring it back (hiding
  and object types are two filters: an object is drawn when its type is ticked and it is not hidden).
  *Special effects*: shrink objects, fill areas (off: edges only), extrude frames, extrude areas. *Other
  items*: restraints and springs, diaphragm extent, grid (off for a new view), analysis mesh.
- **Object Assignments**: for joints, frames, areas and links, the texts and symbols drawn on each
  object. Texts (label, unique name, section or property, *Modified* when the object has its own property
  modifiers) are stacked in one label. Symbols: joint masses (an arrow along X, Y or Z with its value),
  end releases (a dot near the end), end length offsets (thicker lines), local axes (1 red, 2 white,
  3 cyan); frames and areas also their pier and spandrel labels. Everything is off for a new model.
- In a large model texts are written on the first 2000 objects in the view only; a note says how many
  were left out (a plan or elevation view shows fewer objects).
- **View > Show Labels**, **Show Local Axes**, **Extrude**, **Show Analysis Mesh** and **Show Grid** are
  quick switches of the same options (labels and local axes for every object kind at once).

## Mouse and keys
| Action | How |
|---|---|
| Zoom | mouse wheel |
| Pan | middle-drag (right-drag in plan and elevation views) |
| Rotate (3D view) | right-drag: the view turns about the point you start on (a joint, member or area; the model's middle on empty space, shown by a dot), Z stays up |
| Zoom to fit | Home |
| Turn the 3D view to a side, edge or corner | click the navigation cube (bottom right of the 3D view); drag it to rotate; its ⌂ button goes back to the default 3D view |
| Parallel 3D view (no perspective) or perspective | **Iso** on the toolbar, or View > Parallel 3D View; kept with the model's view options |
| Default display (model, no loads, labels, extrusion…) | F4 |
| Set display options (View > Set Display Options) | Ctrl+W |
| Show / hide the grid (View > Show Grid) | Ctrl+D |
| Next / previous story or grid line | PgUp / PgDn |
| Undo / redo | Ctrl+Z / Ctrl+Y |
| Delete selection | Del |
| Move / replicate the selection | Ctrl+M / Ctrl+R |
| Save / open | Ctrl+S / Ctrl+O |

Models are saved as `.osab` files: JSON inside, the same the solver reads ([MODEL_FORMAT.md](MODEL_FORMAT.md)), so
`osab solve model.osab` works on them too. Older `.json` model files still open; **Save** on one keeps it, and
**Save As** offers `.osab`. In the desktop app, double-clicking an `.osab` file opens it in OSAB.

OSAB is a tool for engineers: the engineer using it is responsible for the model, for checking its results and for the design decisions made with them.
