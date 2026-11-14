//! Frame section shapes (Define > Frame Sections): a shape's dimensions become the numbers the solver
//! reads (A, I22, I33, I23, J, As2, As3, the box and the centroid's place in it). The mesh step does this
//! for every section with a `shape` before the analysis; the app asks for the same numbers (command
//! `section`) to show them and to draw the shape.
//!
//! The rules (docs/phases/SECTION_SHAPES_PLAN.md, checks in docs/SOURCES.md):
//! - A, I22, I33, I23 and the centroid are the exact integrals of the shape (circles exact).
//! - Shear areas: As = I² / ∫ Q²/b, integrated over 50 strips across the shape, each strip with its
//!   own area (its width averaged over the strip).
//! - J: Saint-Venant torsion, the warping function solved by quadratic triangles on a mesh of the shape,
//!   circles taken as 24-sided polygons; a steel pipe is the polar moment of that polygon ring.
//! - Composite shapes are transformed to their steel: concrete counts with Ec/Es in A, I and the shear
//!   areas, and with Gc/Gs in J; their self weight is each material's own area times its own weight.
//!
//! Coordinates in the section plane are [h, v]: h along local 3 (the width), v along local 2 (the
//! depth), from the middle of the section's box. I33 = ∫ v² dA, I22 = ∫ h² dA, I23 = ∫ h v dA.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use osab_solver::model::Section;
use osab_solver::sparse::{Pattern, SymMatrix};
use osab_solver::{Error, Id, Model};

use crate::geom::P2;
use crate::tri;

/// A section shape. Dimensions in the model's length unit; `depth` along local 2, `width` along 3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Shape {
    /// Solid rectangle (also a steel plate).
    Rectangle { depth: f64, width: f64 },
    /// I-section: the top flange `width` x `flange`, the web `web` thick; a bottom flange of its own
    /// size when given (else the same as the top).
    I {
        depth: f64,
        width: f64,
        flange: f64,
        web: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bottom_width: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bottom_flange: Option<f64>,
    },
    /// Channel: web on the +3 side, flanges towards -3 (`mirror`: the other way).
    Channel {
        depth: f64,
        width: f64,
        flange: f64,
        web: f64,
        #[serde(default, skip_serializing_if = "is_false")]
        mirror: bool,
    },
    /// Tee: flange at the top (+2 side; `mirror`: at the bottom).
    Tee {
        depth: f64,
        width: f64,
        flange: f64,
        web: f64,
        #[serde(default, skip_serializing_if = "is_false")]
        mirror: bool,
    },
    /// Angle: the horizontal leg (`width`, `flange` thick) at the bottom, the vertical leg (`depth`,
    /// `web` thick) on the +3 side. `mirror2` flips it about local 2 (vertical leg on the -3 side),
    /// `mirror3` about local 3 (horizontal leg at the top).
    Angle {
        depth: f64,
        width: f64,
        flange: f64,
        web: f64,
        #[serde(default, skip_serializing_if = "is_false")]
        mirror2: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        mirror3: bool,
    },
    /// Two angles back to back `gap` apart, horizontal legs at the top (`mirror`: at the bottom);
    /// `width` is the whole width (both legs and the gap).
    DoubleAngle {
        depth: f64,
        width: f64,
        flange: f64,
        web: f64,
        gap: f64,
        #[serde(default, skip_serializing_if = "is_false")]
        mirror: bool,
    },
    /// Two channels back to back `gap` apart; `width` is the whole width.
    DoubleChannel {
        depth: f64,
        width: f64,
        flange: f64,
        web: f64,
        gap: f64,
    },
    /// Box (steel tube or concrete box): flanges `flange` thick (top and bottom), webs `web` thick.
    Box {
        depth: f64,
        width: f64,
        flange: f64,
        web: f64,
    },
    /// Steel pipe.
    Pipe { diameter: f64, wall: f64 },
    /// Solid circle (also a steel rod).
    Circle { diameter: f64 },
    /// Concrete tee: the web `web` thick at the flange, `web_tip` at its tip (missing = the same).
    ConcreteTee {
        depth: f64,
        width: f64,
        flange: f64,
        web: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        web_tip: Option<f64>,
        #[serde(default, skip_serializing_if = "is_false")]
        mirror: bool,
    },
    /// Concrete L: like an angle, the vertical leg `web` thick at the corner and `web_tip` at its tip.
    ConcreteL {
        depth: f64,
        width: f64,
        flange: f64,
        web: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        web_tip: Option<f64>,
        #[serde(default, skip_serializing_if = "is_false")]
        mirror2: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        mirror3: bool,
    },
    /// Concrete pipe (its J from the ring itself, not the thin-wall polar moment of a steel pipe).
    ConcretePipe { diameter: f64, wall: f64 },
    /// Cross: a horizontal bar `width` x `flange` and a vertical bar `depth` x `web`, both centred.
    Cross {
        depth: f64,
        width: f64,
        flange: f64,
        web: f64,
    },
    /// Trapezoid: `width` at the top, `bottom_width` at the bottom.
    Trapezoid {
        depth: f64,
        width: f64,
        bottom_width: f64,
    },
    /// Concrete-filled steel tube: the box in `material` (the frame's), filled with `fill`.
    FilledTube {
        depth: f64,
        width: f64,
        flange: f64,
        web: f64,
        material: Id,
        fill: Id,
    },
    /// Concrete-filled steel pipe.
    FilledPipe {
        diameter: f64,
        wall: f64,
        material: Id,
        fill: Id,
    },
    /// A steel I-section (`core`, an `i` shape) in a concrete rectangle of `encasement`.
    EncasedRectangle {
        depth: f64,
        width: f64,
        core: std::boxed::Box<Shape>,
        material: Id,
        encasement: Id,
    },
    /// A steel I-section in a concrete circle.
    EncasedCircle {
        diameter: f64,
        core: std::boxed::Box<Shape>,
        material: Id,
        encasement: Id,
    },
    /// A shape of the steel section list (`crate::aisc`) by its AISC name. `like`: the plain shape of its
    /// dimensions in the model's unit, kept for drawing (the numbers come from the list).
    Aisc {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        like: Option<std::boxed::Box<Shape>>,
    },
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// One region of a shape: polygons (counterclockwise; holes clockwise) and circles (a negative `sign`
/// for a hole), of one material.
#[derive(Debug, Clone, Default)]
struct Region {
    polys: Vec<Vec<P2>>,
    circles: Vec<(P2, f64, f64)>,
}

/// Sides of the polygon a circle becomes for the torsion constant.
const CIRCLE_SIDES: usize = 24;

/// Strips the shear areas are integrated over.
const STRIPS: usize = 50;

/// Strips for shapes with circles (integrated exactly).
const FINE_STRIPS: usize = 4000;

/// Empty strips between two parts beyond which the shear area is the whole area.
const GAP_STRIPS: usize = 6;

