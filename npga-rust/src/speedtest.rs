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
    let mut last_error = None;
    for url in CANDIDATES {
        let begin = Instant::now();
        match client.get(*url).send() {
            Ok(response) => {
                if !response.status().is_success() {
                    last_error = Some(format!("{url} 返回 {}", response.status()));
                    continue;
                }
                let mut total = 0u64;
                let mut response = response;
                let mut buffer = [0u8; 32768];
                let mut read_error: Option<String> = None;
                loop {
                    match std::io::Read::read(&mut response, &mut buffer) {
                        Ok(0) => break,
                        Ok(n) => total += n as u64,
                        Err(e) => {
                            read_error = Some(e.to_string());
                            break;
                        }
                    }
                }
                let seconds = begin.elapsed().as_secs_f64();
                if total < 512 * 1024 || seconds <= 0.05 {
                    // Too small to call a speed sample; treat as failure.
                    last_error = Some(match read_error {
                        Some(e) => format!("{url} 传输中断: {e}"),
                        None => format!("{url} 传输量过小"),
                    });
                    continue;
                }
                return Ok(SpeedResult {
                    mbps: total as f64 * 8.0 / seconds / 1_000_000.0,
                    bytes: total,
                    seconds,
                    endpoint: (*url).into(),
                });
            }
            Err(e) => last_error = Some(format!("{url}: {e}")),
        }
    }
    Err(anyhow::anyhow!(
        "测速失败：{}",
        last_error.unwrap_or_else(|| "没有可用测速节点".into())
    ))
}
