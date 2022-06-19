use bevy::{prelude::*};
use bevy_egui::{egui, EguiContext};
use crate::network_plugin;

/// this system handles rendering the ui
pub fn render_ui(mut egui_context: ResMut<EguiContext>, mut networkstate: ResMut<network_plugin::NetworkState>) {
    if networkstate.connected {
        render_lobby_view(egui_context);
    } else {
        render_connect_view(egui_context, networkstate);
    }
    
}

fn render_connect_view(mut egui_context: ResMut<EguiContext>, mut networkstate: ResMut<network_plugin::NetworkState>) {
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
            println!("Connect to {} at port {}", networkstate.address, networkstate.port);
            network_plugin::connect(networkstate);
        }
    });
}

fn render_lobby_view(mut egui_context: ResMut<EguiContext>) {
    egui::CentralPanel::default().show(egui_context.ctx_mut(), |ui| {
        ui.heading("Connected!");
    });
}