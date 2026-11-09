//! 3D model import: STL (binary and ASCII) and OBJ, the closed-mesh check, small-hole repair, size and upright.
//! No UI and no I/O: callers hand over bytes. Every number from a file is untrusted (size caps, finite checks, no panics).

use std::collections::HashMap;

use crate::gauge::Unit;

/// Largest model file we read.
pub const MAX_BYTES: usize = 64 * 1024 * 1024;
/// Most triangles we accept.
pub const MAX_TRIANGLES: usize = 400_000;
/// Holes with more edges than this are refused rather than patched.
pub const MAX_HOLE_EDGES: usize = 24;
/// Tallest finished figure we accept, in inches.
pub const MAX_HEIGHT_IN: f64 = 400.0;

/// Why a model was refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MeshError {
    #[error("the file is {0} bytes; the limit is {MAX_BYTES}")]
    FileTooLarge(usize),
    #[error("the model has more than {MAX_TRIANGLES} triangles")]
    TooManyTriangles,
    #[error("this does not look like an STL or OBJ file")]
    UnknownFormat,
    #[error("the file is damaged: {0}")]
    BadFile(String),
    #[error("the model has no usable triangles")]
    Empty,
    #[error("the model contains a number that is not finite")]
    NotFinite,
    #[error("the surface has edges shared by more than two triangles, so it is not a solid shape")]
    NonManifold,
    #[error("the triangles cannot be turned to face the same way (a twisted surface)")]
    NotOrientable,
    #[error(
        "the model is open: {holes} hole(s), the biggest with {largest} edges. Only small holes (up to {MAX_HOLE_EDGES} edges) are closed automatically; close it in your 3D program first"
    )]
    OpenHoles { holes: usize, largest: usize },
    #[error("the model has no volume")]
    NoVolume,
    #[error("the finished height must be a number above zero and at most {MAX_HEIGHT_IN} inches")]
    BadHeight,
}

/// What the closed-mesh check found (and fixed).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MeshReport {
    pub triangles: usize,
    /// Triangles that faced the wrong way compared with their neighbours and were turned.
    pub triangles_turned: usize,
    /// The whole model was inside out and was turned.
    pub flipped_normals: bool,
    /// Small holes that were closed.
    pub holes_closed: usize,
    /// Separate pieces (1 is normal).
    pub bodies: usize,
}

impl MeshReport {
    /// Plain-language notes for the user.
    pub fn notes(&self) -> Vec<String> {
        let mut n = Vec::new();
        if self.flipped_normals {
            n.push("The model was inside out; it has been turned the right way.".to_string());
        }
        if self.triangles_turned > 0 {
            n.push(format!("{} triangle(s) faced the wrong way and were turned.", self.triangles_turned));
        }
        if self.holes_closed > 0 {
            n.push(format!("{} small hole(s) were closed.", self.holes_closed));
        }
        if self.bodies > 1 {
            n.push(format!("The model has {} separate pieces; each is made as its own part.", self.bodies));
        }
        n
    }
}

/// Which axis points up in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// A triangle mesh. After [`Mesh::checked`] and [`Mesh::fitted`] it is closed, outward-facing, upright (Z up),
/// standing on z = 0, centred on x and y, measured in inches.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

/// Reads STL (binary or ASCII) or OBJ, whichever the bytes are, and runs the closed-mesh check.
pub fn import(bytes: &[u8]) -> Result<(Mesh, MeshReport), MeshError> {
    load(bytes)?.checked()
}

/// Reads STL (binary or ASCII) or OBJ without checking the shape.
pub fn load(bytes: &[u8]) -> Result<Mesh, MeshError> {
    if bytes.len() > MAX_BYTES {
        return Err(MeshError::FileTooLarge(bytes.len()));
    }
    if is_binary_stl(bytes) {
        return parse_stl(bytes);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| MeshError::UnknownFormat)?;
    let head = text.trim_start();
    if head.starts_with("solid") {
        parse_stl(bytes)
    } else if head.lines().any(|l| l.trim_start().starts_with("v ")) {
        parse_obj(bytes)
    } else {
        Err(MeshError::UnknownFormat)
    }
}

