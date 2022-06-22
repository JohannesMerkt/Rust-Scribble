use egui::*;
use serde_json::json;

use rust_scribble_common::network_common::{NetworkInfo, send_message};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Painting{
    pub all_lines: Vec<Line>,
    curr_stroke: Stroke,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Line {
    pub position: Vec<Pos2>,
    pub stroke: Stroke,
}

impl Default for Line {
    fn default() -> Self {
        Self {
            position: Vec::new(),
            stroke: Stroke::new(1.0, Color32::from_rgb(25, 200, 100)),
        }
    }
}

impl Default for Painting {
    fn default() -> Self {
        Self {
            all_lines: vec![Line::default()],
            curr_stroke: Stroke::new(1.0, Color32::from_rgb(25, 200, 100)),
        }
    }
}

impl Painting {
    pub fn ui_control(&mut self, ui: &mut egui::Ui) -> egui::Response {
        ui.horizontal(|ui| {
            let epaint::Stroke { width, color } = &mut self.all_lines.last_mut().unwrap().stroke;
            ui.add(Slider::new(width, 1.0..=10.0).text("width"));
            if ui.color_edit_button_srgba(&mut self.curr_stroke.color).clicked_elsewhere() {
                *color = self.curr_stroke.color;
            };
            if ui.button("Eraser").clicked() {
                *color = Color32::from_rgb(255,255,255); 
            }
            if ui.button("Color").clicked() {
                *color = self.curr_stroke.color;
            }
            let (_id, stroke_rect) = ui.allocate_space(ui.spacing().interact_size);
            let left = stroke_rect.left_center();
            let right = stroke_rect.right_center();
            ui.painter().line_segment([left, right], (*width, self.curr_stroke.color));
            ui.separator();
            if ui.button("Clear Painting").clicked() {
                self.all_lines.clear();
            }
        })
        .response
    }

    pub fn ui_content(&mut self, ui: &mut Ui, net_info: &mut Option<NetworkInfo>) -> egui::Response {
        let (mut response, painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), Sense::drag());

        let to_screen = emath::RectTransform::from_to(
            Rect::from_min_size(Pos2::ZERO, response.rect.square_proportions()),
            response.rect,
        );
        let from_screen = to_screen.inverse();

        if self.all_lines.is_empty() {
            self.all_lines.push(Line::default());
        }

        let current_line = self.all_lines.last_mut().unwrap();

        if let Some(pointer_pos) = response.interact_pointer_pos() {
            let canvas_pos = from_screen * pointer_pos;
            if current_line.position.last() != Some(&canvas_pos) {
                current_line.position.push(canvas_pos);
                response.mark_changed();
            }
        } else if !current_line.position.is_empty() {
            let x_values: Vec<f32> = current_line.position.iter().map(|pos2| pos2.x).collect();
            let y_values: Vec<f32> = current_line.position.iter().map(|pos2| pos2.y).collect();
            let width = current_line.stroke.width;
            let color = current_line.stroke.color;
            let msg = json!({
                "kind": "add_line",
                "line": {
                    "posx": x_values,
                    "posy": y_values,
                    "width": width,
                    "color": color,
                }
            });
            if let Some(network_info) = net_info.as_mut() {
                let _ = send_message(network_info, &msg);
            }
            let new_line = Line { position: vec![], stroke: current_line.stroke};
            self.all_lines.push(new_line);
            response.mark_changed();
        }

        let mut shapes = vec![];
        for line in &self.all_lines {
            if line.position.len() >= 2 {
                let points: Vec<Pos2> = line.position.iter().map(|p| to_screen * *p).collect();
                shapes.push(egui::Shape::line(points, line.stroke));
            }
        }
        painter.extend(shapes);

        response
    }

    pub fn ui(&mut self, ui: &mut Ui, net_info: &mut Option<NetworkInfo>) {
        self.ui_control(ui);
        egui::warn_if_debug_build(ui);
        ui.label("Paint with your mouse/touch!");
        Frame::canvas(ui.style()).show(ui, |ui| {
            self.ui_content(ui, net_info);
        });
    }
}