// macOS GUI 进程由 launchd 启动，PATH 只有 `/usr/bin:/bin:/usr/sbin:/sbin`，
// 也不会继承用户在 shell 里 export 的 `http_proxy` / `https_proxy` 之类。
// 启动时 spawn 一次用户登录 shell 拿 env，挑出几个关键变量注入当前进程。
// 失败/超时不阻塞 App 启动——退化到 launchd 默认行为，proxy 那边还有 scutil 兜底。

use std::process::Command;
use std::time::Duration;

const KEYS: &[&str] = &[
    "PATH",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
];

pub fn inherit_shell_env() {
    if cfg!(not(target_os = "macos")) {
        return;
    }
    // debug build = `tauri dev`，从 terminal 起，env 已经齐了。
    if cfg!(debug_assertions) {
        return;
    }

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let Some(output) = wait_with_timeout(
        Command::new(&shell).args(["-ilc", "/usr/bin/env -0"]),
        Duration::from_secs(3),
    ) else {
        return;
    };
    if !output.status.success() {
        return;
    }

    for entry in String::from_utf8_lossy(&output.stdout).split('\0') {
        let Some((key, value)) = entry.split_once('=') else {
            continue;
        };
        if !KEYS.contains(&key) {
            continue;
        }
        // SAFETY: 启动早期单线程，main thread setup hook 跑之前调用。
        unsafe {
            std::env::set_var(key, value);
        }
    }
}

// std::process::Command 没有原生超时——zsh init 偶尔会被 plugin 拖死，
// 必须有兜底，否则 App 启动就 hang。
fn wait_with_timeout(cmd: &mut Command, timeout: Duration) -> Option<std::process::Output> {
    use std::io::Read;
    use std::process::Stdio;

    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                if let Some(mut s) = child.stdout.take() {
                    let _ = s.read_to_end(&mut stdout);
                }
                return Some(std::process::Output {
                    status,
                    stdout,
                    stderr: Vec::new(),
                });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}
