// Verhindert im Release-Build ein zusätzliches Konsolenfenster unter Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    agentdashboard_lib::run()
}
