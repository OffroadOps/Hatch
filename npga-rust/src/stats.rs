//! Periodic traffic polling from the running core.
//!
//! Two backends:
//! - sing-box: `GET /traffic` on the experimental Clash API
//!   (cumulative up/down bytes, verified against sing-box 1.13).
//! - Xray: the official `xray api statsquery` CLI over the dokodemo-door API
//!   inbound, summing the `proxy` outbound's up/down counters.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

pub enum StatsBackend {
    /// Clash API external controller port of a sing-box instance.
    SingBox(u16),
    /// dokodemo-door API port plus the xray executable for the CLI.
    Xray { port: u16, executable: PathBuf },
}

pub struct StatsPoller {
    backend: StatsBackend,
    client: reqwest::blocking::Client,
    /// CLI spawns are serialized; two overlapping xray processes would race.
    xray_lock: Mutex<()>,
}

impl StatsPoller {
    pub fn sing_box(port: u16) -> Self {
        Self {
            backend: StatsBackend::SingBox(port),
            client: reqwest::blocking::Client::builder()
                .timeout(Duration::from_millis(1500))
                .connect_timeout(Duration::from_millis(800))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new()),
            xray_lock: Mutex::new(()),
        }
    }
    pub fn xray(port: u16, executable: PathBuf) -> Self {
        Self {
            backend: StatsBackend::Xray { port, executable },
            client: reqwest::blocking::Client::builder()
                .timeout(Duration::from_millis(1500))
                .connect_timeout(Duration::from_millis(800))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new()),
            xray_lock: Mutex::new(()),
        }
    }
    /// Cumulative (up, down) bytes for the accelerated outbound, if reachable.
    pub fn traffic(&self) -> Option<(u64, u64)> {
        match &self.backend {
            StatsBackend::SingBox(port) => {
                let url = format!("http://127.0.0.1:{port}/traffic");
                let response = self.client.get(&url).send().ok()?;
                if !response.status().is_success() {
                    return None;
                }
                let body: serde_json::Value = response.json().ok()?;
                let up = body.get("up").and_then(|v| v.as_u64())?;
                let down = body.get("down").and_then(|v| v.as_u64())?;
                Some((up, down))
            }
            StatsBackend::Xray { port, executable } => {
                let _guard = self.xray_lock.lock().ok()?;
                let output = Command::new(executable)
                    .args([
                        "api",
                        "statsquery",
                        "--server",
                        &format!("127.0.0.1:{port}"),
                        "-pattern",
                        "outbound",
                    ])
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output()
                    .ok()?;
                if !output.status.success() {
                    return None;
                }
                let body: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
                let stats = body.get("stat")?.as_array()?;
                let mut up = 0u64;
                let mut down = 0u64;
                for entry in stats {
                    let name = entry.get("name").and_then(|v| v.as_str())?;
                    let value = entry.get("value").and_then(|v| v.as_u64())?;
                    if name.contains("proxy") && name.ends_with("uplink") {
                        up += value;
                    } else if name.contains("proxy") && name.ends_with("downlink") {
                        down += value;
                    }
                }
                Some((up, down))
            }
        }
    }
}

/// Fetches the public exit address as seen by an IP echo service through the
/// given HTTP proxy. Used to label the accelerated connection's location.
pub fn exit_ip(http_proxy: &str, timeout_ms: u64) -> anyhow::Result<std::net::IpAddr> {
    let proxy = reqwest::Proxy::http(http_proxy)
        .map_err(|e| anyhow::anyhow!("代理地址无效 {http_proxy}: {e}"))?;
    let client = reqwest::blocking::Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_millis(timeout_ms.max(6000)))
        .connect_timeout(Duration::from_millis(4000))
        .build()
        .context("创建出口探测客户端失败")?;
    let mut last_error = None;
    for url in [
        "https://api.ip.sb/ip",
        "https://ifconfig.me/ip",
        "https://myip.ipip.net",
    ] {
        match client.get(url).send() {
            Ok(response) if response.status().is_success() => {
                let body = response.text().unwrap_or_default();
                // Echo services answer with a bare address or "IP：1.2.3.4".
                let candidate = body.split_whitespace().next().unwrap_or_default();
                if let Ok(address) = candidate
                    .trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.' && c != ':')
                    .parse()
                {
                    return Ok(address);
                }
                last_error = Some(format!("{url} 返回内容无法解析"));
            }
            Ok(response) => last_error = Some(format!("{url} 返回 {}", response.status())),
            Err(e) => last_error = Some(format!("{url}: {e}")),
        }
    }
    Err(anyhow::anyhow!(
        "获取出口 IP 失败：{}",
        last_error.unwrap_or_else(|| "没有可用服务".into())
    ))
}

/// Ports for the two API surfaces, derived from the SOCKS port so they never
/// collide with the app's own inbounds. Returns `None` when the derivation
/// would overflow.
pub fn api_ports(socks_port: u16) -> Option<(u16, u16)> {
    let xray = socks_port.checked_add(3)?;
    let sing_box = socks_port.checked_add(4)?;
    Some((xray, sing_box))
}

/// Attaches the Xray stats API to a generated config (dokodemo-door API inbound
/// plus the routing rule that feeds it).
pub fn attach_xray_stats(config: &mut serde_json::Value, api_port: u16) -> Result<()> {
    let listen = config
        .get("inbounds")
        .and_then(|i| i.get(0))
        .and_then(|i| i.get("listen"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!("127.0.0.1"));
    config["api"] = serde_json::json!({"tag": "api", "services": ["StatsService"]});
    config["stats"] = serde_json::json!({});
    let inbounds = config
        .get_mut("inbounds")
        .and_then(|i| i.as_array_mut())
        .ok_or_else(|| anyhow::anyhow!("Xray 配置缺少 inbounds"))?;
    inbounds.push(serde_json::json!({
        "tag": "api-stats-in",
        "listen": listen,
        "port": api_port,
        "protocol": "dokodemo-door",
        "settings": {"address": "127.0.0.1"}
    }));
    // Generated configs may not carry a routing section yet; create it if so.
    if config.get("routing").and_then(|r| r.as_object()).is_none() {
        config["routing"] = serde_json::json!({});
    }
    let routing = config
        .get_mut("routing")
        .and_then(|r| r.as_object_mut())
        .unwrap();
    if routing.get("rules").and_then(|r| r.as_array()).is_none() {
        routing.insert("rules".into(), serde_json::json!([]));
    }
    let rules = routing
        .get_mut("rules")
        .and_then(|r| r.as_array_mut())
        .unwrap();
    rules.insert(
        0,
        serde_json::json!({"type": "field", "inboundTag": ["api-stats-in"], "outboundTag": "api"}),
    );
    Ok(())
}

/// Attaches the sing-box Clash API external controller.
pub fn attach_sing_box_stats(config: &mut serde_json::Value, api_port: u16) {
    config["experimental"] = serde_json::json!({
        "clash_api": {"external_controller": format!("127.0.0.1:{api_port}")}
    });
}
