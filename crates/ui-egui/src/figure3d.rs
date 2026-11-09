//! The 3D preview: the original model beside the crocheted result (stacked rings coloured by part, every 5th round
//! lighter). Drag to turn, scroll to zoom. It is drawn through egui's painter from the render crate's projected
//! polygons, so it works the same on the desktop and in the browser build.

use yarnfoundry_core::{Figure, Mesh};
use yarnfoundry_render::view3d::{Camera, Fit, Frame, PART_COLOURS, Poly, figure_polys, mesh_polys};

/// Camera and options of the preview.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FigureView {
    pub cam: Camera,
    /// Show only this part of the crocheted result.
    pub focus: Option<usize>,
    /// Turn slowly by itself.
    pub spin: bool,
}

fn colour(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

/// All the polygons as one coloured mesh (convex polygons are fanned into triangles).
pub fn polys_to_mesh(polys: &[Poly]) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    for p in polys {
        let base = mesh.vertices.len() as u32;
        for &(x, y) in &p.pts {
            mesh.colored_vertex(egui::pos2(x, y), colour(p.fill));
        }
        for k in 1..p.pts.len().saturating_sub(1) {
            mesh.add_triangle(base, base + k as u32, base + k as u32 + 1);
        }
    }
    mesh
}

impl FigureView {
    /// Draws the two views side by side with a part list below. `mesh` is the fitted model, `figure` its parts.
    pub fn ui(&mut self, ui: &mut egui::Ui, mesh: &Mesh, figure: &Figure) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.spin, "Spin");
            if ui.button("Reset view").clicked() {
                self.cam = Camera::default();
            }
            ui.label(egui::RichText::new("Drag to turn, scroll to zoom. Every 5th round is lighter.").color(crate::theme::HINT));
        });
        if self.spin {
            self.cam.yaw += 0.01;
            ui.ctx().request_repaint();
        }
        let avail = ui.available_size();
        let side = ((avail.x - 12.0) / 2.0).max(120.0).min((avail.y - 70.0).max(120.0));
        let fit = Fit::of_mesh(mesh);
        let mut scroll = 0.0_f32;
        let mut drag = egui::Vec2::ZERO;
        ui.horizontal(|ui| {
            for (i, title) in ["Original model", "Crocheted result"].iter().enumerate() {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(*title).strong());
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click_and_drag());
                    let painter = ui.painter_at(rect);
                    painter.rect_filled(rect, 6.0, crate::theme::CANVAS_BG);
                    let frame = Frame { x: rect.min.x, y: rect.min.y, w: rect.width(), h: rect.height() };
                    let polys = if i == 0 {
                        mesh_polys(mesh, &self.cam, &fit, frame, [196, 120, 96])
                    } else {
                        figure_polys(figure, &self.cam, &fit, frame, self.focus)
                    };
                    painter.add(egui::Shape::mesh(polys_to_mesh(&polys)));
                    if resp.dragged() {
                        drag += resp.drag_delta();
                    }
                    if resp.hovered() {
                        scroll += ui.input(|inp| inp.smooth_scroll_delta.y);
                    }
                });
            }
        });
        if drag != egui::Vec2::ZERO {
            self.cam = self.cam.dragged(f64::from(drag.x), f64::from(drag.y));
        }
        if scroll != 0.0 {
            self.cam = self.cam.zoomed(f64::from((scroll * 0.002).exp()));
        }
        ui.horizontal_wrapped(|ui| {
            for (i, part) in figure.parts.iter().enumerate() {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, colour(PART_COLOURS[i % PART_COLOURS.len()]));
                let on = self.focus == Some(i);
                if ui.selectable_label(on, format!("{} ({} rounds)", part.name, part.rounds.len())).clicked() {
                    self.focus = if on { None } else { Some(i) };
                }
            }
            if self.focus.is_some() && ui.button("Show all parts").clicked() {
                self.focus = None;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yarnfoundry_core::{Gauge, ManualSplits, Unit, models, split_figure};

    #[test]
    fn the_preview_draws_both_views_and_survives_hostile_cameras() {
        let g = Gauge::new(20.0, 20.0, 4.0, Unit::Inches).unwrap();
        let mesh = models::snowman().fitted(6.0, Unit::Inches, None).unwrap();
        let figure = split_figure(&mesh, &g, &ManualSplits::default()).unwrap();
        let ctx = egui::Context::default();
        let mut view = FigureView::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| view.ui(ui, &mesh, &figure));
        });
        out.textures_delta.clear();
        let meshes = out.shapes.iter().filter(|s| matches!(s.shape, egui::Shape::Mesh(_))).count();
        assert!(meshes >= 2, "two 3D views, found {meshes}");
        view.cam = Camera { yaw: f64::NAN, pitch: 1e9, zoom: 0.0 };
        view.focus = Some(99);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| view.ui(ui, &mesh, &figure));
        });
        out.textures_delta.clear();
    }

    #[test]
    fn polygons_become_triangles() {
        let m = polys_to_mesh(&[Poly { pts: vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)], fill: [1, 2, 3] }]);
        assert_eq!((m.vertices.len(), m.indices.len()), (4, 6));
    }
}
