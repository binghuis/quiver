use crate::skills;
use std::fs;
use std::path::{Path, PathBuf};

fn home_dir() -> Result<PathBuf, String> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| "HOME env var not set".into())
}

fn move_to_system_trash(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|e| format!("move to system trash: {}", e))
}

fn installed_plugins_path() -> Result<PathBuf, String> {
    Ok(home_dir()?
        .join(".claude")
        .join("plugins")
        .join("installed_plugins.json"))
}

fn read_installed_plugins() -> serde_json::Value {
    let Ok(path) = installed_plugins_path() else {
        return serde_json::json!({ "version": 2, "plugins": {} });
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return serde_json::json!({ "version": 2, "plugins": {} });
    };
    serde_json::from_str(&raw)
        .unwrap_or_else(|_| serde_json::json!({ "version": 2, "plugins": {} }))
}

fn write_installed_plugins(v: &serde_json::Value) -> Result<(), String> {
    let path = installed_plugins_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

fn remove_installed_entries_by_marketplace(marketplace: &str) -> Result<(), String> {
    let mut data = read_installed_plugins();
    let suffix = format!("@{}", marketplace);
    if let Some(plugins) = data.get_mut("plugins").and_then(|v| v.as_object_mut()) {
        let keys: Vec<String> = plugins
            .keys()
            .filter(|k| k.ends_with(&suffix))
            .cloned()
            .collect();
        for k in keys {
            plugins.remove(&k);
        }
    }
    write_installed_plugins(&data)
}

#[tauri::command]
pub async fn delete_marketplace(name: String) -> Result<(), String> {
    let home = home_dir()?;
    let cache_path = home.join(".claude").join("plugins").join("cache").join(&name);
    let marketplace_path = home
        .join(".claude")
        .join("plugins")
        .join("marketplaces")
        .join(&name);

    let source_path = if marketplace_path.exists() {
        marketplace_path.clone()
    } else if cache_path.exists() {
        cache_path.clone()
    } else {
        return Err(format!("marketplace not found: {}", name));
    };

    // 先把 marketplace 源丢进系统垃圾桶；成功后再改 installed_plugins.json，
    // 避免「配置已改但文件没动」的半截状态。
    move_to_system_trash(&source_path)?;
    remove_installed_entries_by_marketplace(&name)?;

    // cache/ 是 Claude 按需 materialize 的派生物，可由 CLI 再生，不进垃圾桶。
    if cache_path.exists() && cache_path != source_path {
        let _ = fs::remove_dir_all(&cache_path);
    }

    Ok(())
}

/// Delete a single ecosystem presence of a skill (one copy on disk). Restricted to
/// user/project scope — plugin-scope skills are owned by their plugin bundle and
/// cannot be removed individually.
#[tauri::command]
pub async fn delete_skill_presence(
    skill_id: String,
    project_dir: Option<String>,
) -> Result<(), String> {
    let skill = skills::find_skill_by_id(&skill_id, project_dir.as_deref())
        .ok_or_else(|| format!("skill not found: {}", skill_id))?;

    if matches!(skill.scope, skills::Scope::Plugin) {
        return Err("插件自带的 skill 不能单独删除".into());
    }

    let skill_md = PathBuf::from(&skill.path);
    let dir = skill_md
        .parent()
        .ok_or_else(|| "skill path has no parent".to_string())?
        .to_path_buf();

    move_to_system_trash(&dir)?;
    skills::remove_disabled_entry(&skill_id);
    Ok(())
}
