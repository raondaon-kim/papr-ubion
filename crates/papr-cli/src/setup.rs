//! `papr setup` — register an ambient SessionStart integration so every agent
//! conversation starts with the current unread state already in context.
//! Supports Claude Code, Codex and OpenCode; installs are idempotent and
//! repair a stale binary path on re-run.

use crate::{AxiError, Doc};
use serde_json::{json, Value};
use std::path::PathBuf;

/// Which agent host(s) to wire up.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum App {
    Claude,
    Codex,
    OpenCode,
}

impl App {
    fn parse(s: &str) -> Option<Vec<App>> {
        match s {
            "all" => Some(vec![App::Claude, App::Codex, App::OpenCode]),
            "claude" => Some(vec![App::Claude]),
            "codex" => Some(vec![App::Codex]),
            "opencode" => Some(vec![App::OpenCode]),
            _ => None,
        }
    }
}

pub fn run(app: &str) -> Result<String, AxiError> {
    let apps = App::parse(app).ok_or_else(|| {
        AxiError::usage(
            format!("unknown app target `{app}`"),
            vec!["Run `papr setup --app all|claude|codex|opencode`".into()],
        )
    })?;
    let bin = resolve_bin();

    let mut apps_rows: Vec<Value> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for a in apps {
        let (name, result) = match a {
            App::Claude => ("claude", install_claude(&bin)),
            App::Codex => ("codex", install_codex(&bin)),
            App::OpenCode => ("opencode", install_opencode(&bin)),
        };
        let row = match result {
            Ok(status) => json!({ "app": name, "status": "ok", "detail": status }),
            Err(e) => {
                errors.push(format!("{name}: {e}"));
                json!({ "app": name, "status": "error", "detail": e })
            }
        };
        apps_rows.push(row);
    }

    // A failed install must surface as a non-zero exit, not a clean table — the
    // caller (and the agent) would otherwise read a broken setup as success.
    if !errors.is_empty() {
        return Err(AxiError::runtime_help(
            format!("setup failed for {} target(s)", errors.len()),
            errors,
        ));
    }

    let mut d = Doc::new();
    d.set("setup", json!({ "bin": bin }));
    d.set("apps", Value::Array(apps_rows));
    d.help(vec![
        "Start a new agent session — the unread dashboard loads automatically".into(),
        "Run `papr` to preview the context that will be injected".into(),
    ]);
    Ok(d.into_toon())
}

/// The command an integration should invoke. Prefer the bare name `papr` when it
/// is on PATH and resolves to *this* executable (keeps a global install
/// portable); otherwise fall back to the absolute path.
fn resolve_bin() -> String {
    let current = std::env::current_exe().ok();
    if let (Some(cur), Ok(path)) = (current.as_ref(), std::env::var("PATH")) {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join("papr");
            if candidate.is_file() {
                // Compare canonical paths so a symlink to this binary still counts.
                if let (Ok(a), Ok(b)) = (candidate.canonicalize(), cur.canonicalize()) {
                    if a == b {
                        return "papr".to_string();
                    }
                }
            }
        }
    }
    current
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "papr".to_string())
}

fn home() -> Result<PathBuf, AxiError> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| AxiError::runtime("could not resolve HOME"))
}

// ───────────────────────────── Claude Code ─────────────────────────────

/// Merge a SessionStart command hook into `~/.claude/settings.json`, preserving
/// every other key. Re-running repairs the binary path; a matching hook is a
/// silent no-op.
fn install_claude(bin: &str) -> Result<String, String> {
    let dir = home().map_err(|e| e.message.clone())?.join(".claude");
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir .claude: {e}"))?;
    let file = dir.join("settings.json");

    let mut root: serde_json::Value = if file.exists() {
        let text = std::fs::read_to_string(&file).map_err(|e| format!("read settings.json: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("settings.json is not valid JSON: {e}"))?
    } else {
        serde_json::json!({})
    };

    if !root.is_object() {
        return Err("settings.json root is not an object".into());
    }
    let hooks = root
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));
    let sessions = hooks
        .as_object_mut()
        .ok_or("hooks is not an object")?
        .entry("SessionStart")
        .or_insert_with(|| serde_json::json!([]));
    let arr = sessions.as_array_mut().ok_or("SessionStart is not an array")?;

    // Find an existing papr hook (command basename == "papr") to repair in place.
    let is_papr = |cmd: &str| cmd == "papr" || cmd.ends_with("/papr");
    let changed;
    let mut found = false;
    for group in arr.iter_mut() {
        if let Some(inner) = group.get_mut("hooks").and_then(|h| h.as_array_mut()) {
            for h in inner.iter_mut() {
                if let Some(cmd) = h.get("command").and_then(|c| c.as_str()) {
                    if is_papr(cmd) {
                        found = true;
                        if cmd != bin {
                            h["command"] = serde_json::json!(bin);
                        }
                    }
                }
            }
        }
    }
    if found {
        changed = "updated";
    } else {
        arr.push(serde_json::json!({
            "hooks": [ { "type": "command", "command": bin } ]
        }));
        changed = "installed";
    }

    let pretty =
        serde_json::to_string_pretty(&root).map_err(|e| format!("serialize settings.json: {e}"))?;
    std::fs::write(&file, pretty + "\n").map_err(|e| format!("write settings.json: {e}"))?;
    Ok(format!("{changed} → {}", collapse(&file)))
}

