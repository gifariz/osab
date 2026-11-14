# B34 - Two-story wall core with a door, floors meeting the walls (auto-mesh)

An L-shaped core of 0.25 m Shell-Thin walls (4 m along X, 3 m along Y; two 3 m stories, one wall object per
story) with a 1 x 2 m door in the first-story X wall, 0.2 m floors (4 x 3 m) at z = 3 and 6 m, and a 0.4 x 0.4
m column at the free corner. The mesh step meshes the walls along their local axes (Auto Rectangular, 1.25 m)
through the door's lines and the floors' joints, and the floors to 1 x 1 m, so floors and walls share their
joints. Wall bases (with edge restraints), the door's bottom corners and the column base are fixed. DEAD: 5
kN/m^2 on both floors; WIND: 30 kN in X and 20 kN in Y at the roof corner, 15 kN in X at the first floor.

Hand: base reaction totals.

Reference files: `hand.json` (3 values).
