use crate::{model::Settings, platform};
use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const SECRETS: &[&str] = &[
    "Password",
    "Username",
    "UserID",
    "QUICSecret",
    "PrivateKey",
    "PreSharedKey",
    "ObfuscationPassword",
];

fn credentials(settings: &mut Settings, encrypt: bool) -> Result<()> {
    for node in &mut settings.nodes {
        for &key in SECRETS {
            if let Some(Value::String(value)) = node.fields.get_mut(key) {
                *value = if encrypt {
                    platform::protect(value)?
                } else {
                    platform::unprotect(value)?
                };
            }
        }
    }
    for sub in &mut settings.subscriptions {
        sub.link = if encrypt {
            platform::protect(&sub.link)?
        } else {
            platform::unprotect(&sub.link)?
        };
    }
    Ok(())
}

pub fn import(path: &Path) -> Result<Settings> {
    let bytes = fs::read(path).context("读取配置失败")?;
    let raw: Value =
        serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes))
            .context("配置 JSON 无效")?;
    let has_rust_selection = raw.get("SelectedNode").is_some();
    let mut settings: Settings = serde_json::from_value(raw).context("配置格式无效")?;
    credentials(&mut settings, false)?;
    if !has_rust_selection
        && let Some(index) = settings
            .legacy
            .get("ServerComboBoxSelectedIndex")
            .and_then(Value::as_u64)
    {
        settings.selected_node = index as usize;
    }
    settings.selected_node = settings
        .selected_node
        .min(settings.nodes.len().saturating_sub(1));
    for sub in &mut settings.subscriptions {
        if sub.id.is_empty() {
            sub.id = uuid::Uuid::new_v4().to_string();
        }
    }
    settings.validate()?;
    Ok(settings)
}

#[derive(Clone)]
pub struct Store {
    pub directory: PathBuf,
}
impl Store {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
    pub fn file(&self) -> PathBuf {
        self.directory.join("settings.json")
    }
    pub fn load(&self) -> Result<(Settings, Option<String>)> {
        if !self.file().exists() {
            return Ok((Settings::default(), None));
        }
        match import(&self.file()) {
            Ok(s) => Ok((s, None)),
            Err(e) => {
                let backup = self.directory.join("settings.json.bak");
                let s = import(&backup).context("主配置及备份读取失败，已保留原文件")?;
                Ok((s, Some(format!("主配置无法读取，已加载备份：{e}"))))
            }
        }
    }
    pub fn save(&self, settings: &Settings) -> Result<()> {
        settings.validate()?;
        let mut encrypted = settings.clone();
        credentials(&mut encrypted, true)?;
        fs::create_dir_all(&self.directory)?;
        let temp = self
            .directory
            .join(format!("settings.{}.tmp", uuid::Uuid::new_v4()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(&serde_json::to_vec_pretty(&encrypted)?)?;
        file.sync_all()?;
        drop(file);
        let result = (|| {
            // Never replace a valid backup with an unreadable primary configuration.
            if self.file().exists() {
                if import(&self.file()).is_ok() {
                    fs::copy(self.file(), self.directory.join("settings.json.bak"))?;
                } else {
                    fs::copy(
                        self.file(),
                        self.directory
                            .join(format!("settings.corrupt.{}.json", uuid::Uuid::new_v4())),
                    )?;
                }
            }
            platform::atomic_replace(&temp, &self.file())
        })();
        if temp.exists() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}
