#![crate_name = "rust_scribble"]
mod app;
pub use app::TemplateApp;
mod painting;
pub use painting::Painting;
mod network;