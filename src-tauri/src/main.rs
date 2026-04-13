// SPDX-License-Identifier: BUSL-1.1
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Initialize ~/.postlane directory as first operation
    postlane_desktop_lib::init::init_postlane_dir()
        .expect("Failed to initialize ~/.postlane directory");

    postlane_desktop_lib::run()
}
