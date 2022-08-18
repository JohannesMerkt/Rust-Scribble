use std::thread::current;

use bevy::prelude::*;
use bevy::render::color;
use bevy_egui::{egui, EguiContext};
use egui::{Stroke, Color32, Rounding, RichText, Visuals};
use rayon::prelude::*;
use regex::Regex;

use crate::clientstate::ClientState;
use crate::network_plugin;
use rust_scribble_common::gamestate_common::*;

/// this system handles rendering the ui
///
/// # Arguments
/// * `egui_context` - The egui context used for rendering the egui
/// * `networkstate` - Holding information about the connection to a server
/// * `clientstate` - The state of the client holding information about the gamestate, canvas lines, chat messages and players in the game
///
pub fn render_ui(
    mut egui_context: ResMut<EguiContext>,
    mut networkstate: ResMut<network_plugin::NetworkState>,
    mut clientstate: ResMut<ClientState>,
) {
    if networkstate.info.is_none() {
        render_connect_view(&mut egui_context, &mut networkstate);
    } else if clientstate.game_state.in_game {
        render_ingame_view(&mut egui_context, &mut networkstate, &mut clientstate);
    } else {
        render_lobby_view(&mut egui_context, &mut networkstate, &mut clientstate);
    }
}

/// renders the view when connecting to a server
///
/// # Arguments
/// * `egui_context` - The egui context used for rendering the egui
/// * `networkstate` - Holding information about the connection to a server
///
fn render_connect_view(
    egui_context: &mut ResMut<EguiContext>,
    networkstate: &mut ResMut<network_plugin::NetworkState>,
) {
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

/// renders the view when connected to a server and in the lobby
///
/// # Arguments
/// * `egui_context` - The egui context used for rendering the egui
/// * `networkstate` - Holding information about the connection to a server
/// * `clientstate` - The state of the client holding information about the gamestate, canvas lines, chat messages and players in the game
///
fn render_lobby_view(
    egui_context: &mut ResMut<EguiContext>,
    networkstate: &mut ResMut<network_plugin::NetworkState>,
    clientstate: &mut ResMut<ClientState>,
) {
    egui::SidePanel::right("side_panel").show(egui_context.ctx_mut(), |ui| {
        render_game_time(ui, clientstate);
        render_player_list(ui, networkstate, clientstate);
        render_chat_area(ui, networkstate, clientstate);
    });

    egui::CentralPanel::default().show(egui_context.ctx_mut(), |ui| {
        ui.label(egui::RichText::new("Lobby").font(egui::FontId::proportional(40.0)));
        if let Some(net_info) = networkstate.info.as_mut() {
            let player_result = clientstate
                .players
                .iter()
                .find(|player| player.id == net_info.id);
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

/// renders the view when connected to a server and playing the game
///
/// # Arguments
/// * `egui_context` - The egui context used for rendering the egui
/// * `networkstate` - Holding information about the connection to a server
/// * `clientstate` - The state of the client holding information about the gamestate, canvas lines, chat messages and players in the game
///
fn render_ingame_view(
    egui_context: &mut ResMut<EguiContext>,
    networkstate: &mut ResMut<network_plugin::NetworkState>,
    clientstate: &mut ResMut<ClientState>,
) {
    egui::SidePanel::right("side_panel").show(egui_context.ctx_mut(), |ui| {
        render_game_time(ui, clientstate);
        render_player_list(ui, networkstate, clientstate);
        render_chat_area(ui, networkstate, clientstate);

        if ui.button("Disconnect").clicked() {
            network_plugin::send_disconnect(networkstate);
            //TODO change back to main screen
        }
    });

    let net_info = networkstate.info.as_ref().unwrap();
    //TODO FIX: This is dangerous at the moment Thread Panic!
    let is_drawer = clientstate
        .players
        .iter()
        .find(|player| player.id == net_info.id)
        .unwrap()
        .drawing;

    egui::CentralPanel::default().show(egui_context.ctx_mut(), |ui| {
        if is_drawer {
            ui.label("Paint the word with mouse/touch!".to_string());
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.horizontal(|ui| {
                let colors: Vec<Color32> = vec![Color32::YELLOW, Color32::from_rgb(255, 165, 0), Color32::RED, Color32::from_rgb(255, 192, 203), Color32::GREEN, Color32::BLUE, Color32::BROWN, Color32::BLACK];
                let color_chunks = colors.chunks(colors.len()/2);

                ui.vertical(|ui| {
                    for color_row in color_chunks {
                        ui.horizontal(|ui|{
                            for color in color_row {
                                ui.selectable_value(&mut clientstate.current_stroke.color, *color, RichText::new("🔴").color(*color));
                            }
                        });
                    }
                });

                ui.add(
                    egui::Slider::new(&mut clientstate.current_stroke.width, 1.0..=10.0)
                        .text("width"),
                );

                ui.selectable_value(&mut clientstate.current_stroke.color, Color32::WHITE, "Eraser");

               
                // Preview for color and width of stroke
                let (_id, stroke_rect) = ui.allocate_space(ui.spacing().interact_size);
                let center_pos = stroke_rect.center();
                // let right = stroke_rect.right_center();
                ui.painter()
                    .circle_filled(center_pos, clientstate.current_stroke.width, clientstate.current_stroke.color);

                ui.separator();
                ui.label(
                    egui::RichText::new(format!("Word: {}", clientstate.game_state.word))
                        .font(egui::FontId::proportional(40.0)),
                );
            });
        } else {
            ui.label("Guess the word!");
            ui.label(
                egui::RichText::new(format!(
                    "Word: {}",
                    get_word_as_underscores(&clientstate.game_state.word)
                ))
                .font(egui::FontId::proportional(40.0)),
            );
        }

        egui::Frame::canvas(ui.style()).show(ui, |ui| {
            let (mut response, painter) =
                ui.allocate_painter(ui.available_size_before_wrap(), egui::Sense::drag());
            painter.rect_filled(response.rect, Rounding::none(), Color32::WHITE);

            let to_screen = egui::emath::RectTransform::from_to(
                egui::Rect::from_min_size(egui::Pos2::ZERO, response.rect.square_proportions()),
                response.rect,
            );
            let from_screen = to_screen.inverse();

            if is_drawer {
                if response.drag_started() {
                    let new_line = Line {
                        positions: vec![],
                        stroke: clientstate.current_stroke,
                    };
                    clientstate.lines.push(new_line);
                };

                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    let current_line = clientstate.lines.last_mut().unwrap();
                    let canvas_pos = from_screen * pointer_pos;
                    if current_line.positions.last() != Some(&canvas_pos) {
                        current_line.positions.push(canvas_pos);
                        response.mark_changed();
                    }
                }
                if response.drag_released() {
                    let current_line = clientstate.lines.last_mut().unwrap();
                    network_plugin::send_line(networkstate, current_line);
                }
            }
            let mut shapes = vec![];
            for line in &clientstate.lines {
                if line.positions.len() >= 2 {
                    let points: Vec<egui::Pos2> =
                        line.positions.par_iter().map(|p| to_screen * *p).collect();
                    shapes.push(egui::Shape::line(points, line.stroke));
                }
            }
            painter.extend(shapes);
            response
        });
    });
}

/// renders a chat area with chat history and message input
///
/// # Arguments
/// * `ui` - The current UI context to draw the chat area on
/// * `networkstate` - Holding information about the connection to a server
/// * `clientstate` - The state of the client holding information about the gamestate, canvas lines, chat messages and players in the game
///
fn render_chat_area(
    ui: &mut egui::Ui,
    networkstate: &mut ResMut<network_plugin::NetworkState>,
    clientstate: &mut ResMut<ClientState>,
) {
    ui.group(|ui| {
        ui.heading("Chat");
        let text_style = egui::TextStyle::Body;
        let row_height = ui.text_style_height(&text_style);
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom()
            .max_height(200.0)
            .show_rows(ui, row_height, 100, |ui, _| {
                for chat_message in clientstate.chat_messages.iter() {
                    let search_player_result = clientstate
                        .players
                        .par_iter()
                        .find_any(|player| player.id == chat_message.id);
                    if let Some(player) = search_player_result {
                        ui.label(format!("{}: {}", player.name, chat_message.message));
                        ui.set_min_width(100.0);
                    }
                }
            });
        ui.horizontal(|ui| {
            ui.label("Chat: ");
            ui.text_edit_singleline(&mut clientstate.chat_message_input);
            if ui.button("Send").clicked()
                || (ui.input().key_pressed(egui::Key::Enter)
                    && !clientstate.chat_message_input.is_empty())
            {
                network_plugin::send_chat_message(
                    networkstate,
                    clientstate.chat_message_input.clone(),
                );
                clientstate.chat_message_input.clear();
            }
        });
    });
}

/// renders a game time area
///
/// # Arguments
/// * `ui` - The current UI context to draw the chat area on
/// * `clientstate` - The state of the client holding information about the gamestate, canvas lines, chat messages and players in the game
///
fn render_game_time(ui: &mut egui::Ui, clientstate: &mut ResMut<ClientState>) {
    ui.group(|ui| {
        ui.label(format!("Time: {}s", clientstate.game_state.time));
    });
}

/// renders a player list area
///
/// # Arguments
/// * `ui` - The current UI context to draw the chat area on
/// * `networkstate` - Holding information about the connection to a server
/// * `clientstate` - The state of the client holding information about the gamestate, canvas lines, chat messages and players in the game
///
fn render_player_list(
    ui: &mut egui::Ui,
    networkstate: &mut ResMut<network_plugin::NetworkState>,
    clientstate: &mut ResMut<ClientState>,
) {
    ui.group(|ui| {
        let mut playing_count = 0;
        let mut lobby_count = 0;
        for player in &clientstate.players {
            if player.playing {
                playing_count += 1;
            } else {
                lobby_count += 1;
            }
        }
        if playing_count > 0 {
            ui.heading("Playing");
            ui.columns(3, |cols| {
                cols[0].label("Name");
                cols[1].label("Status");
                cols[2].label("Score");
            });
            ui.separator();
            for player in &clientstate.players {
                if player.playing {
                    ui.columns(3, |cols| {
                        cols[0].label(get_player_name_with_you(networkstate, player));
                        let mut player_status = "❓";
                        if player.drawing {
                            player_status = "✏";
                        } else if player.guessed_word {
                            player_status = "✔";
                        }
                        cols[1].label(player_status);
                        cols[2].label(player.score.to_string());
                    });
                }
            }
        }
        if lobby_count > 0 {
            ui.heading("Waiting in Lobby");
            ui.columns(3, |cols| {
                cols[0].label("Name");
                cols[1].label("Ready");
                cols[2].label("Score");
            });
            ui.separator();
            for player in &clientstate.players {
                if !player.playing {
                    ui.columns(3, |cols| {
                        cols[0].label(get_player_name_with_you(networkstate, player));
                        let mut ready_state = "✖";
                        if player.ready {
                            ready_state = "✔";
                        }
                        cols[1].label(ready_state);
                        cols[2].label(player.score.to_string());
                    });
                }
            }
        }
    });
}

/// returns the player name as a string and in case its the player name of the client adds (You) to the end
///
/// # Arguments
/// * `networkstate` - Holding information about the connection to a server
/// * `player` - The player to render the name for
///
fn get_player_name_with_you(
    networkstate: &mut ResMut<network_plugin::NetworkState>,
    player: &Player,
) -> std::string::String {
    let net_info = networkstate.info.as_ref().unwrap();
    if net_info.id == player.id {
        return format!("{} (You)", player.name);
    }
    player.name.to_string()
}

/// returns a word with all letters replaced for underscores
///
/// # Arguments
/// * `word` - The word to render as underscores
///
fn get_word_as_underscores(word: &std::string::String) -> std::string::String {
    let re = Regex::new(r"[A-Za-z]").unwrap();
    re.replace_all(&word.to_string(), " _ ").to_string()
}
