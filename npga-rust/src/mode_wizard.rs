//! Creates user game modes from a program directory.
//!
//! A wizard mode is a JSON mode file carrying one `pathRegex` directory rule:
//! every executable under that directory — including exes added by later game
//! updates or临时拉起的启动器进程 — is proxied, without rescanning. The scan
//! in this module only feeds the "将加速这些程序" confirmation list.
//!
//! User modes live in `<data>/modes`, separate from the preset `mode/`
//! library, so repackaging never overwrites them.

use crate::core_config;
use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Directory holding user-created modes, next to `settings.json`.
pub fn user_modes_dir(data: &Path) -> PathBuf {
    data.join("modes")
}

/// True when the mode file was created by the wizard (and may be edited or
/// deleted from the UI). Preset files under `mode/` are read-only.
pub fn is_user_mode(file: &Path, data: &Path) -> bool {
    file.starts_with(user_modes_dir(data))
}

/// Builds the case-insensitive, path-anchored directory rule, e.g.
/// `(?i)^D:\\Games\\Game\\`. The trailing separator keeps `D:\Games\Game1`
/// from also matching `…\Game10\`.
pub fn directory_rule(dir: &Path) -> String {
    let mut prefix = dir.to_string_lossy().trim().to_owned();
    while prefix.ends_with('\\') || prefix.ends_with('/') {
        prefix.pop();
    }
    prefix.push('\\');
    format!("(?i)^{}", core_config::regex_escape(&prefix))
}

/// Recovers the directory from a wizard-generated rule (edit round-trip).
/// Hand-edited exotic regexes may not round-trip; the caller treats `None`
/// as "directory unknown, please pick again".
pub fn directory_from_rule(rule: &str) -> Option<String> {
    let body = rule.strip_prefix("(?i)^")?;
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('.') => out.push('.'),
                Some(other) => {
                    out.push(c);
                    out.push(other);
                }
                None => out.push(c),
            }
        } else {
            out.push(c);
        }
    }
    while out.ends_with('\\') || out.ends_with('/') {
        out.pop();
    }
    Some(out)
}

/// Recursively lists executable names under `dir` for the confirmation list.
/// Reparse points are never followed; `total` counts every exe even past
/// `limit`.
pub fn scan_executables(dir: &Path, limit: usize) -> (Vec<String>, usize) {
    let mut found = vec![];
    let mut total = 0usize;
    fn walk(dir: &Path, found: &mut Vec<String>, total: &mut usize, limit: usize) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                walk(&path, found, total, limit);
            } else if path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
            {
                *total += 1;
                if found.len() < limit
                    && let Some(name) = path.file_name().and_then(|n| n.to_str())
                {
                    found.push(name.to_owned());
                }
            }
        }
    }
    walk(dir, &mut found, &mut total, limit);
    found.sort();
    (found, total)
}

/// Writes a user mode with one directory rule. The file name is derived from
/// the title, so editing a mode overwrites its own file. An empty title is
/// rejected by the wizard UI before calling this.
pub fn save(data: &Path, title: &str, directory: &Path) -> Result<PathBuf> {
    let dir = user_modes_dir(data);
    fs::create_dir_all(&dir)?;
    let mode = serde_json::json!({
        "type": "ProcessMode",
        "remark": {"zh-CN": title.trim()},
        "pathRegex": [directory_rule(directory)],
        "handle": [],
        "bypass": [],
    });
    let file = dir.join(format!("{}.json", file_stem(title)));
    fs::write(&file, serde_json::to_vec_pretty(&mode)?)
        .with_context(|| format!("写入模式文件失败：{}", file.display()))?;
    Ok(file)
}

/// Deletes a user mode file.
pub fn delete(file: &Path) -> Result<()> {
    fs::remove_file(file).with_context(|| format!("删除模式文件失败：{}", file.display()))
}

/// Windows-safe file stem: keeps letters/digits (including CJK), spaces,
/// `-` and `_`; everything else collapses to a space.
fn file_stem(title: &str) -> String {
    let sanitized: String = title
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let collapsed = sanitized.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        "未命名模式".into()
    } else {
        collapsed
    }
}

