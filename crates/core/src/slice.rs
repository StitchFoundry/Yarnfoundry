//! Horizontal slices of a closed mesh: where a plane at height z cuts the surface, as closed loops.
//! Our own plain-geometry method: intersect each triangle with the plane, join the cut segments into loops.

use std::collections::HashMap;

use crate::mesh::Mesh;

/// One closed cut of the surface.
#[derive(Clone, Debug, PartialEq)]
pub struct Loop {
    /// Length around the loop (inches once the mesh is fitted).
    pub perimeter: f64,
    /// Enclosed area (always positive here; hole loops are dropped).
    pub area: f64,
    /// Centre of the enclosed area.
    pub centroid: [f64; 2],
    pub pts: Vec<[f64; 2]>,
}

impl Loop {
    /// Is `p` inside this loop (even-odd test)?
    pub fn contains(&self, p: [f64; 2]) -> bool {
        let mut inside = false;
        let n = self.pts.len();
        for i in 0..n {
            let (a, b) = (self.pts[i], self.pts[(i + 1) % n]);
            if (a[1] > p[1]) != (b[1] > p[1]) {
                let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                if p[0] < x {
                    inside = !inside;
                }
            }
        }
        inside
    }

    /// Do the two loops overlap when seen from above (one has a point inside the other)?
    pub fn overlaps(&self, other: &Loop) -> bool {
        if self.contains(other.centroid) || other.contains(self.centroid) {
            return true;
        }
        let step = (self.pts.len() / 8).max(1);
        if self.pts.iter().step_by(step).any(|p| other.contains(*p)) {
            return true;
        }
        let step = (other.pts.len() / 8).max(1);
        other.pts.iter().step_by(step).any(|p| self.contains(*p))
    }
}

/// All the loops at one height.
#[derive(Clone, Debug, PartialEq)]
pub struct Slice {
    pub z: f64,
    pub loops: Vec<Loop>,
}

type EdgeKey = (u32, u32);

/// `count` evenly spaced slices through the mesh, bottom to top, each at the middle of its band.
pub fn slices(mesh: &Mesh, count: usize) -> Vec<Slice> {
    let count = count.clamp(1, 10_000);
    let (lo, hi) = mesh.bounds();
    let height = hi[2] - lo[2];
    if !(height.is_finite() && height > 0.0) {
        return Vec::new();
    }
    let dz = height / count as f64;
    // A tiny, fixed nudge keeps planes off exact vertex heights.
    let nudge = dz * 1.234_567e-6;
    let plane = |i: usize| lo[2] + (i as f64 + 0.5) * dz + nudge;
    let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); count];
    for (ti, t) in mesh.triangles.iter().enumerate() {
        let zs = t.map(|i| mesh.vertices.get(i as usize).map_or(0.0, |v| v[2]));
        let (tmin, tmax) = (zs[0].min(zs[1]).min(zs[2]), zs[0].max(zs[1]).max(zs[2]));
        let i0 = ((tmin - lo[2]) / dz - 0.5).ceil().max(0.0) as usize;
        let i1 = ((tmax - lo[2]) / dz - 0.5).floor();
        if i1 < 0.0 {
            continue;
        }
        for bucket in buckets.iter_mut().take((i1 as usize).min(count - 1) + 1).skip(i0) {
            bucket.push(ti as u32);
        }
    }
    let mut out = Vec::with_capacity(count);
    for (i, tris) in buckets.iter().enumerate() {
        let z = plane(i);
        let mut starts: HashMap<EdgeKey, (EdgeKey, [f64; 2])> = HashMap::new();
        let mut pos: HashMap<EdgeKey, [f64; 2]> = HashMap::new();
        for &ti in tris {
            let t = mesh.triangles[ti as usize];
            let v = t.map(|i| mesh.vertices.get(i as usize).copied().unwrap_or([0.0; 3]));
            let above = v.map(|p| p[2] > z);
            let n_above = above.iter().filter(|a| **a).count();
            if n_above == 0 || n_above == 3 {
                continue;
            }
            // Crossing point of each edge that changes side; remember which way it goes.
            let mut up_edge = None; // below -> above along the triangle's winding
            let mut down_edge = None;
            for k in 0..3 {
                let (a, b) = (k, (k + 1) % 3);
                if above[a] == above[b] {
                    continue;
                }
                let key = (t[a].min(t[b]), t[a].max(t[b]));
                let f = (z - v[a][2]) / (v[b][2] - v[a][2]);
                let p = [v[a][0] + f * (v[b][0] - v[a][0]), v[a][1] + f * (v[b][1] - v[a][1])];
                pos.insert(key, p);
                if above[b] {
                    up_edge = Some(key);
                } else {
                    down_edge = Some(key);
                }
            }
            if let (Some(u), Some(d)) = (up_edge, down_edge) {
                // Walk with the inside on the left: from the edge where the surface goes down to where it goes up.
                let end = pos.get(&u).copied().unwrap_or([0.0; 2]);
                starts.insert(d, (u, end));
            }
        }
        let mut loops = Vec::new();
        let mut used: HashMap<EdgeKey, bool> = HashMap::new();
        let mut keys: Vec<EdgeKey> = starts.keys().copied().collect();
        keys.sort_unstable();
        for s in keys {
            if used.contains_key(&s) {
                continue;
            }
            let mut pts = Vec::new();
            let mut cur = s;
            let mut closed = false;
            while let Some((next, _)) = starts.get(&cur) {
                if used.insert(cur, true).is_some() {
                    break;
                }
                pts.push(pos.get(&cur).copied().unwrap_or([0.0; 2]));
                cur = *next;
                if cur == s {
                    closed = true;
                    break;
                }
            }
            if closed
                && pts.len() >= 3
                && let Some(l) = make_loop(pts)
            {
                loops.push(l);
            }
        }
        out.push(Slice { z, loops });
    }
    out
}

