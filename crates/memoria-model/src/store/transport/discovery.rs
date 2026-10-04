//! Engine data-dir discovery: `engine.lock.json` lives in the platform data
//! dir, renamed Kosmos → Mundus in 0.10.0. A new app may run against a
//! pre-rename Engine (the Engine moves the dir itself on first start, but the
//! move can lag), so the legacy candidates stay as fallbacks.
// MIGRATION(KOS-267): drop the legacy candidates after 2026-11-01.

use std::path::PathBuf;

use super::EngineError;

/// Per-OS config dir: `%APPDATA%` (Windows), `$XDG_CONFIG_HOME` / `~/.config`
/// (all unix, incl. macOS) — mirrors the Engine's `lock_file` resolution.
#[cfg(windows)]
fn config_base(get: &dyn Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    get("APPDATA")
}
#[cfg(unix)]
fn config_base(get: &dyn Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    get("XDG_CONFIG_HOME").or_else(|| get("HOME").map(|v| v.join(".config")))
}

/// Candidates in priority order: `MUNDUS_DATA_DIR` → `KOSMOS_DATA_DIR`
/// (legacy) → `<config>/Mundus` → `<config>/Kosmos` (legacy).
fn candidates(get: &dyn Fn(&str) -> Option<PathBuf>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for key in [
        crate::brand::DATA_DIR_ENV,
        crate::brand::LEGACY_DATA_DIR_ENV,
    ] {
        if let Some(path) = get(key).filter(|v| !v.as_os_str().is_empty()) {
            dirs.push(path);
        }
    }
    if let Some(base) = config_base(get) {
        dirs.push(base.join(crate::brand::DATA_DIR_NAME));
        dirs.push(base.join(crate::brand::LEGACY_DATA_DIR_NAME));
    }
    dirs
}

/// First candidate holding `engine.lock.json` wins. With no lock anywhere the
/// primary candidate is returned so callers still see a stable path (settings
/// files sit beside the lock) and `lock()` reports the same missing-lock error.
pub(super) fn resolve(get: &dyn Fn(&str) -> Option<PathBuf>) -> Result<PathBuf, EngineError> {
    let dirs = candidates(get);
    dirs.iter()
        .find(|dir| dir.join("engine.lock.json").is_file())
        .cloned()
        .or_else(|| dirs.into_iter().next())
        .ok_or_else(|| {
            EngineError::local(
                super::ErrorKind::NotRunning,
                "no data dir candidate resolves",
            )
        })
}

pub(super) fn data_dir() -> Result<PathBuf, EngineError> {
    resolve(&|key| std::env::var_os(key).map(PathBuf::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Env-var lookup over a fixed map — tests never touch the real process env.
    fn fake_env(pairs: Vec<(&'static str, PathBuf)>) -> impl Fn(&str) -> Option<PathBuf> {
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.clone())
        }
    }

    fn tmpdir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("memoria-gpui-kos266-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_lock(dir: &std::path::Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("engine.lock.json"), "{}").unwrap();
    }

    /// Env var that feeds `config_base` on this OS, bound to `root`.
    #[cfg(windows)]
    fn config_env(root: &std::path::Path) -> Vec<(&'static str, PathBuf)> {
        vec![("APPDATA", root.to_path_buf())]
    }
    #[cfg(unix)]
    fn config_env(root: &std::path::Path) -> Vec<(&'static str, PathBuf)> {
        vec![("XDG_CONFIG_HOME", root.to_path_buf())]
    }

    #[test]
    fn env_override_wins() {
        let dir = tmpdir("env");
        write_lock(&dir);
        let env = fake_env(vec![(crate::brand::DATA_DIR_ENV, dir.clone())]);
        assert_eq!(resolve(&env).unwrap(), dir);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn legacy_env_used_when_primary_has_no_lock() {
        let root = tmpdir("legacy-env");
        let primary = root.join("new");
        let legacy = root.join("old");
        std::fs::create_dir_all(&primary).unwrap();
        write_lock(&legacy);
        let env = fake_env(vec![
            (crate::brand::DATA_DIR_ENV, primary),
            (crate::brand::LEGACY_DATA_DIR_ENV, legacy.clone()),
        ]);
        assert_eq!(resolve(&env).unwrap(), legacy);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn config_dir_and_legacy_fallback() {
        let root = tmpdir("config");
        let legacy = root.join(crate::brand::LEGACY_DATA_DIR_NAME);
        write_lock(&legacy);
        let env = fake_env(config_env(&root));
        assert_eq!(resolve(&env).unwrap(), legacy);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mundus_dir_beats_legacy_dir() {
        let root = tmpdir("both");
        let mundus = root.join(crate::brand::DATA_DIR_NAME);
        write_lock(&mundus);
        write_lock(&root.join(crate::brand::LEGACY_DATA_DIR_NAME));
        let env = fake_env(config_env(&root));
        assert_eq!(resolve(&env).unwrap(), mundus);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn no_lock_anywhere_returns_primary_candidate() {
        let root = tmpdir("nolock");
        let primary = root.join(crate::brand::DATA_DIR_NAME);
        let env = fake_env(config_env(&root));
        assert_eq!(resolve(&env).unwrap(), primary);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn empty_env_var_is_ignored() {
        let root = tmpdir("empty");
        let mundus = root.join(crate::brand::DATA_DIR_NAME);
        write_lock(&mundus);
        let mut pairs = vec![(crate::brand::DATA_DIR_ENV, PathBuf::from(""))];
        pairs.extend(config_env(&root));
        let env = fake_env(pairs);
        assert_eq!(resolve(&env).unwrap(), mundus);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn no_candidates_is_an_error() {
        let env = fake_env(vec![]);
        assert!(matches!(
            resolve(&env),
            Err(EngineError {
                kind: crate::store::transport::ErrorKind::NotRunning,
                ..
            })
        ));
    }
}
