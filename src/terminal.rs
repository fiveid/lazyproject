use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalBackend {
    Ghostty,
    AppleTerminal,
    ITerm2,
    WezTerm,
    Kitty,
    FallbackMacOs,
}

pub fn detect_terminal_backend(term_program: Option<&str>) -> TerminalBackend {
    match term_program.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("ghostty") => TerminalBackend::Ghostty,
        Some("apple_terminal") => TerminalBackend::AppleTerminal,
        Some("iterm.app") => TerminalBackend::ITerm2,
        Some("wezterm") => TerminalBackend::WezTerm,
        Some("kitty") => TerminalBackend::Kitty,
        _ => TerminalBackend::FallbackMacOs,
    }
}

/// Open `dir` in a new terminal tab or window, without shell string interpolation.
pub fn open_in_new_tab(dir: &Path) -> Result<String, String> {
    let term_program = std::env::var("TERM_PROGRAM").ok();
    let backend = detect_terminal_backend(term_program.as_deref());
    let dir_str = dir.to_string_lossy();

    match backend {
        TerminalBackend::Ghostty => {
            let arg = format!("--working-directory={dir_str}");
            run_detached("open", &["-na", "Ghostty", "--args", &arg])?;
            Ok(format!("Opened {} in Ghostty", dir.display()))
        }
        TerminalBackend::AppleTerminal => {
            let script = r#"on run argv
    set targetDir to item 1 of argv
    tell application "Terminal"
        activate
        tell application "System Events" to keystroke "t" using command down
        delay 0.15
        do script ("cd " & quoted form of targetDir) in front window
    end tell
end run"#;
            run_detached("osascript", &["-e", script, &dir_str])?;
            Ok(format!("Opened {} in Terminal tab", dir.display()))
        }
        TerminalBackend::ITerm2 => {
            let script = r#"on run argv
    set targetDir to item 1 of argv
    tell application "iTerm2"
        activate
        tell current window
            create tab with default profile
            tell current session
                write text ("cd " & quoted form of targetDir)
            end tell
        end tell
    end tell
end run"#;
            run_detached("osascript", &["-e", script, &dir_str])?;
            Ok(format!("Opened {} in iTerm2 tab", dir.display()))
        }
        TerminalBackend::WezTerm => {
            run_detached("wezterm", &["cli", "spawn", "--cwd", &dir_str])?;
            Ok(format!("Opened {} in WezTerm tab", dir.display()))
        }
        TerminalBackend::Kitty => {
            run_detached("kitty", &["@", "launch", "--type=tab", "--cwd", &dir_str])?;
            Ok(format!("Opened {} in kitty tab", dir.display()))
        }
        TerminalBackend::FallbackMacOs => {
            run_detached("open", &["-a", "Terminal", &dir_str])?;
            Ok(format!("Opened {} in Terminal", dir.display()))
        }
    }
}

fn run_detached(bin: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| format!("failed to launch {bin}: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            format!("{bin} exited with {}", output.status)
        } else {
            stderr
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_terminal_backends() {
        assert_eq!(
            detect_terminal_backend(Some("ghostty")),
            TerminalBackend::Ghostty
        );
        assert_eq!(
            detect_terminal_backend(Some("Apple_Terminal")),
            TerminalBackend::AppleTerminal
        );
        assert_eq!(
            detect_terminal_backend(Some("iTerm.app")),
            TerminalBackend::ITerm2
        );
        assert_eq!(
            detect_terminal_backend(Some("WezTerm")),
            TerminalBackend::WezTerm
        );
        assert_eq!(
            detect_terminal_backend(Some("kitty")),
            TerminalBackend::Kitty
        );
        assert_eq!(
            detect_terminal_backend(None),
            TerminalBackend::FallbackMacOs
        );
    }
}
