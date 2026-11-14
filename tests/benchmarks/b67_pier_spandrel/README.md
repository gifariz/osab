# B67: pier and spandrel forces

A 6 x 3 m shear wall (0.2 m, Shell-Thin, auto mesh) with a 2 x 2 m door in the middle and a header above it
drawn as its own area, fixed along the base; model in kN and m. Pier label P on the wall and the header,
spandrel label S on the header; one story (Base at 0, L1 at 3 m). Six load cases at the top right joint (FX,
FY, FZ, MX, MY, MZ) and all six together (L). The pier's bottom cut runs either side of the door (4 x 0.2 m2),
its top cut along the whole wall (6 x 0.2 m2), both centred at x = 3; the spandrel's cuts are the header's
ends at x = 2 and x = 4.

- Hand: the pier by statics (the whole structure is the pier, so its forces are the load moved to the cuts'
  centroids, checks/make_pier_benchmark.py).

Reference files: `hand.json` (84 values).
