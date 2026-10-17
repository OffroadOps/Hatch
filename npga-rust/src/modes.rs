use crate::model::RouteKind;
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Mode {
    pub file: PathBuf,
    pub title: String,
    pub kind: RouteKind,
    pub handle: Vec<String>,
    pub bypass: Vec<String>,
    pub options: Value,
    pub argument: String,
}

pub fn scan(root: &Path) -> (Vec<Mode>, Vec<String>) {
    let mut files = vec![];
    fn walk(root: &Path, files: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if entry.file_type().is_ok_and(|t| t.is_symlink()) {
                    continue;
                }
                if path.is_dir() {
                    walk(&path, files);
                } else if matches!(
                    path.extension().and_then(|s| s.to_str()),
                    Some("txt" | "json")
                ) {
                    files.push(path);
                }
            }
        }
    }
    walk(root, &mut files);
    files.sort();
    let mut modes = vec![];
    let mut errors = vec![];
    for file in files {
        match load(&file, root, &mut HashSet::new()) {
            Ok(mode) => modes.push(mode),
            Err(e) => errors.push(format!("{}: {e}", file.display())),
        }
    }
    (modes, errors)
}

fn load(file: &Path, root: &Path, visited: &mut HashSet<PathBuf>) -> Result<Mode> {
    let canonical = file.canonicalize()?;
    if !canonical.starts_with(root.canonicalize()?) {
        bail!("include 超出模式目录");
    }
    if visited.len() >= 32 || !visited.insert(canonical.clone()) {
        bail!("模式 include 循环或层级过深");
    }
    let text = fs::read_to_string(file).context("模式文件编码无效")?;
    let text = text.trim_start_matches('\u{feff}');
    let mut mode = Mode {
        file: file.into(),
        title: String::new(),
        kind: RouteKind::Process,
        handle: vec![],
        bypass: vec![],
        options: Value::Null,
        argument: "--preset uu".into(),
    };
    if file.extension().is_some_and(|s| s == "json") {
        let v: Value = serde_json::from_str(text)?;
        mode.title = v["remark"]["zh-CN"]
            .as_str()
            .or(v["remark"]["zh"].as_str())
            .or(v["remark"]["en"].as_str())
            .unwrap_or("未命名模式")
            .into();
        mode.kind = match v["type"].as_str() {
            Some("ProcessMode") => RouteKind::Process,
            Some("TunMode") => RouteKind::Tun,
            Some("ShareMode") => RouteKind::Share,
            _ => bail!("不支持的 JSON 模式类型"),
        };
        for (key, target) in [("handle", &mut mode.handle), ("bypass", &mut mode.bypass)] {
            if let Some(values) = v[key].as_array() {
                for value in values {
                    target.push(value.as_str().context("路由规则必须为文本")?.into());
                }
            }
        }
        mode.argument = v["argument"].as_str().unwrap_or("--preset uu").into();
        mode.options = v;
    } else {
        let mut lines = text.lines();
        let head = lines
            .next()
            .and_then(|s| s.strip_prefix('#'))
            .context("模式缺少元数据行")?;
        let fields: Vec<_> = head.split(',').map(str::trim).collect();
        mode.title = fields[0].into();
        let kind = fields.get(1).copied().unwrap_or("0");
        mode.kind = match kind {
            "0" => RouteKind::Process,
            "1" | "2" => RouteKind::Tun,
            "6" => RouteKind::Share,
            _ => bail!("不支持的 TXT 模式类型"),
        };
        for line in lines
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.starts_with("//"))
        {
            if let Some(include) = line.strip_prefix("#include") {
                let name = include
                    .trim()
                    .trim_matches(['<', '>'])
                    .replace(".h", ".txt");
                let relative = file.parent().unwrap_or(root).join(&name);
                let include_file = if relative.exists() {
                    relative
                } else {
                    root.join(&name)
                };
                let nested = load(&include_file, root, visited)?;
                if mode.kind != nested.kind {
                    bail!("include 的模式类型不一致");
                }
                mode.handle.extend(nested.handle);
                mode.bypass.extend(nested.bypass);
            } else if mode.kind == RouteKind::Share {
                mode.argument = line.into();
            } else if kind == "2" {
                mode.bypass.push(line.into());
            } else if let Some(rule) = line.strip_prefix('!') {
                mode.bypass.push(rule.into());
            } else {
                mode.handle.push(line.into());
            }
        }
        if kind == "2" {
            mode.handle.push("0.0.0.0/0".into());
        }
    }
    if mode.kind == RouteKind::Tun {
        for value in mode.handle.iter().chain(&mode.bypass) {
            value
                .parse::<ipnet::IpNet>()
                .with_context(|| format!("无效 CIDR: {value}"))?;
        }
    }
    visited.remove(&canonical);
    Ok(mode)
}
