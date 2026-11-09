//! Rounds: turn a closed shape into crochet rounds with a stitch count each.
//!
//! Our own method, from basic geometry: slice the shape at fine heights, take each slice's perimeter, place rounds
//! one row-height apart *along the surface* (so a ball's rounds get closer together near the poles, like a real
//! ball), and set each round's stitches to perimeter / stitch width. Then smooth the counts so each round can be
//! made from the one before: start with a magic ring of 6, change by at most 6 stitches a round, close at 6.

use std::f64::consts::PI;

use crate::gauge::{Gauge, GaugeError};
use crate::mesh::Mesh;
use crate::slice::slices;

/// A magic ring starts with this many stitches, and a closed end finishes with this many.
pub const RING: u32 = 6;
/// Most stitches a round may gain or lose compared with the one before.
pub const MAX_CHANGE: u32 = 6;
/// Most rounds in one part.
pub const MAX_ROUNDS: usize = 3000;

/// Size of one stitch in inches, from the gauge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stitch {
    pub width: f64,
    pub height: f64,
}

impl Stitch {
    pub fn from_gauge(g: &Gauge) -> Result<Self, GaugeError> {
        g.validate()?;
        Ok(Self { width: 1.0 / g.stitches_per_inch(), height: 1.0 / g.rows_per_inch() })
    }
}

/// Why rounds could not be made.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RoundsError {
    #[error("gauge: {0}")]
    Gauge(#[from] GaugeError),
    #[error("the model is empty or flat")]
    Flat,
    #[error("the figure would need more than {MAX_ROUNDS} rounds; make it smaller or use a thicker yarn")]
    TooManyRounds,
}

/// One round of a part.
#[derive(Clone, Debug, PartialEq)]
pub struct Round {
    /// Round number, starting at 1.
    pub index: usize,
    /// Stitches in this round.
    pub stitches: u32,
    /// Height in the model (inches) and centre of the round.
    pub z: f64,
    pub center: [f64; 2],
    /// Radius of the crocheted ring (stitches x stitch width / 2 pi).
    pub radius: f64,
}

/// One point of a part's outline, in the order it is crocheted: height, perimeter and centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProfilePoint {
    pub z: f64,
    pub perimeter: f64,
    pub center: [f64; 2],
}

/// Rounds for a whole single-piece shape (closed at both ends), starting at the bottom.
pub fn rounds_for_mesh(mesh: &Mesh, gauge: &Gauge) -> Result<Vec<Round>, RoundsError> {
    let stitch = Stitch::from_gauge(gauge)?;
    let (lo, hi) = mesh.bounds();
    let height = hi[2] - lo[2];
    if !(height.is_finite() && height > 0.0) {
        return Err(RoundsError::Flat);
    }
    let n = ((height / (stitch.height / 6.0)).ceil() as usize).clamp(48, 3000);
    let profile: Vec<ProfilePoint> = slices(mesh, n)
        .iter()
        .map(|s| {
            let perimeter: f64 = s.loops.iter().map(|l| l.perimeter).sum();
            let area: f64 = s.loops.iter().map(|l| l.area).sum();
            let center = if area > 0.0 {
                [
                    s.loops.iter().map(|l| l.centroid[0] * l.area).sum::<f64>() / area,
                    s.loops.iter().map(|l| l.centroid[1] * l.area).sum::<f64>() / area,
                ]
            } else {
                [0.0, 0.0]
            };
            ProfilePoint { z: s.z, perimeter, center }
        })
        .collect();
    make_rounds(&profile, true, true, stitch)
}

