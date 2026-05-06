use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::SystemTime;

static STATE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    Claude,
    Codex,
    Gemini,
}

impl Ecosystem {
    fn tag(self) -> &'static str {
        match self {
            Ecosystem::Claude => "claude",
            Ecosystem::Codex => "codex",
            Ecosystem::Gemini => "gemini",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    User,
    Project,
    Plugin,
}

#[derive(Debug, Clone, Serialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub ecosystem: Ecosystem,
    pub scope: Scope,
    /// 文件级 enabled——SKILL.md 是不是 .disabled。前端展示用。
    pub enabled: bool,
    pub plugin: Option<String>,
    pub path: String,
    pub body: String,
    pub content_hash: String,
    /// 仅对 Claude plugin scope 有意义：宿主插件在 settings.json `enabledPlugins`
    /// 里是不是 true。false 时 Claude 整个插件不加载——SKILL.md 文件级 enabled
    /// 完全不起作用，UI 应当锁住单个 skill 的 toggle。
    /// user / project / 其他生态：永远 true。
    pub plugin_enabled: bool,
    /// 来自 SKILL.md frontmatter 的 `disable-model-invocation: true`。
    /// 模型不会自己用，只能 `/skill-name` 手动调起。UI 上要标。
    pub disable_model_invocation: bool,
}

fn home_dir() -> Result<PathBuf, String> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| "HOME env var not set".into())
}

fn quiver_state_path() -> Result<PathBuf, String> {
    Ok(home_dir()?.join(".claude").join("quiver").join("state.json"))
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    #[serde(default)]
    disabled: HashSet<String>,
}

fn read_state() -> State {
    let Ok(path) = quiver_state_path() else {
        return State::default();
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return State::default();
    };
    let mut state: State = serde_json::from_str(&raw).unwrap_or_default();
    // Migrate pre-ecosystem ids ("user:foo", "project:bar", "plugin:...:baz")
    // to the Claude-scoped form. Silently rewrite to disk so toggle state survives.
    let mut migrated = false;
    let legacy_prefixes = ["user:", "project:", "plugin:"];
    let new_set: HashSet<String> = state
        .disabled
        .drain()
        .map(|id| {
            if legacy_prefixes.iter().any(|p| id.starts_with(p)) {
                migrated = true;
                format!("claude:{}", id)
            } else {
                id
            }
        })
        .collect();
    state.disabled = new_set;
    if migrated {
        let _ = write_state(&state);
    }
    state
}

fn write_state(state: &State) -> Result<(), String> {
    let path = quiver_state_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

struct Frontmatter {
    name: Option<String>,
    description: Option<String>,
    disable_model_invocation: bool,
    body: String,
}

fn parse_frontmatter(content: &str) -> Frontmatter {
    let Some(rest) = content.strip_prefix("---\n").or_else(|| content.strip_prefix("---\r\n"))
    else {
        return Frontmatter {
            name: None,
            description: None,
            disable_model_invocation: false,
            body: content.to_string(),
        };
    };
    let Some(end) = rest.find("\n---").map(|i| (i, 4)).or_else(|| rest.find("\r\n---").map(|i| (i, 5)))
    else {
        return Frontmatter {
            name: None,
            description: None,
            disable_model_invocation: false,
            body: content.to_string(),
        };
    };
    let (idx, marker_len) = end;
    let yaml = &rest[..idx];
    let body_start = idx + marker_len;
    let body = rest[body_start..]
        .trim_start_matches('\n')
        .trim_start_matches('\r')
        .to_string();

    let mut name = None;
    let mut description = None;
    let mut disable_model_invocation = false;
    let mut current_key: Option<String> = None;
    let mut buf = String::new();

    let flush = |key: &Option<String>,
                 buf: &str,
                 name: &mut Option<String>,
                 description: &mut Option<String>,
                 disable_model_invocation: &mut bool| {
        let Some(k) = key else { return };
        let v = buf.trim().trim_matches('"').trim_matches('\'').to_string();
        match k.as_str() {
            "name" => *name = Some(v),
            "description" => *description = Some(v),
            "disable-model-invocation" => {
                // YAML 1.2 严格只认 true/false，但容忍习惯写法（True/yes/on）。
                // 解析失败一律按 false——不能把没禁用的 skill 误标成禁用。
                let lower = v.to_lowercase();
                *disable_model_invocation =
                    matches!(lower.as_str(), "true" | "yes" | "on" | "1");
            }
            _ => {}
        }
    };

    for line in yaml.lines() {
        if let Some((k, v)) = split_yaml_line(line) {
            flush(
                &current_key,
                &buf,
                &mut name,
                &mut description,
                &mut disable_model_invocation,
            );
            current_key = Some(k);
            buf.clear();
            buf.push_str(v.trim());
        } else {
            if current_key.is_some() {
                if !buf.is_empty() {
                    buf.push(' ');
                }
                buf.push_str(line.trim());
            }
        }
    }
    flush(
        &current_key,
        &buf,
        &mut name,
        &mut description,
        &mut disable_model_invocation,
    );

    Frontmatter {
        name,
        description,
        disable_model_invocation,
        body,
    }
}

fn split_yaml_line(line: &str) -> Option<(String, &str)> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || trimmed.is_empty() {
        return None;
    }
    let colon = line.find(':')?;
    let key = line[..colon].trim();
    if key.is_empty() || key.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    let value = &line[colon + 1..];
    Some((key.to_string(), value))
}

fn is_disabled_marker(name: &str) -> bool {
    name.ends_with(".disabled")
}

// FNV-1a 64-bit — stable across Rust versions, no external deps.
// Collisions on legitimately different SKILL.md content are astronomically unlikely.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn content_hash_hex(content: &str) -> String {
    format!("{:016x}", fnv1a_64(content.as_bytes()))
}

