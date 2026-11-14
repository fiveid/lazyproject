use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiseTool {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcosystemInfo {
    pub name: String,
    pub package_manager: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolchainInfo {
    pub mise_source: Option<String>,
    pub mise_tools: Vec<MiseTool>,
    pub ecosystems: Vec<EcosystemInfo>,
}

impl ToolchainInfo {
    pub fn is_empty(&self) -> bool {
        self.mise_tools.is_empty() && self.ecosystems.is_empty()
    }
}

pub fn inspect_toolchain(project_path: &Path) -> ToolchainInfo {
    let mut info = ToolchainInfo::default();

    // 1. Mise configuration (mise.toml, .mise.toml, .tool-versions)
    for filename in ["mise.toml", ".mise.toml"] {
        let candidate = project_path.join(filename);
        if let Ok(contents) = fs::read_to_string(&candidate) {
            let tools = parse_mise_toml(&contents);
            if !tools.is_empty() {
                info.mise_source = Some(filename.to_string());
                info.mise_tools = tools;
                break;
            }
        }
    }

    if info.mise_tools.is_empty() {
        let tool_versions = project_path.join(".tool-versions");
        if let Ok(contents) = fs::read_to_string(&tool_versions) {
            let tools = parse_tool_versions(&contents);
            if !tools.is_empty() {
                info.mise_source = Some(".tool-versions".to_string());
                info.mise_tools = tools;
            }
        }
    }

    // 2. Node.js
    if let Ok(pkg_str) = fs::read_to_string(project_path.join("package.json")) {
        info.ecosystems
            .push(detect_node_ecosystem(project_path, &pkg_str));
    }

    // 3. Rust
    if let Ok(cargo_str) = fs::read_to_string(project_path.join("Cargo.toml")) {
        info.ecosystems.push(detect_rust_ecosystem(&cargo_str));
    }

    // 4. Python
    if project_path.join("pyproject.toml").is_file()
        || project_path.join("requirements.txt").is_file()
    {
        info.ecosystems.push(detect_python_ecosystem(project_path));
    }

    // 5. Go
    if let Ok(gomod_str) = fs::read_to_string(project_path.join("go.mod")) {
        info.ecosystems.push(detect_go_ecosystem(&gomod_str));
    }

    info
}

pub fn parse_mise_toml(contents: &str) -> Vec<MiseTool> {
    let Ok(val) = contents.parse::<toml::Value>() else {
        return Vec::new();
    };
    let Some(tools_table) = val.get("tools").and_then(|t| t.as_table()) else {
        return Vec::new();
    };

    let mut tools = Vec::new();
    for (name, v) in tools_table {
        let version = match v {
            toml::Value::String(s) => s.clone(),
            toml::Value::Table(tbl) => tbl
                .get("version")
                .and_then(|vv| vv.as_str())
                .unwrap_or("configured")
                .to_string(),
            toml::Value::Array(arr) => arr
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            _ => continue,
        };
        tools.push(MiseTool {
            name: name.clone(),
            version,
        });
    }
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    tools
}

pub fn parse_tool_versions(contents: &str) -> Vec<MiseTool> {
    let mut tools = Vec::new();
    for line in contents.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        if let (Some(name), Some(version)) = (parts.next(), parts.next()) {
            tools.push(MiseTool {
                name: name.to_string(),
                version: version.to_string(),
            });
        }
    }
    tools
}

fn detect_node_ecosystem(project_path: &Path, pkg_str: &str) -> EcosystemInfo {
    let parsed: Option<serde_json::Value> = serde_json::from_str(pkg_str).ok();

    let pkg_mgr_field = parsed
        .as_ref()
        .and_then(|v| v.get("packageManager"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let lockfile_mgr = if project_path.join("pnpm-lock.yaml").is_file() {
        Some("pnpm")
    } else if project_path.join("yarn.lock").is_file() {
        Some("yarn")
    } else if project_path.join("bun.lockb").is_file() || project_path.join("bun.lock").is_file() {
        Some("bun")
    } else if project_path.join("package-lock.json").is_file() {
        Some("npm")
    } else {
        None
    };

    let package_manager = pkg_mgr_field.or_else(|| lockfile_mgr.map(String::from));

    let node_engine = parsed
        .as_ref()
        .and_then(|v| v.get("engines"))
        .and_then(|e| e.get("node"))
        .and_then(|n| n.as_str())
        .map(|n| format!("node {n}"));

    EcosystemInfo {
        name: "Node.js".to_string(),
        package_manager,
        detail: node_engine,
    }
}

fn detect_rust_ecosystem(cargo_str: &str) -> EcosystemInfo {
    let parsed: Option<toml::Value> = cargo_str.parse::<toml::Value>().ok();
    let pkg = parsed.as_ref().and_then(|v| v.get("package"));
    let edition = pkg
        .and_then(|p| p.get("edition"))
        .and_then(|e| e.as_str())
        .map(|e| format!("edition {e}"));
    let msrv = pkg
        .and_then(|p| p.get("rust-version"))
        .and_then(|r| r.as_str())
        .map(|r| format!("MSRV {r}"));

    let detail = match (edition, msrv) {
        (Some(ed), Some(ms)) => Some(format!("{ed}, {ms}")),
        (Some(ed), None) => Some(ed),
        (None, Some(ms)) => Some(ms),
        (None, None) => None,
    };

    EcosystemInfo {
        name: "Rust".to_string(),
        package_manager: Some("cargo".to_string()),
        detail,
    }
}

fn detect_python_ecosystem(project_path: &Path) -> EcosystemInfo {
    let package_manager = if project_path.join("uv.lock").is_file() {
        Some("uv".to_string())
    } else if project_path.join("poetry.lock").is_file() {
        Some("poetry".to_string())
    } else if project_path.join("Pipfile.lock").is_file() {
        Some("pipenv".to_string())
    } else if project_path.join("requirements.txt").is_file() {
        Some("pip".to_string())
    } else {
        None
    };

    let detail = fs::read_to_string(project_path.join("pyproject.toml"))
        .ok()
        .and_then(|s| s.parse::<toml::Value>().ok())
        .and_then(|v| {
            v.get("project")
                .and_then(|p| p.get("requires-python"))
                .and_then(|r| r.as_str())
                .map(|r| format!("python {r}"))
        });

    EcosystemInfo {
        name: "Python".to_string(),
        package_manager,
        detail,
    }
}

fn detect_go_ecosystem(gomod_str: &str) -> EcosystemInfo {
    let go_ver = gomod_str
        .lines()
        .find_map(|line| line.strip_prefix("go "))
        .map(|v| format!("go {}", v.trim()));

    EcosystemInfo {
        name: "Go".to_string(),
        package_manager: Some("go mod".to_string()),
        detail: go_ver,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mise_toml_variants() {
        let toml = r#"
[tools]
node = "22.1.0"
rust = { version = "1.85.0" }
python = ["3.12", "3.11"]
"#;
        let tools = parse_mise_toml(toml);
        assert_eq!(
            tools,
            vec![
                MiseTool {
                    name: "node".into(),
                    version: "22.1.0".into()
                },
                MiseTool {
                    name: "python".into(),
                    version: "3.12, 3.11".into()
                },
                MiseTool {
                    name: "rust".into(),
                    version: "1.85.0".into()
                },
            ]
        );
    }

    #[test]
    fn parses_tool_versions_with_comments() {
        let tv = "
# comment
nodejs 20.11.0
pnpm 9.1.0 # inline comment
";
        let tools = parse_tool_versions(tv);
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "nodejs");
        assert_eq!(tools[0].version, "20.11.0");
        assert_eq!(tools[1].name, "pnpm");
        assert_eq!(tools[1].version, "9.1.0");
    }
}