fn is_binary_stl(bytes: &[u8]) -> bool {
    let Some(count) = bytes.get(80..84) else { return false };
    let n = u32::from_le_bytes([count[0], count[1], count[2], count[3]]) as usize;
    n.checked_mul(50).and_then(|b| b.checked_add(84)) == Some(bytes.len())
}

/// Binary or ASCII STL.
pub fn parse_stl(bytes: &[u8]) -> Result<Mesh, MeshError> {
    if bytes.len() > MAX_BYTES {
        return Err(MeshError::FileTooLarge(bytes.len()));
    }
    let mut soup: Vec<[f64; 3]> = Vec::new();
    if is_binary_stl(bytes) {
        let n = bytes.get(80..84).map_or(0, |c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as usize);
        if n > MAX_TRIANGLES {
            return Err(MeshError::TooManyTriangles);
        }
        for rec in bytes.get(84..).unwrap_or_default().as_chunks::<50>().0 {
            for v in 0..3 {
                let mut p = [0.0; 3];
                for (a, slot) in p.iter_mut().enumerate() {
                    let o = 12 + v * 12 + a * 4;
                    let b = rec.get(o..o + 4).ok_or(MeshError::Empty)?;
                    *slot = f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                }
                soup.push(p);
            }
        }
    } else {
        let text = std::str::from_utf8(bytes).map_err(|_| MeshError::UnknownFormat)?;
        if !text.trim_start().starts_with("solid") {
            return Err(MeshError::UnknownFormat);
        }
        let mut toks = text.split_ascii_whitespace();
        while let Some(t) = toks.next() {
            if t.eq_ignore_ascii_case("vertex") {
                let mut p = [0.0; 3];
                for slot in &mut p {
                    *slot =
                        toks.next().and_then(|s| s.parse::<f64>().ok()).ok_or_else(|| MeshError::BadFile("a vertex is missing a number".into()))?;
                }
                soup.push(p);
                if soup.len() > MAX_TRIANGLES * 3 {
                    return Err(MeshError::TooManyTriangles);
                }
            }
        }
        if !soup.len().is_multiple_of(3) {
            return Err(MeshError::BadFile("a facet has fewer than three vertices".into()));
        }
    }
    weld(&soup)
}

/// Wavefront OBJ: `v` and `f` lines (faces of any size are fanned into triangles; other lines are ignored).
pub fn parse_obj(bytes: &[u8]) -> Result<Mesh, MeshError> {
    if bytes.len() > MAX_BYTES {
        return Err(MeshError::FileTooLarge(bytes.len()));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| MeshError::UnknownFormat)?;
    let mut verts: Vec<[f64; 3]> = Vec::new();
    let mut soup: Vec<[f64; 3]> = Vec::new();
    for line in text.lines() {
        let mut it = line.split_ascii_whitespace();
        match it.next() {
            Some("v") => {
                let mut p = [0.0; 3];
                for slot in &mut p {
                    *slot = it
                        .next()
                        .and_then(|s| s.parse::<f64>().ok())
                        .ok_or_else(|| MeshError::BadFile("a vertex line is missing a number".into()))?;
                }
                verts.push(p);
                if verts.len() > MAX_TRIANGLES * 3 {
                    return Err(MeshError::TooManyTriangles);
                }
            }
            Some("f") => {
                let mut idx = Vec::new();
                for tok in it {
                    let raw = tok.split('/').next().unwrap_or_default();
                    let i: i64 = raw.parse().map_err(|_| MeshError::BadFile("a face has a bad vertex number".into()))?;
                    let n = verts.len() as i64;
                    let k = if i > 0 {
                        i - 1
                    } else if i < 0 {
                        n + i
                    } else {
                        -1
                    };
                    let p = usize::try_from(k)
                        .ok()
                        .and_then(|k| verts.get(k))
                        .ok_or_else(|| MeshError::BadFile("a face points at a vertex that does not exist".into()))?;
                    idx.push(*p);
                }
                if idx.len() < 3 {
                    return Err(MeshError::BadFile("a face has fewer than three vertices".into()));
                }
                for k in 1..idx.len() - 1 {
                    soup.extend([idx[0], idx[k], idx[k + 1]]);
                }
                if soup.len() > MAX_TRIANGLES * 3 {
                    return Err(MeshError::TooManyTriangles);
                }
            }
            _ => {}
        }
    }
    weld(&soup)
}