fn id_for(ecosystem: Ecosystem, scope: &Scope, plugin: &Option<String>, dir_name: &str) -> String {
    let eco = ecosystem.tag();
    match (scope, plugin) {
        (Scope::User, _) => format!("{}:user:{}", eco, dir_name),
        (Scope::Project, _) => format!("{}:project:{}", eco, dir_name),
        (Scope::Plugin, Some(p)) => format!("{}:plugin:{}:{}", eco, p, dir_name),
        (Scope::Plugin, None) => format!("{}:plugin::{}", eco, dir_name),
    }
}

fn load_skill_from_dir(
    dir: &Path,
    ecosystem: Ecosystem,
    scope: Scope,
    plugin: Option<String>,
    plugin_enabled: bool,
    state: &State,
) -> Option<Skill> {
    let dir_name = dir.file_name()?.to_string_lossy().to_string();
    let id = id_for(ecosystem, &scope, &plugin, &dir_name);

    let skill_md = dir.join("SKILL.md");
    let disabled_md = dir.join("SKILL.md.disabled");

    // 自愈：state 记得这个 skill 该禁用但磁盘上只剩 SKILL.md（典型场景是插件
    // cache 被 refresh 清掉后 Claude 重新 materialize 了一份全新的 SKILL.md），
    // 把后缀改回 .disabled，让 Claude/Codex/Gemini 的加载器继续看不到它。两个
    // 文件同时存在时不动，避免任何一边的内容被覆盖。
    if state.disabled.contains(&id) && skill_md.is_file() && !disabled_md.exists() {
        let _ = fs::rename(&skill_md, &disabled_md);
    }

    let (path, file_disabled) = if skill_md.is_file() {
        (skill_md, false)
    } else if disabled_md.is_file() {
        (disabled_md, true)
    } else {
        return None;
    };

    let content = fs::read_to_string(&path).ok()?;
    let hash = content_hash_hex(&content);
    let fm = parse_frontmatter(&content);
    let name = fm.name.unwrap_or_else(|| dir_name.clone());
    let description = fm.description.unwrap_or_default();

    let enabled = !file_disabled && !state.disabled.contains(&id);

    Some(Skill {
        id,
        name,
        description,
        ecosystem,
        scope,
        enabled,
        plugin,
        path: path.to_string_lossy().to_string(),
        body: fm.body,
        content_hash: hash,
        plugin_enabled,
        disable_model_invocation: fm.disable_model_invocation,
    })
}

fn scan_skills_root(
    root: &Path,
    ecosystem: Ecosystem,
    scope: Scope,
    plugin: Option<String>,
    plugin_enabled: bool,
    state: &State,
) -> Vec<Skill> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if is_disabled_marker(&name) || name.starts_with('.') {
            continue;
        }
        if let Some(skill) = load_skill_from_dir(
            &entry.path(),
            ecosystem,
            scope.clone(),
            plugin.clone(),
            plugin_enabled,
            state,
        ) {
            out.push(skill);
        }
    }
    out
}

/// Pick the "latest" version subdir inside a plugin cache entry. Claude's cache
/// directory names aren't guaranteed to be semver (could be commit shas, dates,
/// etc.), so we rank by semver when parseable and fall back to filesystem mtime.
/// `Option<Version>` sorts with `None < Some`, so a parseable semver always beats
/// an unparseable sibling.
fn pick_latest_version(entries: Vec<fs::DirEntry>) -> Option<fs::DirEntry> {
    entries
        .into_iter()
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let ver =
                Version::parse(name.strip_prefix('v').unwrap_or(&name)).ok();
            let mtime = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            (ver, mtime, e)
        })
        .max_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, _, e)| e)
}

/// 读 `~/.claude/settings.json` 的 `enabledPlugins`。Claude Code 把缺失的 key
/// 当作 false（plugin list 里直接显示 disabled），所以这里也按缺失 = false 返回。
fn read_enabled_plugins() -> std::collections::HashMap<String, bool> {
    let mut out = std::collections::HashMap::new();
    let Ok(home) = home_dir() else { return out };
    let path = home.join(".claude").join("settings.json");
    let Ok(raw) = fs::read_to_string(&path) else { return out };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else { return out };
    let Some(obj) = json.get("enabledPlugins").and_then(|v| v.as_object()) else {
        return out;
    };
    for (k, v) in obj {
        if let Some(b) = v.as_bool() {
            out.insert(k.clone(), b);
        }
    }
    out
}

fn scan_claude_plugin_skills(
    state: &State,
    enabled_plugins: &std::collections::HashMap<String, bool>,
) -> Vec<Skill> {
    // ~/.claude/plugins/cache/<marketplace>/<plugin>/<version>/skills/*
    let Ok(home) = home_dir() else {
        return Vec::new();
    };
    let cache = home.join(".claude").join("plugins").join("cache");
    let Ok(marketplaces) = fs::read_dir(&cache) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for mp in marketplaces.flatten() {
        let mp_name = mp.file_name().to_string_lossy().to_string();
        let Ok(plugins) = fs::read_dir(mp.path()) else {
            continue;
        };
        for pl in plugins.flatten() {
            let pl_name = pl.file_name().to_string_lossy().to_string();
            let Ok(versions) = fs::read_dir(pl.path()) else {
                continue;
            };
            let version_dirs: Vec<fs::DirEntry> = versions
                .flatten()
                .filter(|v| v.path().is_dir())
                .collect();
            let Some(latest) = pick_latest_version(version_dirs) else {
                continue;
            };
            let skills_dir = latest.path().join("skills");
            if skills_dir.is_dir() {
                let plugin_label = format!("{}@{}", pl_name, mp_name);
                let plugin_enabled = enabled_plugins
                    .get(&plugin_label)
                    .copied()
                    .unwrap_or(false);
                out.extend(scan_skills_root(
                    &skills_dir,
                    Ecosystem::Claude,
                    Scope::Plugin,
                    Some(plugin_label),
                    plugin_enabled,
                    state,
                ));
            }
        }
    }
    out
}

