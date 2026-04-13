// SPDX-License-Identifier: BUSL-1.1
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    postlane_desktop_lib::run()
}
