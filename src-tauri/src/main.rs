// Evita que en Windows (release) se abra una consola adicional.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    dictamelo_lib::run();
}
