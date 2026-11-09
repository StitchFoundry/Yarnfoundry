//! A simple 3D view drawn as flat-shaded polygons on a [`Page`], so the PNG, SVG and PDF back ends can all show a model.
//! Painter's algorithm (far to near), one light, no textures. Used for the build-story pictures and the PDF cover.

use yarnfoundry_core::{Figure, Mesh, Round};

use crate::draw::{Page, Rgb};

/// A turntable camera: `yaw` turns around the vertical axis, `pitch` tilts down (both in radians).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub yaw: f64,
    pub pitch: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self { yaw: 0.6, pitch: 0.35 }
    }
}

type V3 = [f64; 3];
type Drawn = (f64, Vec<(f32, f32)>, Rgb);
type Tri = (f64, [(f32, f32); 3], Rgb);

impl Camera {
    /// Rotates a model point into view space: x right, y up, z towards the viewer.
    pub fn rotate(&self, p: V3) -> V3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let x1 = p[0] * cy - p[1] * sy;
        let y1 = p[0] * sy + p[1] * cy; // depth axis before tilt
        // Tilt: the camera sits above and in front, looking down by `pitch`.
        [x1, y1 * sp + p[2] * cp, -y1 * cp + p[2] * sp]
    }
}

/// Where the model sits on a page: a box and the scale that fits the whole model into it.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Scale (points per model unit) and view-space centre so a model of this radius fits the frame.
pub fn fit(mesh: &Mesh, cam: &Camera, frame: Frame) -> (f64, V3) {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for v in &mesh.vertices {
        let r = cam.rotate(*v);
        for a in 0..3 {
            lo[a] = lo[a].min(r[a]);
            hi[a] = hi[a].max(r[a]);
        }
    }
    let ew = (hi[0] - lo[0]).max(1e-9);
    let eh = (hi[1] - lo[1]).max(1e-9);
    let scale = (f64::from(frame.w) / ew).min(f64::from(frame.h) / eh) * 0.92;
    (scale, [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0])
}

/// Draws `mesh` flat-shaded in `colour` into `frame` on `page`.
pub fn draw_mesh(page: &mut Page, mesh: &Mesh, cam: &Camera, frame: Frame, colour: Rgb) {
    let (scale, c) = fit(mesh, cam, frame);
    let cx = f64::from(frame.x) + f64::from(frame.w) / 2.0;
    let cy = f64::from(frame.y) + f64::from(frame.h) / 2.0;
    let light = normalize([-0.4, 0.5, 0.8]);
    let view: Vec<V3> = mesh.vertices.iter().map(|v| cam.rotate(*v)).collect();
    let mut tris: Vec<Tri> = Vec::with_capacity(mesh.triangles.len());
    for t in &mesh.triangles {
        let (Some(a), Some(b), Some(d)) = (view.get(t[0] as usize), view.get(t[1] as usize), view.get(t[2] as usize)) else { continue };
        let n = normalize(cross(sub(*b, *a), sub(*d, *a)));
        if n[2] < -0.02 {
            continue; // facing away
        }
        let lum = (0.25 + 0.75 * dot(n, light).max(0.0)).clamp(0.0, 1.0);
        let shade = [0, 1, 2].map(|k| (f64::from(colour[k]) * lum).round().clamp(0.0, 255.0) as u8);
        let pt = |p: &V3| ((cx + (p[0] - c[0]) * scale) as f32, (cy - (p[1] - c[1]) * scale) as f32);
        tris.push(((a[2] + b[2] + d[2]) / 3.0, [pt(a), pt(b), pt(d)], shade));
    }
    tris.sort_by(|x, y| x.0.total_cmp(&y.0));
    for (_, pts, fill) in tris {
        // A hairline of the same colour hides seams between neighbours.
        page.prims.push(crate::draw::Prim::Poly { pts: pts.to_vec(), fill: Some(fill), stroke: Some((fill, 0.3)) });
    }
}

/// Colours for parts (colour-blind-friendly, mid-tones), by part number.
pub const PART_COLOURS: [Rgb; 8] =
    [[204, 102, 85], [86, 140, 190], [224, 170, 60], [112, 168, 110], [160, 110, 180], [210, 130, 160], [120, 180, 180], [170, 140, 100]];

