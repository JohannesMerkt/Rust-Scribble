#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
mod clientstate;
mod network;
mod network_plugin;
mod ui;

use bevy::{prelude::*, window::WindowResizeConstraints};
use bevy_egui::{EguiContext, EguiPlugin};
use egui::{
    style::{WidgetVisuals, Widgets},
    Color32, Rounding, Stroke, TextureId, Visuals,
};

fn main() {
    App::new()
        .insert_resource(WindowDescriptor {
            title: "Scribble".to_string(),
            resize_constraints: WindowResizeConstraints {
                min_width: 1200.0,
                min_height: 600.0,
                ..default()
            },
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .add_plugin(clientstate::ClientStatePlugin)
        .add_plugin(network_plugin::NetworkPlugin)
        .add_startup_system(configure_visuals)
        .add_startup_system(load_images)
        // Systems that create Egui widgets should be run during the `CoreStage::Update` stage,
        // or after the `EguiSystem::BeginFrame` system (which belongs to the `CoreStage::PreUpdate` stage).
        .add_system(ui::render_ui)
        .run();
}

pub struct Textures {
    crab: TextureId,
}

// To display an image, convert it to a texture
// should not be called in main GUI code
fn load_images(
    mut commands: Commands,
    asset_server: ResMut<AssetServer>,
    mut egui_context: ResMut<EguiContext>,
) {
    let crab_handle: Handle<Image> = asset_server.load("rustacean-flat-happy.png"); // pointer to image
    let textures = Textures {
        crab: egui_context.add_image(crab_handle),
    };
    commands.insert_resource(textures);
}

fn configure_visuals(mut egui_context: ResMut<EguiContext>) {
    let visuals = Visuals {
        dark_mode: false,
        widgets: Widgets {
            noninteractive: WidgetVisuals {
                bg_fill: Color32::LIGHT_BLUE, // change background colour to light blue
                bg_stroke: Stroke::new(1.0, Color32::from_gray(190)), // separators, indentation lines, windows outlines
                fg_stroke: Stroke::new(1.0, Color32::from_gray(80)),  // normal text color
                rounding: Rounding::same(2.0),
                expansion: 0.0,
            },
            ..Widgets::light()
        },
        ..Visuals::light()
    };
    egui_context.ctx_mut().set_visuals(visuals);
}
