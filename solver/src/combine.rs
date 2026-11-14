//! Load combinations. A Linear Add of load cases and one-valued combinations is one
//! linear static case with the scaled loads (`as_linear_case`), solved like any other case. Every other
//! combination has a maximum and a minimum, worked out value by value from its parts ([`Order`]). An SRSS
//! combination is the square root of the sum of its parts' squares, as +R (max) and -R (min); a part with
//! a maximum and a minimum counts with the larger size of the two. A response spectrum case is a part
//! with two values, +R and -R (its results are sizes without a sign).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use crate::axes;
use crate::elements::{self, Props, Side};
use crate::model::{ComboType, Id, LoadCase, LoadCombination, Model};
use crate::results::{station_layout, CaseResults, ElementResults, Step};

/// Frame stations without point loads (the solver's default).
pub const STATIONS: usize = 11;

fn combo<'a>(model: &'a Model, id: &Id) -> &'a LoadCombination {
    model
        .load_combinations
        .iter()
        .find(|c| &c.id == id)
        .expect("validated combination id")
}

fn case<'a>(model: &'a Model, id: &Id) -> &'a LoadCase {
    model
        .load_cases
        .iter()
        .find(|c| &c.id == id)
        .expect("validated case id")
}

/// True when a combination has one value per result: a Linear Add of one-valued parts.
pub fn is_single(model: &Model, c: &LoadCombination) -> bool {
    c.kind == ComboType::LinearAdd
        && c.items.iter().all(|it| match (&it.combo, &it.case) {
            (Some(id), _) => is_single(model, combo(model, id)),
            (None, Some(id)) => !model.is_spectrum(id),
            (None, None) => true,
        })
}

/// The load case a one-valued combination equals: every part's loads, scaled (nested scale factors
/// multiply). None for combinations with a maximum and a minimum.
pub fn as_linear_case(model: &Model, c: &LoadCombination) -> Option<LoadCase> {
    if !is_single(model, c) {
        return None;
    }
    fn add(model: &Model, c: &LoadCombination, s: f64, out: &mut LoadCase) {
        for it in &c.items {
            match (&it.case, &it.combo) {
                (Some(id), _) => out.add(case(model, id), s * it.scale),
                (None, Some(id)) => add(model, combo(model, id), s * it.scale, out),
                (None, None) => {}
            }
        }
    }
    let mut out = LoadCase {
        id: c.id.clone(),
        ..Default::default()
    };
    add(model, c, 1.0, &mut out);
    Some(out)
}

/// Load cases and one-valued combinations a combination is made of (directly or through others).
fn leaves<'a>(model: &'a Model, c: &'a LoadCombination, out: &mut Vec<&'a Id>) {
    for it in &c.items {
        let (id, nested) = match (&it.case, &it.combo) {
            (Some(id), _) => (id, None),
            (None, Some(id)) => {
                let sub = combo(model, id);
                (id, (!is_single(model, sub)).then_some(sub))
            }
            (None, None) => continue,
        };
        match nested {
            Some(sub) => leaves(model, sub, out),
            None if !out.contains(&id) => out.push(id),
            None => {}
        }
    }
}

/// Every value of a result in a fixed order: joint displacements, reactions, frame and link stations
/// (P V2 V3 T M2 M3 ux uy uz), shell corners (F11 … V23), pier and spandrel forces, the base reaction,
/// joint drifts, story forces (last: the spectrum's eccentricity finds them there).
pub(crate) fn values(r: &CaseResults) -> Vec<f64> {
    values_with(r, &r.frames)
}

