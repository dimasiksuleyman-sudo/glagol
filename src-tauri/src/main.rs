// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(code) = glagol_lib::moonshine_worker_entry() {
        std::process::exit(code);
    }
    glagol_lib::run()
}