/// Draws the crocheted result: every round of every part as a ring of the right size and height, coloured by part,
/// with every 5th round lighter so rounds can be counted. `focus` limits the picture to one part.
pub fn draw_figure(page: &mut Page, figure: &Figure, cam: &Camera, frame: Frame, focus: Option<usize>) {
    const SEG: usize = 28;
    let mut pts_all: Vec<V3> = Vec::new();
    let mut quads: Vec<([V3; 4], Rgb, bool)> = Vec::new(); // corners (model space), colour, highlight
    for (pi, part) in figure.parts.iter().enumerate() {
        if focus.is_some_and(|f| f != pi) {
            continue;
        }
        let base = PART_COLOURS[pi % PART_COLOURS.len()];
        let ring = |r: &Round, k: usize| -> V3 {
            let a = std::f64::consts::TAU * k as f64 / SEG as f64;
            [r.center[0] + r.radius * a.cos(), r.center[1] + r.radius * a.sin(), r.z]
        };
        for w in part.rounds.windows(2) {
            let hl = w[1].index % 5 == 0;
            for k in 0..SEG {
                let q = [ring(&w[0], k), ring(&w[0], k + 1), ring(&w[1], k + 1), ring(&w[1], k)];
                pts_all.extend(q);
                quads.push((q, base, hl));
            }
        }
        // Caps on closed tips so there are no holes.
        for (r, first) in [(part.rounds.first(), true), (part.rounds.last(), false)] {
            if let Some(r) = r {
                let closed = if first { true } else { part.closed_last };
                if closed {
                    let c = [r.center[0], r.center[1], r.z];
                    for k in 0..SEG {
                        let q = [c, ring(r, k), ring(r, k + 1), c];
                        pts_all.extend(q);
                        quads.push((q, base, false));
                    }
                }
            }
        }
    }
    if quads.is_empty() {
        return;
    }
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in &pts_all {
        let r = cam.rotate(*p);
        for a in 0..3 {
            lo[a] = lo[a].min(r[a]);
            hi[a] = hi[a].max(r[a]);
        }
    }
    let scale = (f64::from(frame.w) / (hi[0] - lo[0]).max(1e-9)).min(f64::from(frame.h) / (hi[1] - lo[1]).max(1e-9)) * 0.92;
    let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let cx = f64::from(frame.x) + f64::from(frame.w) / 2.0;
    let cy = f64::from(frame.y) + f64::from(frame.h) / 2.0;
    let light = normalize([-0.4, 0.5, 0.8]);
    let mut drawn: Vec<Drawn> = Vec::new();
    for (q, col, hl) in &quads {
        let v: Vec<V3> = q.iter().map(|p| cam.rotate(*p)).collect();
        let n = normalize(cross(sub(v[1], v[0]), sub(v[2], v[0])));
        // Rings have no inside/outside from winding alone, so light by the absolute angle.
        let lum = (0.35 + 0.65 * dot(n, light).abs()).clamp(0.0, 1.0);
        let lift = if *hl { 1.0 } else { 0.0 };
        let shade = [0, 1, 2].map(|k| {
            let base = f64::from(col[k]) * lum;
            (base + (255.0 - base) * 0.45 * lift).round().clamp(0.0, 255.0) as u8
        });
        let pts: Vec<(f32, f32)> = v.iter().map(|p| ((cx + (p[0] - c[0]) * scale) as f32, (cy - (p[1] - c[1]) * scale) as f32)).collect();
        drawn.push((v.iter().map(|p| p[2]).sum::<f64>() / 4.0, pts, shade));
    }
    drawn.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, pts, fill) in drawn {
        page.prims.push(crate::draw::Prim::Poly { pts, fill: Some(fill), stroke: Some((fill, 0.3)) });
    }
}

/// A page showing `mesh` at the default camera on a light background.
pub fn mesh_page(mesh: &Mesh, width: f32, height: f32, cam: &Camera, colour: Rgb) -> Page {
    let mut page = Page::new(width, height);
    page.rect(0.0, 0.0, width, height, Some([244, 241, 235]), None);
    draw_mesh(&mut page, mesh, cam, Frame { x: 0.0, y: 0.0, w: width, h: height }, colour);
    page
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn normalize(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 0.0 { [a[0] / l, a[1] / l, a[2] / l] } else { [0.0, 0.0, 1.0] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yarnfoundry_core::models;

    #[test]
    fn a_sphere_draws_lit_and_inside_the_page() {
        let page = mesh_page(&models::sphere(1.0), 400.0, 400.0, &Camera::default(), [200, 120, 90]);
        assert!(page.prims.len() > 100);
        for p in &page.prims {
            if let crate::draw::Prim::Poly { pts, .. } = p {
                for (x, y) in pts {
                    assert!((-1.0..=401.0).contains(x) && (-1.0..=401.0).contains(y));
                }
            }
        }
    }
}