/// [`values`] with `frames` in place of the result's own frames (the same frames at other stations).
fn values_with(r: &CaseResults, frames: &[ElementResults]) -> Vec<f64> {
    let mut v = Vec::new();
    for d in &r.displacements {
        v.extend(d.values.as_array());
    }
    for f in &r.reactions {
        v.extend(f.values.as_array());
    }
    for e in frames.iter().chain(&r.links) {
        for s in &e.stations {
            v.extend([s.P, s.V2, s.V3, s.T, s.M2, s.M3, s.ux, s.uy, s.uz]);
        }
    }
    for sh in &r.shells {
        for c in &sh.corners {
            v.extend(c.as_array());
        }
    }
    for p in &r.pier_forces {
        v.extend(p.top);
        v.extend(p.bottom);
    }
    for s in &r.spandrel_forces {
        v.extend(s.left);
        v.extend(s.right);
    }
    if let Some(b) = r.base_reaction {
        v.extend(b);
    }
    for j in &r.joint_drifts {
        v.extend(j.drift);
    }
    for f in &r.story_forces {
        v.extend(f.top);
        v.extend(f.bottom);
    }
    v
}

/// `template` with its values replaced by `v` (same order as [`values`]).
pub(crate) fn with_values(template: &CaseResults, v: &[f64]) -> CaseResults {
    let mut r = template.clone();
    let mut it = v.iter().copied();
    let mut next = || it.next().expect("as many values as the template");
    for d in &mut r.displacements {
        d.values = std::array::from_fn::<f64, 6, _>(|_| next()).into();
    }
    for f in &mut r.reactions {
        f.values = std::array::from_fn::<f64, 6, _>(|_| next()).into();
    }
    for e in r.frames.iter_mut().chain(r.links.iter_mut()) {
        for s in &mut e.stations {
            for x in [
                &mut s.P, &mut s.V2, &mut s.V3, &mut s.T, &mut s.M2, &mut s.M3, &mut s.ux,
                &mut s.uy, &mut s.uz,
            ] {
                *x = next();
            }
        }
    }
    for sh in &mut r.shells {
        for c in &mut sh.corners {
            let a: [f64; 8] = std::array::from_fn(|_| next());
            *c = crate::results::ShellCorner::new(c.node.clone(), a);
        }
    }
    for p in &mut r.pier_forces {
        p.top = std::array::from_fn(|_| next());
        p.bottom = std::array::from_fn(|_| next());
    }
    for s in &mut r.spandrel_forces {
        s.left = std::array::from_fn(|_| next());
        s.right = std::array::from_fn(|_| next());
    }
    if let Some(b) = &mut r.base_reaction {
        *b = std::array::from_fn(|_| next());
    }
    for j in &mut r.joint_drifts {
        j.drift = std::array::from_fn(|_| next());
    }
    for f in &mut r.story_forces {
        f.top = std::array::from_fn(|_| next());
        f.bottom = std::array::from_fn(|_| next());
    }
    r.story_drifts = crate::story::story_drifts(&r.joint_drifts);
    r
}

/// A result made into the frame of an envelope: values only at the stations (no end forces or loads
/// to evaluate elsewhere), no equilibrium check.
pub(crate) fn envelope_template(r: &CaseResults) -> CaseResults {
    let mut t = r.clone();
    for er in t.frames.iter_mut().chain(t.links.iter_mut()) {
        er.enveloped = true;
        er.loads.clear();
        er.end_forces = [0.0; 12];
        er.end_displacements = [0.0; 12];
    }
    t.equilibrium = None;
    t
}

/// A frame's results evaluated at other stations (forces exactly, deflections from the member's own
/// end displacements and loads); an envelope's values are taken from its own stations (it has one at
/// every position asked for).
fn restation(er: &ElementResults, props: &Props, at: &[(f64, Side)]) -> ElementResults {
    let mut out = er.clone();
    if er.enveloped {
        let tol = 1e-9 * er.length.max(1.0);
        out.stations = at
            .iter()
            .map(|&(x, side)| {
                let mut near = er.stations.iter().filter(|s| (s.x - x).abs() <= tol);
                let s = match side {
                    Side::Left => near.next(),
                    Side::Right => near.next_back(),
                };
                let mut s = *s.expect("an envelope has a station at every combination station");
                s.x = x;
                s
            })
            .collect();
        return out;
    }
    out.set_stations(at);
    let xs: Vec<f64> = at.iter().map(|a| a.0).collect();
    let disp = elements::local_displacements(
        props,
        &er.end_displacements,
        &er.end_forces,
        &er.loads,
        &er.hinges,
        &xs,
    );
    for (s, d) in out.stations.iter_mut().zip(disp) {
        [s.ux, s.uy, s.uz] = axes::to_global(&er.axes, d);
    }
    out
}