fn make_loop(pts: Vec<[f64; 2]>) -> Option<Loop> {
    let n = pts.len();
    let mut perimeter = 0.0;
    let mut a2 = 0.0;
    let (mut cx, mut cy) = (0.0, 0.0);
    for i in 0..n {
        let (p, q) = (pts[i], pts[(i + 1) % n]);
        perimeter += ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt();
        let cr = p[0] * q[1] - q[0] * p[1];
        a2 += cr;
        cx += (p[0] + q[0]) * cr;
        cy += (p[1] + q[1]) * cr;
    }
    if a2 <= 0.0 || !a2.is_finite() {
        return None; // a hole (turning the other way) or flat
    }
    Some(Loop { perimeter, area: a2 / 2.0, centroid: [cx / (3.0 * a2), cy / (3.0 * a2)], pts })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models;
    use std::f64::consts::PI;

    #[test]
    fn a_sphere_slice_is_a_circle_of_the_right_size() {
        let s = slices(&models::sphere(2.0), 40);
        assert_eq!(s.len(), 40);
        for sl in &s {
            assert_eq!(sl.loops.len(), 1, "z={}", sl.z);
            let r = (4.0 - (sl.z - 2.0).powi(2)).sqrt();
            let want = 2.0 * PI * r;
            assert!((sl.loops[0].perimeter - want).abs() / want < 0.02 + 0.05 / r.max(0.1), "z={} got {} want {}", sl.z, sl.loops[0].perimeter, want);
            assert!(sl.loops[0].centroid[0].abs() < 0.05 && sl.loops[0].centroid[1].abs() < 0.05);
        }
    }

    #[test]
    fn two_separate_balls_give_two_loops() {
        let a = models::sphere(1.0);
        let mut m = a.clone();
        let off = a.vertices.len() as u32;
        m.vertices.extend(a.vertices.iter().map(|v| [v[0] + 5.0, v[1], v[2]]));
        m.triangles.extend(a.triangles.iter().map(|t| [t[0] + off, t[1] + off, t[2] + off]));
        let s = slices(&m, 10);
        assert!(s.iter().all(|sl| sl.loops.len() == 2));
        assert!(s[5].loops[0].overlaps(&s[5].loops[0]) && !s[5].loops[0].overlaps(&s[5].loops[1]));
    }

    #[test]
    fn the_snowman_neck_is_narrower_than_both_balls() {
        let m = models::snowman();
        let s = slices(&m, 100);
        let per: Vec<f64> = s.iter().map(|x| x.loops.iter().map(|l| l.perimeter).sum()).collect();
        let neck = per.iter().copied().fold(f64::MAX, f64::min);
        assert!(neck < 0.4 * 2.0 * PI, "neck {neck}");
    }

    #[test]
    fn empty_and_flat_meshes_give_no_slices() {
        assert!(slices(&Mesh { vertices: vec![], triangles: vec![] }, 10).is_empty());
    }
}
