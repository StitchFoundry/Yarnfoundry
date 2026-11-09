//! A simple 3D view drawn as flat-shaded polygons, so the PNG, SVG and PDF back ends and the app can all show a
//! model or the crocheted result. Painter's algorithm (far to near), one light, no textures.

use std::f64::consts::TAU;

use yarnfoundry_core::{Figure, Mesh, Round};

use crate::draw::{Anchor, Page, Prim, Rgb};

/// A turntable camera: `yaw` turns around the vertical axis, `pitch` tilts down (both in radians); `zoom` scales.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub yaw: f64,
    pub pitch: f64,
    pub zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self { yaw: 0.6, pitch: 0.35, zoom: 1.0 }
    }
}

type V3 = [f64; 3];

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

    /// A camera turned by a mouse drag of `dx`, `dy` points, or zoomed by `scroll` (clamped to a sensible range).
    pub fn dragged(self, dx: f64, dy: f64) -> Self {
        Self { yaw: self.yaw - dx * 0.01, pitch: (self.pitch + dy * 0.01).clamp(-1.4, 1.4), ..self }
    }

    pub fn zoomed(self, factor: f64) -> Self {
        Self { zoom: (self.zoom * factor).clamp(0.3, 6.0), ..self }
    }
}

/// A box on a page or screen, in points.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Where the model sits: its centre and the radius of a ball that holds it, so rotating never changes the size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub center: V3,
    pub radius: f64,
}

impl Fit {
    /// Fit around some points.
    pub fn around<'a>(pts: impl Iterator<Item = &'a V3> + Clone) -> Fit {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for p in pts.clone() {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
        if lo[0] > hi[0] {
            return Fit { center: [0.0; 3], radius: 1.0 };
        }
        let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let radius = pts.map(|p| ((p[0] - center[0]).powi(2) + (p[1] - center[1]).powi(2) + (p[2] - center[2]).powi(2)).sqrt()).fold(1e-9, f64::max);
        Fit { center, radius }
    }

    pub fn of_mesh(mesh: &Mesh) -> Fit {
        Fit::around(mesh.vertices.iter())
    }

    pub fn of_figure(figure: &Figure) -> Fit {
        let mut pts: Vec<V3> = Vec::new();
        for p in &figure.parts {
            for r in &p.rounds {
                for k in 0..4 {
                    let a = TAU * f64::from(k) / 4.0;
                    pts.push([r.center[0] + r.radius * a.cos(), r.center[1] + r.radius * a.sin(), r.z]);
                }
            }
        }
        Fit::around(pts.iter())
    }
}

/// A filled polygon in screen points.
#[derive(Clone, Debug, PartialEq)]
pub struct Poly {
    pub pts: Vec<(f32, f32)>,
    pub fill: Rgb,
}

struct Projector {
    cam: Camera,
    fit: Fit,
    scale: f64,
    cx: f64,
    cy: f64,
}

impl Projector {
    fn new(cam: &Camera, fit: &Fit, frame: Frame) -> Self {
        let scale = f64::from(frame.w.min(frame.h)) / (2.0 * fit.radius) * 0.9 * cam.zoom;
        Self { cam: *cam, fit: *fit, scale, cx: f64::from(frame.x) + f64::from(frame.w) / 2.0, cy: f64::from(frame.y) + f64::from(frame.h) / 2.0 }
    }

    fn view(&self, p: V3) -> V3 {
        self.cam.rotate([p[0] - self.fit.center[0], p[1] - self.fit.center[1], p[2] - self.fit.center[2]])
    }

    fn screen(&self, v: V3) -> (f32, f32) {
        ((self.cx + v[0] * self.scale) as f32, (self.cy - v[1] * self.scale) as f32)
    }
}

/// Most triangles drawn at once; very big models are drawn with every few-th triangle skipped.
pub const MAX_DRAWN_TRIANGLES: usize = 60_000;

