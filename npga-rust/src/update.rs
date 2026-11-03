//! GitHub release checking, ported from the C# `UpdateChecker`.
//!
//! Fetches the repository release list, filters pre-releases, selects the
//! newest by version comparison, and parses the `| file | sha256 |` table from
//! the release body for integrity verification after download.

use anyhow::{Context, Result, bail};
use serde::Deserialize;

// The product was renamed to NPGA, but the GitHub repository still lives at
// `OffroadOps/Hatch` — the update check must point at the real slug. If the
// repository is ever renamed, change it here AND on GitHub with a redirect in
// place, otherwise `check()` silently finds no releases.
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

/// Compares dotted version strings ("v1.2.3" or "3.0.0-rc.1+build" style).
///
/// Numeric segments compare numerically and a missing segment counts as 0, so
/// "3.0.0" equals "3.0.0.0". A pre-release suffix always sorts *below* the
/// release it belongs to (semver §11), which is what makes "3.0.0-rc.1" older
/// than "3.0.0"; pre-release identifiers compare field by field, numerically
/// when both sides are numbers. Build metadata never affects precedence.
pub fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    struct Version {
        numbers: Vec<u64>,
        pre: Option<String>,
    }

    fn parse(version: &str) -> Version {
        let version = version.trim().trim_start_matches(['v', 'V']);
        // Everything after '+' is build metadata and does not order releases.
        let version = version.split('+').next().unwrap_or(version);
        let (core, pre) = match version.split_once('-') {
            Some((core, pre)) => (core, Some(pre.to_owned())),
            None => (version, None),
        };
        let numbers = core
            .split('.')
            .map(|part| {
                let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
                digits.parse().unwrap_or(0)
            })
            .collect();
        Version { numbers, pre }
    }

    fn compare_pre(left: &str, right: &str) -> Ordering {
        let mut left = left.split('.');
        let mut right = right.split('.');
        loop {
            match (left.next(), right.next()) {
                (None, None) => return Ordering::Equal,
                // A shorter identifier list is older: 1.0.0-rc < 1.0.0-rc.1.
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
                (Some(l), Some(r)) => {
                    let order = match (l.parse::<u64>(), r.parse::<u64>()) {
                        (Ok(l), Ok(r)) => l.cmp(&r),
                        // Numeric identifiers always sort below alphanumeric ones.
                        (Ok(_), Err(_)) => Ordering::Less,
                        (Err(_), Ok(_)) => Ordering::Greater,
                        (Err(_), Err(_)) => l.cmp(r),
                    };
                    if order != Ordering::Equal {
                        return order;
                    }
                }
            }
        }
    }

    let left = parse(left);
    let right = parse(right);
    let width = left.numbers.len().max(right.numbers.len());
    for i in 0..width {
        let l = left.numbers.get(i).copied().unwrap_or(0);
        let r = right.numbers.get(i).copied().unwrap_or(0);
        match l.cmp(&r) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    match (left.pre.as_deref(), right.pre.as_deref()) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(l), Some(r)) => compare_pre(l, r),
    }
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
    fn prerelease_versions_sort_below_their_release() {
        use std::cmp::Ordering;
        // A release candidate must never win over the release it precedes,
        // otherwise `check` would keep offering the same pre-release forever.
        assert_eq!(compare_versions("v3.0.0-rc.1", "3.0.0"), Ordering::Less);
        assert_eq!(compare_versions("3.0.0", "v3.0.0-beta"), Ordering::Greater);
        assert_eq!(compare_versions("3.0.0-beta.2", "3.0.0"), Ordering::Less);
        assert_eq!(
            compare_versions("3.0.0-alpha", "3.0.0-beta"),
            Ordering::Less
        );
        assert_eq!(
            compare_versions("3.0.0-beta.10", "3.0.0-beta.2"),
            Ordering::Greater
        );
        assert_eq!(
            compare_versions("3.0.0-rc.1", "3.0.0-rc.1.1"),
            Ordering::Less
        );
        // Build metadata is ignored and numeric pre-release fields are not
        // compared as strings ("10" must be newer than "2").
        assert_eq!(compare_versions("3.0.0+build.7", "v3.0.0"), Ordering::Equal);
        assert_eq!(
            compare_versions("3.0.1-beta", "3.0.0-rc.9"),
            Ordering::Greater
        );
    }

    #[test]
    fn checksum_table_parsing() {
        let release = Release {
            tag_name: "v9.9.9".into(),
            html_url: "https://example.com".into(),
            prerelease: false,
            body: Some(
                "# NPGA\n\n| NPGA.zip | ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789 |\n| NPGA-Rust.zip | 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF |\n| NPGASetup.exe | FEDCBA9876543210FEDCBA9876543210FEDCBA9876543210FEDCBA9876543210 |\n"
                    .into(),
            ),
        };
        let all = checksums(&release, None);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].0, "NPGA.zip");
        let filtered = checksums(&release, Some("setup"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].0, "NPGASetup.exe");
        // The published asset is named NPGA-Rust.zip; "npga.zip" would never
        // match it as a substring, so the runtime keyword must stay npga-rust.
        let package = checksums(&release, Some("npga-rust"));
        assert_eq!(package.len(), 1);
        assert_eq!(package[0].0, "NPGA-Rust.zip");
        assert_eq!(package[0].1.len(), 64);
    }
}