/// What a shape gives the solver, and its outline for drawing.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[allow(non_snake_case)]
pub struct SectionProps {
    pub A: f64,
    pub I22: f64,
    pub I33: f64,
    pub I23: f64,
    pub J: f64,
    pub As2: f64,
    pub As3: f64,
    /// The box: depth along local 2, width along 3.
    pub depth: f64,
    pub width: f64,
    /// The centroid from the middle of the box, along local 2 and 3.
    pub centroid_offset: [f64; 2],
    /// Self weight per length of a composite section (each material's area times its weight).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_weight: Option<f64>,
    /// The outline to draw: the steel (or only) material's polygons, then the fill's (composite), each a
    /// list of polygons in [h, v] from the box's middle (holes clockwise).
    pub outline: Vec<Vec<P2>>,
    /// A list section's plain shape (its dimensions in the model's unit), for drawing it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub like: Option<Shape>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fill_outline: Vec<Vec<P2>>,
}

fn bad(msg: impl Into<String>) -> Error {
    Error::Invalid(msg.into())
}

fn rect(h0: f64, h1: f64, v0: f64, v1: f64) -> Vec<P2> {
    vec![[h0, v0], [h1, v0], [h1, v1], [h0, v1]]
}

/// The polygon mirrored (h -> -h when `h`, v -> -v when `v`), kept counterclockwise.
fn mirrored(p: Vec<P2>, h: bool, v: bool) -> Vec<P2> {
    let mut q: Vec<P2> = p
        .into_iter()
        .map(|[a, b]| [if h { -a } else { a }, if v { -b } else { b }])
        .collect();
    if h != v {
        q.reverse();
    }
    q
}

fn positive(what: &str, vals: &[f64]) -> Result<(), Error> {
    if vals.iter().all(|v| v.is_finite() && *v > 0.0) {
        Ok(())
    } else {
        Err(bad(format!("{what}: dimensions must be > 0")))
    }
}

/// The I-section outline (box `width` = the wider flange).
fn i_outline(d: f64, bt: f64, tft: f64, tw: f64, bb: f64, tfb: f64) -> Vec<P2> {
    let (y0, y1) = (-d / 2.0, d / 2.0);
    vec![
        [-bb / 2.0, y0],
        [bb / 2.0, y0],
        [bb / 2.0, y0 + tfb],
        [tw / 2.0, y0 + tfb],
        [tw / 2.0, y1 - tft],
        [bt / 2.0, y1 - tft],
        [bt / 2.0, y1],
        [-bt / 2.0, y1],
        [-bt / 2.0, y1 - tft],
        [-tw / 2.0, y1 - tft],
        [-tw / 2.0, y0 + tfb],
        [-bb / 2.0, y0 + tfb],
    ]
}

/// The I-section's dimensions [depth, top width, top flange, web, bottom width, bottom flange].
fn i_dims(s: &Shape) -> Result<[f64; 6], Error> {
    match s {
        Shape::I {
            depth,
            width,
            flange,
            web,
            bottom_width,
            bottom_flange,
        } => {
            let d = [
                *depth,
                *width,
                *flange,
                *web,
                bottom_width.unwrap_or(*width),
                bottom_flange.unwrap_or(*flange),
            ];
            positive("I-section", &d)?;
            if d[2] + d[5] >= d[0] {
                return Err(bad("I-section: the flanges must be thinner than the depth"));
            }
            if d[3] > d[1].min(d[4]) {
                return Err(bad("I-section: the web must be no wider than the flanges"));
            }
            Ok(d)
        }
        Shape::Aisc { name, .. } => {
            match crate::aisc::find(name).and_then(|l| crate::aisc::properties(l).ok()) {
                Some(crate::aisc::ListProps {
                    like: like @ Shape::I { .. },
                    ..
                }) => i_dims(&like),
                _ => Err(bad("an encased section's core must be an I-section")),
            }
        }
        _ => Err(bad("an encased section's core must be an I-section")),
    }
}

/// The shape's material regions: the main (steel, or the only) material, and the fill (composite), with
/// the thickness that sets the torsion mesh.
struct Geometry {
    main: Region,
    fill: Option<Region>,
    /// The thinnest part, for the torsion mesh.
    thin: f64,
    /// Main material and fill material of a composite shape.
    materials: Option<(Id, Id)>,
    /// A steel pipe: J is the polar moment of the 24-sided ring.
    thin_pipe: Option<(f64, f64)>,
    /// A pipe (steel or concrete): outer and inner radius, for its shear area.
    ring: Option<(f64, f64)>,
    /// A composite shape's torsion: the outline of both materials, the line between them, and whether
    /// the inside of that line is the fill (filled sections) or the steel (encased).
    bonded: Option<(Vec<P2>, Vec<P2>, bool)>,
}

