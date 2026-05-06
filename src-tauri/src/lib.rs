mod deletion;
mod skills;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            skills::list_skills,
            skills::toggle_skills,
            skills::toggle_plugin,
            skills::import_from_github,
            skills::reveal_in_finder,
            skills::sync_skill_to_ecosystem,
            skills::refresh_claude_marketplace,
            skills::marketplace_path,
            deletion::delete_marketplace,
            deletion::delete_skill_presence,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
