use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const HOOK_MARKER: &str = "";
// For UserPromptSubmit: propagate the exit code the server signals via HTTP status.
// 200 → allow (exit 0), 403 → block (exit 2).
const HOOK_COMMAND_PROMPT: &str = "\
payload=$(cat); \
response=$(echo \"$payload\" | curl -s -w '\\n%{http_code}' -X POST http://127.0.0.1:7421/hook -H 'Content-Type: application/json' -d @-); \
body=$(echo \"$response\" | head -n -1); \
code=$(echo \"$response\" | tail -n 1); \
echo \"$body\"; \
[ \"$code\" = \"403\" ] && exit 2 || exit 0";
// For all other hooks: fire-and-forget, always allow.
const HOOK_COMMAND: &str =
    "curl -s -X POST http://127.0.0.1:7421/hook -H 'Content-Type: application/json' -d @-";
const HOOK_EVENTS: &[&str] = &["PreToolUse", "PostToolUse", "Notification", "Stop"];
const PROMPT_EVENTS: &[&str] = &["UserPromptSubmit"];

pub fn install() -> Result<()> {
    let binary = current_binary()?;
    platform_install(&binary)?;
    configure_claude_hooks()?;
    Ok(())
}

pub fn uninstall() -> Result<()> {
    remove_claude_hooks()?;
    platform_uninstall()
}

fn current_binary() -> Result<PathBuf> {
    std::env::current_exe().context("could not determine binary path")
}

// ─── Claude Code hooks ───────────────────────────────────────────────────────

fn claude_settings_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home).join(".claude/settings.json"))
}

fn configure_claude_hooks() -> Result<()> {
    let path = claude_settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut settings: serde_json::Value = if path.exists() {
        let raw = fs::read_to_string(&path)?;
        serde_json::from_str(&raw).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let hooks = settings
        .as_object_mut()
        .context("settings.json root is not an object")?
        .entry("hooks")
        .or_insert(serde_json::json!({}));

    let all_events: &[(&[&str], &str)] = &[
        (HOOK_EVENTS, HOOK_COMMAND),
        (PROMPT_EVENTS, HOOK_COMMAND_PROMPT),
    ];

    for (events, command) in all_events {
        let entry = serde_json::json!({
            "matcher": HOOK_MARKER,
            "hooks": [{ "type": "command", "command": command }]
        });
        for event in *events {
            let list = hooks
                .as_object_mut()
                .context("hooks is not an object")?
                .entry(*event)
                .or_insert(serde_json::json!([]));

            let already = list
                .as_array()
                .map(|a| a.iter().any(|e| e.get("matcher").and_then(|m| m.as_str()) == Some(HOOK_MARKER)))
                .unwrap_or(false);

            if !already {
                list.as_array_mut()
                    .context("hook event list is not an array")?
                    .push(entry.clone());
            }
        }
    }

    let json = serde_json::to_string_pretty(&settings)?;
    fs::write(&path, json)?;
    tracing::info!("Claude Code hooks configured at {}", path.display());
    Ok(())
}

fn remove_claude_hooks() -> Result<()> {
    let path = claude_settings_path()?;
    if !path.exists() {
        return Ok(());
    }

    let raw = fs::read_to_string(&path)?;
    let mut settings: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::json!({}));

    if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        let all_events = HOOK_EVENTS.iter().chain(PROMPT_EVENTS.iter());
        for event in all_events {
            if let Some(list) = hooks.get_mut(*event).and_then(|l| l.as_array_mut()) {
                list.retain(|e| {
                    e.get("matcher").and_then(|m| m.as_str()) != Some(HOOK_MARKER)
                });
            }
        }
    }

    let json = serde_json::to_string_pretty(&settings)?;
    fs::write(&path, json)?;
    tracing::info!("Claude Code hooks removed from {}", path.display());
    Ok(())
}

// ─── macOS ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn platform_install(binary: &std::path::Path) -> Result<()> {
    let plist_path = launchd_plist_path()?;
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
            "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
            <plist version="1.0">
            <dict>
                <key>Label</key>
                <string>com.llm-observer.daemon</string>
                <key>ProgramArguments</key>
                <array>
                    <string>{binary}</string>
                    <string>run</string>
                </array>
                <key>RunAtLoad</key>
                <true/>
                <key>KeepAlive</key>
                <true/>
                <key>StandardOutPath</key>
                <string>/tmp/llm-observer.log</string>
                <key>StandardErrorPath</key>
                <string>/tmp/llm-observer.log</string>
            </dict>
            </plist>
            "#,
        binary = binary.display()
    );

    if let Some(parent) = plist_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&plist_path, plist)?;

    Command::new("launchctl")
        .args(["load", "-w", &plist_path.to_string_lossy()])
        .status()
        .context("launchctl load failed")?;

    tracing::info!("launchd agent installed at {}", plist_path.display());
    Ok(())
}

#[cfg(target_os = "macos")]
fn platform_uninstall() -> Result<()> {
    let plist_path = launchd_plist_path()?;
    if plist_path.exists() {
        Command::new("launchctl")
            .args(["unload", "-w", &plist_path.to_string_lossy()])
            .status()
            .context("launchctl unload failed")?;
        fs::remove_file(&plist_path)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn launchd_plist_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home)
        .join("Library/LaunchAgents/com.llm-observer.daemon.plist"))
}