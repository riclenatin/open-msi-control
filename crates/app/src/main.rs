#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    open_msi_control::run();
}