fn geometry(s: &Shape) -> Result<Geometry, Error> {
    let one = |polys: Vec<Vec<P2>>, thin: f64| Geometry {
        main: Region {
            polys,
            circles: Vec::new(),
        },
        fill: None,
        thin,
        materials: None,
        thin_pipe: None,
        ring: None,
        bonded: None,
    };
    Ok(match s {
        Shape::Rectangle { depth, width } => {
            positive("rectangle", &[*depth, *width])?;
            one(
                vec![rect(-width / 2.0, width / 2.0, -depth / 2.0, depth / 2.0)],
                depth.min(*width),
            )
        }
        Shape::I { .. } => {
            let [d, bt, tft, tw, bb, tfb] = i_dims(s)?;
            one(
                vec![i_outline(d, bt, tft, tw, bb, tfb)],
                tft.min(tw).min(tfb),
            )
        }
        Shape::Channel {
            depth: d,
            width: b,
            flange: tf,
            web: tw,
            mirror,
        } => {
            positive("channel", &[*d, *b, *tf, *tw])?;
            if 2.0 * tf >= *d || tw >= b {
                return Err(bad("channel: the flanges or the web are too thick"));
            }
            let (h0, h1, y0, y1) = (-b / 2.0, b / 2.0, -d / 2.0, d / 2.0);
            let p = vec![
                [h0, y0],
                [h1, y0],
                [h1, y1],
                [h0, y1],
                [h0, y1 - tf],
                [h1 - tw, y1 - tf],
                [h1 - tw, y0 + tf],
                [h0, y0 + tf],
            ];
            one(vec![mirrored(p, *mirror, false)], tf.min(*tw))
        }
        Shape::Tee {
            depth: d,
            width: b,
            flange: tf,
            web: tw,
            mirror,
        } => {
            positive("tee", &[*d, *b, *tf, *tw])?;
            if tf >= d || tw > b {
                return Err(bad("tee: the flange or the web is too thick"));
            }
            let (y0, y1) = (-d / 2.0, d / 2.0);
            let p = vec![
                [-tw / 2.0, y0],
                [tw / 2.0, y0],
                [tw / 2.0, y1 - tf],
                [b / 2.0, y1 - tf],
                [b / 2.0, y1],
                [-b / 2.0, y1],
                [-b / 2.0, y1 - tf],
                [-tw / 2.0, y1 - tf],
            ];
            one(vec![mirrored(p, false, *mirror)], tf.min(*tw))
        }
        Shape::Angle {
            depth: d,
            width: b,
            flange: tf,
            web: tw,
            mirror2,
            mirror3,
        } => {
            positive("angle", &[*d, *b, *tf, *tw])?;
            if tf >= d || tw >= b {
                return Err(bad("angle: the legs are too thick"));
            }
            one(
                vec![mirrored(
                    angle_outline(*d, *b, *tf, *tw, *tw),
                    *mirror2,
                    *mirror3,
                )],
                tf.min(*tw),
            )
        }
        Shape::DoubleAngle {
            depth: d,
            width: b,
            flange: tf,
            web: tw,
            gap,
            mirror,
        } => {
            positive("double angle", &[*d, *b, *tf, *tw])?;
            let leg = (b - gap) / 2.0;
            if !(*gap >= 0.0) || tf >= d || tw >= &leg {
                return Err(bad(
                    "double angle: the legs are too thick or the gap too wide",
                ));
            }
            // The right angle: vertical leg at the gap, horizontal leg at the top, outwards.
            let (y0, y1, g) = (-d / 2.0, d / 2.0, gap / 2.0);
            let right = vec![
                [g, y0],
                [g + tw, y0],
                [g + tw, y1 - tf],
                [b / 2.0, y1 - tf],
                [b / 2.0, y1],
                [g, y1],
            ];
            let left = mirrored(right.clone(), true, false);
            let polys = if *gap > 0.0 {
                vec![
                    mirrored(right, false, *mirror),
                    mirrored(left, false, *mirror),
                ]
            } else {
                // Touching: one outline.
                let p = vec![
                    [-tw, y0],
                    [*tw, y0],
                    [*tw, y1 - tf],
                    [b / 2.0, y1 - tf],
                    [b / 2.0, y1],
                    [-b / 2.0, y1],
                    [-b / 2.0, y1 - tf],
                    [-tw, y1 - tf],
                ];
                vec![mirrored(p, false, *mirror)]
            };
            one(polys, tf.min(*tw))
        }
        Shape::DoubleChannel {
            depth: d,
            width: b,
            flange: tf,
            web: tw,
            gap,
        } => {
            positive("double channel", &[*d, *b, *tf, *tw])?;
            let leg = (b - gap) / 2.0;
            if !(*gap >= 0.0) || 2.0 * tf >= *d || tw >= &leg {
                return Err(bad(
                    "double channel: the parts are too thick or the gap too wide",
                ));
            }
            let (y0, y1, g) = (-d / 2.0, d / 2.0, gap / 2.0);
            let right = vec![
                [g, y0],
                [b / 2.0, y0],
                [b / 2.0, y0 + tf],
                [g + tw, y0 + tf],
                [g + tw, y1 - tf],
                [b / 2.0, y1 - tf],
                [b / 2.0, y1],
                [g, y1],
            ];
            let polys = if *gap > 0.0 {
                let left = mirrored(right.clone(), true, false);
                vec![right, left]
            } else {
                vec![i_like_double_channel(*d, *b, *tf, *tw)]
            };
            one(polys, tf.min(*tw))
        }
        Shape::Box {
            depth: d,
            width: b,
            flange: tf,
            web: tw,
        } => {
            positive("box", &[*d, *b, *tf, *tw])?;
            if 2.0 * tf >= *d || 2.0 * tw >= *b {
                return Err(bad("box: the walls are too thick"));
            }
            let mut hole = rect(-b / 2.0 + tw, b / 2.0 - tw, -d / 2.0 + tf, d / 2.0 - tf);
            hole.reverse();
            one(
                vec![rect(-b / 2.0, b / 2.0, -d / 2.0, d / 2.0), hole],
                tf.min(*tw),
            )
        }
        Shape::Pipe { diameter, wall } | Shape::ConcretePipe { diameter, wall } => {
            positive("pipe", &[*diameter, *wall])?;
            if 2.0 * wall >= *diameter {
                return Err(bad("pipe: the wall is too thick"));
            }
            let (r, ri) = (diameter / 2.0, diameter / 2.0 - wall);
            Geometry {
                main: Region {
                    polys: Vec::new(),
                    circles: vec![([0.0, 0.0], r, 1.0), ([0.0, 0.0], ri, -1.0)],
                },
                fill: None,
                thin: *wall,
                materials: None,
                thin_pipe: matches!(s, Shape::Pipe { .. }).then_some((r, ri)),
                ring: Some((r, ri)),
                bonded: None,
            }
        }
        Shape::Circle { diameter } => {
            positive("circle", &[*diameter])?;
            Geometry {
                main: Region {
                    polys: Vec::new(),
                    circles: vec![([0.0, 0.0], diameter / 2.0, 1.0)],
                },
                fill: None,
                thin: *diameter,
                materials: None,
                thin_pipe: None,
                ring: None,
                bonded: None,
            }
        }
        Shape::ConcreteTee {
            depth: d,
            width: b,
            flange: tf,
            web: twf,
            web_tip,
            mirror,
        } => {
            let twt = web_tip.unwrap_or(*twf);
            positive("concrete tee", &[*d, *b, *tf, *twf, twt])?;
            if tf >= d || twf.max(twt) > *b {
                return Err(bad("concrete tee: the flange or the web is too thick"));
            }
            let (y0, y1) = (-d / 2.0, d / 2.0);
            let p = vec![
                [-twt / 2.0, y0],
                [twt / 2.0, y0],
                [twf / 2.0, y1 - tf],
                [b / 2.0, y1 - tf],
                [b / 2.0, y1],
                [-b / 2.0, y1],
                [-b / 2.0, y1 - tf],
                [-twf / 2.0, y1 - tf],
            ];
            one(vec![mirrored(p, false, *mirror)], tf.min(twt).min(*twf))
        }
        Shape::ConcreteL {
            depth: d,
            width: b,
            flange: tf,
            web: twc,
            web_tip,
            mirror2,
            mirror3,
        } => {
            let twt = web_tip.unwrap_or(*twc);
            positive("concrete L", &[*d, *b, *tf, *twc, twt])?;
            if tf >= d || twc.max(twt) >= *b {
                return Err(bad("concrete L: the legs are too thick"));
            }
            one(
                vec![mirrored(
                    angle_outline(*d, *b, *tf, *twc, twt),
                    *mirror2,
                    *mirror3,
                )],
                tf.min(*twc).min(twt),
            )
        }
        Shape::Cross {
            depth: d,
            width: b,
            flange: tf,
            web: tw,
        } => {
            positive("cross", &[*d, *b, *tf, *tw])?;
            if tf >= d || tw >= b {
                return Err(bad("cross: the bars are too thick"));
            }
            let (h, v, a, c) = (b / 2.0, d / 2.0, tw / 2.0, tf / 2.0);
            let p = vec![
                [-a, -v],
                [a, -v],
                [a, -c],
                [h, -c],
                [h, c],
                [a, c],
                [a, v],
                [-a, v],
                [-a, c],
                [-h, c],
                [-h, -c],
                [-a, -c],
            ];
            one(vec![p], tf.min(*tw))
        }
        Shape::Trapezoid {
            depth: d,
            width: b,
            bottom_width: bb,
        } => {
            positive("trapezoid", &[*d, *b, *bb])?;
            let p = vec![
                [-bb / 2.0, -d / 2.0],
                [bb / 2.0, -d / 2.0],
                [b / 2.0, d / 2.0],
                [-b / 2.0, d / 2.0],
            ];
            one(vec![p], d.min(b.min(*bb)))
        }
        Shape::FilledTube {
            depth: d,
            width: b,
            flange: tf,
            web: tw,
            material,
            fill,
        } => {
            let mut g = geometry(&Shape::Box {
                depth: *d,
                width: *b,
                flange: *tf,
                web: *tw,
            })?;
            let core = rect(-b / 2.0 + tw, b / 2.0 - tw, -d / 2.0 + tf, d / 2.0 - tf);
            g.fill = Some(Region {
                polys: vec![core.clone()],
                circles: Vec::new(),
            });
            g.materials = Some((material.clone(), fill.clone()));
            g.bonded = Some((rect(-b / 2.0, b / 2.0, -d / 2.0, d / 2.0), core, true));
            g
        }
        Shape::FilledPipe {
            diameter,
            wall,
            material,
            fill,
        } => {
            positive("filled pipe", &[*diameter, *wall])?;
            if 2.0 * wall >= *diameter {
                return Err(bad("filled pipe: the wall is too thick"));
            }
            // Every circle of a filled pipe counts as the inscribed 32-sided polygon.
            let outer = sided([0.0, 0.0], diameter / 2.0, 32);
            let core = sided([0.0, 0.0], diameter / 2.0 - wall, 32);
            let mut hole = core.clone();
            hole.reverse();
            Geometry {
                main: Region {
                    polys: vec![outer.clone(), hole],
                    circles: Vec::new(),
                },
                fill: Some(Region {
                    polys: vec![core.clone()],
                    circles: Vec::new(),
                }),
                thin: *wall,
                materials: Some((material.clone(), fill.clone())),
                thin_pipe: None,
                ring: None,
                bonded: Some((outer, core, true)),
            }
        }
        Shape::EncasedRectangle {
            depth: d,
            width: b,
            core,
            material,
            encasement,
        } => {
            positive("encased rectangle", &[*d, *b])?;
            let [cd, bt, tft, tw, bb, tfb] = i_dims(core)?;
            if cd >= *d || bt.max(bb) >= *b {
                return Err(bad("encased rectangle: the I-section must fit inside"));
            }
            let steel = i_outline(cd, bt, tft, tw, bb, tfb);
            let steel_line = steel.clone();
            let outer = rect(-b / 2.0, b / 2.0, -d / 2.0, d / 2.0);
            let mut hole = steel.clone();
            hole.reverse();
            Geometry {
                main: Region {
                    polys: vec![steel],
                    circles: Vec::new(),
                },
                fill: Some(Region {
                    polys: vec![outer.clone(), hole],
                    circles: Vec::new(),
                }),
                thin: tft.min(tw).min(tfb),
                materials: Some((material.clone(), encasement.clone())),
                thin_pipe: None,
                ring: None,
                bonded: Some((outer, steel_line, false)),
            }
        }
        Shape::Aisc { .. } => return Err(bad("a list section has no geometry of its own")),
        Shape::EncasedCircle {
            diameter,
            core,
            material,
            encasement,
        } => {
            positive("encased circle", &[*diameter])?;
            let [cd, bt, tft, tw, bb, tfb] = i_dims(core)?;
            let corner = |w: f64| ((cd / 2.0).powi(2) + (w / 2.0).powi(2)).sqrt();
            if corner(bt).max(corner(bb)) >= diameter / 2.0 {
                return Err(bad("encased circle: the I-section must fit inside"));
            }
            let steel = i_outline(cd, bt, tft, tw, bb, tfb);
            let steel_line = steel.clone();
            // The concrete circle counts as the 24-sided polygon in every property.
            let outer = polygon([0.0, 0.0], diameter / 2.0);
            let mut hole = steel.clone();
            hole.reverse();
            Geometry {
                main: Region {
                    polys: vec![steel],
                    circles: Vec::new(),
                },
                fill: Some(Region {
                    polys: vec![outer.clone(), hole],
                    circles: Vec::new(),
                }),
                thin: tft.min(tw).min(tfb),
                materials: Some((material.clone(), encasement.clone())),
                thin_pipe: None,
                ring: None,
                bonded: Some((outer, steel_line, false)),
            }
        }
    })
}

