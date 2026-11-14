//! The steel section list: the AISC Shapes Database v16.0 (American Institute of Steel Construction), as
//! `aisc16.json` (SI units; written by checks/aisc_list.py from the Excel file aisc.org publishes).
//!
//! A list section takes A, Ix (I33), Iy (I22) and J from the table (a double angle's J is twice its single
//! angle's: the table gives none) and the rest from its dimensions (docs/phases/SECTION_SHAPES_PLAN.md):
//! - I-shapes (W, M, S, HP): As2 = d tw, As3 = 5/3 bf tf.
//! - Channels (C, MC): As2 = d tw, As3 = 2 bf tf; the centroid x from the back of the web.
//! - Tees (WT, MT, ST): As2 = d tw, As3 = 5/6 bf tf; the centroid y below the top of the flange.
//! - Angles (L): the table's leg d vertical, b at the bottom, the vertical leg on the +3 side; As2 = d t,
//!   As3 = b t; I23 from the outline with its root fillet (radius kdes - t, sharp toes); the centroid from
//!   the table's x and y.
//! - Double angles (2L): legs back to back the spacing apart (long legs, or short legs for SLBB), the
//!   outstanding legs at the top; As2 = 2 × vertical leg × t, As3 = 4 × outstanding leg × t.
//! - Rectangular HSS: As2 = 2 Ht tdes, As3 = 2 B tdes. Round HSS and pipes: the pipe rule of
//!   `section.rs` on the table's area. Drawn with sharp corners, without fillets.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::section::Shape;

/// One shape of the list (SI: m, m², m⁴; missing values are not in the table).
#[derive(Debug, Clone, Deserialize)]
#[allow(non_snake_case)]
pub struct ListShape {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub A: Option<f64>,
    pub Ix: Option<f64>,
    pub Iy: Option<f64>,
    pub J: Option<f64>,
    pub d: Option<f64>,
    pub bf: Option<f64>,
    pub tw: Option<f64>,
    pub tf: Option<f64>,
    pub b: Option<f64>,
    pub t: Option<f64>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub Iz: Option<f64>,
    pub kdes: Option<f64>,
    pub od: Option<f64>,
    pub tdes: Option<f64>,
    pub ht: Option<f64>,
    pub b_out: Option<f64>,
}

#[derive(Deserialize)]
struct Table {
    shapes: Vec<ListShape>,
}

/// The whole list.
pub fn list() -> &'static [ListShape] {
    static LIST: OnceLock<Vec<ListShape>> = OnceLock::new();
    LIST.get_or_init(|| {
        serde_json::from_str::<Table>(include_str!("aisc16.json"))
            .expect("aisc16.json is valid")
            .shapes
    })
}

/// A shape of the list by name (any case).
pub fn find(name: &str) -> Option<&'static ListShape> {
    list().iter().find(|s| s.name.eq_ignore_ascii_case(name))
}

/// A list section's numbers, in metres. `I23` and the centroid offset [along 2, along 3] from the middle
/// of the box; `like`: the plain shape of its dimensions (drawing, box).
#[allow(non_snake_case)]
pub struct ListProps {
    pub A: f64,
    pub I22: f64,
    pub I33: f64,
    pub I23: f64,
    pub J: f64,
    pub As2: f64,
    pub As3: f64,
    pub centroid: [f64; 2],
    pub like: Shape,
}

/// A spacing in inches as written in a double angle's name ("3/8", "3/4", "1-1/2").
fn inches(s: &str) -> Option<f64> {
    let (whole, frac) = match s.split_once('-') {
        Some((w, f)) => (w.parse::<f64>().ok()?, f),
        None if s.contains('/') => (0.0, s),
        None => return s.parse().ok(),
    };
    let (n, d) = frac.split_once('/')?;
    Some(whole + n.parse::<f64>().ok()? / d.parse::<f64>().ok()?)
}

