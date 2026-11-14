//! Response spectrum cases (docs/phases/RESPONSE_SPECTRUM_PLAN.md): spectrum functions, each mode's
//! amplitude under a ground acceleration, and the modal and directional combination of every result
//! value. Each value is combined on its own, so the results are positive sizes that need not belong
//! together.

use serde::{Deserialize, Serialize};

use crate::model::Id;
use crate::Error;

fn five_percent() -> f64 {
    0.05
}

fn one() -> f64 {
    1.0
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// A spectrum function: a value (no units: the case's scale factor carries them) against the period
/// (s), for damping ratio `damping`; a table of points or the ASCE 7-16 design spectrum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumFunction {
    pub id: Id,
    #[serde(default = "five_percent")]
    pub damping: f64,
    /// [period, value] pairs, periods increasing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<[f64; 2]>,
    /// The ASCE 7-16 design spectrum (section 11.4) instead of a table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asce7_16: Option<Asce716>,
}

impl SpectrumFunction {
    /// The function's points: the table, or the design spectrum's points.
    pub fn table(&self) -> Vec<[f64; 2]> {
        match &self.asce7_16 {
            Some(a) => a.points(),
            None => self.points.clone(),
        }
    }

    /// The value at period `t` (see [`value_at`]).
    pub fn value(&self, t: f64) -> f64 {
        value_at(&self.table(), t)
    }
}

/// The value of a function's points at period `t`: linear in the period between points, constant before
/// the first point and after the last.
pub fn value_at(p: &[[f64; 2]], t: f64) -> f64 {
    if t <= p[0][0] {
        return p[0][1];
    }
    for w in p.windows(2) {
        let ([t0, v0], [t1, v1]) = (w[0], w[1]);
        if t <= t1 {
            return if t1 > t0 {
                v0 + (v1 - v0) * (t - t0) / (t1 - t0)
            } else {
                v1
            };
        }
    }
    p[p.len() - 1][1]
}

/// Site class of ASCE 7-16 section 20.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SiteClass {
    A,
    B,
    C,
    D,
    E,
    F,
}

/// The ASCE 7-16 design spectrum from the mapped accelerations `ss` (0.2 s) and `s1` (1 s), in g, the
/// long-period transition `tl` (s) and the site class; `fa` and `fv` replace the site coefficients of
/// Tables 11.4-1 and 11.4-2 (needed for class F and where the tables ask for a site study).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asce716 {
    pub ss: f64,
    pub s1: f64,
    pub tl: f64,
    pub site_class: SiteClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fa: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fv: Option<f64>,
}

/// A site coefficient from its table row: straight-line interpolation between the columns at `at`, the
/// end values beyond them; a blank column (a site study) stops the row there.
fn coefficient(at: &[f64; 6], row: [Option<f64>; 6], x: f64) -> Option<f64> {
    let k = at.iter().rposition(|a| *a <= x).unwrap_or(0);
    let here = row[k]?;
    match row.get(k + 1).copied().flatten() {
        Some(next) if x > at[k] => Some(here + (next - here) * (x - at[k]) / (at[k + 1] - at[k])),
        _ => Some(here),
    }
}

