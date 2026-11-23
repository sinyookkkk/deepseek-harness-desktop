#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod job;
mod logging;
mod paths;
mod runtime;
mod sidecar;

fn main() {
    app::run();
}
