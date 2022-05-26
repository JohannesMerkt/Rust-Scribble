use egui::*;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Painting{
    all_lines: Vec<Line>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Line {
    position: Vec<Pos2>,
    stroke: Stroke,
}

impl Default for Line {
    fn default() -> Self {
        Self {
            position: Default::default(),
            stroke: Stroke::new(1.0, Color32::from_rgb(25, 200, 100)),
        }
    }
}

impl Default for Painting {
    fn default() -> Self {
        Self {
            all_lines: vec![Line::default()],
        }
    }
}

impl Painting {
    pub fn ui_control(&mut self, ui: &mut egui::Ui) -> egui::Response {
        ui.horizontal(|ui| {
            // egui::stroke_ui(ui, &mut self.current_stroke, "Stroke");
            let epaint::Stroke { width, color } = &mut self.all_lines.last_mut().unwrap().stroke;
            // ui.add(DragValue::new(width).speed(0.1).clamp_range(0..=10i32));
            ui.add(Slider::new(width, 1.0..=10.0).text("width"));
            ui.color_edit_button_srgba(color);
            let (_id, stroke_rect) = ui.allocate_space(ui.spacing().interact_size);
            let left = stroke_rect.left_center();
            let right = stroke_rect.right_center();
            ui.painter().line_segment([left, right], (*width, *color));
            ui.separator();
            if ui.button("Clear Painting").clicked() {
                self.all_lines.clear();
            }
        })
        .response
    }

    pub fn ui_content(&mut self, ui: &mut Ui) -> egui::Response {
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
            let test = Line { position: vec![], stroke: current_line.stroke};
            self.all_lines.push(test);
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

    pub fn ui(&mut self, ui: &mut Ui) {
        self.ui_control(ui);
        egui::warn_if_debug_build(ui);
        ui.label("Paint with your mouse/touch!");
        Frame::canvas(ui.style()).show(ui, |ui| {
            self.ui_content(ui);
        });
    }
}