fn scan_codex_skills(project_dir: Option<&str>, state: &State) -> Vec<Skill> {
    // ~/.codex/skills/ and ~/.agents/skills/ are both valid user-scope roots per
    // OpenAI docs. Canonicalize to catch symlinks; also dedupe by dir name since
    // a user may legitimately have both paths populated with the same skill.
    let Ok(home) = home_dir() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut seen_roots: HashSet<PathBuf> = HashSet::new();
    let mut seen_user_names: HashSet<String> = HashSet::new();

    for root in [
        home.join(".codex").join("skills"),
        home.join(".agents").join("skills"),
    ] {
        if !root.is_dir() {
            continue;
        }
        let canonical = fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
        if !seen_roots.insert(canonical) {
            continue;
        }
        let Ok(entries) = fs::read_dir(&root) else { continue };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if is_disabled_marker(&name) || name.starts_with('.') {
                continue;
            }
            if !seen_user_names.insert(name.clone()) {
                continue;
            }
            if let Some(skill) = load_skill_from_dir(
                &entry.path(),
                Ecosystem::Codex,
                Scope::User,
                None,
                true,
                state,
            ) {
                out.push(skill);
            }
        }
    }

    if let Some(project) = project_dir.filter(|s| !s.is_empty()) {
        out.extend(scan_skills_root(
            &PathBuf::from(project).join(".agents").join("skills"),
            Ecosystem::Codex,
            Scope::Project,
            None,
            true,
            state,
        ));
    }

    out
}

fn scan_gemini_skills(project_dir: Option<&str>, state: &State) -> Vec<Skill> {
    let Ok(home) = home_dir() else {
        return Vec::new();
    };
    let mut out = Vec::new();

    // ~/.gemini/skills/<name>/SKILL.md  — user-scope, no extension wrapper.
    out.extend(scan_skills_root(
        &home.join(".gemini").join("skills"),
        Ecosystem::Gemini,
        Scope::User,
        None,
        true,
        state,
    ));

    // ~/.gemini/extensions/<ext>/skills/<name>/SKILL.md  — each extension = plugin.
    // Gemini extension 没有 Claude 那种 `enabledPlugins` 开关，扫到即视为加载，
    // plugin_enabled 一律 true。
    let extensions_root = home.join(".gemini").join("extensions");
    if let Ok(exts) = fs::read_dir(&extensions_root) {
        for ext in exts.flatten() {
            let Ok(ft) = ext.file_type() else { continue };
            if !ft.is_dir() {
                continue;
            }
            let ext_name = ext.file_name().to_string_lossy().to_string();
            if ext_name.starts_with('.') {
                continue;
            }
            let skills_dir = ext.path().join("skills");
            if skills_dir.is_dir() {
                out.extend(scan_skills_root(
                    &skills_dir,
                    Ecosystem::Gemini,
                    Scope::Plugin,
                    Some(ext_name),
                    true,
                    state,
                ));
            }
        }
    }

    if let Some(project) = project_dir.filter(|s| !s.is_empty()) {
        let pp = PathBuf::from(project).join(".gemini").join("skills");
        out.extend(scan_skills_root(
            &pp,
            Ecosystem::Gemini,
            Scope::Project,
            None,
            true,
            state,
        ));
    }

    out
}

fn list_skills_internal(project_dir: Option<&str>, state: &State) -> Vec<Skill> {
    let Ok(home) = home_dir() else {
        return Vec::new();
    };
    let enabled_plugins = read_enabled_plugins();
    let mut skills = Vec::new();
    skills.extend(scan_skills_root(
        &home.join(".claude").join("skills"),
        Ecosystem::Claude,
        Scope::User,
        None,
        true,
        state,
    ));
    if let Some(project) = project_dir.filter(|s| !s.is_empty()) {
        let pp = PathBuf::from(project).join(".claude").join("skills");
        skills.extend(scan_skills_root(
            &pp,
            Ecosystem::Claude,
            Scope::Project,
            None,
            true,
            state,
        ));
    }
    skills.extend(scan_claude_plugin_skills(state, &enabled_plugins));
    // Also scan raw marketplace sources — Quiver-installed marketplaces that
    // Claude hasn't yet materialized into cache still need to show up in the
    // list so users can toggle plugins immediately. Dedupe against cache by id.
    let seen: HashSet<String> = skills.iter().map(|s| s.id.clone()).collect();
    for s in scan_claude_marketplace_plugins(state, &enabled_plugins) {
        if !seen.contains(&s.id) {
            skills.push(s);
        }
    }
    skills.extend(scan_codex_skills(project_dir, state));
    skills.extend(scan_gemini_skills(project_dir, state));
    skills
}

/// Scan `~/.claude/plugins/marketplaces/<mp>/<plugin>/skills/*`. This covers the
/// case where a marketplace has been cloned but Claude hasn't run to materialize
/// the cache yet. Skills found here are labeled `<plugin>@<mp>` just like cache
/// entries, so the rest of the UI treats them identically.
fn scan_claude_marketplace_plugins(
    state: &State,
    enabled_plugins: &std::collections::HashMap<String, bool>,
) -> Vec<Skill> {
    let Ok(home) = home_dir() else {
        return Vec::new();
    };
    let root = home.join(".claude").join("plugins").join("marketplaces");
    let Ok(mps) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for mp in mps.flatten() {
        let Ok(ft) = mp.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let mp_name = mp.file_name().to_string_lossy().to_string();
        if mp_name.starts_with('.') {
            continue;
        }
        let Ok(plugins) = fs::read_dir(mp.path()) else {
            continue;
        };
        for pl in plugins.flatten() {
            let Ok(ft) = pl.file_type() else { continue };
            if !ft.is_dir() {
                continue;
            }
            let pl_name = pl.file_name().to_string_lossy().to_string();
            if pl_name.starts_with('.') {
                continue;
            }
            let skills_dir = pl.path().join("skills");
            if skills_dir.is_dir() {
                let plugin_label = format!("{}@{}", pl_name, mp_name);
                let plugin_enabled = enabled_plugins
                    .get(&plugin_label)
                    .copied()
                    .unwrap_or(false);
                out.extend(scan_skills_root(
                    &skills_dir,
                    Ecosystem::Claude,
                    Scope::Plugin,
                    Some(plugin_label),
                    plugin_enabled,
                    state,
                ));
            }
        }
    }
    out
}

