# B57 - Load patterns

Load patterns (type, self-weight multiplier, joint, frame and area loads) and linear static load cases that
apply patterns times scale factors. One bay of four 3 m columns with four beams and a Shell-Thin slab (one
element, meshing off), and a separate 3 m cantilever column at x = 10 m.

Patterns: Dead (dead, self-weight x 1), SDL (super dead: slab and beam B1), Live (live: slab, a point load on
B1, the cantilever's top), Roof (roof live: part of B3), Wind (wind: joint loads in X and Y), E (seismic), SW2
(other: self-weight x 0.5 and a moment).

Cases: one per pattern for Dead, SDL, Live and Wind; C1 = 1.2 Dead + 1.2 SDL + 1.6 Live - 0.8 Wind; TWICE =
Dead + Dead + 0.5 Live (a pattern applied twice); EMPTY (no patterns); SW2; MIXED = Roof + 2 E. Combinations:
COMB1 = C1 + 0.5 Wind (Linear Add), ENV = envelope of C1 and TWICE. Modal case (6 modes) with mass from the
elements and from the patterns SDL x 1 and Live x 0.25.

References: hand (statics of the reactions).

Reference files: `hand.json` (50 values).
