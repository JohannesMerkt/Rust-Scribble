#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

fn main() {
    tracing_subscriber::fmt::init();

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Scribble",
        native_options,
        Box::new(|cc| Box::new(rust_scribble::TemplateApp::new())),
    );
}