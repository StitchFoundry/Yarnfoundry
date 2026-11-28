//! Shapes: click a figure together from simple solids, no 3D file needed.
//!
//! Each shape is a solid of revolution (plain geometry) that goes through the same slicer and rounds engine as an
//! imported model, so a sphere of a given size gets exactly the counts the engine gives a sphere mesh.

use std::f64::consts::PI;

use crate::gauge::Gauge;
use crate::mesh::Mesh;
use crate::models::lathe;
use crate::parts::Part;
use crate::rounds::{RoundsError, rounds_for_mesh};

const SEGMENTS: usize = 48;
/// Largest size of any shape side, in inches (keeps hostile files from asking for huge meshes).
pub const MAX_SIZE: f64 = 200.0;
/// Stretch is limited to this range (1 = no stretch).
pub const STRETCH_RANGE: (f64, f64) = (0.1, 10.0);

/// The kinds of shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Sphere,
    Ellipsoid,
    Cylinder,
    Cone,
    Capsule,
    /// A flat disc, for ears and soles.
    Disc,
}

/// One shape: `width` is the diameter and `height` the total height, both in inches. `stretch` multiplies the height.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub kind: ShapeKind,
    pub name: String,
    pub width: f64,
    pub height: f64,
    pub stretch: f64,
}

/// Why a shape is not valid.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ShapeError {
    #[error("shape sizes must be numbers between 0 and {MAX_SIZE} inches")]
    Size,
    #[error("stretch must be a number between {} and {}", STRETCH_RANGE.0, STRETCH_RANGE.1)]
    Stretch,
    #[error(transparent)]
    Rounds(#[from] RoundsError),
}

impl Shape {
    /// A ball of the given diameter.
    pub fn sphere(name: &str, diameter: f64) -> Self {
        Self { kind: ShapeKind::Sphere, name: name.to_string(), width: diameter, height: diameter, stretch: 1.0 }
    }

    /// The same shape for the other side (arms, ears): identical size, so an identical pattern.
    pub fn mirrored(&self, name: &str) -> Self {
        Self { name: name.to_string(), ..self.clone() }
    }

    /// Height after stretch.
    pub fn stretched_height(&self) -> f64 {
        // A sphere stays a ball whatever its stored height.
        let h = if self.kind == ShapeKind::Sphere { self.width } else { self.height };
        h * self.stretch
    }

    pub fn validate(&self) -> Result<(), ShapeError> {
        let ok = |v: f64| v.is_finite() && v > 0.0 && v <= MAX_SIZE;
        if !(self.stretch.is_finite() && self.stretch >= STRETCH_RANGE.0 && self.stretch <= STRETCH_RANGE.1) {
            return Err(ShapeError::Stretch);
        }
        if !ok(self.width) || !ok(self.height) || !ok(self.stretched_height()) {
            return Err(ShapeError::Size);
        }
        Ok(())
    }

    /// The solid, upright, starting at height 0.
    pub fn mesh(&self) -> Result<Mesh, ShapeError> {
        self.validate()?;
        let (r, h) = (self.width / 2.0, self.stretched_height());
        let profile = match self.kind {
            ShapeKind::Sphere | ShapeKind::Ellipsoid => (0..=SEGMENTS)
                .map(|i| {
                    let a = PI * i as f64 / SEGMENTS as f64;
                    (r * a.sin(), h / 2.0 - h / 2.0 * a.cos())
                })
                .collect(),
            ShapeKind::Cylinder | ShapeKind::Disc => vec![(0.0, 0.0), (r, 0.0), (r, h), (0.0, h)],
            ShapeKind::Cone => vec![(0.0, 0.0), (r, 0.0), (0.0, h)],
            ShapeKind::Capsule => {
                // Half balls of radius r (or as much as fits) on a straight middle.
                let cap = r.min(h / 2.0);
                let squash = r / cap;
                let half = SEGMENTS / 2;
                let mut p: Vec<(f64, f64)> = (0..=half)
                    .map(|i| {
                        let a = PI / 2.0 * i as f64 / half as f64;
                        (squash * cap * a.sin(), cap - cap * a.cos())
                    })
                    .collect();
                p.extend((0..=half).map(|i| {
                    let a = PI / 2.0 + PI / 2.0 * i as f64 / half as f64;
                    (squash * cap * a.sin(), h - cap - cap * a.cos())
                }));
                p
            }
        };
        Ok(lathe(&profile, SEGMENTS))
    }