/// Wizard dialog state. Pure data — the UI (ui.rs) renders and mutates it.
#[derive(Default)]
pub struct ModeWizard {
    pub title: String,
    pub directory: String,
    /// Executable names found under `previewed` (capped, sorted).
    pub preview: Vec<String>,
    /// Every executable found, even those past the preview cap.
    pub preview_total: usize,
    /// Directory text the current preview belongs to.
    pub previewed: String,
    /// Set when editing an existing user mode (the file to overwrite).
    pub editing: Option<PathBuf>,
    pub error: Option<String>,
}

impl ModeWizard {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            directory: String::new(),
            preview: vec![],
            preview_total: 0,
            previewed: String::new(),
            editing: None,
            error: None,
        }
    }

    /// Prefills from an existing user mode.
    pub fn edit(mode: &crate::modes::Mode) -> Self {
        let directory = mode
            .path_regexes
            .first()
            .and_then(|rule| directory_from_rule(rule))
            .unwrap_or_default();
        Self {
            title: mode.title.clone(),
            directory,
            preview: vec![],
            preview_total: 0,
            previewed: String::new(),
            editing: Some(mode.file.clone()),
            error: None,
        }
    }

    /// Refreshes the executable preview when the directory text changed.
    /// Called on folder pick or when the text field loses focus — never per
    /// keystroke, so a huge tree cannot stall the UI thread.
    pub fn rescan_preview(&mut self) {
        if self.previewed == self.directory {
            return;
        }
        self.previewed = self.directory.clone();
        let dir = PathBuf::from(self.directory.trim());
        if dir.is_dir() {
            let (names, total) = scan_executables(&dir, 200);
            self.preview = names;
            self.preview_total = total;
        } else {
            self.preview.clear();
            self.preview_total = 0;
        }
    }

    /// Persists the mode; returns the written file (the new selection).
    pub fn save(&mut self, data: &Path) -> Result<PathBuf> {
        let dir = PathBuf::from(self.directory.trim());
        if self.title.trim().is_empty() {
            self.error = Some("请先给模式起个名字".into());
            anyhow::bail!("标题为空");
        }
        if !dir.is_dir() {
            self.error = Some("程序目录不存在，请重新选择".into());
            anyhow::bail!("目录无效");
        }
        let file = save(data, &self.title, &dir)?;
        self.error = None;
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_rules_anchor_and_escape() {
        let rule = directory_rule(Path::new(r"D:\Games\Battle.net"));
        assert_eq!(rule, r"(?i)^D:\\Games\\Battle\.net\\");
        // Round-trips back to the picked directory.
        assert_eq!(
            directory_from_rule(&rule).as_deref(),
            Some(r"D:\Games\Battle.net")
        );
        // `Game1\` must not be matched by a `Game\` rule — the trailing
        // separator is part of the rule.
        let rule = directory_rule(Path::new(r"D:\Games\Game1"));
        assert!(rule.ends_with(r"Game1\\"));
    }

    #[test]
    fn save_and_scan_round_trip() {
        let temp = std::env::temp_dir().join(format!("npga-wizard-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp).unwrap();
        let game_dir = temp.join("My Game");
        fs::create_dir_all(game_dir.join("bin")).unwrap();
        fs::write(game_dir.join("game.exe"), b"MZ").unwrap();
        fs::write(game_dir.join("bin").join("helper.exe"), b"MZ").unwrap();
        fs::write(game_dir.join("readme.txt"), b"x").unwrap();

        let (names, total) = scan_executables(&game_dir, 200);
        assert_eq!(names, vec!["game.exe", "helper.exe"]);
        assert_eq!(total, 2);

        let data = temp.join("data");
        let file = save(&data, "我的游戏 1", &game_dir).unwrap();
        assert!(file.starts_with(user_modes_dir(&data)));
        assert!(is_user_mode(&file, &data));

        // The written file must load back as a wizard-editable mode.
        let (mut modes, errors) = crate::modes::scan(&user_modes_dir(&data));
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(modes.len(), 1);
        let mode = modes.pop().unwrap();
        assert_eq!(mode.title, "我的游戏 1");
        assert_eq!(
            mode.path_regexes,
            vec![directory_rule(&game_dir)],
            "CJK 标题与目录规则应原样往返"
        );
        assert_eq!(
            ModeWizard::edit(&mode).directory,
            game_dir.to_string_lossy()
        );
        // A same-title re-save overwrites the same file (edit semantics).
        let again = save(&data, "我的游戏 1", &game_dir).unwrap();
        assert_eq!(again, file);

        delete(&file).unwrap();
        assert!(!file.exists());
        fs::remove_dir_all(&temp).ok();
    }
}