/// Rounds for one outline. `closed_first` / `closed_last` say that end is a closed tip (the outline shrinks to a
/// point there, or ends in a flat cap); an open end is where the part is sewn to another.
pub fn make_rounds(profile: &[ProfilePoint], closed_first: bool, closed_last: bool, stitch: Stitch) -> Result<Vec<Round>, RoundsError> {
    if profile.is_empty() || !(stitch.width > 0.0 && stitch.height > 0.0) {
        return Err(RoundsError::Flat);
    }
    // The outline as (arc distance along the surface, radius, z, centre).
    let mut pts: Vec<(f64, f64, f64, [f64; 2])> = Vec::new();
    let step = if profile.len() > 1 { (profile[1].z - profile[0].z).abs() / 2.0 } else { stitch.height / 2.0 };
    if closed_first {
        let dir = if profile.len() > 1 && profile[1].z < profile[0].z { 1.0 } else { -1.0 };
        pts.push((0.0, 0.0, profile[0].z + dir * step, profile[0].center));
    }
    for p in profile {
        pts.push((0.0, (p.perimeter / (2.0 * PI)).max(0.0), p.z, p.center));
    }
    if closed_last {
        let n = profile.len();
        let last_step = if n > 1 { (profile[n - 1].z - profile[n - 2].z).abs() / 2.0 } else { step };
        let dir = if n > 1 && profile[n - 1].z < profile[n - 2].z { -1.0 } else { 1.0 };
        let l = profile[n - 1];
        pts.push((0.0, 0.0, l.z + dir * last_step, l.center));
    }
    for i in 1..pts.len() {
        let (a, b) = (pts[i - 1], pts[i]);
        pts[i].0 = a.0 + ((b.2 - a.2).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    }
    let total = pts.last().map_or(0.0, |p| p.0);
    if !(total.is_finite() && total > 0.0) {
        return Err(RoundsError::Flat);
    }
    let k = if closed_last {
        ((total / stitch.height).round() as usize).saturating_sub(1).max(2)
    } else {
        ((total / stitch.height).round() as usize).max(1)
    };
    if k > MAX_ROUNDS {
        return Err(RoundsError::TooManyRounds);
    }
    let spacing = if closed_last { total / (k as f64 + 1.0) } else { total / k as f64 };
    let mut targets = Vec::with_capacity(k);
    let mut at = Vec::with_capacity(k);
    let mut seg = 0;
    for i in 1..=k {
        let s = spacing * i as f64;
        while seg + 2 < pts.len() && pts[seg + 1].0 < s {
            seg += 1;
        }
        let (a, b) = (pts[seg], pts[seg + 1]);
        let f = if b.0 > a.0 { ((s - a.0) / (b.0 - a.0)).clamp(0.0, 1.0) } else { 0.0 };
        let lerp = |x: f64, y: f64| x + f * (y - x);
        let r = lerp(a.1, b.1);
        targets.push(((2.0 * PI * r / stitch.width).round().max(f64::from(RING))) as u32);
        at.push((lerp(a.2, b.2), [lerp(a.3[0], b.3[0]), lerp(a.3[1], b.3[1])]));
    }
    let counts = smooth(&targets, closed_last);
    Ok(counts
        .iter()
        .zip(&at)
        .enumerate()
        .map(|(i, (&c, &(z, center)))| Round { index: i + 1, stitches: c, z, center, radius: f64::from(c) * stitch.width / (2.0 * PI) })
        .collect())
}

/// Makes a list of wanted counts buildable: first round 6, never below 6, each round within 6 of its neighbours
/// (both ways, so an end can also come down gently), and the last round 6 when `pin_last`.
pub fn smooth(targets: &[u32], pin_last: bool) -> Vec<u32> {
    let mut c: Vec<u32> = targets.iter().map(|&t| t.max(RING)).collect();
    let n = c.len();
    if n == 0 {
        return c;
    }
    let limit = |x: u32, around: u32| x.clamp(around.saturating_sub(MAX_CHANGE).max(RING), around + MAX_CHANGE);
    for _ in 0..200 {
        let before = c.clone();
        c[0] = RING;
        for i in 1..n {
            c[i] = limit(c[i], c[i - 1]);
        }
        if pin_last {
            c[n - 1] = RING;
        }
        for i in (0..n - 1).rev() {
            c[i] = limit(c[i], c[i + 1]);
        }
        c[0] = RING;
        if c == before {
            break;
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gauge::Unit;
    use crate::models;
    use proptest::prelude::*;

    fn gauge(w: f64, h: f64) -> Gauge {
        // w and h in inches per stitch / row, as a gauge over 4 inches.
        Gauge::new(4.0 / w, 4.0 / h, 4.0, Unit::Inches).unwrap()
    }

    /// Every round can be worked from the one before.
    fn buildable(c: &[u32], closed_last: bool) {
        assert_eq!(c.first(), Some(&RING));
        if closed_last {
            assert_eq!(c.last(), Some(&RING));
        }
        for w in c.windows(2) {
            let (a, b) = (w[0], w[1]);
            assert!(b >= RING && a >= RING);
            if b >= a {
                assert!(b - a <= MAX_CHANGE.min(a), "{a}->{b}"); // each increase uses one stitch
            } else {
                assert!(a - b <= MAX_CHANGE && 2 * (a - b) <= a, "{a}->{b}"); // each decrease uses two
            }
        }
    }

    #[test]
    fn a_ball_gives_the_classic_counts_and_back_down() {
        // 36 stitches round the middle: R = 36 w / 2 pi.
        let w = 0.2;
        let r = 36.0 * w / (2.0 * PI);
        let rounds = rounds_for_mesh(&models::sphere(r), &gauge(w, w)).unwrap();
        let c: Vec<u32> = rounds.iter().map(|r| r.stitches).collect();
        buildable(&c, true);
        // Expected from the circle itself: rounds one row apart along the surface.
        let total = PI * r;
        let k = c.len();
        let spacing = total / (k as f64 + 1.0);
        for (i, &got) in c.iter().enumerate() {
            let want = (2.0 * PI * r * (spacing * (i + 1) as f64 / r).sin() / w).round().max(6.0);
            assert!((f64::from(got) - want).abs() <= 1.0, "round {}: got {got}, want {want} ({c:?})", i + 1);
        }
        assert_eq!(&c[..3], &[6, 12, 18], "{c:?}");
        assert!(c.iter().copied().max() >= Some(35) && c.iter().copied().max() <= Some(37));
        // Back down: the second half mirrors the first within one.
        for i in 0..k / 2 {
            assert!((f64::from(c[i]) - f64::from(c[k - 1 - i])).abs() <= 1.0, "{c:?}");
        }
        assert!((16..=18).contains(&k), "{k} rounds");
    }

    #[test]
    fn a_cylinder_holds_steady() {
        let w = 0.2;
        let r = 30.0 * w / (2.0 * PI);
        let rounds = rounds_for_mesh(&models::cylinder(r, 3.0), &gauge(w, w)).unwrap();
        let c: Vec<u32> = rounds.iter().map(|r| r.stitches).collect();
        buildable(&c, true);
        let steady = c.iter().filter(|&&x| (29..=31).contains(&x)).count();
        assert!(steady >= c.len() - 11, "{c:?}"); // 5 rounds each end to grow from 6 and shrink to 6
        assert!(c[c.len() / 2 - 2..c.len() / 2 + 2].iter().all(|&x| (29..=31).contains(&x)));
    }

    #[test]
    fn a_cone_comes_to_a_point_and_other_shapes_work() {
        let g = gauge(0.2, 0.2);
        for m in [models::cone(1.0, 2.0), models::capsule(0.8, 1.5), models::snowman()] {
            let c: Vec<u32> = rounds_for_mesh(&m, &g).unwrap().iter().map(|r| r.stitches).collect();
            buildable(&c, true);
        }
    }

    #[test]
    fn heights_and_radii_are_in_the_model() {
        let w = 0.2;
        let rounds = rounds_for_mesh(&models::sphere(1.0), &gauge(w, w)).unwrap();
        for r in &rounds {
            assert!(r.z > 0.0 && r.z < 2.0 && r.radius > 0.0 && r.radius <= 1.05);
        }
        assert!(rounds.windows(2).all(|p| p[1].z > p[0].z - 1e-9 || p[1].z < p[0].z + 1e-9));
        assert_eq!(rounds[0].index, 1);
    }

    #[test]
    fn hostile_gauges_and_flat_shapes_are_errors() {
        assert!(Stitch::from_gauge(&Gauge { stitches: 0.0, rows: 1.0, measure: 1.0, unit: Unit::Inches }).is_err());
        let flat = Mesh { vertices: vec![[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]], triangles: vec![[0, 1, 2]] };
        assert_eq!(rounds_for_mesh(&flat, &gauge(0.2, 0.2)), Err(RoundsError::Flat));
        // Tiny stitches on a big model: too many rounds, an error and not a hang.
        let big = models::sphere(1000.0);
        assert!(rounds_for_mesh(&big, &gauge(0.01, 0.01)).is_err());
    }

    proptest! {
        #[test]
        fn smoothing_always_gives_buildable_counts(t in proptest::collection::vec(0u32..200, 1..60), pin in any::<bool>()) {
            let c = smooth(&t, pin);
            prop_assert_eq!(c.len(), t.len());
            prop_assert_eq!(c[0], RING);
            if pin { prop_assert_eq!(*c.last().unwrap(), RING); }
            for w in c.windows(2) {
                let (a, b) = (w[0], w[1]);
                prop_assert!(a >= RING && b >= RING);
                if b >= a { prop_assert!(b - a <= MAX_CHANGE.min(a)); } else { prop_assert!(a - b <= MAX_CHANGE && 2 * (a - b) <= a); }
            }
        }

        #[test]
        fn random_outlines_make_buildable_rounds(per in proptest::collection::vec(0.0f64..30.0, 2..80), cf in any::<bool>(), cl in any::<bool>()) {
            let prof: Vec<ProfilePoint> = per.iter().enumerate().map(|(i, p)| ProfilePoint { z: i as f64 * 0.1, perimeter: *p, center: [0.0, 0.0] }).collect();
            if let Ok(r) = make_rounds(&prof, cf, cl, Stitch { width: 0.2, height: 0.2 }) {
                let c: Vec<u32> = r.iter().map(|x| x.stitches).collect();
                prop_assert_eq!(c[0], RING);
                if cl { prop_assert_eq!(*c.last().unwrap(), RING); }
                for w in c.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    let ok = if b >= a { b - a <= MAX_CHANGE.min(a) } else { a - b <= MAX_CHANGE && 2 * (a - b) <= a };
                    prop_assert!(ok);
                }
            }
        }
    }
}
