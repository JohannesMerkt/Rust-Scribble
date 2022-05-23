use egui::{TextStyle, ScrollArea};

use crate::Painting;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state

pub struct TemplateApp {
    // Example stuff:
    name: String,
    view: u8,
    message: String,
    painting: Painting,
    // this how you opt-out of serialization of a member
    #[serde(skip)]
    value: f32,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            name: "Player".to_owned(),
            view: 0,
            message: "".to_owned(),
            painting: Default::default(),
            value: 2.7,
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customized the look at feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.
        // cc.egui_ctx.set_visuals = 
        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        cc.storage.and_then(|storage| eframe::get_value(storage, eframe::APP_KEY)).unwrap_or_default()
    }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let Self { name, view, message, painting, value } = self;

        // Examples of how to create different panels and windows.
        // Pick whichever suits you.
        // Tip: a good default choice is to just keep the `CentralPanel`.
        // For inspiration and more examples, go to https://emilk.github.io/egui
        if *view == 1 {
            egui::SidePanel::right("side_panel").show(ctx, |ui| {
                ui.heading("Chat");
                let text_style = TextStyle::Body;
                let row_height = ui.text_style_height(&text_style);
                ScrollArea::vertical().stick_to_bottom().max_height(200.0).show_rows(
                    ui,
                    row_height,
                    100,
                    |ui, row_range| {
                        for row in row_range {
                            let text = format!("This is message {}", row + 1);
                            ui.label(text);
                        }
                    },
                );

                ui.horizontal(|ui| {
                    ui.label("Guess: ");
                    ui.text_edit_singleline(message);
                });

                if ui.button("Disconnect").clicked() {
                    *view = 0;
                }
            });
            egui::CentralPanel::default().show(ctx, |ui| {
                // The central panel the region left after adding TopPanel's and SidePanel's
                painting.ui(ui);
            });
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                // The central panel the region left after adding TopPanel's and SidePanel's
    
                ui.heading("Give yourself a Name:");
                ui.text_edit_singleline(name);

                if ui.button("Connect").clicked() {
                    *view = 1;
                }
                egui::warn_if_debug_build(ui);
            });
        }

        if false {
            egui::Window::new("Window").show(ctx, |ui| {
                ui.label("Windows can be moved by dragging them.");
                ui.label("They are automatically sized based on contents.");
                ui.label("You can turn on resizing and scrolling if you like.");
                ui.label("You would normally chose either panels OR windows.");
            });
        }
    }
}

