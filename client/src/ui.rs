use bevy::{prelude::*};
use bevy_egui::{egui, EguiContext};
use rayon::prelude::*;

use rust_scribble_common::gamestate_common::*;
use crate::network_plugin;
use crate::clientstate::ClientState;

/// this system handles rendering the ui
pub fn render_ui(mut egui_context: ResMut<EguiContext>, mut networkstate: ResMut<network_plugin::NetworkState>, mut clientstate: ResMut<ClientState>) {
    if networkstate.info.is_none() {
        render_connect_view(&mut egui_context, &mut networkstate);
    } else if clientstate.game_state.in_game {
        render_ingame_view(&mut egui_context, &mut networkstate, &mut clientstate);
    } else {
        render_lobby_view(&mut egui_context, &mut networkstate, &mut clientstate);
    }
}

fn render_connect_view(egui_context: &mut ResMut<EguiContext>, networkstate: &mut ResMut<network_plugin::NetworkState>) {
    egui::CentralPanel::default().show(egui_context.ctx_mut(), |ui| {
        ui.heading("Rust Scribble:");
        ui.label("Name");
        ui.text_edit_singleline(&mut networkstate.name);
        ui.label("Server Address");
        ui.text_edit_singleline(&mut networkstate.address);
        ui.label("Server Port");
        ui.add(egui::widgets::DragValue::new(&mut networkstate.port).speed(1.0));
        if ui.button("Connect").clicked() || ui.input().key_pressed(egui::Key::Enter) {
            // connect to the server
            network_plugin::connect(networkstate);
        }
    });
}

fn render_lobby_view(egui_context: &mut ResMut<EguiContext>, networkstate: &mut ResMut<network_plugin::NetworkState>, clientstate: &mut ResMut<ClientState>) {
    egui::CentralPanel::default().show(egui_context.ctx_mut(), |ui| {
        ui.heading("Connected!");
        render_chat_area(ui, networkstate, clientstate);
        render_player_list(ui, clientstate);

        if let Some(net_info) = networkstate.info.as_mut() {
            let player_result = clientstate.game_state.players.iter().find(|player| player.id == net_info.id);
            if let Some(player) = player_result {
                if player.ready {
                    if ui.button("Not Ready").clicked() {
                        network_plugin::send_ready(networkstate, false);
                    }
                } else if ui.button("Ready").clicked() {
                    network_plugin::send_ready(networkstate, true);
                }
            }
        }
    });
}

fn render_ingame_view(egui_context: &mut ResMut<EguiContext>, networkstate: &mut ResMut<network_plugin::NetworkState>, clientstate: &mut ResMut<ClientState>) {
    egui::SidePanel::right("side_panel").show(egui_context.ctx_mut(), |ui| {
        render_chat_area(ui, networkstate, clientstate);
        render_player_list(ui, clientstate);

        if ui.button("Disconnect").clicked() {
            network_plugin::send_disconnect(networkstate);
            clientstate.game_state.in_game = false;
        }
    });
    egui::CentralPanel::default().show(egui_context.ctx_mut(), |ui| {
        // The central panel the region left after adding TopPanel's and SidePanel's
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut clientstate.stroke.width, 1.0..=10.0).text("width"));
            if ui.color_edit_button_srgba(&mut clientstate.stroke.color).clicked_elsewhere() {
            };
            if ui.button("Eraser").clicked() {
                clientstate.stroke.color = egui::Color32::from_rgb(255,255,255); 
            }
            /*if ui.button("Color").clicked() {
                *color = self.curr_stroke.color;
            }*/
            let (_id, stroke_rect) = ui.allocate_space(ui.spacing().interact_size);
            let left = stroke_rect.left_center();
            let right = stroke_rect.right_center();
            ui.painter().line_segment([left, right], clientstate.stroke);
            ui.separator();
            /*if ui.button("Clear Painting").clicked() {
                self.all_lines.clear();
            }*/
        }); 
        ui.label("Paint with your mouse/touch!");
        egui::Frame::canvas(ui.style()).show(ui, |ui| {
            let (mut response, painter) = ui.allocate_painter(ui.available_size_before_wrap(), egui::Sense::drag());

            let to_screen = egui::emath::RectTransform::from_to(egui::Rect::from_min_size(egui::Pos2::ZERO, response.rect.square_proportions()), response.rect);
            let from_screen = to_screen.inverse();

            if clientstate.lines.is_empty() {
                let width = clientstate.stroke.width;
                let color = clientstate.stroke.color;
                clientstate.lines.push(Line {
                    positions: Vec::new(),
                    stroke: egui::Stroke::new(width, color)
                });
            }

            let current_line = clientstate.lines.last_mut().unwrap();

            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let canvas_pos = from_screen * pointer_pos;
                if current_line.positions.last() != Some(&canvas_pos) {
                    current_line.positions.push(canvas_pos);
                    //let width = &gamestate.stroke.width;
                    //let color = gamestate.stroke.color;
                    //let stroke = egui::Stroke::new(width, color);
                    //current_line.stroke = stroke;
                    response.mark_changed();
                }
            } else if !current_line.positions.is_empty() {
                network_plugin::send_line(networkstate, current_line);
                let width = clientstate.stroke.width;
                let color = clientstate.stroke.color;
                let new_line = Line { positions: vec![], stroke: egui::Stroke::new(width, color)};
                clientstate.lines.push(new_line);
                response.mark_changed();
            }

            let mut shapes = vec![];
            for line in &clientstate.lines {
                if line.positions.len() >= 2 {
                    let points: Vec<egui::Pos2> = line.positions.par_iter().map(|p| to_screen * *p).collect();
                    shapes.push(egui::Shape::line(points, line.stroke));
                }
            }
            painter.extend(shapes);

            response
        });
    });
}

fn render_chat_area(ui: &mut egui::Ui, networkstate: &mut ResMut<network_plugin::NetworkState>, clientstate: &mut ResMut<ClientState>) {
    ui.heading("Chat");
    let text_style = egui::TextStyle::Body;
    let row_height = ui.text_style_height(&text_style);
    egui::ScrollArea::vertical().auto_shrink([false; 2]).stick_to_bottom().max_height(200.0).show_rows(
        ui,
        row_height,
        100,
        |ui, _| {
            for chat_message in clientstate.chat_messages.iter() {
                let search_player_result = clientstate.game_state.players.par_iter().find_any(|player| player.id == chat_message.player_id);
                if let Some(player) = search_player_result {
                    ui.label(format!("{}: {}",player.name, chat_message.message));
                    ui.set_min_width(100.0);
                }

            }
        },
    );
    ui.horizontal(|ui| {
        ui.label("Chat: ");
        ui.text_edit_singleline(&mut clientstate.chat_message_input);
        if ui.button("Send").clicked() || (ui.input().key_pressed(egui::Key::Enter) && !clientstate.chat_message_input.is_empty()) {
            network_plugin::send_chat_message(networkstate, clientstate.chat_message_input.clone());
            clientstate.chat_message_input.clear();
        }

    });
}

fn render_player_list(ui: &mut egui::Ui, clientstate: &mut ResMut<ClientState>) {
    ui.heading("Players");
    for player in &clientstate.game_state.players {
        ui.label(format!("{} - ready: {} - playing: {} - drawing: {} - guessed word: {}",player.name, player.ready, player.playing, player.drawing, player.guessed_word));
    }
}