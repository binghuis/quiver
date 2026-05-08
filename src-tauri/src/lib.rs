mod deletion;
mod env_bootstrap;
mod skills;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 先注入 shell env（PATH/proxy）再让 Tauri 起来。GUI 进程默认拿不到用户在
    // ~/.zshrc 里配的 brew PATH 和 VPN proxy 环境变量，git 子进程会因此连不上 GitHub。
    env_bootstrap::inherit_shell_env();

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
            skills::get_git_proxy,
            skills::set_git_proxy,
            deletion::delete_marketplace,
            deletion::delete_skill_presence,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