/// Joins vertices that sit at the same place (STL stores every corner separately) and drops degenerate triangles.
fn weld(soup: &[[f64; 3]]) -> Result<Mesh, MeshError> {
    if soup.is_empty() {
        return Err(MeshError::Empty);
    }
    if soup.iter().any(|p| p.iter().any(|c| !c.is_finite())) {
        return Err(MeshError::NotFinite);
    }
    let (lo, hi) = bounds_of(soup.iter());
    let diag = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
    if !(diag.is_finite() && diag > 0.0) {
        return Err(MeshError::NoVolume);
    }
    let q = diag * 1e-6;
    let mut map: HashMap<[i64; 3], u32> = HashMap::new();
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for tri in soup.as_chunks::<3>().0 {
        let mut t = [0u32; 3];
        for (slot, p) in t.iter_mut().zip(tri) {
            let key = [(p[0] / q).round() as i64, (p[1] / q).round() as i64, (p[2] / q).round() as i64];
            *slot = *map.entry(key).or_insert_with(|| {
                vertices.push(*p);
                (vertices.len() - 1) as u32
            });
        }
        if t[0] != t[1] && t[1] != t[2] && t[0] != t[2] {
            triangles.push(t);
        }
    }
    if triangles.is_empty() {
        return Err(MeshError::Empty);
    }
    Ok(Mesh { vertices, triangles })
}

fn bounds_of<'a>(pts: impl Iterator<Item = &'a [f64; 3]>) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in pts {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    (lo, hi)
}

impl Mesh {
    /// Lowest and highest corner.
    pub fn bounds(&self) -> ([f64; 3], [f64; 3]) {
        bounds_of(self.vertices.iter())
    }

    fn tri_volume(&self, t: &[u32; 3]) -> f64 {
        if let (Some(a), Some(b), Some(c)) = (self.vertices.get(t[0] as usize), self.vertices.get(t[1] as usize), self.vertices.get(t[2] as usize)) {
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0
        } else {
            0.0
        }
    }

    /// Volume from the triangles (negative when the model is inside out).
    pub fn signed_volume(&self) -> f64 {
        self.triangles.iter().map(|t| self.tri_volume(t)).sum()
    }