// ─────────────────────────────── Codex ───────────────────────────────

/// Write a documented SessionStart command hook into `~/.codex/hooks.json`
/// and ensure `[features].hooks = true` in `~/.codex/config.toml`.
fn install_codex(bin: &str) -> Result<String, String> {
    let dir = home().map_err(|e| e.message.clone())?.join(".codex");
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir .codex: {e}"))?;
    let file = dir.join("hooks.json");

    let mut root: serde_json::Value = if file.exists() {
        let text = std::fs::read_to_string(&file).map_err(|e| format!("read hooks.json: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("hooks.json is not valid JSON: {e}"))?
    } else {
        serde_json::json!({})
    };
    let change = install_papr_session_start_hook(&mut root, bin)?;
    let pretty =
        serde_json::to_string_pretty(&root).map_err(|e| format!("serialize hooks.json: {e}"))?;
    std::fs::write(&file, pretty + "\n").map_err(|e| format!("write hooks.json: {e}"))?;

    // Enable the hooks feature in config.toml (text-level, to avoid a TOML dep).
    let cfg = dir.join("config.toml");
    let existing = std::fs::read_to_string(&cfg).unwrap_or_default();
    if let Some(next) = ensure_codex_hooks(&existing) {
        std::fs::write(&cfg, next).map_err(|e| format!("write config.toml: {e}"))?;
    }
    Ok(format!("{} → {}", change, collapse(&file)))
}

/// Merge Papr's command into Codex's current hook schema without replacing
/// hooks from the user or other tools. Older Papr versions wrote a flat
/// `SessionStart` array at the root of `hooks.json`; convert that form on the
/// next setup run so upgrading repairs the integration rather than duplicating
/// a stale hook.
fn install_papr_session_start_hook(root: &mut Value, bin: &str) -> Result<&'static str, String> {
    const TIMEOUT_SECS: u64 = 10;
    const CONTEXT_LIMIT: u64 = 1_200;

    let root = root
        .as_object_mut()
        .ok_or("hooks.json root is not an object")?;

    // Migrate the flat schema written by Papr before Codex adopted the
    // `hooks -> event -> matcher group -> handlers` nesting.
    let legacy = root.remove("SessionStart");
    let legacy_handlers = match legacy {
        Some(Value::Array(handlers)) => Some(handlers),
        Some(_) => return Err("legacy SessionStart is not an array".into()),
        None => None,
    };

    let hooks = root.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks
        .as_object_mut()
        .ok_or("hooks.json hooks is not an object")?;
    let sessions = hooks.entry("SessionStart").or_insert_with(|| json!([]));
    let sessions = sessions
        .as_array_mut()
        .ok_or("hooks.SessionStart is not an array")?;

    let migrated = legacy_handlers.is_some();
    if let Some(handlers) = legacy_handlers {
        // Preserve every legacy command rather than silently dropping a hook
        // owned by another integration. Its old event array maps to one
        // unfiltered SessionStart matcher group.
        sessions.push(json!({ "matcher": "startup|resume", "hooks": handlers }));
    }

    let command = hook_command(bin);
    let mut found = false;
    for group in sessions.iter_mut() {
        let Some(handlers) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
            continue;
        };
        for handler in handlers.iter_mut() {
            let Some(handler) = handler.as_object_mut() else {
                continue;
            };
            let is_papr = handler
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(is_papr_command);
            if is_papr {
                found = true;
                handler.insert("type".into(), json!("command"));
                handler.insert("command".into(), json!(&command));
                handler
                    .entry("timeout")
                    .or_insert_with(|| json!(TIMEOUT_SECS));
                handler
                    .entry("additionalContextLimit")
                    .or_insert_with(|| json!(CONTEXT_LIMIT));
            }
        }
    }
    if !found {
        sessions.push(json!({
            "matcher": "startup|resume",
            "hooks": [{
                "type": "command",
                "command": command,
                "timeout": TIMEOUT_SECS,
                "additionalContextLimit": CONTEXT_LIMIT,
            }],
        }));
    }

    Ok(if migrated {
        "migrated"
    } else if found {
        "updated"
    } else {
        "installed"
    })
}