/// The model as flat-shaded polygons, far to near.
pub fn mesh_polys(mesh: &Mesh, cam: &Camera, fit: &Fit, frame: Frame, colour: Rgb) -> Vec<Poly> {
    let pr = Projector::new(cam, fit, frame);
    let light = normalize([-0.4, 0.5, 0.8]);
    let view: Vec<V3> = mesh.vertices.iter().map(|v| pr.view(*v)).collect();
    let stride = mesh.triangles.len().div_ceil(MAX_DRAWN_TRIANGLES).max(1);
    let mut tris: Vec<(f64, Poly)> = Vec::new();
    for t in mesh.triangles.iter().step_by(stride) {
        let (Some(a), Some(b), Some(d)) = (view.get(t[0] as usize), view.get(t[1] as usize), view.get(t[2] as usize)) else { continue };
        let n = normalize(cross(sub(*b, *a), sub(*d, *a)));
        if n[2] < -0.02 {
            continue; // facing away
        }
        let lum = (0.25 + 0.75 * dot(n, light).max(0.0)).clamp(0.0, 1.0);
        let fill = [0, 1, 2].map(|k| (f64::from(colour[k]) * lum).round().clamp(0.0, 255.0) as u8);
        tris.push(((a[2] + b[2] + d[2]) / 3.0, Poly { pts: vec![pr.screen(*a), pr.screen(*b), pr.screen(*d)], fill }));
    }
    tris.sort_by(|x, y| x.0.total_cmp(&y.0));
    tris.into_iter().map(|t| t.1).collect()
}

/// Colours for parts (mid-tones), by part number.
pub const PART_COLOURS: [Rgb; 8] =
    [[204, 102, 85], [86, 140, 190], [224, 170, 60], [112, 168, 110], [160, 110, 180], [210, 130, 160], [120, 180, 180], [170, 140, 100]];

/// The crocheted result as polygons: every round of every part a ring of the right size and height, coloured by part,
/// every 5th round lighter so rounds can be counted. `focus` limits the picture to one part.
pub fn figure_polys(figure: &Figure, cam: &Camera, fit: &Fit, frame: Frame, focus: Option<usize>) -> Vec<Poly> {
    const SEG: usize = 28;
    let pr = Projector::new(cam, fit, frame);
    let light = normalize([-0.4, 0.5, 0.8]);
    let mut drawn: Vec<(f64, Poly)> = Vec::new();
    let mut push = |q: [V3; 4], col: Rgb, highlight: bool| {
        let v: Vec<V3> = q.iter().map(|p| pr.view(*p)).collect();
        let n = normalize(cross(sub(v[1], v[0]), sub(v[2], v[0])));
        // A ring has no inside/outside from winding alone, so light by the absolute angle.
        let lum = (0.35 + 0.65 * dot(n, light).abs()).clamp(0.0, 1.0);
        let lift = if highlight { 0.45 } else { 0.0 };
        let fill = [0, 1, 2].map(|k| {
            let base = f64::from(col[k]) * lum;
            (base + (255.0 - base) * lift).round().clamp(0.0, 255.0) as u8
        });
        drawn.push((v.iter().map(|p| p[2]).sum::<f64>() / 4.0, Poly { pts: v.iter().map(|p| pr.screen(*p)).collect(), fill }));
    };
    let ring = |r: &Round, k: usize| -> V3 {
        let a = TAU * k as f64 / SEG as f64;
        [r.center[0] + r.radius * a.cos(), r.center[1] + r.radius * a.sin(), r.z]
    };
    for (pi, part) in figure.parts.iter().enumerate() {
        if focus.is_some_and(|f| f != pi) {
            continue;
        }
        let base = PART_COLOURS[pi % PART_COLOURS.len()];
        for w in part.rounds.windows(2) {
            let highlight = w[1].index % 5 == 0;
            for k in 0..SEG {
                push([ring(&w[0], k), ring(&w[0], k + 1), ring(&w[1], k + 1), ring(&w[1], k)], base, highlight);
            }
        }
        // Caps on closed tips so there are no holes.
        for (r, closed) in [(part.rounds.first(), true), (part.rounds.last(), part.closed_last)] {
            if let (Some(r), true) = (r, closed) {
                let c = [r.center[0], r.center[1], r.z];
                for k in 0..SEG {
                    push([c, ring(r, k), ring(r, k + 1), c], base, false);
                }
            }
        }
    }
    drawn.sort_by(|a, b| a.0.total_cmp(&b.0));
    drawn.into_iter().map(|d| d.1).collect()
}

