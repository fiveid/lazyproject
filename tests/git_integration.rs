use std::fs;
use std::path::Path;
use std::process::Command;

use lazyproject::git::RemoteProvider;
use lazyproject::scanner::scan_workspace;
use tempfile::tempdir;

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("failed to execute git");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn integration_scans_ahead_dirty_credentials_and_toolchains_readonly() {
    let workspace = tempdir().unwrap();

    // 1. Create a bare remote repo
    let remote_dir = tempdir().unwrap();
    git(remote_dir.path(), &["init", "--bare", "-b", "main"]);

    // 2. Clone it into workspace/my-app
    let app_dir = workspace.path().join("my-app");
    let clone_out = Command::new("git")
        .args([
            "clone",
            remote_dir.path().to_str().unwrap(),
            app_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(clone_out.status.success());

    git(&app_dir, &["config", "user.name", "Test"]);
    git(&app_dir, &["config", "user.email", "test@example.com"]);

    // Initial commit & push
    fs::write(app_dir.join("README.md"), "# My App\n").unwrap();
    git(&app_dir, &["add", "README.md"]);
    git(&app_dir, &["commit", "-m", "chore: initial commit"]);
    git(&app_dir, &["push", "-u", "origin", "main"]);

    // Add a second local unpushed commit (ahead by 1)
    fs::write(
        app_dir.join("mise.toml"),
        "[tools]\nnode = \"22.0.0\"\nrust = \"1.85.0\"\n",
    )
    .unwrap();
    fs::write(
        app_dir.join("package.json"),
        "{\"name\":\"my-app\",\"engines\":{\"node\":\">=22\"}}",
    )
    .unwrap();
    fs::write(app_dir.join("pnpm-lock.yaml"), "lockfileVersion: '9.0'\n").unwrap();
    git(
        &app_dir,
        &["add", "mise.toml", "package.json", "pnpm-lock.yaml"],
    );
    git(&app_dir, &["commit", "-m", "feat: add mise and node"]);

    // Make working tree dirty (1 untracked)
    fs::write(app_dir.join("notes.txt"), "uncommitted").unwrap();

    // 3. Create a second repo with a credentialed GitHub URL
    let cred_dir = workspace.path().join("secret-url-repo");
    fs::create_dir(&cred_dir).unwrap();
    git(&cred_dir, &["init", "-b", "main"]);
    git(
        &cred_dir,
        &[
            "remote",
            "add",
            "origin",
            "https://alice:ghp_SUPERSECRETTOKEN@github.com/alice/secret.git",
        ],
    );

    // 4. Create a non-git directory
    fs::create_dir(workspace.path().join("scratch-notes")).unwrap();

    // Run scan
    let res = scan_workspace(workspace.path()).unwrap();
    assert_eq!(res.git_projects.len(), 2);
    assert_eq!(res.non_git_projects.len(), 1);
    assert_eq!(res.non_git_projects[0].name, "scratch-notes");

    let my_app = res
        .git_projects
        .iter()
        .find(|p| p.name == "my-app")
        .unwrap();
    let git_info = my_app.git.as_ref().unwrap();
    assert_eq!(git_info.branch, "main");
    assert_eq!(git_info.ahead, 1);
    assert_eq!(git_info.behind, 0);
    assert_eq!(git_info.untracked, 1);
    assert!(git_info.is_dirty());
    assert_eq!(my_app.toolchain.mise_source.as_deref(), Some("mise.toml"));
    assert_eq!(my_app.toolchain.mise_tools.len(), 2);
    assert_eq!(my_app.toolchain.ecosystems.len(), 1);
    assert_eq!(
        my_app.toolchain.ecosystems[0].package_manager.as_deref(),
        Some("pnpm")
    );

    // Verify credentials are stripped from remote URL
    let cred_repo = res
        .git_projects
        .iter()
        .find(|p| p.name == "secret-url-repo")
        .unwrap();
    let cred_git = cred_repo.git.as_ref().unwrap();
    assert_eq!(cred_git.provider, RemoteProvider::GitHub);
    assert_eq!(
        cred_git.remote_url.as_deref(),
        Some("https://github.com/alice/secret.git")
    );
    assert!(
        !cred_git
            .remote_url
            .as_deref()
            .unwrap()
            .contains("SUPERSECRETTOKEN")
    );
}