#[tauri::command]
pub fn list_skills(project_dir: Option<String>) -> Result<Vec<Skill>, String> {
    let _guard = STATE_LOCK.lock().map_err(|e| e.to_string())?;
    // Quiver 的产品契约：「在这个 App 里管的插件 = 全局可用」。Claude Code 有
    // 两道闸门，都要打开：
    //   1. installed_plugins.json 里有 scope:user 项（决定能不能识别）
    //   2. settings.json 的 enabledPlugins[id]=true（决定要不要加载）
    // 两道分别处理，失败静默不影响主流程。
    let promoted_ids = ensure_user_scope_for_marketplace_plugins();
    enable_plugins_in_user_settings(&promoted_ids);
    let state = read_state();
    let mut skills = list_skills_internal(project_dir.as_deref(), &state);
    skills.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(skills)
}

/// 扫 `~/.claude/plugins/marketplaces/<mp>/<plugin>/`，返回所有 plugin_id
/// 形如 `<plugin>@<mp>`，仅包含「长得像插件」的目录（含 skills/commands/
/// hooks/agents 任一子目录）。
fn discover_marketplace_plugin_ids() -> Vec<(String, PathBuf, String, String)> {
    let Ok(home) = home_dir() else { return Vec::new() };
    let marketplaces_root = home.join(".claude").join("plugins").join("marketplaces");
    let Ok(mps) = fs::read_dir(&marketplaces_root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for mp in mps.flatten() {
        let Ok(ft) = mp.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let mp_path = mp.path();
        let mp_name = mp.file_name().to_string_lossy().to_string();
        if mp_name.starts_with('.') {
            continue;
        }
        let Ok(plugins) = fs::read_dir(&mp_path) else { continue };
        for pl in plugins.flatten() {
            let Ok(ft) = pl.file_type() else { continue };
            if !ft.is_dir() {
                continue;
            }
            let pl_path = pl.path();
            let pl_name = pl.file_name().to_string_lossy().to_string();
            if pl_name.starts_with('.') {
                continue;
            }
            let looks_like_plugin = ["skills", "commands", "hooks", "agents"]
                .iter()
                .any(|sub| pl_path.join(sub).is_dir());
            if !looks_like_plugin {
                continue;
            }
            let plugin_id = format!("{}@{}", pl_name, mp_name);
            out.push((plugin_id, mp_path.clone(), mp_name.clone(), pl_name));
        }
    }
    out
}

/// 确保每个 marketplace 插件在 `installed_plugins.json` 里有 `scope: "user"`
/// 项。已存在则跳过，没有就：
///   1. 优先克隆同插件的任一 project 项（继承 installPath/version/sha），
///      改 scope=user 并删 projectPath
///   2. 没有任何项时按 cache 最新版本目录 / marketplace 源路径合成
fn ensure_user_scope_for_marketplace_plugins() -> Vec<String> {
    let Ok(home) = home_dir() else { return Vec::new() };
    let installed_path = home
        .join(".claude")
        .join("plugins")
        .join("installed_plugins.json");
    let cache_root = home.join(".claude").join("plugins").join("cache");

    let plugins = discover_marketplace_plugin_ids();
    let plugin_ids: Vec<String> = plugins.iter().map(|(id, _, _, _)| id.clone()).collect();
    if plugins.is_empty() {
        return plugin_ids;
    }

    let raw = fs::read_to_string(&installed_path)
        .unwrap_or_else(|_| r#"{"version":2,"plugins":{}}"#.to_string());
    let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return plugin_ids;
    };
    if !json.is_object() {
        return plugin_ids;
    }
    if !json
        .get("plugins")
        .map(|v| v.is_object())
        .unwrap_or(false)
    {
        json["plugins"] = serde_json::json!({});
    }
    if !json
        .get("version")
        .map(|v| v.is_number())
        .unwrap_or(false)
    {
        json["version"] = serde_json::json!(2);
    }

    let mut changed = false;
    for (plugin_id, mp_path, mp_name, pl_name) in &plugins {
        let existing = json["plugins"].get(plugin_id).and_then(|v| v.as_array()).cloned();
        let has_user = existing
            .as_ref()
            .map(|arr| {
                arr.iter()
                    .any(|e| e.get("scope") == Some(&serde_json::json!("user")))
            })
            .unwrap_or(false);
        if has_user {
            continue;
        }

        let user_entry = existing
            .as_ref()
            .and_then(|arr| {
                arr.iter()
                    .find(|e| e.get("scope") == Some(&serde_json::json!("project")))
                    .cloned()
            })
            .map(|mut proj| {
                if let Some(obj) = proj.as_object_mut() {
                    obj.insert("scope".into(), serde_json::json!("user"));
                    obj.remove("projectPath");
                }
                proj
            })
            .unwrap_or_else(|| synthesize_user_entry(&cache_root, mp_name, pl_name, mp_path));

        let plugins_obj = json["plugins"].as_object_mut().unwrap();
        plugins_obj
            .entry(plugin_id.clone())
            .or_insert_with(|| serde_json::Value::Array(vec![]))
            .as_array_mut()
            .unwrap()
            .push(user_entry);
        changed = true;
    }

    if changed {
        if let Some(parent) = installed_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(pretty) = serde_json::to_string_pretty(&json) {
            let tmp = installed_path.with_extension("json.tmp");
            if fs::write(&tmp, pretty).is_ok() {
                let _ = fs::rename(&tmp, &installed_path);
            }
        }
    }
    plugin_ids
}

/// 把 plugin_ids 里**没在 enabledPlugins 出现过**的全部置为 true，写回
/// `~/.claude/settings.json`。已经登记的（无论 true/false）一律不动——
/// 既不覆盖用户在 CLI 里特意 `disable` 的插件，也不每次 reload 都强制启用。
/// 同时清掉 enabledPlugins 里指向**marketplace 已经不存在**的死引用插件
/// （比如 plugin 改名后旧条目残留），避免 Claude 启动时报 "failed to load"。
fn enable_plugins_in_user_settings(plugin_ids: &[String]) {
    if plugin_ids.is_empty() {
        return;
    }
    let Ok(home) = home_dir() else { return };
    let settings_path = home.join(".claude").join("settings.json");

    let raw = match fs::read_to_string(&settings_path) {
        Ok(s) => s,
        Err(_) => "{}".to_string(),
    };
    let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return;
    };
    if !json.is_object() {
        return;
    }
    if !json
        .get("enabledPlugins")
        .map(|v| v.is_object())
        .unwrap_or(false)
    {
        json["enabledPlugins"] = serde_json::json!({});
    }
    let mut changed = false;
    let enabled = json["enabledPlugins"].as_object_mut().unwrap();

    // 先清死引用：enabledPlugins 里指向 marketplaces 已经不存在的插件目录的项。
    // 典型场景是插件改了名、上游被删，旧条目还残留——Claude 启动时会反复报
    // "failed to load Plugin xxx not found in marketplace yyy"。
    let dead: Vec<String> = enabled
        .keys()
        .filter(|k| !plugin_id_exists(k))
        .cloned()
        .collect();
    for k in dead {
        enabled.remove(&k);
        changed = true;
    }

    // 再补：扫到的插件如果完全没登记过，置 true。
    // 已经登记的（无论 true/false）一律不动——尊重用户在 CLI 里特意做的 disable。
    for id in plugin_ids {
        if !enabled.contains_key(id) {
            enabled.insert(id.clone(), serde_json::json!(true));
            changed = true;
        }
    }

    if !changed {
        return;
    }
    if let Some(parent) = settings_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(pretty) = serde_json::to_string_pretty(&json) {
        let tmp = settings_path.with_extension("json.tmp");
        if fs::write(&tmp, pretty).is_ok() {
            let _ = fs::rename(&tmp, &settings_path);
        }
    }
}

