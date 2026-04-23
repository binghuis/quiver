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
    pub enabled: bool,
    pub plugin: Option<String>,
    pub path: String,
    pub body: String,
    pub content_hash: String,
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

fn parse_frontmatter(content: &str) -> (Option<String>, Option<String>, String) {
    let Some(rest) = content.strip_prefix("---\n").or_else(|| content.strip_prefix("---\r\n"))
    else {
        return (None, None, content.to_string());
    };
    let Some(end) = rest.find("\n---").map(|i| (i, 4)).or_else(|| rest.find("\r\n---").map(|i| (i, 5)))
    else {
        return (None, None, content.to_string());
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
    let mut current_key: Option<String> = None;
    let mut buf = String::new();

    let flush = |key: &Option<String>,
                 buf: &str,
                 name: &mut Option<String>,
                 description: &mut Option<String>| {
        let Some(k) = key else { return };
        let v = buf.trim().trim_matches('"').trim_matches('\'').to_string();
        match k.as_str() {
            "name" => *name = Some(v),
            "description" => *description = Some(v),
            _ => {}
        }
    };

    for line in yaml.lines() {
        if let Some((k, v)) = split_yaml_line(line) {
            flush(&current_key, &buf, &mut name, &mut description);
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
    flush(&current_key, &buf, &mut name, &mut description);

    (name, description, body)
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
    let (fm_name, fm_description, body) = parse_frontmatter(&content);
    let name = fm_name.unwrap_or_else(|| dir_name.clone());
    let description = fm_description.unwrap_or_default();

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
        body,
        content_hash: hash,
    })
}

fn scan_skills_root(
    root: &Path,
    ecosystem: Ecosystem,
    scope: Scope,
    plugin: Option<String>,
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

fn scan_claude_plugin_skills(state: &State) -> Vec<Skill> {
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
                out.extend(scan_skills_root(
                    &skills_dir,
                    Ecosystem::Claude,
                    Scope::Plugin,
                    Some(plugin_label),
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
        state,
    ));

    // ~/.gemini/extensions/<ext>/skills/<name>/SKILL.md  — each extension = plugin.
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
            state,
        ));
    }

    out
}

fn list_skills_internal(project_dir: Option<&str>, state: &State) -> Vec<Skill> {
    let Ok(home) = home_dir() else {
        return Vec::new();
    };
    let mut skills = Vec::new();
    skills.extend(scan_skills_root(
        &home.join(".claude").join("skills"),
        Ecosystem::Claude,
        Scope::User,
        None,
        state,
    ));
    if let Some(project) = project_dir.filter(|s| !s.is_empty()) {
        let pp = PathBuf::from(project).join(".claude").join("skills");
        skills.extend(scan_skills_root(
            &pp,
            Ecosystem::Claude,
            Scope::Project,
            None,
            state,
        ));
    }
    skills.extend(scan_claude_plugin_skills(state));
    // Also scan raw marketplace sources — Quiver-installed marketplaces that
    // Claude hasn't yet materialized into cache still need to show up in the
    // list so users can toggle plugins immediately. Dedupe against cache by id.
    let seen: HashSet<String> = skills.iter().map(|s| s.id.clone()).collect();
    for s in scan_claude_marketplace_plugins(state) {
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
fn scan_claude_marketplace_plugins(state: &State) -> Vec<Skill> {
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
                out.extend(scan_skills_root(
                    &skills_dir,
                    Ecosystem::Claude,
                    Scope::Plugin,
                    Some(plugin_label),
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
    let state = read_state();
    let mut skills = list_skills_internal(project_dir.as_deref(), &state);
    skills.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(skills)
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
pub fn import_from_github(repo_url: String) -> Result<ImportResult, String> {
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
        let skill = load_skill_from_dir(&dest, Ecosystem::Claude, Scope::User, None, &state)
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
pub fn sync_skill_to_ecosystem(
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

    load_skill_from_dir(&target_dir, target_ecosystem, Scope::User, None, &state)
        .ok_or_else(|| "synced skill could not be loaded".into())
}

/// 从远端拉取 marketplace 最新代码，并清掉该 marketplace 下整块插件 cache。
/// 只 git pull 不清 cache 等于白做——Claude 下次启动仍会读旧 cache 里的快照。
/// Cache 清空后 Claude 会在下次启动时自己重新 materialize；届时
/// `load_skill_from_dir` 的自愈逻辑会把 state 里仍标记为 disabled 的 skill
/// 重新改回 `SKILL.md.disabled`，所以用户之前的禁用设置不会丢。
#[tauri::command]
pub fn refresh_claude_marketplace(name: String) -> Result<(), String> {
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