/// The parts of a combination in the order they are worked through, a part used twice named twice:
/// load cases and one-valued combinations, those of its combinations with a maximum and a minimum
/// in their place.
fn parts_in_turn<'a>(model: &'a Model, c: &'a LoadCombination, out: &mut Vec<&'a Id>) {
    for it in &c.items {
        match (&it.case, &it.combo) {
            (Some(id), _) => out.push(id),
            (None, Some(id)) => {
                let sub = combo(model, id);
                if is_single(model, sub) {
                    out.push(id);
                } else {
                    parts_in_turn(model, sub, out);
                }
            }
            (None, None) => {}
        }
    }
}

/// Where frames have point loads inside them, per frame (model order) with any: the positions,
/// sorted, each once. A combination's stations are equally spaced plus both sides of these.
type Points = BTreeMap<usize, Vec<f64>>;

fn layout(er: &ElementResults, points: Option<&Vec<f64>>) -> Vec<(f64, Side)> {
    station_layout(
        er.offsets[0],
        er.length - er.offsets[1],
        points.cloned().unwrap_or_default(),
        STATIONS,
    )
}

/// A result's frames at the stations of `points`.
fn restationed(r: &CaseResults, props: &[Props], points: &Points) -> Vec<ElementResults> {
    r.frames
        .iter()
        .enumerate()
        .map(|(e, er)| restation(er, &props[e], &layout(er, points.get(&e))))
        .collect()
}

/// Adds a part's values, times `s`, to a combination's maximum and minimum so far. `hi` is the
/// part's maximum and `lo(k)` its minimum (the same value for a part with one value).
fn fold<L: Fn(usize) -> f64>(
    kind: ComboType,
    s: f64,
    max: &mut [f64],
    min: &mut [f64],
    hi: &[f64],
    lo: L,
) {
    // A negative factor turns the part's minimum into the larger value.
    let pair = |k: usize| {
        if s >= 0.0 {
            (s * hi[k], s * lo(k))
        } else {
            (s * lo(k), s * hi[k])
        }
    };
    match kind {
        ComboType::LinearAdd => {
            for k in 0..hi.len() {
                let (a, b) = pair(k);
                max[k] += a;
                min[k] += b;
            }
        }
        ComboType::Envelope => {
            for k in 0..hi.len() {
                let (a, b) = pair(k);
                max[k] = max[k].max(a);
                min[k] = min[k].min(b);
            }
        }
        ComboType::Srss => {
            for k in 0..hi.len() {
                let (a, b) = pair(k);
                let r = a.abs().max(b.abs());
                max[k] += r * r;
            }
        }
    }
}

/// A combination being worked out: its next part, and its maximum and minimum so far (for SRSS,
/// `max` collects the sum of squares until the end).
struct Level<'a> {
    combo: &'a LoadCombination,
    item: usize,
    max: Vec<f64>,
    min: Vec<f64>,
}

impl<'a> Level<'a> {
    fn new(combo: &'a LoadCombination, n: usize) -> Self {
        let (max, min) = match combo.kind {
            ComboType::Envelope => (vec![f64::NEG_INFINITY; n], vec![f64::INFINITY; n]),
            _ => (vec![0.0; n], vec![0.0; n]),
        };
        Level {
            combo,
            item: 0,
            max,
            min,
        }
    }
}

/// A station layout (`Order::stations`) and a part.
type At = (usize, Id);