/// 给定 `<plugin>@<mp>`，判断 marketplace 源里是否还存在该插件目录。
/// 用于识别 enabledPlugins 里指向已被改名/删除插件的死引用。
fn plugin_id_exists(plugin_id: &str) -> bool {
    let Some(at) = plugin_id.rfind('@') else { return false };
    let plugin_name = &plugin_id[..at];
    let mp_name = &plugin_id[at + 1..];
    if plugin_name.is_empty() || mp_name.is_empty() {
        return false;
    }
    let Ok(home) = home_dir() else { return false };
    home.join(".claude")
        .join("plugins")
        .join("marketplaces")
        .join(mp_name)
        .join(plugin_name)
        .is_dir()
}

fn synthesize_user_entry(
    cache_root: &Path,
    mp_name: &str,
    pl_name: &str,
    marketplace_dir: &Path,
) -> serde_json::Value {
    // installPath 优先指向 cache 最新版本；cache 还没物化就回退到 marketplace
    // 源路径，让 Claude 下次启动自己 materialize。version 也跟着 cache 目录名走。
    let pl_cache = cache_root.join(mp_name).join(pl_name);
    let (install_path, version) = if let Ok(versions) = fs::read_dir(&pl_cache) {
        let v: Vec<fs::DirEntry> = versions.flatten().filter(|v| v.path().is_dir()).collect();
        if let Some(latest) = pick_latest_version(v) {
            let ver_name = latest.file_name().to_string_lossy().to_string();
            (latest.path().to_string_lossy().to_string(), ver_name)
        } else {
            (
                marketplace_dir.join(pl_name).to_string_lossy().to_string(),
                "0.0.0".to_string(),
            )
        }
    } else {
        (
            marketplace_dir.join(pl_name).to_string_lossy().to_string(),
            "0.0.0".to_string(),
        )
    };

    let now = iso8601_now();
    let mut obj = serde_json::Map::new();
    obj.insert("scope".into(), serde_json::json!("user"));
    obj.insert("installPath".into(), serde_json::json!(install_path));
    obj.insert("version".into(), serde_json::json!(version));
    obj.insert("installedAt".into(), serde_json::json!(now));
    obj.insert("lastUpdated".into(), serde_json::json!(now));
    // gitCommitSha 故意省略——非必填，避免在 list_skills 同步路径里 fork 子进程
    // 跑 git rev-parse（按项目 CLAUDE.md 约定，慢 IO 不能塞进同步命令）。
    serde_json::Value::Object(obj)
}

fn iso8601_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() as i64;
    let millis = now.subsec_millis();
    let days = secs.div_euclid(86400);
    let secs_of_day = secs.rem_euclid(86400) as u64;
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    let (y, m, d) = days_to_ymd(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        y, m, d, hour, minute, second, millis
    )
}

/// Howard Hinnant 的 civil_from_days：input 是带符号的「自 1970-01-01 的天数」，
/// 输出 (year, month, day)。算法是 chrono/std 时间库都在用的标准实现。
fn days_to_ymd(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    if m <= 2 {
        y += 1;
    }
    (y, m, d)
}