impl Asce716 {
    /// Fa (Table 11.4-1, by Ss) and Fv (Table 11.4-2, by S1), unless given; None where the table asks
    /// for a site study.
    pub fn coefficients(&self) -> (Option<f64>, Option<f64>) {
        use SiteClass::*;
        let (s, n) = (Some, None);
        let fa = match self.site_class {
            A => [s(0.8); 6],
            B => [s(0.9); 6],
            C => [s(1.3), s(1.3), s(1.2), s(1.2), s(1.2), s(1.2)],
            D => [s(1.6), s(1.4), s(1.2), s(1.1), s(1.0), s(1.0)],
            E => [s(2.4), s(1.7), s(1.3), n, n, n],
            F => [n; 6],
        };
        let fv = match self.site_class {
            A | B => [s(0.8); 6],
            C => [s(1.5), s(1.5), s(1.5), s(1.5), s(1.5), s(1.4)],
            D => [s(2.4), s(2.2), s(2.0), s(1.9), s(1.8), s(1.7)],
            E => [s(4.2), n, n, n, n, n],
            F => [n; 6],
        };
        (
            self.fa
                .or_else(|| coefficient(&[0.25, 0.5, 0.75, 1.0, 1.25, 1.5], fa, self.ss)),
            self.fv
                .or_else(|| coefficient(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6], fv, self.s1)),
        )
    }

    /// SDS = 2/3 Fa Ss and SD1 = 2/3 Fv S1 (sections 11.4.4 and 11.4.5).
    pub fn sds_sd1(&self) -> (f64, f64) {
        let (fa, fv) = self.coefficients();
        (
            2.0 / 3.0 * fa.unwrap_or(0.0) * self.ss,
            2.0 / 3.0 * fv.unwrap_or(0.0) * self.s1,
        )
    }

    /// The spectrum of section 11.4.6 as points: 0.4 SDS at T = 0, SDS from T0 = 0.2 SD1 / SDS to
    /// TS = SD1 / SDS, then SD1 / T up to TL and SD1 TL / T^2 beyond it, at 0.4 to 2 s every 0.2 s and
    /// 2.5 to 10 s every 0.5 s from the first such period more than 0.1 s above TS; values between
    /// points are linear in the period.
    pub fn points(&self) -> Vec<[f64; 2]> {
        let (sds, sd1) = self.sds_sd1();
        let (t0, ts) = (0.2 * sd1 / sds, sd1 / sds);
        let mut p = vec![[0.0, 0.4 * sds], [t0, sds], [ts, sds]];
        let grid = (2..=10)
            .map(|k| k as f64 / 5.0)
            .chain((5..=20).map(|k| k as f64 / 2.0));
        for t in grid.filter(|t| *t > ts + 0.1) {
            let v = if t <= self.tl {
                sd1 / t
            } else {
                sd1 * self.tl / (t * t)
            };
            p.push([t, v]);
        }
        p
    }
}

/// Spectrum value factor for a case with damping ratio `xi` on a function given at `xi_f`:
/// (2.31 - 0.41 ln 100 xi) / (2.31 - 0.41 ln 100 xi_f), the same at every period; none when either
/// damping is 0.
pub fn damping_factor(xi: f64, xi_f: f64) -> f64 {
    if xi <= 0.0 || xi_f <= 0.0 || xi == xi_f {
        return 1.0;
    }
    let b = |x: f64| 2.31 - 0.41 * (100.0 * x).ln();
    b(xi) / b(xi_f)
}

/// Correlation of two modes with circular frequencies `wi`, `wj` and the same damping ratio `xi`
/// (complete quadratic combination); 1 for equal frequencies, 0 between different ones at no damping.
pub fn correlation(wi: f64, wj: f64, xi: f64) -> f64 {
    if wi == wj {
        return 1.0;
    }
    let r = wj / wi;
    let x2 = xi * xi;
    8.0 * x2 * (1.0 + r) * r.powf(1.5) / ((1.0 - r * r).powi(2) + 4.0 * x2 * r * (1.0 + r).powi(2))
}

/// Direction of a ground acceleration: X and Y horizontal, turned by the load's angle about Z; Z
/// vertical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Accel {
    X,
    Y,
    Z,
}

/// One ground acceleration of a spectrum case: `function` times `scale` (acceleration units of the
/// model) along `dir`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumLoad {
    pub dir: Accel,
    pub function: Id,
    pub scale: f64,
    /// Degrees about Z from the global axes (X and Y only).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub angle: f64,
}

impl SpectrumLoad {
    /// Unit vector of the acceleration in global X, Y, Z.
    pub fn direction(&self) -> [f64; 3] {
        let (s, c) = self.angle.to_radians().sin_cos();
        match self.dir {
            Accel::X => [c, s, 0.0],
            Accel::Y => [-s, c, 0.0],
            Accel::Z => [0.0, 0.0, 1.0],
        }
    }
}

/// How the modes' responses to one acceleration are combined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModalCombination {
    /// Complete quadratic combination with the case's damping.
    #[default]
    Cqc,
    /// Square root of the sum of squares.
    Srss,
    /// Sum of the sizes.
    Absolute,
}

/// How the responses to the case's accelerations are combined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DirectionalCombination {
    /// Square root of the sum of squares.
    #[default]
    Srss,
    /// The largest of R_k + f x (sum of the others) (f = `abs_factor`; 0.3 gives the 100/30 rule).
    Absolute,
    /// The largest response over every direction of the horizontal pair (two horizontal loads with
    /// scale factors S and g S); the vertical load adds by squares.
    Cqc3,
}

/// A linear response spectrum case on the model's modal case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumCase {
    pub id: Id,
    pub loads: Vec<SpectrumLoad>,
    /// Modal damping ratio, the same for every mode.
    #[serde(default = "five_percent")]
    pub damping: f64,
    #[serde(default)]
    pub modal_combination: ModalCombination,
    #[serde(default)]
    pub directional_combination: DirectionalCombination,
    /// The factor of the absolute directional combination (missing: 1, the plain sum).
    #[serde(default = "one")]
    pub abs_factor: f64,
    /// Diaphragm eccentricity as a ratio of the diaphragm's width.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub eccentricity: f64,
}

