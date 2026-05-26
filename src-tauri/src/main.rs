#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri_appuniversal_launcher_lib::run()
}