    /// The shape as a crocheted part: worked from the bottom, closed at the end.
    pub fn part(&self, gauge: &Gauge) -> Result<Part, ShapeError> {
        let rounds = rounds_for_mesh(&self.mesh()?, gauge)?;
        let peak_perimeter = rounds.iter().map(|r| r.radius * 2.0 * PI).fold(0.0, f64::max);
        Ok(Part { name: self.name.clone(), rounds, closed_last: true, from_top: false, attach: Vec::new(), peak_perimeter })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gauge::Unit;
    use crate::models;

    fn gauge() -> Gauge {
        Gauge::new(20.0, 20.0, 4.0, Unit::Inches).unwrap()
    }

    fn counts(p: &Part) -> Vec<u32> {
        p.rounds.iter().map(|r| r.stitches).collect()
    }

    #[test]
    fn a_sphere_gives_the_same_counts_as_the_slicer() {
        let s = Shape::sphere("Head", 3.0);
        let want = rounds_for_mesh(&models::sphere(1.5), &gauge()).unwrap();
        let got = s.part(&gauge()).unwrap();
        assert_eq!(counts(&got), want.iter().map(|r| r.stitches).collect::<Vec<_>>());
        assert_eq!(got.rounds.first().map(|r| r.stitches), Some(6));
        assert_eq!(got.rounds.last().map(|r| r.stitches), Some(6));
    }

    #[test]
    fn mirrored_parts_have_identical_patterns() {
        let mut ear = Shape::sphere("Left ear", 1.5);
        ear.kind = ShapeKind::Ellipsoid;
        ear.height = 2.0;
        let (a, b) = (ear.part(&gauge()).unwrap(), ear.mirrored("Right ear").part(&gauge()).unwrap());
        assert_eq!(a.rounds, b.rounds);
        assert_ne!(a.name, b.name);
    }

    #[test]
    fn every_kind_makes_buildable_rounds_and_stretch_makes_it_taller() {
        for kind in [ShapeKind::Sphere, ShapeKind::Ellipsoid, ShapeKind::Cylinder, ShapeKind::Cone, ShapeKind::Capsule, ShapeKind::Disc] {
            let mut s = Shape { kind, name: "x".into(), width: 2.0, height: if kind == ShapeKind::Disc { 0.3 } else { 3.0 }, stretch: 1.0 };
            let c = counts(&s.part(&gauge()).unwrap());
            assert_eq!((c.first(), c.last()), (Some(&6), Some(&6)), "{kind:?}");
            s.stretch = 2.0;
            if kind != ShapeKind::Disc {
                assert!(counts(&s.part(&gauge()).unwrap()).len() > c.len(), "{kind:?}");
            }
        }
    }

    #[test]
    fn hostile_sizes_are_errors() {
        let bad = [
            (f64::NAN, 1.0, 1.0),
            (0.0, 1.0, 1.0),
            (-1.0, 1.0, 1.0),
            (1e9, 1.0, 1.0),
            (1.0, f64::INFINITY, 1.0),
            (1.0, 1.0, 0.0),
            (1.0, 1.0, f64::NAN),
            (1.0, 100.0, 10.0),
        ];
        for (w, h, st) in bad {
            let s = Shape { kind: ShapeKind::Ellipsoid, name: "x".into(), width: w, height: h, stretch: st };
            assert!(s.part(&gauge()).is_err(), "{w} {h} {st}");
        }
    }
}
