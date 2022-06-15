#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
mod app;
pub use app::TemplateApp;
mod painting;
pub use painting::Painting;
mod network;

fn main() {
    tracing_subscriber::fmt::init();

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Scribble",
        native_options,
        Box::new(|_| Box::new(TemplateApp::new())),
    );
}