/// 翻 `~/.claude/settings.json` 的 `enabledPlugins[plugin_id]`。这是 Claude Code
/// **插件级**开关：false 时整个插件不加载（skill / commands / hooks / agents
/// 全部失效），跟改单个 SKILL.md 文件名是两套机制。前端的「插件 toggle」走这条，
/// 「单个 skill toggle」走 toggle_skills。
///
/// 入参 `plugin_id` 形如 `<plugin>@<marketplace>`，跟 installed_plugins.json
/// 与 enabledPlugins 用的是同一套 key。
#[tauri::command]
pub fn toggle_plugin(plugin_id: String, enabled: bool) -> Result<(), String> {
    if plugin_id.is_empty() {
        return Err("plugin_id is empty".into());
    }
    let home = home_dir()?;
    let settings_path = home.join(".claude").join("settings.json");

    let raw = fs::read_to_string(&settings_path).unwrap_or_else(|_| "{}".to_string());
    let mut json: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("settings.json 解析失败：{}", e))?;
    if !json.is_object() {
        json = serde_json::json!({});
    }
    if !json
        .get("enabledPlugins")
        .map(|v| v.is_object())
        .unwrap_or(false)
    {
        json["enabledPlugins"] = serde_json::json!({});
    }
    let map = json["enabledPlugins"].as_object_mut().unwrap();
    let prev = map.get(&plugin_id).and_then(|v| v.as_bool());
    if prev == Some(enabled) {
        return Ok(()); // 已经是目标态，免写盘
    }
    map.insert(plugin_id, serde_json::json!(enabled));

    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let pretty = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
    let tmp = settings_path.with_extension("json.tmp");
    fs::write(&tmp, pretty).map_err(|e| format!("write tmp: {}", e))?;
    fs::rename(&tmp, &settings_path).map_err(|e| format!("rename: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn toggle_skills(ids: Vec<String>, enabled: bool) -> Result<(), String> {
    // 真禁用：把 SKILL.md 改名成 SKILL.md.disabled，让 Claude/Codex/Gemini
    // 的加载器自己就看不到这个 skill。只改 state.json 不够，各家生态不会读
    // Quiver 的 state；它们依赖文件名/后缀这类自己就能识别的信号。
    //
    // 命令是**纯执行器**：精确翻 `ids` 里列出的副本，不在后端做任何"聚合"。
    // 逻辑 skill 边界（跨生态同名联动、splitNameGroup 拆分）由前端的
    // `toLogicalSkills` 决定，它已经考虑了 hash 互斥等条件——后端再按 name
    // 自作主张联动会超出前端的分组边界，把无关插件的同名 skill 也翻了。
    let _guard = STATE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut state = read_state();
    let skills = list_skills_internal(None, &state);

    let mut errors: Vec<String> = Vec::new();
    for id in &ids {
        let target = skills.iter().find(|s| &s.id == id);
        if let Some(skill) = target {
            let path = PathBuf::from(&skill.path);
            let Some(dir) = path.parent() else { continue };
            let enabled_path = dir.join("SKILL.md");
            let disabled_path = dir.join("SKILL.md.disabled");
            let (src, dst) = if enabled {
                (&disabled_path, &enabled_path)
            } else {
                (&enabled_path, &disabled_path)
            };
            // 已经在目标态就跳过；两个都存在时也不覆盖，避免丢内容。
            if src.is_file() && !dst.exists() {
                if let Err(e) = fs::rename(src, dst) {
                    errors.push(format!("{}: {}", id, e));
                    continue; // rename 失败就不改 state，避免 state 与磁盘背离
                }
            }
        }
        // target 找不到时（如 project-scope 没传 project_dir）：只写 state，
        // 下次带 project_dir 扫描时 `load_skill_from_dir` 的自愈兜底。
        if enabled {
            state.disabled.remove(id);
        } else {
            state.disabled.insert(id.clone());
        }
    }

    write_state(&state)?;

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "部分副本未能切换（state 已同步到磁盘实际状态）：{}",
            errors.join("; ")
        ))
    }
}

pub fn remove_disabled_entry(id: &str) {
    let Ok(_guard) = STATE_LOCK.lock() else { return };
    let mut state = read_state();
    if state.disabled.remove(id) {
        let _ = write_state(&state);
    }
}