/// The values of solved parts that a combination still has to take, by station layout and part,
/// each with the number of times it is still to be taken.
#[derive(Default)]
struct Held {
    values: HashMap<At, (Rc<Vec<f64>>, usize)>,
    /// How many times the combinations of a layout take each part.
    uses: HashMap<At, usize>,
}

impl Held {
    fn put(&mut self, stations: usize, id: &Id, v: Vec<f64>) {
        let key = (stations, id.clone());
        let uses = self.uses[&key];
        self.values.insert(key, (Rc::new(v), uses));
    }

    fn len(&self, stations: usize, id: &Id) -> Option<usize> {
        self.values
            .get(&(stations, id.clone()))
            .map(|(v, _)| v.len())
    }

    /// A part's values for one use; forgotten after the last.
    fn take(&mut self, stations: usize, id: &Id) -> Option<Rc<Vec<f64>>> {
        let key = (stations, id.clone());
        let (v, left) = self.values.get_mut(&key)?;
        let v = v.clone();
        *left -= 1;
        if *left == 0 {
            self.values.remove(&key);
        }
        Some(v)
    }
}

/// A combination with a maximum and a minimum, worked out part by part in the order of its parts
/// (and of the parts of its own combinations with a maximum and a minimum), each as soon as it is
/// solved and the ones before it are in: only the two running results are kept, not the parts.
struct Running<'a> {
    /// Which station layout (`Order::stations`).
    stations: usize,
    /// The first part: the frame of the two results is made from its result.
    first: &'a Id,
    template: Option<Rc<CaseResults>>,
    /// The combination, and inside it the combinations with a maximum and a minimum being worked out.
    stack: Vec<Level<'a>>,
    /// Number of values of a result (known with the first part).
    n: usize,
    done: Option<(Vec<f64>, Vec<f64>)>,
}

impl<'a> Running<'a> {
    /// Takes every part that is in, as far as their order allows.
    fn advance(&mut self, model: &'a Model, top: &'a LoadCombination, held: &mut Held) {
        if self.done.is_some() {
            return;
        }
        if self.stack.is_empty() {
            let Some(n) = held.len(self.stations, self.first) else {
                return;
            };
            self.n = n;
            self.stack.push(Level::new(top, n));
        }
        loop {
            let at = self.stack.len() - 1;
            let combo_at = self.stack[at].combo;
            let Some(it) = combo_at.items.get(self.stack[at].item) else {
                // This combination is complete: its values go into the one it is part of.
                let mut level = self.stack.pop().expect("a level");
                if level.combo.kind == ComboType::Srss {
                    for (hi, lo) in level.max.iter_mut().zip(&mut level.min) {
                        *hi = hi.sqrt();
                        *lo = -*hi;
                    }
                }
                let Some(parent) = self.stack.last_mut() else {
                    self.done = Some((level.max, level.min));
                    return;
                };
                let s = parent.combo.items[parent.item].scale;
                let lo = &level.min;
                fold(
                    parent.combo.kind,
                    s,
                    &mut parent.max,
                    &mut parent.min,
                    &level.max,
                    |k| lo[k],
                );
                parent.item += 1;
                continue;
            };
            let (id, spectrum) = match (&it.case, &it.combo) {
                (Some(id), _) => (id, model.is_spectrum(id)),
                (None, Some(id)) => {
                    let sub = combo(model, id);
                    if !is_single(model, sub) {
                        self.stack.push(Level::new(sub, self.n));
                        continue;
                    }
                    (id, false)
                }
                (None, None) => {
                    self.stack[at].item += 1;
                    continue;
                }
            };
            let Some(v) = held.take(self.stations, id) else {
                return;
            };
            let level = &mut self.stack[at];
            let (kind, s) = (level.combo.kind, it.scale);
            if spectrum {
                fold(kind, s, &mut level.max, &mut level.min, &v, |k| -v[k]);
            } else {
                fold(kind, s, &mut level.max, &mut level.min, &v, |k| v[k]);
            }
            level.item += 1;
        }
    }
}