/// An angle (or concrete L) outline: horizontal leg at the bottom, vertical leg on the +3 side, its
/// inner face from `twc` thick at the corner to `twt` at the tip.
fn angle_outline(d: f64, b: f64, tf: f64, twc: f64, twt: f64) -> Vec<P2> {
    let (h0, h1, y0, y1) = (-b / 2.0, b / 2.0, -d / 2.0, d / 2.0);
    vec![
        [h0, y0],
        [h1, y0],
        [h1, y1],
        [h1 - twt, y1],
        [h1 - twc, y0 + tf],
        [h0, y0 + tf],
    ]
}

/// Two channels back to back with no gap: one outline (webs together).
fn i_like_double_channel(d: f64, b: f64, tf: f64, tw: f64) -> Vec<P2> {
    i_outline(d, b, tf, 2.0 * tw, b, tf)
}

// ---- Area integrals ----

/// ∫ 1, ∫ h, ∫ v, ∫ h², ∫ v², ∫ h v over a region (signed polygons and circles).
#[derive(Debug, Clone, Copy, Default)]
struct Moments {
    a: f64,
    sh: f64,
    sv: f64,
    hh: f64,
    vv: f64,
    hv: f64,
}

impl Moments {
    fn add(&mut self, o: &Moments, f: f64) {
        self.a += f * o.a;
        self.sh += f * o.sh;
        self.sv += f * o.sv;
        self.hh += f * o.hh;
        self.vv += f * o.vv;
        self.hv += f * o.hv;
    }
}

fn poly_moments(p: &[P2]) -> Moments {
    let mut m = Moments::default();
    for k in 0..p.len() {
        let [x0, y0] = p[k];
        let [x1, y1] = p[(k + 1) % p.len()];
        let c = x0 * y1 - x1 * y0;
        m.a += c / 2.0;
        m.sh += (x0 + x1) * c / 6.0;
        m.sv += (y0 + y1) * c / 6.0;
        m.hh += (x0 * x0 + x0 * x1 + x1 * x1) * c / 12.0;
        m.vv += (y0 * y0 + y0 * y1 + y1 * y1) * c / 12.0;
        m.hv += (x0 * y1 + 2.0 * x0 * y0 + 2.0 * x1 * y1 + x1 * y0) * c / 24.0;
    }
    m
}