/// Draws polygons on a page.
pub fn draw_polys(page: &mut Page, polys: Vec<Poly>) {
    for p in polys {
        // A hairline of the same colour hides seams between neighbours.
        page.prims.push(Prim::Poly { pts: p.pts, fill: Some(p.fill), stroke: Some((p.fill, 0.3)) });
    }
}

/// Draws `mesh` flat-shaded in `colour` into `frame` on `page`.
pub fn draw_mesh(page: &mut Page, mesh: &Mesh, cam: &Camera, frame: Frame, colour: Rgb) {
    draw_polys(page, mesh_polys(mesh, cam, &Fit::of_mesh(mesh), frame, colour));
}

/// Draws the crocheted result (see [`figure_polys`]) into `frame`.
pub fn draw_figure(page: &mut Page, figure: &Figure, cam: &Camera, frame: Frame, focus: Option<usize>) {
    draw_polys(page, figure_polys(figure, cam, &Fit::of_figure(figure), frame, focus));
}

/// A page showing `mesh` at the default camera on a light background.
pub fn mesh_page(mesh: &Mesh, width: f32, height: f32, cam: &Camera, colour: Rgb) -> Page {
    let mut page = Page::new(width, height);
    page.rect(0.0, 0.0, width, height, Some([244, 241, 235]), None);
    draw_mesh(&mut page, mesh, cam, Frame { x: 0.0, y: 0.0, w: width, h: height }, colour);
    page
}

/// A page with a label above and a picture: used for build-story and cover renders.
pub fn caption(page: &mut Page, x: f32, y: f32, size: f32, text: &str, bold: bool) {
    page.prims.push(Prim::Text { x, y, size, text: text.to_string(), anchor: Anchor::Start, color: [40, 40, 40], bold });
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
    use yarnfoundry_core::{Gauge, ManualSplits, Unit, models, split_figure};

    #[test]
    fn a_sphere_draws_lit_and_inside_the_page() {
        let page = mesh_page(&models::sphere(1.0), 400.0, 400.0, &Camera::default(), [200, 120, 90]);
        assert!(page.prims.len() > 100);
        for p in &page.prims {
            if let Prim::Poly { pts, .. } = p {
                for (x, y) in pts {
                    assert!((-1.0..=401.0).contains(x) && (-1.0..=401.0).contains(y));
                }
            }
        }
    }

    #[test]
    fn size_does_not_change_while_turning() {
        let m = models::snowman();
        let fit = Fit::of_mesh(&m);
        let frame = Frame { x: 0.0, y: 0.0, w: 300.0, h: 300.0 };
        let extent = |yaw: f64| {
            let polys = mesh_polys(&m, &Camera { yaw, pitch: 0.2, zoom: 1.0 }, &fit, frame, [200; 3]);
            let xs: Vec<f32> = polys.iter().flat_map(|p| p.pts.iter().map(|q| q.0)).collect();
            xs.iter().copied().fold(f32::MIN, f32::max) - xs.iter().copied().fold(f32::MAX, f32::min)
        };
        assert!((extent(0.0) - extent(1.3)).abs() < 12.0, "{} {}", extent(0.0), extent(1.3));
    }

    #[test]
    fn the_figure_view_has_rings_and_obeys_focus_and_zoom() {
        let g = Gauge::new(20.0, 20.0, 4.0, Unit::Inches).unwrap();
        let m = models::snowman().fitted(6.0, Unit::Inches, None).unwrap();
        let f = split_figure(&m, &g, &ManualSplits::default()).unwrap();
        let fit = Fit::of_figure(&f);
        let frame = Frame { x: 0.0, y: 0.0, w: 300.0, h: 300.0 };
        let all = figure_polys(&f, &Camera::default(), &fit, frame, None);
        let one = figure_polys(&f, &Camera::default(), &fit, frame, Some(0));
        assert!(all.len() > one.len() && !one.is_empty());
        // Every 5th round is lighter: more than one distinct shade level exists per part colour family.
        let light: std::collections::HashSet<[u8; 3]> = all.iter().map(|p| p.fill).collect();
        assert!(light.len() > 10);
        let z = Camera::default().zoomed(100.0);
        assert!((z.zoom - 6.0).abs() < 1e-9);
        let d = Camera::default().dragged(10_000.0, 10_000.0);
        assert!(d.pitch <= 1.4 && d.yaw.is_finite());
    }
}