/// Checks the spectrum functions and cases; `taken` tells whether a name is already a load case,
/// combination or the modal case.
pub fn validate(
    functions: &[SpectrumFunction],
    cases: &[SpectrumCase],
    has_modal: bool,
    taken: &dyn Fn(&Id) -> bool,
) -> Result<(), Error> {
    let bad = |msg: String| Err(Error::Invalid(msg));
    for (k, f) in functions.iter().enumerate() {
        if functions[..k].iter().any(|g| g.id == f.id) {
            return bad(format!(
                "spectrum function {}: the name is used twice",
                f.id
            ));
        }
        if !(f.damping >= 0.0 && f.damping < 1.0) {
            return bad(format!(
                "spectrum function {}: damping must be at least 0 and below 1",
                f.id
            ));
        }
        if let Some(a) = &f.asce7_16 {
            if !f.points.is_empty() {
                return bad(format!(
                    "spectrum function {}: either points or asce7_16, not both",
                    f.id
                ));
            }
            if !(a.ss > 0.0 && a.s1 > 0.0 && a.tl > 0.0) {
                return bad(format!(
                    "spectrum function {}: ss, s1 and tl must be numbers above 0",
                    f.id
                ));
            }
            match a.coefficients() {
                (Some(fa), Some(fv)) if fa > 0.0 && fv > 0.0 => {}
                (Some(_), Some(_)) => {
                    return bad(format!(
                        "spectrum function {}: fa and fv must be above 0",
                        f.id
                    ))
                }
                _ => {
                    return bad(format!(
                        "spectrum function {}: site class {:?} needs fa and fv here (a site study), give them",
                        f.id, a.site_class
                    ))
                }
            }
            continue;
        }
        if f.points.is_empty() {
            return bad(format!("spectrum function {}: no points", f.id));
        }
        if f.points.iter().flatten().any(|v| !v.is_finite()) || f.points.iter().any(|p| p[0] < 0.0)
        {
            return bad(format!(
                "spectrum function {}: periods must be numbers of 0 or more and values numbers",
                f.id
            ));
        }
        if f.points.windows(2).any(|w| w[1][0] <= w[0][0]) {
            return bad(format!(
                "spectrum function {}: periods must increase from point to point",
                f.id
            ));
        }
    }
    for (k, c) in cases.iter().enumerate() {
        if cases[..k].iter().any(|d| d.id == c.id) || taken(&c.id) {
            return bad(format!(
                "spectrum case {}: the name is already a load case, combination, modal or spectrum case",
                c.id
            ));
        }
        if !has_modal {
            return bad(format!(
                "spectrum case {}: the model needs a modal case",
                c.id
            ));
        }
        if c.loads.is_empty() {
            return bad(format!("spectrum case {}: no accelerations in it", c.id));
        }
        for l in &c.loads {
            if !functions.iter().any(|f| f.id == l.function) {
                return bad(format!(
                    "spectrum case {}: unknown spectrum function {}",
                    c.id, l.function
                ));
            }
            if !l.scale.is_finite() || !l.angle.is_finite() {
                return bad(format!(
                    "spectrum case {}: scale factors and angles must be numbers",
                    c.id
                ));
            }
        }
        if !(c.damping >= 0.0 && c.damping < 1.0) {
            return bad(format!(
                "spectrum case {}: damping must be at least 0 and below 1",
                c.id
            ));
        }
        if !(c.abs_factor.is_finite() && c.abs_factor >= 0.0) || !(c.eccentricity >= 0.0) {
            return bad(format!(
                "spectrum case {}: abs_factor and eccentricity must be numbers of 0 or more",
                c.id
            ));
        }
        if c.directional_combination == DirectionalCombination::Cqc3 {
            let of = |d: Accel| c.loads.iter().filter(|l| l.dir == d).collect::<Vec<_>>();
            let (x, y, z) = (of(Accel::X), of(Accel::Y), of(Accel::Z));
            if x.len() != 1 || y.len() != 1 || z.len() > 1 || x[0].angle != y[0].angle {
                return bad(format!(
                    "spectrum case {}: CQC3 needs one X and one Y acceleration at the same angle (and at most one Z)",
                    c.id
                ));
            }
        }
    }
    Ok(())
}