/// The hook runner receives a shell command. Quote an absolute binary path
/// that contains spaces while preserving the portable bare `papr` command.
fn hook_command(bin: &str) -> String {
    if bin.contains(char::is_whitespace) {
        format!("\"{bin}\"")
    } else {
        bin.to_string()
    }
}

/// Recognise Papr commands produced by both Unix and Windows releases so a
/// setup re-run repairs a stale absolute path instead of adding a duplicate.
fn is_papr_command(command: &str) -> bool {
    let command = command.trim().trim_matches('"');
    let name = command.rsplit(['/', '\\']).next().unwrap_or(command);
    name.eq_ignore_ascii_case("papr") || name.eq_ignore_ascii_case("papr.exe")
}

/// Ensure `[features].hooks = true` in a Codex `config.toml`'s text, returning
/// the content to write, or `None` when it is already enabled. A line-level edit
/// (no TOML dependency) that only touches the `[features]` section: it never
/// appends a duplicate section, replaces an existing `hooks = <other>` in place,
/// and ignores commented lines so a `# hooks = true` can't masquerade as set.
fn ensure_codex_hooks(existing: &str) -> Option<String> {
    if existing.trim().is_empty() {
        return Some("[features]\nhooks = true\n".to_string());
    }
    let active = |l: &str| !l.trim_start().starts_with('#');
    let is_header = |l: &str| active(l) && l.trim_start().starts_with('[');
    let is_features = |l: &str| active(l) && l.trim() == "[features]";
    // A `hooks` assignment, whitespace-insensitive: `hooks=true`, `hooks = true`.
    let hooks_kv = |l: &str| active(l) && l.trim_start().replace(' ', "").starts_with("hooks=");

    let lines: Vec<&str> = existing.lines().collect();
    let Some(start) = lines.iter().position(|l| is_features(l)) else {
        // No [features] section — append one.
        let mut next = existing.to_string();
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str("\n[features]\nhooks = true\n");
        return Some(next);
    };

    // Scan the section body (until the next header) for an existing hooks key.
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, l)| is_header(l))
        .map(|(i, _)| i)
        .unwrap_or(lines.len());

    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    if let Some(rel) = lines[start + 1..end].iter().position(|l| hooks_kv(l)) {
        let idx = start + 1 + rel;
        if lines[idx].replace(' ', "").trim_end() == "hooks=true" {
            return None; // already enabled
        }
        out[idx] = "hooks = true".to_string();
    } else {
        out.insert(start + 1, "hooks = true".to_string());
    }
    Some(out.join("\n") + "\n")
}

// ────────────────────────────── OpenCode ──────────────────────────────

/// Install a managed OpenCode plugin that injects the papr dashboard as ambient
/// system context at session start.
fn install_opencode(bin: &str) -> Result<String, String> {
    let dir = home()
        .map_err(|e| e.message.clone())?
        .join(".config/opencode/plugin");
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir opencode/plugin: {e}"))?;
    let file = dir.join("papr.js");
    let plugin = format!(
        r#"// Managed by `papr setup` — injects the Papr unread dashboard at session
// start so the agent can act on your feeds immediately. Safe to delete.
import {{ execFile }} from "node:child_process"
import {{ promisify }} from "node:util"
const run = promisify(execFile)

export const papr = async () => ({{
  "experimental.systemPrompt": async ({{ parts }}) => {{
    try {{
      const {{ stdout }} = await run({bin:?}, [], {{ timeout: 5000 }})
      if (stdout.trim()) parts.push(stdout.trim())
    }} catch (_) {{ /* papr unavailable — skip silently */ }}
  }},
}})
"#,
        bin = bin
    );
    std::fs::write(&file, plugin).map_err(|e| format!("write papr.js: {e}"))?;
    Ok(format!("installed → {}", collapse(&file)))
}

