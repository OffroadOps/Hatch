//! Download throughput measurement through the local HTTP proxy port.
//!
//! The proxy port is served by the running core (Xray/sing-box HTTP inbound),
//! so the measurement includes the full acceleration path. Uses only reqwest's
//! built-in HTTP proxy support — no SOCKS feature dependency.

use anyhow::Context;
use std::time::{Duration, Instant};

/// Candidate endpoints, tried in order. Cloudflare needs no key and honours the
/// byte count in the URL; the others are plain static files for fallback.
const CANDIDATES: &[&str] = &[
    "https://speed.cloudflare.com/__down?bytes=26214400",
    "http://cachefly.cachefly.net/10mb.test",
];

#[derive(Debug)]
pub struct SpeedResult {
    pub mbps: f64,
    pub bytes: u64,
    pub seconds: f64,
    pub endpoint: String,
}

/// Measures download speed through the local HTTP proxy. Returns an error when
/// no endpoint is reachable (connection refused, timeout, or tiny transfer).
pub fn measure(http_proxy: &str, timeout_ms: u64) -> anyhow::Result<SpeedResult> {
    let proxy = reqwest::Proxy::http(http_proxy)
        .map_err(|e| anyhow::anyhow!("代理地址无效 {http_proxy}: {e}"))?;
    let client = reqwest::blocking::Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_millis(timeout_ms))
        .connect_timeout(Duration::from_millis(4000))
        .build()
        .context("创建测速客户端失败")?;
    let download = |url: &str| -> anyhow::Result<u64> { download_once(&client, url) };
    measure_with(&download, CANDIDATES, 512 * 1024, 0.05)
}

/// Downloads one endpoint and returns the number of transferred bytes.
fn download_once(client: &reqwest::blocking::Client, url: &str) -> anyhow::Result<u64> {
    let mut response = client.get(url).send().map_err(|e| anyhow::anyhow!("{e}"))?;
    if !response.status().is_success() {
        anyhow::bail!("返回 {}", response.status());
    }
    let mut total = 0u64;
    let mut buffer = [0u8; 32768];
    loop {
        match std::io::Read::read(&mut response, &mut buffer) {
            Ok(0) => break,
            Ok(n) => total += n as u64,
            Err(e) => anyhow::bail!("传输中断: {e}"),
        }
    }
    Ok(total)
}

/// Measures the download speed with an explicit downloader and endpoint list.
///
/// `min_bytes` and `min_seconds` describe the smallest transfer that still
/// counts as a speed sample; smaller responses are treated as a failure of
/// that endpoint.
fn measure_with(
    download: &dyn Fn(&str) -> anyhow::Result<u64>,
    candidates: &[&str],
    min_bytes: u64,
    min_seconds: f64,
) -> anyhow::Result<SpeedResult> {
    let mut last_error = None;
    for url in candidates {
        let begin = Instant::now();
        match download(url) {
            Ok(total) => {
                let seconds = begin.elapsed().as_secs_f64();
                if total < min_bytes || seconds <= min_seconds {
                    // Too small to call a speed sample; treat as failure.
                    last_error = Some(format!("{url} 传输量过小"));
                    continue;
                }
                return Ok(SpeedResult {
                    mbps: total as f64 * 8.0 / seconds / 1_000_000.0,
                    bytes: total,
                    seconds,
                    endpoint: (*url).into(),
                });
            }
            Err(e) => last_error = Some(format!("{url}: {e:#}")),
        }
    }
    Err(anyhow::anyhow!(
        "测速失败：{}",
        last_error.unwrap_or_else(|| "没有可用测速节点".into())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_is_measured_and_reported() {
        let download = |_url: &str| Ok(8 * 1024 * 1024u64);
        let result =
            measure_with(&download, &["https://example.test/big"], 1024, 0.0).expect("测速应成功");
        assert_eq!(result.bytes, 8 * 1024 * 1024);
        assert!(result.seconds > 0.0);
        assert!(result.mbps > 0.0);
        assert_eq!(result.endpoint, "https://example.test/big");
    }

    #[test]
    fn tiny_responses_are_rejected() {
        let download = |_url: &str| Ok(1024u64);
        let error = measure_with(&download, &["https://example.test/small"], 4096, 0.0)
            .expect_err("过小的传输量应判为失败");
        assert!(format!("{error:#}").contains("传输量过小"), "{error:#}");
    }

    #[test]
    fn failing_endpoints_are_reported() {
        let download = |_url: &str| Err(anyhow::anyhow!("返回 404 Not Found"));
        let error = measure_with(&download, &["https://example.test/missing"], 1024, 0.0)
            .expect_err("404 应判为失败");
        assert!(format!("{error:#}").contains("404"), "{error:#}");
    }

    #[test]
    fn the_next_candidate_is_tried_after_a_failure() {
        let download = |url: &str| {
            if url.contains("bad") {
                Err(anyhow::anyhow!("返回 500"))
            } else {
                Ok(4 * 1024 * 1024u64)
            }
        };
        let result = measure_with(
            &download,
            &["https://example.test/bad", "https://example.test/good"],
            1024,
            0.0,
        )
        .expect("第二个候选点应被使用");
        assert_eq!(result.endpoint, "https://example.test/good");
    }

    #[test]
    fn invalid_proxy_is_rejected() {
        let error = measure("not a proxy url", 1000).expect_err("非法代理地址应报错");
        assert!(format!("{error:#}").contains("代理地址无效"), "{error:#}");
    }
}