pub(crate) fn find_skill_by_id(id: &str, project_dir: Option<&str>) -> Option<Skill> {
    let _guard = STATE_LOCK.lock().ok()?;
    let state = read_state();
    list_skills_internal(project_dir, &state)
        .into_iter()
        .find(|s| s.id == id)
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImportResult {
    /// 单个 skill 仓：克隆到 `~/.claude/skills/<name>/` 作为用户级 skill。
    Skill { skill: Skill },
    /// 插件市场仓：克隆到 `~/.claude/plugins/marketplaces/<name>/`，里面包含
    /// 若干个子插件（每个插件通常有自己的 `skills/` 目录）。
    Marketplace {
        name: String,
        plugin_count: usize,
        skill_count: usize,
    },
}

#[tauri::command]
pub async fn import_from_github(repo_url: String) -> Result<ImportResult, String> {
    // 策略：克隆到临时目录后按结构自动分流——
    //   1. 根目录有 SKILL.md → 单 skill 安装到 ~/.claude/skills/<name>/
    //   2. 有 <plugin-dir>/skills/<x>/SKILL.md 模式 → 按 marketplace 安装到
    //      ~/.claude/plugins/marketplaces/<name>/，让扫描器和 Claude 都能识别
    //   3. 否则：拒绝并把探到的 SKILL.md 路径列给用户参考
    let home = home_dir()?;

    let tmp_base = std::env::temp_dir().join(format!(
        "quiver-import-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    // `--` 终结选项解析：防止以 `-` 开头的 URL 被 git 当 flag 处理
    // （例如 `--upload-pack=…` 会变成任意命令执行面）。
    let output = Command::new("git")
        .args(["clone", "--depth", "1", "--"])
        .arg(&repo_url)
        .arg(&tmp_base)
        .output()
        .map_err(|e| format!("git clone failed to start: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "git clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // —— 分流 1：根目录是单 skill。
    if tmp_base.join("SKILL.md").is_file() {
        let skills_root = home.join(".claude").join("skills");
        fs::create_dir_all(&skills_root).map_err(|e| format!("mkdir skills root: {}", e))?;
        let folder_name =
            derive_repo_name(&repo_url).unwrap_or_else(|| "imported-skill".to_string());
        let dest = resolve_unique_child(&skills_root, &folder_name);
        copy_dir_all(&tmp_base, &dest).map_err(|e| format!("copy skill: {}", e))?;
        let _ = fs::remove_dir_all(&tmp_base);

        let _guard = STATE_LOCK.lock().map_err(|e| e.to_string())?;
        let state = read_state();
        let skill = load_skill_from_dir(&dest, Ecosystem::Claude, Scope::User, None, true, &state)
            .ok_or_else(|| "imported skill could not be loaded".to_string())?;
        return Ok(ImportResult::Skill { skill });
    }

    // —— 分流 2：是否像一个插件市场。
    let plugin_dirs = find_marketplace_plugin_dirs(&tmp_base);
    if !plugin_dirs.is_empty() {
        let marketplaces_root = home.join(".claude").join("plugins").join("marketplaces");
        fs::create_dir_all(&marketplaces_root)
            .map_err(|e| format!("mkdir marketplaces root: {}", e))?;
        let folder_name = derive_repo_name(&repo_url)
            .unwrap_or_else(|| "imported-marketplace".to_string());
        let dest = resolve_unique_child(&marketplaces_root, &folder_name);
        copy_dir_all(&tmp_base, &dest).map_err(|e| format!("copy marketplace: {}", e))?;
        let _ = fs::remove_dir_all(&tmp_base);

        let final_name = dest
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or(folder_name);
        let plugin_count = plugin_dirs.len();
        let skill_count: usize = plugin_dirs
            .iter()
            .map(|p| {
                find_skill_md_dirs(&p.join("skills"), usize::MAX / 2).len()
            })
            .sum();
        return Ok(ImportResult::Marketplace {
            name: final_name,
            plugin_count,
            skill_count,
        });
    }

    // —— 分流 3：都不匹配，列出所有 SKILL.md 路径给用户排查。
    let candidates = find_skill_md_dirs(&tmp_base, 20);
    let relative: Vec<String> = candidates
        .iter()
        .filter_map(|p| p.strip_prefix(&tmp_base).ok())
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .collect();
    let _ = fs::remove_dir_all(&tmp_base);
    if relative.is_empty() {
        Err(format!(
            "仓库里没找到任何 SKILL.md，既不是 skill 仓也不像插件市场：{}",
            repo_url
        ))
    } else {
        Err(format!(
            "仓库结构不标准（根目录无 SKILL.md、也没有 <plugin>/skills/ 结构）。探到的 SKILL.md 位置：\n{}",
            relative.join("\n")
        ))
    }
}

/// 在仓库里识别"插件市场"特征：任意一级子目录，只要含 `skills/` 且里面至少有
/// 一个 SKILL.md，就算一个 plugin。这是 Claude plugin marketplace 的最小共性。
fn find_marketplace_plugin_dirs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let skills = entry.path().join("skills");
        if skills.is_dir() && !find_skill_md_dirs(&skills, 1).is_empty() {
            out.push(entry.path());
        }
    }
    out
}

fn resolve_unique_child(root: &Path, name: &str) -> PathBuf {
    let mut dest = root.join(name);
    let mut suffix = 1;
    while dest.exists() {
        dest = root.join(format!("{}-{}", name, suffix));
        suffix += 1;
    }
    dest
}

/// 在 `root` 下递归找所有含 SKILL.md 的目录（返回目录绝对路径）。用于在导入
/// 失败时给用户列出候选子目录。max 限制防止在巨型仓库里狂扫。
fn find_skill_md_dirs(root: &Path, max: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if out.len() >= max {
            break;
        }
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        let mut subdirs: Vec<PathBuf> = Vec::new();
        let mut has_skill = false;
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            let name = entry.file_name();
            if ft.is_file() && name == "SKILL.md" {
                has_skill = true;
            } else if ft.is_dir() {
                let n = name.to_string_lossy();
                if n == ".git" || n.starts_with('.') || n == "node_modules" {
                    continue;
                }
                subdirs.push(entry.path());
            }
        }
        if has_skill {
            out.push(dir);
        }
        // 深度优先，但顺序无所谓。
        stack.extend(subdirs);
    }
    out.sort();
    out
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        let path = entry.path();
        let target = dst.join(&name);
        if entry.file_type()?.is_dir() {
            copy_dir_all(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn derive_repo_name(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches('/');
    let trimmed = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    trimmed
        .rsplit(|c| c == '/' || c == ':')
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn run_git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|e| format!("git {}: failed to start ({})", args.join(" "), e))?;
    if !output.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), String> {
    let output = Command::new("open")
        .args(["-R", &path])
        .output()
        .map_err(|e| format!("open: {}", e))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Cross-ecosystem sync
// ---------------------------------------------------------------------------

/// Target path layout for each ecosystem's user-scope skill root.
fn user_skill_root(ecosystem: Ecosystem) -> Result<PathBuf, String> {
    let home = home_dir()?;
    Ok(match ecosystem {
        Ecosystem::Claude => home.join(".claude").join("skills"),
        Ecosystem::Codex => home.join(".codex").join("skills"),
        Ecosystem::Gemini => home.join(".gemini").join("skills"),
    })
}

/// Copy a skill directory from `source_id` into the target ecosystem's user-scope
/// root, preserving sibling files (scripts/, references/, agents/openai.yaml, etc.).
/// Fails if the target already has a skill with the same dir name.
#[tauri::command]
pub async fn sync_skill_to_ecosystem(
    source_id: String,
    target_ecosystem: Ecosystem,
    overwrite: Option<bool>,
) -> Result<Skill, String> {
    let _guard = STATE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut state = read_state();

    let source = list_skills_internal(None, &state)
        .into_iter()
        .find(|s| s.id == source_id)
        .ok_or_else(|| format!("source skill not found: {}", source_id))?;

    if source.ecosystem == target_ecosystem {
        return Err("source and target ecosystems are the same".into());
    }

    let source_skill_md = PathBuf::from(&source.path);
    let source_dir = source_skill_md
        .parent()
        .ok_or_else(|| "source path has no parent".to_string())?;
    let dir_name = source_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .ok_or_else(|| "source dir has no name".to_string())?;

    let target_root = user_skill_root(target_ecosystem)?;
    fs::create_dir_all(&target_root)
        .map_err(|e| format!("mkdir target root: {}", e))?;
    let target_dir = target_root.join(&dir_name);
    let force = overwrite.unwrap_or(false);

    // Overwrite 不能直接 remove_dir_all 再 copy——中间崩了/IO 失败，用户原本的
    // 整个目录（scripts/、references/ 等同级资产）就没了。用"先改名备份 → 拷贝 →
    // 成功删备份 / 失败恢复备份"的模式，rename 同盘原子，代价近乎零。
    let backup: Option<PathBuf> = if target_dir.exists() {
        if !force {
            return Err(format!(
                "target already has a skill named '{}' — pass overwrite=true to replace",
                dir_name
            ));
        }
        let bak = target_root.join(format!(".quiver-bak-{}", dir_name));
        let _ = fs::remove_dir_all(&bak); // 清掉上一次失败残留
        fs::rename(&target_dir, &bak)
            .map_err(|e| format!("backup existing target: {}", e))?;
        Some(bak)
    } else {
        None
    };

    if let Err(e) = copy_dir_all(source_dir, &target_dir) {
        // 拷贝失败：把 target_dir 清掉（可能有半成品），把备份改回去。
        let _ = fs::remove_dir_all(&target_dir);
        if let Some(bak) = &backup {
            let _ = fs::rename(bak, &target_dir);
        }
        return Err(format!("copy skill (rolled back): {}", e));
    }
    if let Some(bak) = backup {
        let _ = fs::remove_dir_all(bak);
    }

    // Inherit enabled state: if the source is disabled in our state store, propagate
    // that to the new physical copy so the logical skill stays in sync.
    let new_id = id_for(target_ecosystem, &Scope::User, &None, &dir_name);
    let source_disabled = state.disabled.contains(&source.id);
    let mut state_changed = false;
    if source_disabled {
        state_changed |= state.disabled.insert(new_id.clone());
    } else {
        state_changed |= state.disabled.remove(&new_id);
    }
    if state_changed {
        write_state(&state)?;
    }

    load_skill_from_dir(&target_dir, target_ecosystem, Scope::User, None, true, &state)
        .ok_or_else(|| "synced skill could not be loaded".into())
}

/// 从远端拉取 marketplace 最新代码，并清掉该 marketplace 下整块插件 cache。
/// 只 git pull 不清 cache 等于白做——Claude 下次启动仍会读旧 cache 里的快照。
/// Cache 清空后 Claude 会在下次启动时自己重新 materialize；届时
/// `load_skill_from_dir` 的自愈逻辑会把 state 里仍标记为 disabled 的 skill
/// 重新改回 `SKILL.md.disabled`，所以用户之前的禁用设置不会丢。
#[tauri::command]
pub async fn refresh_claude_marketplace(name: String) -> Result<(), String> {
    let _guard = STATE_LOCK.lock().map_err(|e| e.to_string())?;

    if name.is_empty() || name.contains('/') || name.contains('\\') || name == ".." {
        return Err(format!("unsafe marketplace name: {}", name));
    }

    let home = home_dir()?;
    let plugins_root = home.join(".claude").join("plugins");
    let marketplace_dir = plugins_root.join("marketplaces").join(&name);

    if !marketplace_dir.exists() {
        return Err(format!(
            "marketplace source not found: {}",
            marketplace_dir.display()
        ));
    }

    if !marketplace_dir.join(".git").exists() {
        return Err(format!(
            "marketplace '{}' is not a git repo — cannot refresh from remote",
            name
        ));
    }

    run_git(&marketplace_dir, &["pull", "--ff-only"])?;

    // 清掉 `cache/<name>/` 整块。不存在就跳过（比如该 marketplace 下还没装过
    // 任何插件）。删前再核对一次绝对路径落在 cache 根下，防御极端输入。
    let cache_root = plugins_root.join("cache");
    let market_cache = cache_root.join(&name);
    if market_cache.exists() {
        let canonical_root = fs::canonicalize(&cache_root).map_err(|e| e.to_string())?;
        let canonical_target = fs::canonicalize(&market_cache).map_err(|e| e.to_string())?;
        if !canonical_target.starts_with(&canonical_root) {
            return Err("refusing to delete path outside plugin cache".into());
        }
        fs::remove_dir_all(&market_cache)
            .map_err(|e| format!("remove plugin cache: {}", e))?;
    }

    Ok(())
}

/// 返回 marketplace 的安装目录路径，优先返回源仓库（用户能 git pull 的位置），
/// 没有源就退回到 cache（Claude 实际读取的物化目录）。两者都不存在则报错。
#[tauri::command]
pub fn marketplace_path(name: String) -> Result<String, String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name == ".." {
        return Err(format!("unsafe marketplace name: {}", name));
    }
    let home = home_dir()?;
    let plugins = home.join(".claude").join("plugins");
    let source = plugins.join("marketplaces").join(&name);
    if source.exists() {
        return Ok(source.to_string_lossy().into_owned());
    }
    let cache = plugins.join("cache").join(&name);
    if cache.exists() {
        return Ok(cache.to_string_lossy().into_owned());
    }
    Err(format!("marketplace '{}' not found", name))
}