fn region_moments(r: &Region) -> Moments {
    let mut m = Moments::default();
    for p in &r.polys {
        m.add(&poly_moments(p), 1.0);
    }
    for &([ch, cv], rad, sign) in &r.circles {
        let a = std::f64::consts::PI * rad * rad;
        let i0 = a * rad * rad / 4.0;
        m.add(
            &Moments {
                a,
                sh: a * ch,
                sv: a * cv,
                hh: i0 + a * ch * ch,
                vv: i0 + a * cv * cv,
                hv: a * ch * cv,
            },
            sign,
        );
    }
    m
}

// ---- Shear areas ----

/// The polygon clipped to `lo <= p[axis] <= hi`.
fn clip(p: &[P2], axis: usize, lo: f64, hi: f64) -> Vec<P2> {
    let half = |poly: Vec<P2>, keep: &dyn Fn(P2) -> bool, at: f64| -> Vec<P2> {
        let mut out = Vec::new();
        for k in 0..poly.len() {
            let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
            let (ia, ib) = (keep(a), keep(b));
            if ia {
                out.push(a);
            }
            if ia != ib {
                let t = (at - a[axis]) / (b[axis] - a[axis]);
                out.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
            }
        }
        out
    };
    let p = half(p.to_vec(), &|q: P2| q[axis] >= lo, lo);
    if p.is_empty() {
        return p;
    }
    half(p, &|q: P2| q[axis] <= hi, hi)
}

/// Area of a circle (centre coordinate `c` along the axis, radius `r`) below `t` along that axis.
fn circle_below(c: f64, r: f64, t: f64) -> f64 {
    let d = (t - c).clamp(-r, r);
    r * r * ((d / r).asin() + std::f64::consts::FRAC_PI_2) + d * (r * r - d * d).sqrt()
}

/// The area of `r` (times `f`) between `lo` and `hi` along `axis` (0: h, 1: v).
fn band_area(r: &Region, axis: usize, lo: f64, hi: f64) -> f64 {
    let polys: f64 = r
        .polys
        .iter()
        .map(|p| {
            let q = clip(p, axis, lo, hi);
            if q.len() < 3 {
                0.0
            } else {
                poly_moments(&q).a
            }
        })
        .sum();
    let circles: f64 = r
        .circles
        .iter()
        .map(|&(c, rad, sign)| {
            sign * (circle_below(c[axis], rad, hi) - circle_below(c[axis], rad, lo))
        })
        .sum();
    polys + circles
}

/// The shear area for shear along `axis` (1: v, As2; 0: h, As3) of regions with factors: 50 strips
/// across the box, each with the area of the shape in it (widths averaged over the strip), and As =
/// I² / ∫ Q²/b of those strips, integrated exactly. A shape with circles is integrated exactly (fine
/// strips): a thin pipe gives A/2, a circle 0.9 A. Two parts more than `GAP_STRIPS` strips apart (a double
/// angle or channel with a wide gap) give the whole area.
fn shear_area(regions: &[(&Region, f64)], axis: usize, lo: f64, hi: f64) -> f64 {
    let round = regions.iter().any(|(r, _)| !r.circles.is_empty());
    let n = if round { FINE_STRIPS } else { STRIPS };
    let step = (hi - lo) / n as f64;
    let all: Vec<(f64, f64, f64)> = (0..n)
        .map(|i| {
            let (a, b) = (lo + i as f64 * step, lo + (i + 1) as f64 * step);
            let area: f64 = regions
                .iter()
                .map(|(r, f)| f * band_area(r, axis, a, b))
                .sum();
            (a, b, area / step)
        })
        .collect();
    let full = |s: &(f64, f64, f64)| s.2 > 1e-12 * step;
    let (first, last) = (all.iter().position(full), all.iter().rposition(full));
    if let (false, Some(i0), Some(i1)) = (round, first, last) {
        let mut run = 0;
        for s in &all[i0..=i1] {
            run = if full(s) { 0 } else { run + 1 };
            if run > GAP_STRIPS {
                return all.iter().map(|s| s.2 * step).sum();
            }
        }
    }
    let strips: Vec<(f64, f64, f64)> = all.into_iter().filter(full).collect();
    let area: f64 = strips.iter().map(|(a, b, w)| (b - a) * w).sum();
    let yc = strips
        .iter()
        .map(|(a, b, w)| (b * b - a * a) / 2.0 * w)
        .sum::<f64>()
        / area;
    let inertia: f64 = strips
        .iter()
        .map(|(a, b, w)| w * ((b - yc).powi(3) - (a - yc).powi(3)) / 3.0)
        .sum();
    // Q(y) from the low side; between strips Q is continuous, inside a strip quadratic in y.
    const G3: [(f64, f64); 3] = [
        (-0.774_596_669_241_483_4, 5.0 / 9.0),
        (0.0, 8.0 / 9.0),
        (0.774_596_669_241_483_4, 5.0 / 9.0),
    ];
    let mut q0 = 0.0;
    let mut integral = 0.0;
    // Across empty strips (two parts apart) Q stays and nothing is added.
    for &(a, b, w) in &strips {
        let q = |y: f64| q0 + w * ((y - yc).powi(2) - (a - yc).powi(2)) / 2.0;
        let (m, h) = ((a + b) / 2.0, (b - a) / 2.0);
        integral += G3
            .iter()
            .map(|(x, wt)| wt * q(m + x * h).powi(2))
            .sum::<f64>()
            * h
            / w;
        q0 = q(b);
    }
    inertia * inertia / integral
}

// ---- Torsion ----

/// The inscribed 24-sided polygon of a circle (counterclockwise, a corner on the +h axis).
fn polygon(c: P2, rad: f64) -> Vec<P2> {
    sided(c, rad, CIRCLE_SIDES)
}

/// The inscribed polygon of `n` sides of a circle (counterclockwise, a corner on the +h axis).
fn sided(c: P2, rad: f64, n: usize) -> Vec<P2> {
    (0..n)
        .map(|k| {
            let t = std::f64::consts::TAU * k as f64 / n as f64;
            [c[0] + rad * t.cos(), c[1] + rad * t.sin()]
        })
        .collect()
}

/// The region as polygons for the torsion mesh: circles become inscribed 24-sided polygons.
fn torsion_polys(r: &Region) -> Vec<Vec<P2>> {
    let mut out = r.polys.clone();
    for &(c, rad, sign) in &r.circles {
        let mut p: Vec<P2> = (0..CIRCLE_SIDES)
            .map(|k| {
                let t = std::f64::consts::TAU * k as f64 / CIRCLE_SIDES as f64;
                [c[0] + rad * t.cos(), c[1] + rad * t.sin()]
            })
            .collect();
        if sign < 0.0 {
            p.reverse();
        }
        out.push(p);
    }
    out
}