/// A double angle's single angle, spacing (m) and whether its short legs are back to back.
fn double_angle_parts(name: &str) -> Option<(&'static ListShape, f64, bool)> {
    let rest = name.strip_prefix('2')?;
    let (body, short) = if let Some(b) = rest.strip_suffix("SLBB") {
        (b, true)
    } else {
        (rest.strip_suffix("LLBB").unwrap_or(rest), false)
    };
    // L<d>X<b>X<t>[X<spacing>]
    let parts: Vec<&str> = body.splitn(4, 'X').collect();
    if parts.len() < 3 {
        return None;
    }
    let single = find(&parts[..3].join("X"))?;
    let gap = match parts.get(3) {
        Some(s) => inches(s)? * 0.0254,
        None => 0.0,
    };
    Some((single, gap, short))
}

/// A list shape's numbers (metres).
pub fn properties(s: &ListShape) -> Result<ListProps, String> {
    let need =
        |v: Option<f64>, what: &str| v.ok_or_else(|| format!("{}: no {what} in the list", s.name));
    let (a, ix, iy) = (need(s.A, "A")?, need(s.Ix, "Ix")?, need(s.Iy, "Iy")?);
    let base = |j: f64, as2: f64, as3: f64, like: Shape| ListProps {
        A: a,
        I22: iy,
        I33: ix,
        I23: 0.0,
        J: j,
        As2: as2,
        As3: as3,
        centroid: [0.0, 0.0],
        like,
    };
    Ok(match s.kind.as_str() {
        "W" | "M" | "S" | "HP" => {
            let (d, bf, tw, tf) = (
                need(s.d, "d")?,
                need(s.bf, "bf")?,
                need(s.tw, "tw")?,
                need(s.tf, "tf")?,
            );
            base(
                need(s.J, "J")?,
                d * tw,
                5.0 / 3.0 * bf * tf,
                Shape::I {
                    depth: d,
                    width: bf,
                    flange: tf,
                    web: tw,
                    bottom_width: None,
                    bottom_flange: None,
                },
            )
        }
        "C" | "MC" => {
            let (d, bf, tw, tf) = (
                need(s.d, "d")?,
                need(s.bf, "bf")?,
                need(s.tw, "tw")?,
                need(s.tf, "tf")?,
            );
            let mut p = base(
                need(s.J, "J")?,
                d * tw,
                2.0 * bf * tf,
                Shape::Channel {
                    depth: d,
                    width: bf,
                    flange: tf,
                    web: tw,
                    mirror: false,
                },
            );
            // The web on the +3 side, the centroid x from its back.
            p.centroid = [0.0, bf / 2.0 - need(s.x, "x")?];
            p
        }
        "WT" | "MT" | "ST" => {
            let (d, bf, tw, tf) = (
                need(s.d, "d")?,
                need(s.bf, "bf")?,
                need(s.tw, "tw")?,
                need(s.tf, "tf")?,
            );
            let mut p = base(
                need(s.J, "J")?,
                d * tw,
                5.0 / 6.0 * bf * tf,
                Shape::Tee {
                    depth: d,
                    width: bf,
                    flange: tf,
                    web: tw,
                    mirror: false,
                },
            );
            // The flange at the top, the centroid y below its top.
            p.centroid = [d / 2.0 - need(s.y, "y")?, 0.0];
            p
        }
        "L" => {
            let (d, b, t) = (need(s.d, "d")?, need(s.b, "b")?, need(s.t, "t")?);
            let mut p = base(
                need(s.J, "J")?,
                d * t,
                b * t,
                Shape::Angle {
                    depth: d,
                    width: b,
                    flange: t,
                    web: t,
                    mirror2: false,
                    mirror3: false,
                },
            );
            p.I23 = angle_product(d, b, t, need(s.kdes, "kdes")? - t);
            p.centroid = [need(s.y, "y")? - d / 2.0, b / 2.0 - need(s.x, "x")?];
            p
        }
        "2L" => {
            let (single, gap, short) = double_angle_parts(&s.name)
                .ok_or_else(|| format!("{}: no single angle in the list", s.name))?;
            let (d, b, t) = (need(s.d, "d")?, need(s.b, "b")?, need(s.t, "t")?);
            let (vertical, outstanding) = if short { (b, d) } else { (d, b) };
            let mut p = base(
                2.0 * need(single.J, "J of the single angle")?,
                2.0 * vertical * t,
                4.0 * outstanding * t,
                Shape::DoubleAngle {
                    depth: vertical,
                    width: 2.0 * outstanding + gap,
                    flange: t,
                    web: t,
                    gap,
                    mirror: false,
                },
            );
            p.centroid = [vertical / 2.0 - need(s.y, "y")?, 0.0];
            p
        }
        "HSS" if s.od.is_some() => pipe(s, a)?,
        "PIPE" => pipe(s, a)?,
        "HSS" => {
            let (ht, bo, t) = (
                need(s.ht, "Ht")?,
                need(s.b_out, "B")?,
                need(s.tdes, "tdes")?,
            );
            base(
                need(s.J, "J")?,
                2.0 * ht * t,
                2.0 * bo * t,
                Shape::Box {
                    depth: ht,
                    width: bo,
                    flange: t,
                    web: t,
                },
            )
        }
        other => return Err(format!("{}: type {other} is not in the list", s.name)),
    })
}