/// One entry of the results' order: a load case or one-valued combination (solved as a case), or a
/// combination with a maximum and a minimum (`model.load_combinations[k]`).
enum Next {
    Solved(Id),
    Bounds(usize),
}

/// Hands the results on in their order (load cases, spectrum cases, then combinations in model order, those with a
/// maximum and a minimum as max then min) while the cases come from the solver one by one.
///
/// A combination with a maximum and a minimum takes each of its parts as soon as that part is solved
/// (see [`Running`]), whatever the order the combinations are listed in, so no part's results are kept
/// for it. A part listed after such a combination is solved ahead for it ([`Order::plan`],
/// [`Order::ahead`]) and again at its own turn, so its results do not wait either. Only response spectrum
/// cases, which come before the load cases are solved, wait for their turn.
pub struct Order<'a> {
    model: &'a Model,
    props: &'a [Props],
    order: Vec<Next>,
    at: usize,
    waiting: HashMap<Id, CaseResults>,
    /// The station layouts of the combinations with a maximum and a minimum (most share one).
    stations: Vec<Points>,
    /// Those combinations until they are handed on, by their place in the model.
    running: Vec<Option<Running<'a>>>,
    /// For each part not solved yet, the layouts its values are wanted at.
    wanted: HashMap<Id, Vec<usize>>,
    held: Held,
}

impl<'a> Order<'a> {
    /// `points`: for the load cases and one-valued combinations with point loads inside frames,
    /// the frame (model order) and position of each.
    pub fn new(
        model: &'a Model,
        props: &'a [Props],
        points: &HashMap<Id, Vec<(usize, f64)>>,
    ) -> Self {
        let mut order: Vec<Next> = model
            .load_cases
            .iter()
            .map(|c| &c.id)
            .chain(model.spectrum_cases.iter().map(|c| &c.id))
            .map(|id| Next::Solved(id.clone()))
            .collect();
        let mut stations: Vec<Points> = Vec::new();
        let mut running = Vec::with_capacity(model.load_combinations.len());
        let mut wanted: HashMap<Id, Vec<usize>> = HashMap::new();
        let mut held = Held::default();
        for (k, c) in model.load_combinations.iter().enumerate() {
            if is_single(model, c) {
                order.push(Next::Solved(c.id.clone()));
                running.push(None);
                continue;
            }
            order.push(Next::Bounds(k));
            let mut ids = Vec::new();
            leaves(model, c, &mut ids);
            // Its stations: both sides of any part's point loads.
            let mut own = Points::new();
            for (e, x) in ids.iter().filter_map(|id| points.get(*id)).flatten() {
                own.entry(*e).or_default().push(*x);
            }
            for at in own.values_mut() {
                at.sort_by(|a, b| a.total_cmp(b));
                at.dedup();
            }
            let si = stations.iter().position(|p| *p == own).unwrap_or_else(|| {
                stations.push(own);
                stations.len() - 1
            });
            for id in &ids {
                let at = wanted.entry((*id).clone()).or_default();
                if !at.contains(&si) {
                    at.push(si);
                }
            }
            let mut turns = Vec::new();
            parts_in_turn(model, c, &mut turns);
            for id in turns {
                *held.uses.entry((si, id.clone())).or_default() += 1;
            }
            running.push(Some(Running {
                stations: si,
                first: ids[0],
                template: None,
                stack: Vec::new(),
                n: 0,
                done: None,
            }));
        }
        Order {
            model,
            props,
            order,
            at: 0,
            waiting: HashMap::new(),
            stations,
            running,
            wanted,
            held,
        }
    }