/// Groups polygons into faces: each counterclockwise outline with the clockwise holes inside it.
fn faces(polys: &[Vec<P2>]) -> Vec<(Vec<P2>, Vec<Vec<P2>>)> {
    let signed = |p: &Vec<P2>| poly_moments(p).a;
    let mut out: Vec<(Vec<P2>, Vec<Vec<P2>>)> = polys
        .iter()
        .filter(|p| signed(p) > 0.0)
        .map(|p| (p.clone(), Vec::new()))
        .collect();
    for h in polys.iter().filter(|p| signed(p) < 0.0) {
        let c = h[0];
        let probe = [
            h.iter().map(|p| p[0]).sum::<f64>() / h.len() as f64,
            h.iter().map(|p| p[1]).sum::<f64>() / h.len() as f64,
        ];
        let _ = c;
        if let Some(f) = out
            .iter_mut()
            .find(|(o, _)| crate::geom::inside(probe, o, 0.0))
        {
            f.1.push(h.clone());
        }
    }
    out
}

/// Saint-Venant torsion constant of one face (an outline with holes): the warping function ω minimises
/// ½ ∫ g |∇ω - (v, -h)|² (g: the shear modulus factor of each material) and J = ∫ g (h² + v²) - ωᵀ f,
/// solved by quadratic triangles. `inner`: lines between two materials inside the face (kept as mesh
/// edges); `factor`: g at a point. `size`: the mesh's target edge length.
fn face_torsion(
    outer: &[P2],
    holes: &[Vec<P2>],
    inner: &[Vec<P2>],
    factor: &dyn Fn(P2) -> f64,
    size: f64,
) -> Result<f64, Error> {
    let outer: &[P2] = &clean(outer);
    let holes: &[Vec<P2>] = &holes.iter().map(|h| clean(h)).collect::<Vec<_>>();
    let inner: &[Vec<P2>] = &inner.iter().map(|h| clean(h)).collect::<Vec<_>>();
    // Boundary points every `size` along each edge.
    let mut pts: Vec<P2> = Vec::new();
    let mut edges: Vec<[usize; 2]> = Vec::new();
    for ring in std::iter::once(outer)
        .chain(holes.iter().map(|h| h.as_slice()))
        .chain(inner.iter().map(|h| h.as_slice()))
    {
        let start = pts.len();
        for k in 0..ring.len() {
            let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
            let n = ((crate::geom::dist(a, b) / size).ceil() as usize).max(1);
            for s in 0..n {
                pts.push(crate::geom::lerp(a, b, s as f64 / n as f64));
            }
        }
        let count = pts.len() - start;
        for k in 0..count {
            edges.push([start + k, start + (k + 1) % count]);
        }
    }
    let input = tri::Input {
        pts,
        fixed_edges: edges,
        outer,
        holes,
        h: size,
        origin: [0.0, 0.0],
        hot: &[],
    };
    let (nodes, tris) =
        tri::triangles(&input).map_err(|e| bad(format!("section torsion mesh: {e}")))?;
    let weights: Vec<f64> = tris
        .iter()
        .map(|t| {
            let c = [
                (nodes[t[0]][0] + nodes[t[1]][0] + nodes[t[2]][0]) / 3.0,
                (nodes[t[0]][1] + nodes[t[1]][1] + nodes[t[2]][1]) / 3.0,
            ];
            factor(c)
        })
        .collect();
    torsion_on(&nodes, &tris, &weights)
}

/// The polygon without repeated corners and corners on a straight line (a tee whose web is as wide as
/// its flange is a rectangle).
fn clean(p: &[P2]) -> Vec<P2> {
    let span = p
        .iter()
        .flat_map(|a| p.iter().map(move |b| crate::geom::dist(*a, *b)))
        .fold(0.0, f64::max);
    let mut q: Vec<P2> = p.to_vec();
    loop {
        let n = q.len();
        let drop = (0..n).find(|&k| {
            let (a, b, c) = (q[(k + n - 1) % n], q[k], q[(k + 1) % n]);
            crate::geom::dist(a, b) <= 1e-12 * span
                || crate::geom::cross(crate::geom::sub(b, a), crate::geom::sub(c, b)).abs()
                    <= 1e-12 * span * span
        });
        match drop {
            Some(k) if n > 3 => {
                q.remove(k);
            }
            _ => return q,
        }
    }
}

/// J on a triangle mesh, with quadratic (6-node) triangles: corner nodes and a node at each edge's middle.
fn torsion_on(corners: &[P2], tris: &[[usize; 3]], weights: &[f64]) -> Result<f64, Error> {
    let mut nodes: Vec<P2> = corners.to_vec();
    let mut mid: HashMap<(usize, usize), usize> = HashMap::new();
    let mut elems: Vec<[usize; 6]> = Vec::with_capacity(tris.len());
    for t in tris {
        let mut e = [t[0], t[1], t[2], 0, 0, 0];
        for (k, (a, b)) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])]
            .into_iter()
            .enumerate()
        {
            let key = (a.min(b), a.max(b));
            let id = *mid.entry(key).or_insert_with(|| {
                nodes.push(crate::geom::lerp(corners[a], corners[b], 0.5));
                nodes.len() - 1
            });
            e[3 + k] = id;
        }
        elems.push(e);
    }
    let n = nodes.len();
    // ω is defined up to a constant: node 0 is held at 0, the others are the equations.
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n - 1];
    for e in &elems {
        for &a in e {
            for &b in e {
                if a != b && a > 0 && b > 0 {
                    adj[a - 1].push(b - 1);
                }
            }
        }
    }
    for list in &mut adj {
        list.sort_unstable();
        list.dedup();
    }
    let pattern = std::sync::Arc::new(Pattern::new(&adj, &vec![1; n - 1]));
    let eq = |v: usize| pattern.first_equation(v - 1);
    let mut k = SymMatrix::new(pattern.clone());
    let mut f = vec![0.0; n];
    let mut ip = 0.0;
    // 3-point rule at the edge midpoints (exact for quadratics).
    const QP: [[f64; 3]; 3] = [[0.5, 0.5, 0.0], [0.0, 0.5, 0.5], [0.5, 0.0, 0.5]];
    for (e, &gw) in elems.iter().zip(weights) {
        let [p1, p2, p3] = [nodes[e[0]], nodes[e[1]], nodes[e[2]]];
        let det = (p2[0] - p1[0]) * (p3[1] - p1[1]) - (p3[0] - p1[0]) * (p2[1] - p1[1]);
        if det <= 0.0 {
            return Err(bad("section torsion mesh: a triangle is turned over"));
        }
        let area = det / 2.0;
        // Gradients of the area coordinates L1, L2, L3.
        let g = [
            [(p2[1] - p3[1]) / det, (p3[0] - p2[0]) / det],
            [(p3[1] - p1[1]) / det, (p1[0] - p3[0]) / det],
            [(p1[1] - p2[1]) / det, (p2[0] - p1[0]) / det],
        ];
        let mut ke = [[0.0; 6]; 6];
        for l in QP {
            let w = gw * area / 3.0;
            // Shape function gradients: corners (4Li - 1) ∇Li, mid-edge 4(Li ∇Lj + Lj ∇Li).
            let mut dn = [[0.0; 2]; 6];
            for i in 0..3 {
                for c in 0..2 {
                    dn[i][c] = (4.0 * l[i] - 1.0) * g[i][c];
                }
            }
            for (m, (i, j)) in [(0, 1), (1, 2), (2, 0)].into_iter().enumerate() {
                for c in 0..2 {
                    dn[3 + m][c] = 4.0 * (l[i] * g[j][c] + l[j] * g[i][c]);
                }
            }
            let x = [
                l[0] * p1[0] + l[1] * p2[0] + l[2] * p3[0],
                l[0] * p1[1] + l[1] * p2[1] + l[2] * p3[1],
            ];
            ip += w * (x[0] * x[0] + x[1] * x[1]);
            for a in 0..6 {
                // f = ∫ g ∇N · (v, -h)
                f[e[a]] += w * (dn[a][0] * x[1] - dn[a][1] * x[0]);
                for b in 0..6 {
                    ke[a][b] += w * (dn[a][0] * dn[b][0] + dn[a][1] * dn[b][1]);
                }
            }
        }
        // Each pair of distinct nodes once.
        for a in 0..6 {
            for b in 0..6 {
                let (u, v) = (e[a], e[b]);
                if u > 0 && v > 0 && u <= v {
                    k.add(eq(u), eq(v), ke[a][b]);
                }
            }
        }
    }
    let (factor, _) = k
        .factorize()
        .map_err(|_| bad("section torsion: the mesh is not connected"))?;
    let mut rhs = vec![0.0; n - 1];
    for v in 1..n {
        rhs[eq(v)] = f[v];
    }
    factor.solve(&mut rhs);
    let wf: f64 = (1..n).map(|v| rhs[eq(v)] * f[v]).sum();
    Ok(ip - wf)
}