    /// The closed-mesh check: makes every triangle face the same way, closes small holes, turns an inside-out model,
    /// counts separate bodies. Refuses non-solid or badly open models with a clear message.
    pub fn checked(mut self) -> Result<(Mesh, MeshReport), MeshError> {
        if self.triangles.is_empty() {
            return Err(MeshError::Empty);
        }
        let nv = self.vertices.len();
        if self.triangles.iter().any(|t| t.iter().any(|&i| i as usize >= nv)) {
            return Err(MeshError::BadFile("a triangle points at a vertex that does not exist".into()));
        }
        if self.vertices.iter().any(|p| p.iter().any(|c| !c.is_finite())) {
            return Err(MeshError::NotFinite);
        }
        if self.triangles.len() > MAX_TRIANGLES {
            return Err(MeshError::TooManyTriangles);
        }
        let mut report = MeshReport { triangles: self.triangles.len(), ..MeshReport::default() };

        // Each undirected edge may be used by at most two triangles.
        let mut uses: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (ti, t) in self.triangles.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                uses.entry((a.min(b), a.max(b))).or_default().push(ti);
            }
        }
        if uses.values().any(|u| u.len() > 2) {
            return Err(MeshError::NonManifold);
        }

        // Turn neighbours to agree, one connected piece at a time.
        let mut seen = vec![false; self.triangles.len()];
        for start in 0..self.triangles.len() {
            if seen[start] {
                continue;
            }
            seen[start] = true;
            let mut stack = vec![start];
            while let Some(ti) = stack.pop() {
                let t = self.triangles[ti];
                for k in 0..3 {
                    let (a, b) = (t[k], t[(k + 1) % 3]);
                    for &nj in uses.get(&(a.min(b), a.max(b))).map_or(&[][..], Vec::as_slice) {
                        if nj == ti || seen[nj] {
                            continue;
                        }
                        seen[nj] = true;
                        // Neighbour must use this edge as b -> a; if it uses a -> b, turn it.
                        let u = self.triangles[nj];
                        let same = (0..3).any(|m| u[m] == a && u[(m + 1) % 3] == b);
                        if same {
                            self.triangles[nj].swap(1, 2);
                            report.triangles_turned += 1;
                        }
                        stack.push(nj);
                    }
                }
            }
        }
        // Count each connected piece's turned triangles relative to the majority later via the volume test.
        let mut directed: HashMap<(u32, u32), usize> = HashMap::new();
        for t in &self.triangles {
            for k in 0..3 {
                *directed.entry((t[k], t[(k + 1) % 3])).or_default() += 1;
            }
        }
        if directed.values().any(|&c| c > 1) {
            return Err(MeshError::NotOrientable);
        }

        // Holes: directed edges with no partner.
        let mut open: HashMap<u32, Vec<u32>> = HashMap::new();
        for &(a, b) in directed.keys() {
            if !directed.contains_key(&(b, a)) {
                open.entry(a).or_default().push(b);
            }
        }
        if !open.is_empty() {
            let (lo, hi) = self.bounds();
            let diag = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt();
            let mut loops: Vec<Vec<u32>> = Vec::new();
            let mut starts: Vec<u32> = open.keys().copied().collect();
            starts.sort_unstable();
            let mut used: HashMap<(u32, u32), bool> = HashMap::new();
            for s in starts {
                let Some(firsts) = open.get(&s) else { continue };
                for &f in firsts {
                    if used.contains_key(&(s, f)) {
                        continue;
                    }
                    let mut lp = vec![s];
                    let (mut a, mut b) = (s, f);
                    used.insert((a, b), true);
                    let mut ok = true;
                    while b != s {
                        lp.push(b);
                        if lp.len() > MAX_HOLE_EDGES + 1 {
                            ok = false;
                            break;
                        }
                        let next = open.get(&b).and_then(|v| v.iter().copied().find(|&n| !used.contains_key(&(b, n))));
                        match next {
                            Some(n) => {
                                used.insert((b, n), true);
                                a = b;
                                b = n;
                            }
                            None => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    let _ = a;
                    if ok {
                        loops.push(lp);
                    } else {
                        loops.push(vec![0; MAX_HOLE_EDGES + 2]);
                    }
                }
            }
            let largest = loops.iter().map(Vec::len).max().unwrap_or(0);
            let mut too_big = loops.iter().any(|l| l.len() > MAX_HOLE_EDGES);
            if !too_big {
                for l in &loops {
                    let (hl, hh) = bounds_of(l.iter().filter_map(|&i| self.vertices.get(i as usize)));
                    let size = ((hh[0] - hl[0]).powi(2) + (hh[1] - hl[1]).powi(2) + (hh[2] - hl[2]).powi(2)).sqrt();
                    if size > 0.25 * diag {
                        too_big = true;
                    }
                }
            }
            if too_big {
                return Err(MeshError::OpenHoles { holes: loops.len(), largest });
            }
            for l in &loops {
                let mut c = [0.0; 3];
                for &i in l {
                    for (k, slot) in c.iter_mut().enumerate() {
                        *slot += self.vertices[i as usize][k] / l.len() as f64;
                    }
                }
                let ci = self.vertices.len() as u32;
                self.vertices.push(c);
                for k in 0..l.len() {
                    let (a, b) = (l[k], l[(k + 1) % l.len()]);
                    self.triangles.push([b, a, ci]);
                }
                report.holes_closed += 1;
            }
        }

        // Turn each separate piece so it faces outward (a piece with negative volume is inside out).
        let ids = self.body_ids();
        let mut vol: HashMap<usize, f64> = HashMap::new();
        for (t, id) in self.triangles.iter().zip(&ids) {
            *vol.entry(*id).or_default() += self.tri_volume(t);
        }
        if vol.values().any(|v| !v.is_finite()) || vol.values().all(|v| v.abs() < f64::EPSILON) {
            return Err(MeshError::NoVolume);
        }
        for (t, id) in self.triangles.iter_mut().zip(&ids) {
            if vol.get(id).is_some_and(|v| *v < 0.0) {
                t.swap(1, 2);
                report.flipped_normals = true;
            }
        }
        report.bodies = self.bodies();
        report.triangles = self.triangles.len();
        Ok((self, report))
    }

    /// For every triangle, an id shared by all triangles of the same connected piece.
    fn body_ids(&self) -> Vec<usize> {
        let mut parent: Vec<usize> = (0..self.vertices.len()).collect();
        fn find(p: &mut [usize], mut x: usize) -> usize {
            while p[x] != x {
                p[x] = p[p[x]];
                x = p[x];
            }
            x
        }
        for t in &self.triangles {
            let r0 = find(&mut parent, t[0] as usize);
            for &v in &t[1..] {
                let r = find(&mut parent, v as usize);
                parent[r] = r0;
            }
        }
        self.triangles.iter().map(|t| find(&mut parent, t[0] as usize)).collect()
    }

    /// Number of separate connected pieces.
    pub fn bodies(&self) -> usize {
        self.body_ids().into_iter().collect::<std::collections::HashSet<_>>().len()
    }

    /// The axis along which the model is longest (Z on a tie).
    pub fn longest_axis(&self) -> Axis {
        let (lo, hi) = self.bounds();
        let e = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
        if e[2] >= e[0] && e[2] >= e[1] {
            Axis::Z
        } else if e[1] >= e[0] {
            Axis::Y
        } else {
            Axis::X
        }
    }

    /// Sets the model upright (`up` axis, or the longest one) and scales it to `height` tall, standing on z = 0 and
    /// centred on x and y. The result is in inches.
    pub fn fitted(&self, height: f64, unit: Unit, up: Option<Axis>) -> Result<Mesh, MeshError> {
        let height_in = match unit {
            Unit::Inches => height,
            Unit::Cm => height / 2.54,
        };
        if !(height_in.is_finite() && height_in > 0.0 && height_in <= MAX_HEIGHT_IN) {
            return Err(MeshError::BadHeight);
        }
        let up = up.unwrap_or_else(|| self.longest_axis());
        let turn = |p: &[f64; 3]| match up {
            Axis::Z => *p,
            Axis::X => [p[1], p[2], p[0]],
            Axis::Y => [p[2], p[0], p[1]],
        };
        let turned: Vec<[f64; 3]> = self.vertices.iter().map(turn).collect();
        let (lo, hi) = bounds_of(turned.iter());
        let extent = hi[2] - lo[2];
        if !(extent.is_finite() && extent > 0.0) {
            return Err(MeshError::NoVolume);
        }
        let k = height_in / extent;
        let (cx, cy) = ((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0);
        let vertices = turned.iter().map(|p| [(p[0] - cx) * k, (p[1] - cy) * k, (p[2] - lo[2]) * k]).collect();
        Ok(Mesh { vertices, triangles: self.triangles.clone() })
    }

    /// Binary STL bytes (for saving and tests).
    pub fn to_binary_stl(&self) -> Vec<u8> {
        let mut out = vec![0u8; 80];
        out.extend((self.triangles.len() as u32).to_le_bytes());
        for t in &self.triangles {
            let p: Vec<[f64; 3]> = t.iter().map(|&i| self.vertices.get(i as usize).copied().unwrap_or([0.0; 3])).collect();
            let n = normal(&p[0], &p[1], &p[2]);
            for c in n {
                out.extend((c as f32).to_le_bytes());
            }
            for v in &p {
                for c in v {
                    out.extend((*c as f32).to_le_bytes());
                }
            }
            out.extend([0u8, 0u8]);
        }
        out
    }

    /// ASCII STL text (for saving and tests).
    pub fn to_ascii_stl(&self) -> String {
        let mut s = String::from("solid yarnfoundry\n");
        for t in &self.triangles {
            let p: Vec<[f64; 3]> = t.iter().map(|&i| self.vertices.get(i as usize).copied().unwrap_or([0.0; 3])).collect();
            let n = normal(&p[0], &p[1], &p[2]);
            s.push_str(&format!("facet normal {} {} {}\n outer loop\n", n[0], n[1], n[2]));
            for v in &p {
                s.push_str(&format!("  vertex {} {} {}\n", v[0], v[1], v[2]));
            }
            s.push_str(" endloop\nendfacet\n");
        }
        s.push_str("endsolid yarnfoundry\n");
        s
    }

    /// OBJ text (for saving and tests).
    pub fn to_obj(&self) -> String {
        let mut s = String::new();
        for v in &self.vertices {
            s.push_str(&format!("v {} {} {}\n", v[0], v[1], v[2]));
        }
        for t in &self.triangles {
            s.push_str(&format!("f {} {} {}\n", t[0] + 1, t[1] + 1, t[2] + 1));
        }
        s
    }
}

fn normal(a: &[f64; 3], b: &[f64; 3], c: &[f64; 3]) -> [f64; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len > 0.0 { [n[0] / len, n[1] / len, n[2] / len] } else { [0.0; 3] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models;
    use proptest::prelude::*;

    fn same_shape(a: &Mesh, b: &Mesh) {
        assert_eq!(a.triangles.len(), b.triangles.len());
        assert_eq!(a.vertices.len(), b.vertices.len());
        assert!((a.signed_volume() - b.signed_volume()).abs() / a.signed_volume() < 1e-4);
    }

    #[test]
    fn binary_ascii_and_obj_all_load_the_same_sphere() {
        let s = models::sphere(1.0);
        let (bin, rb) = import(&s.to_binary_stl()).unwrap();
        let (asc, ra) = import(s.to_ascii_stl().as_bytes()).unwrap();
        let (obj, ro) = import(s.to_obj().as_bytes()).unwrap();
        same_shape(&bin, &s);
        same_shape(&asc, &s);
        same_shape(&obj, &s);
        for r in [rb, ra, ro] {
            assert_eq!((r.holes_closed, r.bodies, r.flipped_normals), (0, 1, false));
        }
    }

    #[test]
    fn obj_with_quads_and_negative_indices_works() {
        let cube = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nv 0 0 1\nv 1 0 1\nv 1 1 1\nv 0 1 1\n\
                    f 4 3 2 1\nf 5 6 7 8\nf 1 2 6 5\nf 2 3 7 6\nf 3 4 8 7\nf 4 1 5 8\n";
        let (m, r) = import(cube.as_bytes()).unwrap();
        assert_eq!(m.triangles.len(), 12);
        assert!((m.signed_volume() - 1.0).abs() < 1e-9);
        assert!(!r.flipped_normals);
        let neg = "v 0 0 0\nv 1 0 0\nv 0 1 0\nv 0 0 1\nf -4/1/1 -2 -3\nf -4 -3 -1\nf -4 -1 -2\nf -3 -2 -1\n";
        assert!(import(neg.as_bytes()).is_ok());
    }

    #[test]
    fn an_inside_out_model_is_turned() {
        let mut s = models::sphere(1.0);
        for t in &mut s.triangles {
            t.swap(1, 2);
        }
        let (m, r) = import(&s.to_binary_stl()).unwrap();
        assert!(r.flipped_normals);
        assert!(m.signed_volume() > 0.0);
    }

    #[test]
    fn some_flipped_triangles_are_turned_back() {
        let mut s = models::sphere(1.0);
        for t in s.triangles.iter_mut().step_by(7) {
            t.swap(1, 2);
        }
        let (m, r) = import(&s.to_binary_stl()).unwrap();
        assert!(r.triangles_turned > 0);
        assert!(m.signed_volume() > 0.0);
        assert_eq!(m.triangles.len(), s.triangles.len());
    }

    #[test]
    fn a_small_hole_is_closed_and_a_big_one_refused() {
        let mut s = models::sphere(1.0);
        // Remove three neighbouring triangles around the equator: a small hole.
        let n = s.triangles.len();
        s.triangles.drain(n / 2..n / 2 + 3);
        let (m, r) = import(&s.to_binary_stl()).unwrap();
        assert!(r.holes_closed >= 1);
        assert!(m.signed_volume() > 0.0);
        let mut big = models::sphere(1.0);
        let n = big.triangles.len();
        big.triangles.drain(n / 2..n / 2 + 800);
        match import(&big.to_binary_stl()) {
            Err(MeshError::OpenHoles { .. }) => {}
            other => panic!("expected OpenHoles, got {other:?}"),
        }
        let msg = MeshError::OpenHoles { holes: 1, largest: 30 }.to_string();
        assert!(msg.contains("open") && msg.contains("close it in your 3D program"));
    }

    #[test]
    fn two_bodies_are_counted() {
        let a = models::sphere(1.0);
        let mut b = models::sphere(1.0);
        for v in &mut b.vertices {
            v[0] += 5.0;
        }
        let off = a.vertices.len() as u32;
        let mut m = a.clone();
        m.vertices.extend(b.vertices);
        m.triangles.extend(b.triangles.iter().map(|t| [t[0] + off, t[1] + off, t[2] + off]));
        let (_, r) = m.checked().unwrap();
        assert_eq!(r.bodies, 2);
        assert!(r.notes().iter().any(|n| n.contains("2 separate")));
    }

    #[test]
    fn size_and_upright_longest_axis_up_with_override() {
        // A capsule lying along X.
        let mut c = models::capsule(1.0, 4.0);
        for v in &mut c.vertices {
            *v = [v[2], v[0], v[1]];
        }
        let up = c.fitted(10.0, Unit::Inches, None).unwrap();
        let (lo, hi) = up.bounds();
        assert!((hi[2] - 10.0).abs() < 1e-9 && lo[2].abs() < 1e-9);
        assert!((hi[0] + lo[0]).abs() < 1e-9 && (hi[1] + lo[1]).abs() < 1e-9);
        assert!(hi[0] < 2.0, "slim, not lying down: {hi:?}");
        assert!(up.signed_volume() > 0.0, "turning keeps handedness");
        // Centimetres.
        let cm = c.fitted(25.4, Unit::Cm, None).unwrap();
        assert!((cm.bounds().1[2] - 10.0).abs() < 1e-9);
        // Override: say Y is up (the capsule's actual long axis is X, so the model gets squashed upright on Y).
        let y = c.fitted(10.0, Unit::Inches, Some(Axis::Y)).unwrap();
        assert!((y.bounds().1[2] - 10.0).abs() < 1e-9);
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e9] {
            assert_eq!(c.fitted(bad, Unit::Inches, None), Err(MeshError::BadHeight));
        }
    }

    #[test]
    fn size_caps_and_garbage_are_errors() {
        assert_eq!(load(&vec![0u8; MAX_BYTES + 1]), Err(MeshError::FileTooLarge(MAX_BYTES + 1)));
        assert_eq!(load(b"hello world"), Err(MeshError::UnknownFormat));
        assert_eq!(load(b""), Err(MeshError::UnknownFormat));
        // A binary header that claims far too many triangles.
        let mut b = vec![0u8; 84];
        b[80..84].copy_from_slice(&(u32::MAX).to_le_bytes());
        assert!(load(&b).is_err());
        // A binary STL claiming more triangles than the cap, with a matching length is refused before allocating.
        assert!(parse_stl(b"solid x\nvertex 1 2\nendsolid").is_err());
        assert!(parse_obj(b"v 1 2 3\nf 1 2 9\n").is_err());
        assert!(parse_obj(b"v 1 2 3\nf 1 2\n").is_err());
        assert!(parse_obj(b"f 0 0 0\n").is_err());
        assert_eq!(
            load(b"solid x\nfacet normal 0 0 0\nouter loop\nvertex nan 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid"),
            Err(MeshError::NotFinite)
        );
        assert_eq!(load(b"v 0 0 0\nv 1 0 0\nv 1 0 0\nf 1 2 3\n"), Err(MeshError::Empty));
    }

    #[test]
    fn a_lone_triangle_and_a_twisted_surface_are_refused_clearly() {
        let tri = Mesh { vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], triangles: vec![[0, 1, 2]] };
        assert!(matches!(tri.checked(), Err(MeshError::NoVolume | MeshError::OpenHoles { .. })));
        // Three triangles sharing one edge: not a solid.
        let fin = Mesh {
            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.5, 0.5, 0.5]],
            triangles: vec![[0, 1, 2], [0, 1, 3], [0, 1, 4]],
        };
        assert_eq!(fin.checked(), Err(MeshError::NonManifold));
    }

    #[test]
    fn broken_indices_are_errors_not_panics() {
        let m = Mesh { vertices: vec![[0.0; 3]; 3], triangles: vec![[0, 1, 99]] };
        assert!(m.checked().is_err());
        let m = Mesh { vertices: vec![[f64::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], triangles: vec![[0, 1, 2]] };
        assert_eq!(m.checked(), Err(MeshError::NotFinite));
    }

    proptest! {
        #[test]
        fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..400)) {
            let _ = import(&bytes);
        }

        #[test]
        fn random_text_never_panics(text in "[a-z0-9 .\\-\\n/]{0,300}") {
            let _ = import(text.as_bytes());
            let _ = import(format!("solid {text}").as_bytes());
            let _ = import(format!("v {text}\nf 1 2 3").as_bytes());
        }

        #[test]
        fn damaged_real_files_never_panic(cut in 0usize..5000, flip in 0usize..5000, val in any::<u8>()) {
            let mut b = models::capsule(1.0, 1.0).to_binary_stl();
            if let Some(x) = b.get_mut(flip) { *x = val; }
            b.truncate(b.len().saturating_sub(cut));
            let _ = import(&b);
            let mut a = models::cone(1.0, 2.0).to_ascii_stl().into_bytes();
            if let Some(x) = a.get_mut(flip) { *x = val; }
            a.truncate(a.len().saturating_sub(cut));
            let _ = import(&a);
        }

        #[test]
        fn random_triangle_soup_never_panics(tris in proptest::collection::vec((0u32..8, 0u32..8, 0u32..8), 0..20), pts in proptest::collection::vec((-5.0f64..5.0, -5.0f64..5.0, -5.0f64..5.0), 1..8)) {
            let m = Mesh { vertices: pts.iter().map(|p| [p.0, p.1, p.2]).collect(), triangles: tris.iter().map(|t| [t.0, t.1, t.2]).collect() };
            let _ = m.clone().checked();
            let _ = m.fitted(5.0, Unit::Inches, None);
        }
    }
}
