// Windows: sin consola en release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    gamelibrarymanager_lib::run()
}