/// The torsion constant of a region: the sum over its separate faces.
fn region_torsion(r: &Region, thin: f64) -> Result<f64, Error> {
    let polys = torsion_polys(r);
    let mut j = 0.0;
    for (outer, holes) in faces(&polys) {
        let (lo, hi) = bounds(std::slice::from_ref(&outer));
        let span = (hi[0] - lo[0]).max(hi[1] - lo[1]);
        let size = (thin / 3.0).min(span / 30.0);
        j += face_torsion(&outer, &holes, &[], &|_| 1.0, size)?;
    }
    Ok(j)
}

fn bounds(polys: &[Vec<P2>]) -> (P2, P2) {
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for p in polys {
        for q in p {
            for d in 0..2 {
                lo[d] = lo[d].min(q[d]);
                hi[d] = hi[d].max(q[d]);
            }
        }
    }
    (lo, hi)
}

fn region_bounds(r: &Region) -> (P2, P2) {
    let (mut lo, mut hi) = bounds(&r.polys);
    for &(c, rad, sign) in &r.circles {
        if sign > 0.0 {
            for d in 0..2 {
                lo[d] = lo[d].min(c[d] - rad);
                hi[d] = hi[d].max(c[d] + rad);
            }
        }
    }
    (lo, hi)
}

/// The outline polygons of a region for drawing (circles as 48-sided polygons).
fn outline(r: &Region) -> Vec<Vec<P2>> {
    let mut out = r.polys.clone();
    for &(c, rad, sign) in &r.circles {
        let mut p: Vec<P2> = (0..48)
            .map(|k| {
                let t = std::f64::consts::TAU * k as f64 / 48.0;
                [c[0] + rad * t.cos(), c[1] + rad * t.sin()]
            })
            .collect();
        if sign < 0.0 {
            p.reverse();
        }
        out.push(p);
    }
    out
}

/// The material values a composite shape needs: E, G and weight per volume, by material id.
pub type MaterialValues<'a> = &'a dyn Fn(&Id) -> Option<(f64, f64, f64)>;

/// A shape's properties, lengths in metres. `material` gives E, G and weight of the materials a
/// composite shape names.
pub fn properties(s: &Shape, material: MaterialValues) -> Result<SectionProps, Error> {
    properties_in(s, material, 1.0)
}

/// A shape's properties in a model whose length unit is `metres` m (a list section's table is in metres;
/// other shapes are given in the model's unit).
pub fn properties_in(
    s: &Shape,
    material: MaterialValues,
    metres: f64,
) -> Result<SectionProps, Error> {
    if let Shape::Aisc { name, .. } = s {
        return list_properties(name, metres);
    }
    let g = geometry(s)?;
    // Factors of the fill: Ec/Es (A, I, shear areas) and Gc/Gs (J); the weights.
    let (n_e, n_g, weights) = match &g.materials {
        Some((m, fill)) => {
            let (es, gs, ws) = material(m).ok_or_else(|| bad(format!("unknown material {m}")))?;
            let (ec, gc, wc) =
                material(fill).ok_or_else(|| bad(format!("unknown material {fill}")))?;
            if !(es > 0.0 && gs > 0.0) {
                return Err(bad(format!("material {m}: E and G must be > 0")));
            }
            (ec / es, gc / gs, Some((ws, wc)))
        }
        None => (0.0, 0.0, None),
    };
    let mut regions: Vec<(&Region, f64)> = vec![(&g.main, 1.0)];
    if let Some(f) = &g.fill {
        regions.push((f, n_e));
    }
    let mut m = Moments::default();
    for (r, f) in &regions {
        m.add(&region_moments(r), *f);
    }
    if !(m.a > 0.0) {
        return Err(bad("the section has no area"));
    }
    let (hc, vc) = (m.sh / m.a, m.sv / m.a);
    let i33 = m.vv - m.a * vc * vc;
    let i22 = m.hh - m.a * hc * hc;
    let mut i23 = m.hv - m.a * hc * vc;
    if i23.abs() <= 1e-12 * (i22 * i33).sqrt() {
        i23 = 0.0;
    }
    let (lo, hi) = {
        let (mut lo, mut hi) = region_bounds(&g.main);
        if let Some(f) = &g.fill {
            let (l2, h2) = region_bounds(f);
            for d in 0..2 {
                lo[d] = lo[d].min(l2[d]);
                hi[d] = hi[d].max(h2[d]);
            }
        }
        (lo, hi)
    };
    let (as2, as3) = match (g.ring, &g.fill) {
        // A pipe: As = A 3(1 + m²) / 4(1 + m + m²), m = inner / outer radius (A/2 thin, 3A/4 with a
        // pinhole).
        (Some((r, ri)), None) => {
            let m = ri / r;
            let a = region_moments(&g.main).a;
            let s = a * 3.0 * (1.0 + m * m) / (4.0 * (1.0 + m + m * m));
            (s, s)
        }
        _ => (
            shear_area(&regions, 1, lo[1], hi[1]),
            shear_area(&regions, 0, lo[0], hi[0]),
        ),
    };
    let main_j = match (&g.bonded, g.thin_pipe) {
        (Some((outer, line, inside_fill)), _) => {
            let (fi, fo) = if *inside_fill { (n_g, 1.0) } else { (1.0, n_g) };
            let factor = |p: P2| {
                if crate::geom::inside(p, line, 0.0) {
                    fi
                } else {
                    fo
                }
            };
            let (blo, bhi) = bounds(std::slice::from_ref(outer));
            let span = (bhi[0] - blo[0]).max(bhi[1] - blo[1]);
            // The steel walls get about one quadratic element across; the concrete sets the size.
            let size = g.thin.max(span / 40.0);
            face_torsion(outer, &[], std::slice::from_ref(line), &factor, size)?
        }
        (None, thin_pipe) => match thin_pipe {
            Some((r, ri)) => {
                let s = std::f64::consts::TAU / CIRCLE_SIDES as f64;
                let polygon =
                    CIRCLE_SIDES as f64 * s.sin() * (2.0 + s.cos()) / (6.0 * std::f64::consts::PI);
                polygon * std::f64::consts::PI / 2.0 * (r.powi(4) - ri.powi(4))
            }
            None => region_torsion(&g.main, g.thin)?,
        },
    };
    let self_weight = weights.map(|(ws, wc)| {
        ws * region_moments(&g.main).a + g.fill.as_ref().map_or(0.0, |f| wc * region_moments(f).a)
    });
    let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    Ok(SectionProps {
        A: m.a,
        I22: i22,
        I33: i33,
        I23: i23,
        J: main_j,
        As2: as2,
        As3: as3,
        depth: hi[1] - lo[1],
        width: hi[0] - lo[0],
        centroid_offset: [vc - mid[1], hc - mid[0]],
        self_weight,
        outline: outline(&g.main),
        fill_outline: g.fill.as_ref().map(outline).unwrap_or_default(),
        like: None,
    })
}