    /// The load cases and one-valued combinations in the order to solve them, for the results to be
    /// handed on in their order without waiting: each at its turn (`false`: for [`Order::push`]); one
    /// that is part of a combination with a maximum and a minimum listed before it also ahead of that
    /// combination (`true`: for [`Order::ahead`]).
    pub fn plan(&self) -> Vec<(Id, bool)> {
        let mut seen: HashSet<&Id> = HashSet::new();
        let mut plan = Vec::new();
        for next in &self.order {
            match next {
                Next::Solved(id) => {
                    if !self.model.is_spectrum(id) {
                        seen.insert(id);
                        plan.push((id.clone(), false));
                    }
                }
                Next::Bounds(k) => {
                    let mut ids = Vec::new();
                    leaves(self.model, &self.model.load_combinations[*k], &mut ids);
                    for id in ids {
                        if !self.model.is_spectrum(id) && seen.insert(id) {
                            plan.push((id.clone(), true));
                        }
                    }
                }
            }
        }
        plan
    }

    /// Gives a solved case's values to the combinations with a maximum and a minimum it is part of
    /// (once: a case solved ahead and again at its turn counts the first time).
    fn take_part(&mut self, case: &CaseResults) {
        let Some(layouts) = self.wanted.remove(&case.id) else {
            return;
        };
        for si in layouts {
            let frames = restationed(case, self.props, &self.stations[si]);
            let v = values_with(case, &frames);
            // The frame of the results of the combinations that begin with this part.
            let mut begun = self
                .running
                .iter_mut()
                .flatten()
                .filter(|r| r.stations == si && *r.first == case.id)
                .peekable();
            if begun.peek().is_some() {
                let mut r = case.clone();
                r.frames = frames;
                let template = Rc::new(envelope_template(&r));
                for run in begun {
                    run.template = Some(template.clone());
                }
            }
            self.held.put(si, &case.id, v);
        }
        for (run, c) in self.running.iter_mut().zip(&self.model.load_combinations) {
            if let Some(run) = run {
                run.advance(self.model, c, &mut self.held);
            }
        }
    }

    /// Takes a case solved ahead of its turn (see [`Order::plan`]): only its values are used.
    pub fn ahead(&mut self, case: &CaseResults) {
        let mut clock = crate::timing::Clock::new();
        self.take_part(case);
        clock.lap("combinations: ordering and bounds");
    }

    /// Takes a solved case and hands on every result that is now next in order.
    pub fn push<E>(
        &mut self,
        case: CaseResults,
        out: &mut dyn FnMut(CaseResults) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut clock = crate::timing::Clock::new();
        self.take_part(&case);
        self.waiting.insert(case.id.clone(), case);
        while let Some(next) = self.order.get(self.at) {
            match next {
                Next::Solved(id) => match self.waiting.remove(id) {
                    Some(c) => {
                        clock.lap("combinations: ordering and bounds");
                        out(c)?;
                        clock.lap("results: output callback");
                    }
                    None => break,
                },
                Next::Bounds(k) => {
                    let run = &mut self.running[*k];
                    if run.as_ref().is_none_or(|r| r.done.is_none()) {
                        break;
                    }
                    let run = run.take().expect("a combination worked out");
                    let (max, min) = run.done.expect("worked out");
                    let template = run.template.expect("the first part's result");
                    let c = &self.model.load_combinations[*k];
                    let step = |v: Vec<f64>, step: Step| {
                        let mut r = with_values(&template, &v);
                        r.id = c.id.clone();
                        r.combination = Some(c.kind);
                        r.spectrum = false;
                        r.step = Some(step);
                        r
                    };
                    let hi = step(max, Step::Max);
                    let lo = step(min, Step::Min);
                    drop(template);
                    clock.lap("combinations: ordering and bounds");
                    out(hi)?;
                    out(lo)?;
                    clock.lap("results: output callback");
                }
            }
            self.at += 1;
        }
        clock.lap("combinations: ordering and bounds");
        Ok(())
    }

    /// Whether every result has been handed on.
    pub fn done(&self) -> bool {
        self.at == self.order.len()
    }
}