/// The combination of one case: per acceleration, every mode's amplitude; the rules; for CQC3, the
/// amplitudes of the horizontal pair at the larger of their two scale factors.
pub struct Rule {
    /// The modes' correlations (m x m) for CQC; empty for SRSS and the absolute sum.
    modal: Vec<f64>,
    absolute: bool,
    n_modes: usize,
    /// Per acceleration: the modes' amplitudes (displacement = amplitude x mode shape).
    amps: Vec<Vec<f64>>,
    directional: DirectionalCombination,
    abs_factor: f64,
    /// CQC3: indices into `amps` of the X and Y accelerations (at the larger scale factor), the
    /// ratio g of the smaller to the larger, and the vertical one if any.
    cqc3: Option<(usize, usize, f64, Option<usize>)>,
}

/// A mode of the modal case as a spectrum case needs it.
pub struct ModeData {
    pub period: f64,
    /// phi' M r for global X, Y, Z (mode shapes at unit modal mass).
    pub participation: [f64; 3],
}

impl Rule {
    pub fn new(case: &SpectrumCase, functions: &[SpectrumFunction], modes: &[ModeData]) -> Rule {
        let m = modes.len();
        let w: Vec<f64> = modes
            .iter()
            .map(|md| 2.0 * std::f64::consts::PI / md.period)
            .collect();
        let modal = match case.modal_combination {
            ModalCombination::Cqc => (0..m * m)
                .map(|k| correlation(w[k / m], w[k % m], case.damping))
                .collect(),
            ModalCombination::Srss | ModalCombination::Absolute => Vec::new(),
        };
        let amps_of = |l: &SpectrumLoad, scale: f64| -> Vec<f64> {
            let f = functions
                .iter()
                .find(|f| f.id == l.function)
                .expect("validated function");
            let table = f.table();
            let d = l.direction();
            let factor = damping_factor(case.damping, f.damping);
            modes
                .iter()
                .zip(&w)
                .map(|(md, wi)| {
                    let gamma: f64 = (0..3).map(|a| md.participation[a] * d[a]).sum();
                    gamma * value_at(&table, md.period) * factor * scale / (wi * wi)
                })
                .collect()
        };
        let mut amps: Vec<Vec<f64>> = case.loads.iter().map(|l| amps_of(l, l.scale)).collect();
        let cqc3 = (case.directional_combination == DirectionalCombination::Cqc3).then(|| {
            let at = |d: Accel| case.loads.iter().position(|l| l.dir == d);
            let (i1, i2) = (
                at(Accel::X).expect("validated"),
                at(Accel::Y).expect("validated"),
            );
            let (s1, s2) = (case.loads[i1].scale.abs(), case.loads[i2].scale.abs());
            let big = s1.max(s2);
            let g = if big > 0.0 { s1.min(s2) / big } else { 1.0 };
            amps.push(amps_of(&case.loads[i1], big));
            amps.push(amps_of(&case.loads[i2], big));
            let n = amps.len();
            (n - 2, n - 1, g, at(Accel::Z))
        });
        Rule {
            modal,
            absolute: case.modal_combination == ModalCombination::Absolute,
            n_modes: m,
            amps,
            directional: case.directional_combination,
            abs_factor: case.abs_factor,
            cqc3,
        }
    }

    /// The modes' amplitudes for each acceleration of the case.
    pub fn amplitudes(&self) -> &[Vec<f64>] {
        &self.amps
    }

    /// sum over modes i, j of rho_ij a_i b_j (rho = 1 on the diagonal, 0 off it for SRSS).
    fn quad(&self, a: &[f64], b: &[f64]) -> f64 {
        let m = self.n_modes;
        if self.modal.is_empty() {
            return a.iter().zip(b).map(|(x, y)| x * y).sum();
        }
        let mut s = 0.0;
        for i in 0..m {
            if a[i] == 0.0 {
                continue;
            }
            let row = &self.modal[i * m..(i + 1) * m];
            s += a[i] * row.iter().zip(b).map(|(r, y)| r * y).sum::<f64>();
        }
        s
    }

    fn modal_size(&self, a: &[f64]) -> f64 {
        if self.absolute {
            a.iter().map(|x| x.abs()).sum()
        } else {
            self.quad(a, a).max(0.0).sqrt()
        }
    }

