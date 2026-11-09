//! Original sample 3D models, made in code (no files, no third-party shapes): sphere, capsule, cone, cylinder, snowman.
//! Sizes are in the caller's units; every model stands on z = 0, centred on x and y, outward-facing and closed.

use std::f64::consts::PI;

use crate::mesh::Mesh;

const SEGMENTS: usize = 48;

/// Surface of revolution from a profile of `(radius, z)` points, bottom to top. A point with radius 0 is a pole.
pub fn lathe(profile: &[(f64, f64)], segments: usize) -> Mesh {
    let seg = segments.max(3);
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut rings: Vec<(u32, bool)> = Vec::new(); // first vertex index, is_pole
    for &(r, z) in profile {
        let first = vertices.len() as u32;
        if r <= 1e-12 {
            vertices.push([0.0, 0.0, z]);
            rings.push((first, true));
        } else {
            for k in 0..seg {
                let a = 2.0 * PI * k as f64 / seg as f64;
                vertices.push([r * a.cos(), r * a.sin(), z]);
            }
            rings.push((first, false));
        }
    }
    let mut triangles = Vec::new();
    for w in rings.windows(2) {
        let ((a, a_pole), (b, b_pole)) = (w[0], w[1]);
        if a_pole && b_pole {
            continue;
        }
        for k in 0..seg as u32 {
            let k1 = (k + 1) % seg as u32;
            if a_pole {
                triangles.push([a, b + k, b + k1]);
            } else if b_pole {
                triangles.push([a + k, b, a + k1]);
            } else {
                triangles.push([a + k, b + k, b + k1]);
                triangles.push([a + k, b + k1, a + k1]);
            }
        }
    }
    let mut mesh = Mesh { vertices, triangles };
    if mesh.signed_volume() < 0.0 {
        for t in &mut mesh.triangles {
            t.swap(1, 2);
        }
    }
    mesh
}

fn arc(cx_z: f64, radius: f64, from: f64, to: f64, steps: usize) -> Vec<(f64, f64)> {
    (0..=steps)
        .map(|i| {
            let a = from + (to - from) * i as f64 / steps as f64;
            (radius * a.sin(), cx_z - radius * a.cos())
        })
        .collect()
}

/// A ball of `radius`.
pub fn sphere(radius: f64) -> Mesh {
    lathe(&arc(radius, radius, 0.0, PI, SEGMENTS), SEGMENTS)
}

/// A capsule: a cylinder `length` long with a half ball of `radius` on each end (total height `length + 2 radius`).
pub fn capsule(radius: f64, length: f64) -> Mesh {
    let mut p = arc(radius, radius, 0.0, PI / 2.0, SEGMENTS / 2);
    let top = arc(radius + length, radius, PI / 2.0, PI, SEGMENTS / 2);
    p.extend(top);
    lathe(&p, SEGMENTS)
}

/// A cone with a flat base of `radius` and the given `height`.
pub fn cone(radius: f64, height: f64) -> Mesh {
    lathe(&[(0.0, 0.0), (radius, 0.0), (0.0, height)], SEGMENTS)
}

/// A straight cylinder with flat ends.
pub fn cylinder(radius: f64, height: f64) -> Mesh {
    lathe(&[(0.0, 0.0), (radius, 0.0), (radius, height), (0.0, height)], SEGMENTS)
}

/// A snowman: a body ball of radius 1 and a head ball of radius 0.65 whose centres are 1.5 apart, joined as one
/// closed surface with a narrow neck between them.
pub fn snowman() -> Mesh {
    let (r1, r2, d) = (1.0_f64, 0.65_f64, 1.5_f64);
    let a = (d * d + r1 * r1 - r2 * r2) / (2.0 * d); // height of the neck above the body centre
    let neck_angle_lower = (a / r1).acos(); // measured from the top of the body ball
    let c1 = r1;
    let c2 = r1 + d;
    // Body ball from the bottom pole up to the neck.
    let theta1 = PI - neck_angle_lower;
    let mut p = arc(c1, r1, 0.0, theta1, SEGMENTS / 2);
    // Head ball from the neck up to its top pole: angle measured from the head's bottom pole.
    let phi = ((c1 + a - c2) / -r2).clamp(-1.0, 1.0).acos();
    p.extend(arc(c2, r2, phi, PI, SEGMENTS / 2));
    lathe(&p, SEGMENTS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_volume_and_height() {
        let m = sphere(2.0);
        let (lo, hi) = m.bounds();
        assert!((hi[2] - lo[2] - 4.0).abs() < 1e-9);
        let v = 4.0 / 3.0 * PI * 8.0;
        assert!((m.signed_volume() - v).abs() / v < 0.01);
    }

    #[test]
    fn every_sample_is_closed_and_outward() {
        for m in [sphere(1.0), capsule(1.0, 2.0), cone(1.0, 2.0), cylinder(1.0, 2.0), snowman()] {
            let (checked, report) = m.checked().expect("closed");
            assert!(!report.flipped_normals && report.holes_closed == 0 && report.bodies == 1, "{report:?}");
            assert!(checked.signed_volume() > 0.0);
        }
    }

    #[test]
    fn snowman_is_one_piece_with_a_narrow_neck() {
        let m = snowman();
        let (lo, hi) = m.bounds();
        assert!((hi[2] - lo[2] - (1.0 + 1.5 + 0.65)).abs() < 0.02);
        assert_eq!(m.bodies(), 1);
    }
}
