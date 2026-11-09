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

/// A shape described by a distance function (negative inside), turned into a closed triangle mesh with marching
/// tetrahedra on a grid of `cells` along the longest side of the box `lo`..`hi`. Used for unions of balls and limbs.
pub fn implicit(sdf: &dyn Fn([f64; 3]) -> f64, lo: [f64; 3], hi: [f64; 3], cells: usize) -> Mesh {
    use std::collections::HashMap;
    let size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let step = size[0].max(size[1]).max(size[2]) / cells.max(4) as f64;
    // One spare cell all round so the surface always closes.
    let n = [0, 1, 2].map(|a| (size[a] / step).ceil() as usize + 3);
    let origin = [lo[0] - step, lo[1] - step, lo[2] - step];
    let idx = |x: usize, y: usize, z: usize| (z * n[1] + y) * n[0] + x;
    let pos = |x: usize, y: usize, z: usize| [origin[0] + x as f64 * step, origin[1] + y as f64 * step, origin[2] + z as f64 * step];
    let mut f = vec![0.0; n[0] * n[1] * n[2]];
    for z in 0..n[2] {
        for y in 0..n[1] {
            for x in 0..n[0] {
                f[idx(x, y, z)] = sdf(pos(x, y, z));
            }
        }
    }
    const CORNER: [[usize; 3]; 8] = [[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0], [0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]];
    const TETS: [[usize; 4]; 6] = [[0, 1, 2, 6], [0, 2, 3, 6], [0, 3, 7, 6], [0, 7, 4, 6], [0, 4, 5, 6], [0, 5, 1, 6]];
    let mut cache: HashMap<(usize, usize), u32> = HashMap::new();
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    for z in 0..n[2] - 1 {
        for y in 0..n[1] - 1 {
            for x in 0..n[0] - 1 {
                let c: Vec<(usize, [f64; 3], f64)> = CORNER
                    .iter()
                    .map(|o| {
                        let (cx, cy, cz) = (x + o[0], y + o[1], z + o[2]);
                        (idx(cx, cy, cz), pos(cx, cy, cz), f[idx(cx, cy, cz)])
                    })
                    .collect();
                if c.iter().all(|k| k.2 >= 0.0) || c.iter().all(|k| k.2 < 0.0) {
                    continue;
                }
                for tet in TETS {
                    let v: Vec<&(usize, [f64; 3], f64)> = tet.iter().map(|&i| &c[i]).collect();
                    let inside: Vec<usize> = (0..4).filter(|&i| v[i].2 < 0.0).collect();
                    let outside: Vec<usize> = (0..4).filter(|&i| v[i].2 >= 0.0).collect();
                    if inside.is_empty() || outside.is_empty() {
                        continue;
                    }
                    let mut edge = |a: usize, b: usize| -> u32 {
                        let key = (v[a].0.min(v[b].0), v[a].0.max(v[b].0));
                        *cache.entry(key).or_insert_with(|| {
                            let (fa, fb) = (v[a].2, v[b].2);
                            let t = if (fa - fb).abs() > 0.0 { (fa / (fa - fb)).clamp(0.0, 1.0) } else { 0.5 };
                            let p = [0, 1, 2].map(|k| v[a].1[k] + t * (v[b].1[k] - v[a].1[k]));
                            vertices.push(p);
                            (vertices.len() - 1) as u32
                        })
                    };
                    let mut tris: Vec<[u32; 3]> = Vec::new();
                    match (inside.len(), outside.len()) {
                        (1, 3) => tris.push([edge(inside[0], outside[0]), edge(inside[0], outside[1]), edge(inside[0], outside[2])]),
                        (3, 1) => tris.push([edge(outside[0], inside[0]), edge(outside[0], inside[1]), edge(outside[0], inside[2])]),
                        (2, 2) => {
                            let (a, b, c2, d) = (inside[0], inside[1], outside[0], outside[1]);
                            let (e_ac, e_ad, e_bd, e_bc) = (edge(a, c2), edge(a, d), edge(b, d), edge(b, c2));
                            tris.push([e_ac, e_ad, e_bd]);
                            tris.push([e_ac, e_bd, e_bc]);
                        }
                        _ => {}
                    }
                    // Face outward: away from the inside corners of this tetrahedron.
                    let mid = |ids: &[usize]| {
                        let mut m = [0.0; 3];
                        for &i in ids {
                            for (k, slot) in m.iter_mut().enumerate() {
                                *slot += v[i].1[k] / ids.len() as f64;
                            }
                        }
                        m
                    };
                    let (mi, mo) = (mid(&inside), mid(&outside));
                    for mut t in tris {
                        if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
                            continue;
                        }
                        let (a, b, c3) = (vertices[t[0] as usize], vertices[t[1] as usize], vertices[t[2] as usize]);
                        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                        let w = [c3[0] - a[0], c3[1] - a[1], c3[2] - a[2]];
                        let nrm = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
                        let d = [mo[0] - mi[0], mo[1] - mi[1], mo[2] - mi[2]];
                        if nrm[0] * d[0] + nrm[1] * d[1] + nrm[2] * d[2] < 0.0 {
                            t.swap(1, 2);
                        }
                        triangles.push(t);
                    }
                }
            }
        }
    }
    Mesh { vertices, triangles }
}

