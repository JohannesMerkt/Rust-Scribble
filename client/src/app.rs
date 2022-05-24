use egui::{TextStyle, ScrollArea};
use serde_json::json;
use crate::network::*;

use crate::Painting;

pub struct TemplateApp {
    // Example stuff:
    name: String,
    view: u8,
    message: String,
    painting: Painting,
    net_info: Option<NetworkInfo>,
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
            net_info: None,
        }
    }
}

impl TemplateApp {
    pub fn new() -> Self {
        Self::default()
    }
}

impl eframe::App for TemplateApp {

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let Self { name, view, message, painting, value, net_info } = self;

        {
            //Read a message from the network
            if let Some(network_info) = net_info.as_mut() {
                if let Ok(msg)= read_tcp_message(network_info) {
                    handle_message(msg);
                }
            }
        }

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
                    if ui.button("Send").clicked() {
                        let msg = json!({
                            "kind": "chat_message",
                            "username": name.to_string(),
                            "message": message.to_string(),
                        });
                        
                        if let Some(mut network_info) = net_info.as_mut() {
                            let _ = send_message(&mut network_info, msg);
                        }
                        *message = "".to_string();
                    }

                });

                if ui.button("Disconnect").clicked() {
                    *view = 0;
                }


                //A button that will send a ready message to the server
                if ui.button("Ready").clicked() {
                    let msg = json!({
                        "kind": "ready",
                        "username": name.to_string(),
                        "ready": true,
                    });
                    
                    if let Some(mut network_info) = net_info.as_mut() {
                        let _ = send_message(&mut network_info, msg);
                    }
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

                //Get the name and connect to the server
                if ui.button("Connect").clicked() {
                     let res = connect_to_server("127.0.0.1", 3000, name);
                        match res {
                            Ok(info) => {
                                *net_info = Some(info);
                                *view = 1;
                            },
                            Err(_) => {
                                //TODO ! Display Error Message here when Client cannot connect
                            }
                        }
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

        fn handle_message(msg: serde_json::Value) {
            //TODO handle messages 
            println!("{}", msg.to_string());

            //Display message in the chat window
            if msg["kind"].eq("chat_message") {
                let message = msg["message"].as_str().unwrap();
                println!("{}", message);
            }
        }
    }
}

