use egui::{Pos2, Color32};
use egui::{TextStyle, ScrollArea, Key};
use rust_scribble_common::network_common::NetworkInfo;
use serde_json::json;
use rayon::prelude::*;
use crate::network::*;
use crate::Painting;
use crate::painting;

pub struct TemplateApp {
    // Example stuff:
    name: String,
    view: u8,
    message: String,
    chat_messages: Vec<String>,
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
            chat_messages: vec!["Welcome to the Rust-EGUI Chat!".to_owned()],
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
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let Self { name, view, message, painting, value, net_info, chat_messages } = self;

        {
            //Read a message from the network
            if let Some(network_info) = net_info.as_mut() {
                if message_waiting(network_info) {
                    println!("Message waiting");
                }   
                if let Ok(msg)= read_messages(network_info, 5) {
                    handle_message(msg, chat_messages, painting);
                }
            }
        }

        if *view == 1 {
            egui::SidePanel::right("side_panel").show(ctx, |ui| {
                ui.heading("Chat");
                let text_style = TextStyle::Body;
                let row_height = ui.text_style_height(&text_style);
                //set min_width

                ScrollArea::vertical().stick_to_bottom().max_height(200.0).show_rows(
                    ui,
                    row_height,
                    100,
                    |ui, _| {
                        for row in chat_messages.iter() {
                            ui.label(row.to_string());
                            ui.set_min_width(100.0);
                        }
                    },
                );

                ui.horizontal(|ui| {
                    ui.label("Chat: ");
                    ui.text_edit_singleline(message);
                    if ui.button("Send").clicked() || (ui.input().key_pressed(Key::Enter) && !message.is_empty()) {
                        let msg = json!({
                            "kind": "chat_message",
                            "username": name.to_string(),
                            "message": message.to_string(),
                        });
                        
                        if let Some(network_info) = net_info.as_mut() {
                            let _ = send_message(network_info, msg);
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
                    
                    if let Some(network_info) = net_info.as_mut() {
                        let _ = send_message(network_info, msg);
                    }
                }
            });
            egui::CentralPanel::default().show(ctx, |ui| {
                // The central panel the region left after adding TopPanel's and SidePanel's
                painting.ui(ui, net_info);
            });
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                // The central panel the region left after adding TopPanel's and SidePanel's
    
                ui.heading("Give yourself a Name:");
                ui.text_edit_singleline(name);

                //Get the name and connect to the server
                if ui.button("Connect").clicked() || ui.input().key_pressed(Key::Enter){
                     let res = connect_to_server("127.0.0.1", 3000, name);
                        match res {
                            Ok(info) => {
                                *net_info = Some(info);
                                *view = 1;
                            },
                            Err(_) => {
                                println!("Could not connect to server");
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

        fn handle_message(msg: Vec<serde_json::Value>, chat_messages: &mut Vec<String>, painting: &mut Painting) {
            //TODO handle messages 
            for m in msg {
                println!("{}", m);

                //Display message in the chat window
                if m["kind"].eq("chat_message") {
                    let message = m["message"].as_str().unwrap();
                    let username = m["username"].as_str().unwrap();
                    chat_messages.push(format!("{}: {}", username, message));
                    println!("{} says: {}", username, message);
                } else if m["kind"].eq("add_line") {
                    let posx:Vec<f64> = m["line"]["posx"].as_array().unwrap().par_iter().map(|pos| pos.as_f64().unwrap()).collect();
                    let posy:Vec<f64> = m["line"]["posy"].as_array().unwrap().par_iter().map(|pos| pos.as_f64().unwrap()).collect();
                    let mut pos_line: Vec<Pos2> = Vec::new();
                    for pos in 0..posx.len() {
                        let pos2 = Pos2{x:posx[pos] as f32, y:posy[pos] as f32};
                        pos_line.push(pos2);
                    }
                    let width = m["line"]["width"].as_f64().unwrap();
                    let color_values: Vec<u8> = m["line"]["color"].as_array().unwrap().par_iter().map(|col| col.as_u64().unwrap() as u8).collect();
                    let color = Color32::from_rgb(color_values[0], color_values[1], color_values[2]);
                    let line: painting::Line = painting::Line {
                        position: pos_line,
                        stroke: egui::Stroke::new(width as f32, color),
                    };
                    painting.all_lines.insert(painting.all_lines.len() - 1, line);
                }
            }
        }
    }
}