fn dist_segment(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if len2 > 0.0 { ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let q = [a[0] + t * ab[0] - p[0], a[1] + t * ab[1] - p[1], a[2] + t * ab[2] - p[2]];
    (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt()
}

/// A rounded stick from `a` to `b` of the given radius, as a distance function piece (negative inside).
pub fn stick(p: [f64; 3], a: [f64; 3], b: [f64; 3], radius: f64) -> f64 {
    dist_segment(p, a, b) - radius
}

/// Builds the mesh of a union of sticks (`a`, `b`, radius); a ball is a stick with `a == b`.
pub fn sticks(parts: &[([f64; 3], [f64; 3], f64)], cells: usize) -> Mesh {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for (a, b, r) in parts {
        for k in 0..3 {
            lo[k] = lo[k].min(a[k].min(b[k]) - r);
            hi[k] = hi[k].max(a[k].max(b[k]) + r);
        }
    }
    let parts = parts.to_vec();
    implicit(&move |p| parts.iter().map(|(a, b, r)| stick(p, *a, *b, *r)).fold(f64::INFINITY, f64::min), lo, hi, cells)
}

/// Names of the built-in sample models.
pub const SAMPLES: [&str; 6] = ["sphere", "capsule", "cone", "snowman", "limbs", "bear"];

/// A built-in sample model by name (sizes are arbitrary; set the finished height with `Mesh::fitted`).
pub fn sample(name: &str) -> Option<Mesh> {
    Some(match name {
        "sphere" => sphere(1.0),
        "capsule" => capsule(1.0, 1.5),
        "cone" => cone(1.0, 2.0),
        "snowman" => snowman(),
        "limbs" => limb_figure(),
        "bear" => bear(),
        _ => return None,
    })
}

/// A simple round bear, original and made from code: a ball head, a capsule body, two arms, two legs and two ears
/// (one closed surface). It stands about 3.4 units tall.
pub fn bear() -> Mesh {
    sticks(
        &[
            // body
            ([0.0, 0.0, 1.0], [0.0, 0.0, 2.0], 0.6),
            // head: a ball above the body with a neck between
            ([0.0, 0.0, 3.1], [0.0, 0.0, 3.1], 0.68),
            // ears: balls on top of the head, mostly above it
            ([0.5, 0.0, 3.7], [0.5, 0.0, 3.7], 0.32),
            ([-0.5, 0.0, 3.7], [-0.5, 0.0, 3.7], 0.32),
            // arms, slanting down and out from the shoulders
            ([0.78, 0.0, 1.9], [1.4, 0.0, 1.1], 0.24),
            ([-0.78, 0.0, 1.9], [-1.4, 0.0, 1.1], 0.24),
            // legs, slanting down and out from the hips
            ([0.3, 0.0, 0.9], [0.8, 0.0, -0.5], 0.26),
            ([-0.3, 0.0, 0.9], [-0.8, 0.0, -0.5], 0.26),
        ],
        72,
    )
}

/// A simple figure for tests and demos: a capsule body with four limbs, two legs below and two arms above,
/// each limb slanting outwards (the same shape on both sides).
pub fn limb_figure() -> Mesh {
    sticks(
        &[
            ([0.0, 0.0, 1.0], [0.0, 0.0, 2.0], 0.55),
            ([0.3, 0.0, 0.9], [0.9, 0.0, -0.5], 0.22),
            ([-0.3, 0.0, 0.9], [-0.9, 0.0, -0.5], 0.22),
            ([0.4, 0.0, 2.0], [1.2, 0.0, 2.9], 0.2),
            ([-0.4, 0.0, 2.0], [-1.2, 0.0, 2.9], 0.2),
        ],
        64,
    )
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
    fn a_union_of_sticks_is_one_closed_outward_surface() {
        let m = limb_figure();
        let (c, r) = m.checked().expect("closed");
        assert_eq!((r.bodies, r.holes_closed, r.flipped_normals, r.triangles_turned), (1, 0, false, 0), "{r:?}");
        assert!(c.signed_volume() > 1.0);
        let balls = sticks(&[([0.0, 0.0, 0.0], [0.0; 3], 1.0), ([0.0, 0.0, 1.5], [0.0, 0.0, 1.5], 0.7)], 40);
        let (_, r) = balls.checked().unwrap();
        assert_eq!(r.bodies, 1);
    }

    #[test]
    fn snowman_is_one_piece_with_a_narrow_neck() {
        let m = snowman();
        let (lo, hi) = m.bounds();
        assert!((hi[2] - lo[2] - (1.0 + 1.5 + 0.65)).abs() < 0.02);
        assert_eq!(m.bodies(), 1);
    }
}
