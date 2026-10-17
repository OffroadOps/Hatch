//! GitHub release checking, ported from the C# `UpdateChecker`.
//!
//! Fetches the repository release list, filters pre-releases, selects the
//! newest by version comparison, and parses the `| file | sha256 |` table from
//! the release body for integrity verification after download.

use anyhow::{Context, Result, bail};
use serde::Deserialize;

const REPO: &str = "OffroadOps/Hatch";
const API: &str = "https://api.github.com";

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub html_url: String,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub body: Option<String>,
}

/// Compares dotted version strings ("v1.2.3" or "1.2.3-beta" style).
/// Numeric segments compare numerically; a missing segment counts as 0;
/// trailing non-numeric parts compare lexicographically.
pub fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let clean = |s: &str| s.trim().trim_start_matches(['v', 'V']).to_owned();
    let parse = |s: &str| -> (Vec<u64>, String) {
        let mut numbers = vec![];
        let mut rest = String::new();
        for part in clean(s).split('.') {
            let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                rest.push_str(part);
                rest.push('.');
            } else {
                numbers.push(digits.parse().unwrap_or(0));
                let tail: String = part.chars().skip(digits.len()).collect();
                if !tail.is_empty() {
                    rest.push_str(&tail);
                    rest.push('.');
                }
            }
        }
        (numbers, rest)
    };
    let (left_numbers, left_rest) = parse(left);
    let (right_numbers, right_rest) = parse(right);
    let width = left_numbers.len().max(right_numbers.len());
    for i in 0..width {
        let l = left_numbers.get(i).copied().unwrap_or(0);
        let r = right_numbers.get(i).copied().unwrap_or(0);
        match l.cmp(&r) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    left_rest.cmp(&right_rest)
}

pub fn check(current_version: &str, allow_prerelease: bool) -> Result<Option<Release>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(10000))
        .build()
        .context("创建更新客户端失败")?;
    let response = client
        .get(format!("{API}/repos/{REPO}/releases"))
        .header("User-Agent", "NPGA-Rust")
        .header("Accept", "application/vnd.github+json")
        .send()
        .context("访问 GitHub Releases 失败")?;
    if !response.status().is_success() {
        bail!("GitHub Releases API 返回 {}", response.status());
    }
    let releases: Vec<Release> = response.json().context("解析 Releases 响应失败")?;
    let latest = releases
        .into_iter()
        .filter(|r| allow_prerelease || !r.prerelease)
        .max_by(|a, b| compare_versions(&a.tag_name, &b.tag_name))
        .context("仓库没有符合条件的 Release")?;
    if compare_versions(&latest.tag_name, current_version) == std::cmp::Ordering::Greater {
        Ok(Some(latest))
    } else {
        Ok(None)
    }
}

/// Parses `| filename | sha256 |` rows from the release notes. Returns entries
/// matching `keyword` (case-insensitive) or all rows when no keyword is given.
pub fn checksums(release: &Release, keyword: Option<&str>) -> Vec<(String, String)> {
    let body = release.body.as_deref().unwrap_or_default();
    let mut rows = vec![];
    for line in body.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = trimmed.split('|').skip(1).collect();
        if cells.len() < 2 {
            continue;
        }
        let name = cells[0].trim();
        let sha = cells[1].trim().to_lowercase();
        if sha.len() == 64 && sha.chars().all(|c| c.is_ascii_hexdigit()) {
            match keyword {
                Some(k) if !name.to_lowercase().contains(&k.to_lowercase()) => continue,
                _ => rows.push((name.to_owned(), sha)),
            }
        }
    }
    rows
}

/// Release notes up to the checksum section, mirroring the C# helper.
pub fn changelog(release: &Release) -> String {
    let body = release.body.as_deref().unwrap_or_default();
    let mut lines = vec![];
    let mut started = false;
    for line in body.lines() {
        if !started {
            if line.starts_with('#') {
                started = true;
            } else {
                continue;
            }
        }
        let lower = line.to_lowercase();
        if lower.contains("校验和") || lower.contains("checksum") {
            break;
        }
        lines.push(line);
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_matches_release_semantics() {
        assert_eq!(
            compare_versions("v3.0.1", "3.0.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_versions("v2.0.0", "2.0.0"),
            std::cmp::Ordering::Equal
        );
        assert_eq!(
            compare_versions("3.0.0", "3.0.0.1"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_versions("v10.0.0", "v9.9.9"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn checksum_table_parsing() {
        let release = Release {
            tag_name: "v9.9.9".into(),
            html_url: "https://example.com".into(),
            prerelease: false,
            body: Some(
                "# NPGA\n\n| NPGA.zip | ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789 |\n| NPGASetup.exe | 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF |\n"
                    .into(),
            ),
        };
        let all = checksums(&release, None);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].0, "NPGA.zip");
        let filtered = checksums(&release, Some("setup"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].0, "NPGASetup.exe");
    }
}