fn collapse(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    if let Ok(h) = std::env::var("HOME") {
        if let Some(rest) = s.strip_prefix(&h) {
            return format!("~{rest}");
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::{ensure_codex_hooks, install_papr_session_start_hook, is_papr_command};
    use serde_json::json;

    #[test]
    fn installs_papr_in_the_documented_nested_hook_schema() {
        let mut root = json!({});

        assert_eq!(
            install_papr_session_start_hook(&mut root, "C:\\papr\\papr.exe"),
            Ok("installed")
        );
        assert_eq!(
            root,
            json!({
                "hooks": {
                    "SessionStart": [{
                        "matcher": "startup|resume",
                        "hooks": [{
                            "type": "command",
                            "command": "C:\\papr\\papr.exe",
                            "timeout": 10,
                            "additionalContextLimit": 1200,
                        }],
                    }],
                },
            })
        );
    }

    #[test]
    fn migrates_the_legacy_flat_session_start_array_without_losing_hooks() {
        let mut root = json!({
            "description": "Existing hooks",
            "SessionStart": [
                { "type": "command", "command": "C:\\old\\papr.exe" },
                { "type": "command", "command": "echo retained" },
            ],
            "hooks": {
                "Stop": [{
                    "hooks": [{ "type": "command", "command": "echo stopped" }],
                }],
            },
        });

        assert_eq!(
            install_papr_session_start_hook(&mut root, "C:\\new\\papr.exe"),
            Ok("migrated")
        );
        assert!(root.get("SessionStart").is_none());
        assert_eq!(
            root["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            json!("C:\\new\\papr.exe")
        );
        assert_eq!(
            root["hooks"]["SessionStart"][0]["hooks"][1]["command"],
            json!("echo retained")
        );
        assert_eq!(
            root["hooks"]["Stop"][0]["hooks"][0]["command"],
            json!("echo stopped")
        );
    }

    #[test]
    fn setup_repairs_an_existing_windows_papr_hook_without_duplicating_it() {
        let mut root = json!({
            "hooks": {
                "SessionStart": [{
                    "matcher": "startup|resume",
                    "hooks": [{ "type": "command", "command": "C:\\old path\\papr.exe" }],
                }],
            },
        });

        assert_eq!(
            install_papr_session_start_hook(&mut root, "C:\\new path\\papr.exe"),
            Ok("updated")
        );
        let sessions = root["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(sessions.len(), 1);
        let handler = &sessions[0]["hooks"][0];
        assert_eq!(handler["command"], json!("\"C:\\new path\\papr.exe\""));
        assert_eq!(handler["timeout"], json!(10));
        assert_eq!(handler["additionalContextLimit"], json!(1200));
    }

    #[test]
    fn recognises_portable_and_windows_papr_commands_only() {
        assert!(is_papr_command("papr"));
        assert!(is_papr_command("/usr/local/bin/papr"));
        assert!(is_papr_command("\"C:\\Program Files\\Papr\\papr.exe\""));
        assert!(!is_papr_command("papr-notify"));
    }

    #[test]
    fn empty_config_gets_a_clean_features_section() {
        assert_eq!(ensure_codex_hooks(""), Some("[features]\nhooks = true\n".to_string()));
        assert_eq!(ensure_codex_hooks("   \n"), Some("[features]\nhooks = true\n".to_string()));
    }

    #[test]
    fn appends_section_when_absent_without_clobbering() {
        let cfg = "model = \"gpt-5\"\n[ui]\ntheme = \"dark\"\n";
        let out = ensure_codex_hooks(cfg).unwrap();
        assert!(out.starts_with(cfg));
        assert!(out.contains("[features]\nhooks = true\n"));
        // Exactly one [features] section — never a duplicate.
        assert_eq!(out.matches("[features]").count(), 1);
    }

    #[test]
    fn inserts_hooks_into_existing_features_section() {
        let cfg = "[features]\nweb_search = true\n";
        let out = ensure_codex_hooks(cfg).unwrap();
        assert_eq!(out.matches("[features]").count(), 1);
        assert!(out.contains("hooks = true"));
        assert!(out.contains("web_search = true"));
    }

    #[test]
    fn already_enabled_is_a_noop() {
        assert_eq!(ensure_codex_hooks("[features]\nhooks = true\n"), None);
        assert_eq!(ensure_codex_hooks("[features]\nhooks=true\n"), None);
    }

    #[test]
    fn flips_an_explicitly_disabled_hooks_value() {
        let out = ensure_codex_hooks("[features]\nhooks = false\n").unwrap();
        assert!(out.contains("hooks = true"));
        assert!(!out.contains("hooks = false"));
    }

    #[test]
    fn commented_hooks_does_not_count_as_enabled() {
        // A `# hooks = true` comment must not suppress the real write.
        let out = ensure_codex_hooks("[features]\n# hooks = true\n").unwrap();
        assert!(out.matches("hooks = true").count() >= 1);
        // The active (non-comment) line is present.
        assert!(out.lines().any(|l| l.trim() == "hooks = true"));
    }
}