/// A list section in the model's unit (`metres` m per unit).
fn list_properties(name: &str, metres: f64) -> Result<SectionProps, Error> {
    let shape = crate::aisc::find(name)
        .ok_or_else(|| bad(format!("{name} is not in the steel section list")))?;
    let p = crate::aisc::properties(shape).map_err(bad)?;
    let k = 1.0 / metres;
    let like = scaled(&p.like, k);
    let g = geometry(&like)?;
    let (lo, hi) = region_bounds(&g.main);
    Ok(SectionProps {
        A: p.A * k * k,
        I22: p.I22 * k.powi(4),
        I33: p.I33 * k.powi(4),
        I23: p.I23 * k.powi(4),
        J: p.J * k.powi(4),
        As2: p.As2 * k * k,
        As3: p.As3 * k * k,
        depth: hi[1] - lo[1],
        width: hi[0] - lo[0],
        centroid_offset: [p.centroid[0] * k, p.centroid[1] * k],
        self_weight: None,
        outline: outline(&g.main),
        fill_outline: Vec::new(),
        like: Some(like),
    })
}

/// A plain shape with its lengths times `k`.
fn scaled(s: &Shape, k: f64) -> Shape {
    let mut v = serde_json::to_value(s).expect("shape serializes");
    if let Some(o) = v.as_object_mut() {
        for (key, x) in o.iter_mut() {
            if key != "type" {
                if let Some(f) = x.as_f64() {
                    *x = (f * k).into();
                }
            }
        }
    }
    serde_json::from_value(v).expect("scaled shape")
}

/// Metres per unit of a model length unit.
pub fn metres(length: &str) -> f64 {
    match length {
        "mm" => 0.001,
        "cm" => 0.01,
        "in" => 0.0254,
        "ft" => 0.3048,
        _ => 1.0,
    }
}

/// Fills in the numbers of every section with a shape (the mesh step's first job).
pub fn resolve(model: &Model) -> Result<std::borrow::Cow<'_, Model>, Error> {
    if model.sections.iter().all(|s| s.shape.is_none()) {
        return Ok(std::borrow::Cow::Borrowed(model));
    }
    let mut m = model.clone();
    let material = |id: &Id| {
        model
            .materials
            .iter()
            .find(|x| &x.id == id)
            .map(|x| (x.e, x.shear_modulus(), x.weight))
    };
    for s in &mut m.sections {
        let Some(raw) = &s.shape else { continue };
        let shape: Shape = serde_json::from_value(raw.clone())
            .map_err(|e| bad(format!("section {}: shape: {e}", s.id)))?;
        let p =
            properties_in(&shape, &material, metres(&model.units.length)).map_err(|e| match e {
                Error::Invalid(msg) => bad(format!("section {}: {msg}", s.id)),
                other => other,
            })?;
        apply(s, &p);
        if let Some(steel) = composite_material(&shape) {
            for f in model.frames.iter().filter(|f| f.section == s.id) {
                if &f.material != steel {
                    return Err(bad(format!(
                        "frame {}: section {} is transformed to material {steel}; give the frame that material",
                        f.id, s.id
                    )));
                }
            }
        }
    }
    Ok(std::borrow::Cow::Owned(m))
}

/// The steel material of a composite shape (the one it is transformed to).
fn composite_material(s: &Shape) -> Option<&Id> {
    match s {
        Shape::FilledTube { material, .. }
        | Shape::FilledPipe { material, .. }
        | Shape::EncasedRectangle { material, .. }
        | Shape::EncasedCircle { material, .. } => Some(material),
        _ => None,
    }
}

fn apply(s: &mut Section, p: &SectionProps) {
    s.A = p.A;
    s.I22 = p.I22;
    s.I33 = p.I33;
    s.I23 = p.I23;
    s.J = p.J;
    s.As2 = p.As2;
    s.As3 = p.As3;
    s.depth = Some(p.depth);
    s.width = Some(p.width);
    let off = p.centroid_offset;
    s.centroid_offset =
        (off[0].abs() > 1e-12 * p.depth || off[1].abs() > 1e-12 * p.width).then_some(off);
    s.self_weight = p.self_weight;
}

/// The `section` command: a shape (with the materials of the model it belongs to) to its properties.
#[derive(Debug, Clone, Deserialize)]
pub struct SectionRequest {
    pub shape: Shape,
    #[serde(default)]
    pub materials: Vec<osab_solver::model::Material>,
    /// The model's length unit (a list section's numbers are converted to it); missing = m.
    #[serde(default)]
    pub length: Option<String>,
}

pub fn section_value(v: serde_json::Value) -> Result<SectionProps, osab_solver::api::ErrorReport> {
    let req: SectionRequest = serde_json::from_value(v).map_err(Error::Json)?;
    let material = |id: &Id| {
        req.materials
            .iter()
            .find(|x| &x.id == id)
            .map(|x| (x.e, x.shear_modulus(), x.weight))
    };
    Ok(properties_in(
        &req.shape,
        &material,
        metres(req.length.as_deref().unwrap_or("m")),
    )?)
}

/// The `section_list` command: the steel list's names and types, for picking one.
pub fn section_list() -> Vec<(&'static str, &'static str)> {
    crate::aisc::list()
        .iter()
        .map(|s| (s.name.as_str(), s.kind.as_str()))
        .collect()
}
