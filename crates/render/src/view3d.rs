//! A simple 3D view drawn as flat-shaded polygons on a [`Page`], so the PNG, SVG and PDF back ends can all show a model.
//! Painter's algorithm (far to near), one light, no textures. Used for the build-story pictures and the PDF cover.

use yarnfoundry_core::Mesh;

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