/// The product of inertia of an angle (leg `d` vertical on the +3 side, leg `b` at the bottom,
/// both `t` thick) with a root fillet of radius `r`: its outline as a polygon (the fillet in 64 pieces).
fn angle_product(d: f64, b: f64, t: f64, r: f64) -> f64 {
    let (x0, x1, y0, y1) = (-b / 2.0, b / 2.0, -d / 2.0, d / 2.0);
    let (cx, cy) = (x1 - t, y0 + t);
    let mut pts = vec![[x0, y0], [x1, y0], [x1, y1], [x1 - t, y1]];
    let r = r.max(0.0);
    // The fillet from the vertical leg's face down to the horizontal leg's, centred at (cx - r, cy + r).
    for i in (0..=64).rev() {
        let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::FRAC_PI_2 * i as f64 / 64.0;
        pts.push([cx - r + r * a.cos(), cy + r + r * a.sin()]);
    }
    pts.push([x0, y0 + t]);
    let (mut a, mut sx, mut sy, mut ixy) = (0.0, 0.0, 0.0, 0.0);
    for k in 0..pts.len() {
        let [p0, q0] = pts[k];
        let [p1, q1] = pts[(k + 1) % pts.len()];
        let c = p0 * q1 - p1 * q0;
        a += c / 2.0;
        sx += (p0 + p1) * c / 6.0;
        sy += (q0 + q1) * c / 6.0;
        ixy += (p0 * q1 + 2.0 * p0 * q0 + 2.0 * p1 * q1 + p1 * q0) * c / 24.0;
    }
    ixy - sx * sy / a
}

fn pipe(s: &ListShape, a: f64) -> Result<ListProps, String> {
    let od =
        s.od.ok_or_else(|| format!("{}: no OD in the list", s.name))?;
    let t = s
        .tdes
        .ok_or_else(|| format!("{}: no tdes in the list", s.name))?;
    let m = (od / 2.0 - t) / (od / 2.0);
    let shear = a * 3.0 * (1.0 + m * m) / (4.0 * (1.0 + m + m * m));
    Ok(ListProps {
        A: a,
        I22: s.Iy.unwrap_or_default(),
        I33: s.Ix.unwrap_or_default(),
        I23: 0.0,
        J: s.J.ok_or_else(|| format!("{}: no J in the list", s.name))?,
        As2: shear,
        As3: shear,
        centroid: [0.0, 0.0],
        like: Shape::Pipe {
            diameter: od,
            wall: t,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_of_the_list_has_its_numbers() {
        assert_eq!(list().len(), 2299);
        for s in list() {
            let p = properties(s).unwrap_or_else(|e| panic!("{e}"));
            assert!(
                p.A > 0.0 && p.I22 > 0.0 && p.I33 > 0.0 && p.J > 0.0 && p.As2 > 0.0 && p.As3 > 0.0,
                "{}",
                s.name
            );
        }
    }

    #[test]
    fn double_angle_names() {
        let (single, gap, short) = double_angle_parts("2L6X4X1/2X3/8SLBB").unwrap();
        assert_eq!(single.name, "L6X4X1/2");
        assert!((gap - 0.375 * 0.0254).abs() < 1e-12 && short);
        let (single, gap, short) = double_angle_parts("2L12X12X1-3/8X3/4").unwrap();
        assert_eq!(single.name, "L12X12X1-3/8");
        assert!((gap - 0.75 * 0.0254).abs() < 1e-12 && !short);
    }
}