    /// The case's value from one result's value in each mode (`unit[i]`: the value for mode i at unit
    /// amplitude). `scratch` holds one value per mode.
    pub fn combine(&self, unit: &[f64], scratch: &mut Vec<f64>) -> f64 {
        let along = |k: usize, out: &mut Vec<f64>| {
            out.clear();
            out.extend(self.amps[k].iter().zip(unit).map(|(q, v)| q * v));
        };
        if let Some((ix, iy, g, iz)) = self.cqc3 {
            let mut b = Vec::with_capacity(self.n_modes);
            along(ix, scratch);
            along(iy, &mut b);
            let (rx2, ry2, rxy) = (
                self.quad(scratch, scratch),
                self.quad(&b, &b),
                self.quad(scratch, &b),
            );
            let mut r2 = 0.5 * (1.0 + g * g) * (rx2 + ry2)
                + (1.0 - g * g) * (0.25 * (rx2 - ry2).powi(2) + rxy * rxy).sqrt();
            if let Some(iz) = iz {
                along(iz, scratch);
                r2 += self.quad(scratch, scratch);
            }
            return r2.max(0.0).sqrt();
        }
        let rs: Vec<f64> = (0..self.amps.len())
            .map(|k| {
                along(k, scratch);
                self.modal_size(scratch)
            })
            .collect();
        match self.directional {
            DirectionalCombination::Srss | DirectionalCombination::Cqc3 => {
                rs.iter().map(|r| r * r).sum::<f64>().sqrt()
            }
            DirectionalCombination::Absolute => {
                let total: f64 = rs.iter().sum();
                rs.iter()
                    .map(|r| r + self.abs_factor * (total - r))
                    .fold(0.0, f64::max)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn func() -> SpectrumFunction {
        SpectrumFunction {
            id: "F".into(),
            damping: 0.05,
            points: vec![[0.1, 0.4], [0.3, 1.0], [0.8, 1.0], [1.6, 0.5], [3.2, 0.25]],
            asce7_16: None,
        }
    }

    // Site coefficients as tabulated (and
    // interpolated) for the probe's Ss and S1.
    #[test]
    fn site_coefficients_as_measured() {
        let f = |site, ss, s1| {
            Asce716 {
                ss,
                s1,
                tl: 8.0,
                site_class: site,
                fa: None,
                fv: None,
            }
            .coefficients()
        };
        let close = |a: Option<f64>, b: f64| (a.unwrap() - b).abs() < 1e-12;
        assert!(close(f(SiteClass::D, 0.4, 0.15).0, 1.48));
        assert!(close(f(SiteClass::D, 0.4, 0.15).1, 2.3));
        assert!(close(f(SiteClass::C, 0.6, 0.55).0, 1.26));
        assert!(close(f(SiteClass::C, 0.6, 0.55).1, 1.45));
        assert!(close(f(SiteClass::E, 0.9, 0.15).0, 1.3));
        assert!(close(f(SiteClass::E, 0.9, 0.15).1, 4.2));
        assert_eq!(f(SiteClass::E, 1.0, 0.2), (None, None));
        assert!(close(f(SiteClass::D, 1.8, 0.8).0, 1.0));
    }

    // Reference values: modal spectrum info of probe model A.
    #[test]
    fn values_between_and_outside_the_points() {
        let f = func();
        for (t, v) in [
            (0.05, 0.4),
            (0.2, 0.7),
            (0.5, 1.0),
            (1.2, 0.75),
            (1.6, 0.5),
            (2.4, 0.375),
            (6.0, 0.25),
        ] {
            assert!(
                (f.value(t) - v).abs() < 1e-15,
                "T {t}: {} against {v}",
                f.value(t)
            );
        }
    }

    // Reference spectrum ratios at five digits (probe model A2).
    #[test]
    fn damping_factor_as_measured() {
        for (xi, xf, r) in [
            (0.02, 0.05, 1.22764),
            (0.10, 0.05, 0.82775),
            (0.05, 0.02, 0.81455),
            (0.005, 0.05, 1.57212),
            (0.3, 0.05, 0.55481),
            (0.0, 0.05, 1.0),
        ] {
            assert!((damping_factor(xi, xf) - r).abs() < 4e-5, "{xi} on {xf}");
        }
    }

    #[test]
    fn correlation_is_symmetric_and_one_on_the_diagonal() {
        let (a, b) = (12.566, 11.424);
        assert!((correlation(a, b, 0.05) - correlation(b, a, 0.05)).abs() < 1e-15);
        assert_eq!(correlation(a, a, 0.0), 1.0);
        assert_eq!(correlation(a, b, 0.0), 0.0);
        // Two modes 10 % apart at 5 % damping are strongly correlated.
        assert!(correlation(1.0, 1.1, 0.05) > 0.4);
    }
}
