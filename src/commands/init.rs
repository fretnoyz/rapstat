use anyhow::{Context, Result};
use git2::Repository;
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::Path;

const PRE_COMMIT_HOOK: &str = include_str!("../../.rapstat/hooks/pre-commit");
const PRE_PUSH_HOOK: &str = include_str!("../../.rapstat/hooks/pre-push");

const DEFAULT_CONFIG: &str = r#"[project]
name = "{name}"

[hooks]
pre_commit = true
pre_push = true

[validation]
check_context_md = true
flag_only = true
"#;

pub fn run() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::discover(&cwd).context("not a git repository")?;
    let repo_root = repo
        .workdir()
        .context("bare repositories are not supported")?
        .to_path_buf();

    // Write hook scripts to .rapstat/hooks/ (version-controlled)
    let rapstat_hooks = repo_root.join(".rapstat").join("hooks");
    fs::create_dir_all(&rapstat_hooks)?;
    write_hook(&rapstat_hooks.join("pre-commit"), PRE_COMMIT_HOOK)?;
    write_hook(&rapstat_hooks.join("pre-push"), PRE_PUSH_HOOK)?;

    // Symlink hooks into .git/hooks/ using a path relative to .git/hooks/
    // so the symlinks survive repo moves.
    let git_hooks = repo.path().join("hooks");
    fs::create_dir_all(&git_hooks)?;
    symlink_hook("pre-commit", &git_hooks)?;
    symlink_hook("pre-push", &git_hooks)?;

    // Create .rapstat/config.toml if absent
    let config_path = repo_root.join(".rapstat").join("config.toml");
    if !config_path.exists() {
        let project_name = repo_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let content = DEFAULT_CONFIG.replace("{name}", project_name);
        fs::write(&config_path, content)
            .with_context(|| format!("failed to write {}", config_path.display()))?;
        println!("Created .rapstat/config.toml");
    }

    println!("rapstat initialized");
    Ok(())
}

fn write_hook(path: &Path, content: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, content)
        .with_context(|| format!("failed to write {}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("failed to chmod {}", path.display()))?;
    Ok(())
}

/// Symlink `.git/hooks/<name>` → `../../.rapstat/hooks/<name>` (relative path).
fn symlink_hook(name: &str, git_hooks: &Path) -> Result<()> {
    let link = git_hooks.join(name);
    let target = Path::new("../../.rapstat/hooks").join(name);

    // Remove any existing hook (symlink or file) so we can replace it cleanly.
    if link.symlink_metadata().is_ok() {
        fs::remove_file(&link)
            .with_context(|| format!("failed to remove existing hook {}", link.display()))?;
    }

    unix_fs::symlink(&target, &link)
        .with_context(|| format!("failed to symlink {} -> {}", link.display(), target.display()))?;

    println!("  installed .git/hooks/{}", name);
    Ok(())
}
