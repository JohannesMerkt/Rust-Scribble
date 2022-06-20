use bevy::{prelude::*};
use bevy_egui::{egui, EguiContext};
use crate::network_plugin;
use crate::gamestate;

/// this system handles rendering the ui
pub fn render_ui(mut egui_context: ResMut<EguiContext>, mut networkstate: ResMut<network_plugin::NetworkState>, mut gamestate: ResMut<gamestate::GameState>) {
    if let None = networkstate.info {
        render_connect_view(&mut egui_context, &mut networkstate);
    } else {
        if gamestate.in_game {
            render_ingame_view(&mut egui_context, &mut networkstate, &mut gamestate);
        } else {
            render_lobby_view(&mut egui_context, &mut networkstate, &mut gamestate);
        }
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

fn render_lobby_view(egui_context: &mut ResMut<EguiContext>, networkstate: &mut ResMut<network_plugin::NetworkState>, gamestate: &mut ResMut<gamestate::GameState>) {
    egui::CentralPanel::default().show(egui_context.ctx_mut(), |ui| {
        ui.heading("Connected!");
        render_chat_area(ui, networkstate, gamestate);

        ui.heading("Players");
        for player in &gamestate.players {
            ui.label(format!("{} - ready: {}",player.name, player.ready));
        }

        // render a button for ready or unready
        if let Some(net_info) = networkstate.info.as_mut() {
            let player_result = gamestate.players.iter().find(|player| player.id == net_info.id);
            if let Some(player) = player_result {
                if player.ready {
                    if ui.button("Not Ready").clicked() {
                        network_plugin::send_ready(networkstate, gamestate);
                    }
                } else {
                    if ui.button("Ready").clicked() {
                        network_plugin::send_ready(networkstate, gamestate);
                    }
                }
            }
        }
    });
}

fn render_ingame_view(egui_context: &mut ResMut<EguiContext>, networkstate: &mut ResMut<network_plugin::NetworkState>, gamestate: &mut ResMut<gamestate::GameState>) {
    egui::SidePanel::right("side_panel").show(egui_context.ctx_mut(), |ui| {
        render_chat_area(ui, networkstate, gamestate);

        if ui.button("Disconnect").clicked() {
            println!("Disconnect from server");
        }
    });
    egui::CentralPanel::default().show(egui_context.ctx_mut(), |ui| {
        // The central panel the region left after adding TopPanel's and SidePanel's
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut gamestate.stroke.width, 1.0..=10.0).text("width"));
            if ui.color_edit_button_srgba(&mut gamestate.stroke.color).clicked_elsewhere() {
            };
            if ui.button("Eraser").clicked() {
                gamestate.stroke.color = egui::Color32::from_rgb(255,255,255); 
            }
            /*if ui.button("Color").clicked() {
                *color = self.curr_stroke.color;
            }*/
            let (_id, stroke_rect) = ui.allocate_space(ui.spacing().interact_size);
            let left = stroke_rect.left_center();
            let right = stroke_rect.right_center();
            ui.painter().line_segment([left, right], gamestate.stroke);
            ui.separator();
            /*if ui.button("Clear Painting").clicked() {
                self.all_lines.clear();
            }*/
        }); 
        ui.label("Paint with your mouse/touch!");
    });
}

fn render_chat_area(ui: &mut egui::Ui, networkstate: &mut ResMut<network_plugin::NetworkState>, gamestate: &mut ResMut<gamestate::GameState>) {
    ui.heading("Chat");
    let text_style = egui::TextStyle::Body;
    let row_height = ui.text_style_height(&text_style);
    egui::ScrollArea::vertical().auto_shrink([false; 2]).stick_to_bottom().max_height(200.0).show_rows(
        ui,
        row_height,
        100,
        |ui, _| {
            for chat_message in gamestate.chat_messages.iter() {
                let search_player_result = gamestate.players.iter().find(|player| player.id == chat_message.player_id);
                if let None = search_player_result {

                } else {
                    let player = search_player_result.unwrap();
                    ui.label(format!("{}: {}",player.name, chat_message.message));
                    ui.set_min_width(100.0);
                }

            }
        },
    );
    ui.horizontal(|ui| {
        ui.label("Chat: ");
        ui.text_edit_singleline(&mut gamestate.chat_message_input);
        if ui.button("Send").clicked() || (ui.input().key_pressed(egui::Key::Enter) && !gamestate.chat_message_input.is_empty()) {
            network_plugin::send_chat_message(networkstate, gamestate);
        }

    });
}