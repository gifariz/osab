# B35 - Edge constraints (floors)

Two 2 x 2 m Shell-Thin floors side by side at z = 3 m: F1 meshed 4 x 4 (0.5 m), F2 left as one element
("mesh": "none"), so F1's three joints on the shared edge lie inside F2's edge; a 1.5 m beam starts at the
middle of F2's far edge (not a corner). The mesh step ties such joints with edge constraints (on by default),
and the solver makes them hidden beams along the edge, see docs/phases/PHASE3B_PLAN.md). F1's left edge is
fixed (corners fixed, edge restraints on). DEAD: 4 kN/m^2 on both; TIP: 5 kN down and 2 kN in Y at the beam's
end.

Hand: reaction totals.

Reference files: `hand.json` (3 values).
