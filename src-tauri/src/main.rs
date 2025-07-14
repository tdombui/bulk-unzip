// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod lib;

use lib::*;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            unzip_files,
            strip_metadata,
            scan_zip_files,
            scan_mp3_files
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
} 