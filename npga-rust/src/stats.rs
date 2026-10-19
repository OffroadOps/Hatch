//! Periodic traffic polling from the running core.
//!
//! Two backends:
//! - sing-box: `GET /traffic` on the experimental Clash API
//!   (cumulative up/down bytes, verified against sing-box 1.13).
//! - Xray: the `StatsService` gRPC endpoint of the core's API listener. The
//!   query is issued directly over HTTP/2 so no helper process has to be
//!   spawned; `xray api statsquery` is only used as a fallback for cores
//!   whose API endpoint does not answer gRPC.
//!
//! Both backends talk to a loopback port. A local HTTP filter (AdGuard and
//! similar products) can intercept those requests, in which case the counters
//! stay unavailable and the UI shows "—" instead of a stale value.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

/// gRPC methods of the Xray stats service. Current cores register the
/// `xray.app` name, older ones the `v2ray.core` name.
const XRAY_STATS_METHODS: [&str; 2] = [
    "/xray.app.stats.command.StatsService/QueryStats",
    "/v2ray.core.app.stats.command.StatsService/QueryStats",
];

pub enum StatsBackend {
    /// Clash API external controller port of a sing-box instance.
    SingBox(u16),
    /// API port plus the xray executable for the CLI fallback.
    Xray { port: u16, executable: PathBuf },
}

pub struct StatsPoller {
    backend: StatsBackend,
    client: reqwest::blocking::Client,
    /// Client that speaks cleartext HTTP/2 for the Xray gRPC endpoint.
    grpc_client: reqwest::blocking::Client,
    /// CLI spawns are serialized; two overlapping xray processes would race.
    xray_lock: Mutex<()>,
    /// Index of the gRPC method that answered, so the other one is not tried
    /// again on every poll.
    xray_method: AtomicUsize,
    /// Set once neither the gRPC endpoint nor the CLI answered, so a broken
    /// environment does not spawn a helper process on every tick.
    xray_unavailable: AtomicBool,
}

impl StatsPoller {
    pub fn sing_box(port: u16) -> Self {
        Self {
            backend: StatsBackend::SingBox(port),
            client: http_client(),
            grpc_client: grpc_client(),
            xray_lock: Mutex::new(()),
            xray_method: AtomicUsize::new(0),
            xray_unavailable: AtomicBool::new(false),
        }
    }
    pub fn xray(port: u16, executable: PathBuf) -> Self {
        Self {
            backend: StatsBackend::Xray { port, executable },
            client: http_client(),
            grpc_client: grpc_client(),
            xray_lock: Mutex::new(()),
            xray_method: AtomicUsize::new(0),
            xray_unavailable: AtomicBool::new(false),
        }
    }
    /// Cumulative (up, down) bytes for the accelerated outbound, if reachable.
    pub fn traffic(&self) -> Option<(u64, u64)> {
        match &self.backend {
            StatsBackend::SingBox(port) => self.sing_box_traffic(*port),
            StatsBackend::Xray { port, executable } => {
                if self.xray_unavailable.load(Ordering::Relaxed) {
                    return None;
                }
                if let Ok(value) = self.xray_stats_over_grpc(*port) {
                    return Some(value);
                }
                // The gRPC endpoint did not answer; the CLI uses the same
                // protocol, but older cores may expose the service under a
                // different name, so give it one chance.
                match self.xray_cli_traffic(*port, executable) {
                    Some(value) => Some(value),
                    None => {
                        self.xray_unavailable.store(true, Ordering::Relaxed);
                        None
                    }
                }
            }
        }
    }
    fn sing_box_traffic(&self, port: u16) -> Option<(u64, u64)> {
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
    /// Queries the Xray stats service over gRPC.
    ///
    /// `Err` means the endpoint could not be reached or answered something
    /// that is not gRPC, which lets the caller fall back to the CLI.
    pub fn xray_stats_over_grpc(&self, port: u16) -> Result<(u64, u64)> {
        let preferred = self.xray_method.load(Ordering::Relaxed);
        let mut last_error = None;
        for offset in 0..XRAY_STATS_METHODS.len() {
            let index = (preferred + offset) % XRAY_STATS_METHODS.len();
            match self.query_xray_method(port, XRAY_STATS_METHODS[index]) {
                Ok(stats) => {
                    if !stats.is_empty() || offset + 1 == XRAY_STATS_METHODS.len() {
                        self.xray_method.store(index, Ordering::Relaxed);
                        return Ok(sum_proxy_counters(&stats));
                    }
                    // No counters yet: the other method name may be the one
                    // this core registered, so try it before giving up.
                }
                Err(e) => last_error = Some(e),
            }
        }
        match last_error {
            Some(e) => Err(e),
            None => Ok((0, 0)),
        }
    }
    fn query_xray_method(&self, port: u16, method: &str) -> Result<Vec<(String, u64)>> {
        let url = format!("http://127.0.0.1:{port}{method}");
        let body = grpc_frame(&encode_query_stats("outbound", false));
        let response = self
            .grpc_client
            .post(&url)
            .header("content-type", "application/grpc")
            .header("te", "trailers")
            .body(body)
            .send()
            .context("Xray 统计接口请求失败")?;
        if !response.status().is_success() {
            anyhow::bail!("Xray 统计接口返回 {}", response.status());
        }
        let bytes = response.bytes().context("读取 Xray 统计响应失败")?;
        decode_query_stats(&bytes).context("解析 Xray 统计响应失败")
    }
    /// Fallback path: spawn `xray api statsquery` and read its JSON output.
    fn xray_cli_traffic(&self, port: u16, executable: &PathBuf) -> Option<(u64, u64)> {
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

fn http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(1500))
        .connect_timeout(Duration::from_millis(800))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

fn grpc_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .http2_prior_knowledge()
        .timeout(Duration::from_millis(1500))
        .connect_timeout(Duration::from_millis(800))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

/// Sums the proxy outbound's uplink/downlink counters.
fn sum_proxy_counters(stats: &[(String, u64)]) -> (u64, u64) {
    let mut up = 0u64;
    let mut down = 0u64;
    for (name, value) in stats {
        if name.contains("proxy") && name.ends_with("uplink") {
            up += value;
        } else if name.contains("proxy") && name.ends_with("downlink") {
            down += value;
        }
    }
    (up, down)
}

/// Appends a base-128 varint.
fn push_varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// Reads a base-128 varint, returning the value and the bytes consumed.
fn read_varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut value = 0u64;
    let mut shift = 0u32;
    for (index, byte) in data.iter().enumerate() {
        value |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some((value, index + 1));
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
    None
}

/// Encodes a `QueryStatsRequest { pattern, reset }` protobuf message.
fn encode_query_stats(pattern: &str, reset: bool) -> Vec<u8> {
    let mut message = Vec::new();
    // field 1: pattern (string)
    message.push(0x0a);
    push_varint(&mut message, pattern.len() as u64);
    message.extend_from_slice(pattern.as_bytes());
    // field 2: reset (bool)
    if reset {
        message.push(0x10);
        push_varint(&mut message, 1);
    }
    message
}

/// Wraps a protobuf message in the gRPC length-prefixed frame.
fn grpc_frame(message: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(message.len() + 5);
    frame.push(0); // not compressed
    frame.extend_from_slice(&(message.len() as u32).to_be_bytes());
    frame.extend_from_slice(message);
    frame
}

/// Decodes a `QueryStatsResponse` into `(stat name, value)` pairs.
fn decode_query_stats(data: &[u8]) -> Option<Vec<(String, u64)>> {
    // Skip the 5 byte gRPC frame header (compression flag + length).
    let message = data.get(5..)?;
    let mut stats = Vec::new();
    let mut offset = 0;
    while offset < message.len() {
        let tag = message[offset];
        offset += 1;
        let field = tag >> 3;
        let wire = tag & 0x07;
        match wire {
            2 => {
                let (len, used) = read_varint(message.get(offset..)?)?;
                offset += used;
                let end = offset.checked_add(len as usize)?;
                let bytes = message.get(offset..end)?;
                offset = end;
                if field == 1 {
                    stats.push(decode_stat(bytes)?);
                }
            }
            0 => {
                let (_, used) = read_varint(message.get(offset..)?)?;
                offset += used;
            }
            5 => offset += 4,
            1 => offset += 8,
            _ => return None,
        }
    }
    Some(stats)
}

/// Decodes a `Stat { name, value }` protobuf message.
fn decode_stat(data: &[u8]) -> Option<(String, u64)> {
    let mut name = String::new();
    let mut value = 0u64;
    let mut offset = 0;
    while offset < data.len() {
        let tag = data[offset];
        offset += 1;
        let field = tag >> 3;
        let wire = tag & 0x07;
        match wire {
            2 => {
                let (len, used) = read_varint(data.get(offset..)?)?;
                offset += used;
                let end = offset.checked_add(len as usize)?;
                let bytes = data.get(offset..end)?;
                offset = end;
                if field == 1 {
                    name = String::from_utf8_lossy(bytes).into_owned();
                }
            }
            0 => {
                let (raw, used) = read_varint(data.get(offset..)?)?;
                offset += used;
                if field == 2 {
                    value = raw;
                }
            }
            5 => offset += 4,
            1 => offset += 8,
            _ => return None,
        }
    }
    Some((name, value))
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

/// Attaches the Xray stats API to a generated config.
///
/// Since v1.8.12 the core can host the gRPC API itself: setting `api.listen`
/// makes Xray open a dedicated API listener, so no extra inbound and no
/// routing rule are needed. The legacy dokodemo-door inbound is not used
/// because current cores ignore its `address` setting, which silently
/// corrupted the gRPC preface and made every stats query fail.
pub fn attach_xray_stats(config: &mut serde_json::Value, api_port: u16) -> Result<()> {
    let listen = config
        .get("inbounds")
        .and_then(|i| i.get(0))
        .and_then(|i| i.get("listen"))
        .and_then(|v| v.as_str())
        .unwrap_or("127.0.0.1")
        .to_owned();
    let host = if listen.contains(':') && !listen.starts_with('[') {
        format!("[{listen}]")
    } else {
        listen
    };
    config["api"] = serde_json::json!({
        "tag": "api",
        "listen": format!("{host}:{api_port}"),
        "services": ["StatsService"],
    });
    config["stats"] = serde_json::json!({});
    Ok(())
}

/// Attaches the sing-box Clash API external controller.
pub fn attach_sing_box_stats(config: &mut serde_json::Value, api_port: u16) {
    config["experimental"] = serde_json::json!({
        "clash_api": {"external_controller": format!("127.0.0.1:{api_port}")}
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trip() {
        for value in [0u64, 1, 127, 128, 300, u32::MAX as u64, u64::MAX] {
            let mut buffer = vec![];
            push_varint(&mut buffer, value);
            let (decoded, used) = read_varint(&buffer).unwrap();
            assert_eq!(decoded, value, "value {value}");
            assert_eq!(used, buffer.len(), "value {value}");
        }
    }

    #[test]
    fn query_stats_request_is_encoded_like_protobuf() {
        let message = encode_query_stats("outbound", false);
        // field 1, length delimited, 8 byte payload.
        assert_eq!(message[0], 0x0a);
        assert_eq!(message[1], 8);
        assert_eq!(&message[2..10], b"outbound");
        // The reset flag is omitted when it is false.
        assert_eq!(message.len(), 10);

        let message = encode_query_stats("x", true);
        assert_eq!(message[0], 0x0a);
        assert_eq!(message[1], 1);
        assert_eq!(message[2], b'x');
        // field 2, varint.
        assert_eq!(message[3], 0x10);
        assert_eq!(message[4], 0x01);
    }

    #[test]
    fn grpc_frame_carries_the_message_length() {
        let framed = grpc_frame(b"abc");
        assert_eq!(framed[0], 0);
        assert_eq!(&framed[1..5], &[0, 0, 0, 3]);
        assert_eq!(&framed[5..], b"abc");
    }

    #[test]
    fn query_stats_response_is_decoded() {
        // QueryStatsResponse { stat: [Stat { name, value }] }
        let name = b"outbound>>>proxy>>>traffic>>>uplink";
        let mut stat = Vec::new();
        // Stat.name = 1 (string)
        stat.push(0x0a);
        push_varint(&mut stat, name.len() as u64);
        stat.extend_from_slice(name);
        // Stat.value = 2 (int64)
        stat.push(0x10);
        push_varint(&mut stat, 4096);
        let mut message = Vec::new();
        // QueryStatsResponse.stat = 1 (repeated Stat)
        message.push(0x0a);
        push_varint(&mut message, stat.len() as u64);
        message.extend_from_slice(&stat);
        let framed = grpc_frame(&message);
        let stats = decode_query_stats(&framed).unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].0, "outbound>>>proxy>>>traffic>>>uplink");
        assert_eq!(stats[0].1, 4096);
        assert_eq!(sum_proxy_counters(&stats), (4096, 0));
    }

    #[test]
    fn truncated_responses_are_rejected() {
        assert!(decode_query_stats(b"").is_none());
        assert!(decode_query_stats(b"\x00\x00\x00").is_none());
        // A frame header that promises more bytes than are present.
        assert!(decode_query_stats(&[0, 0, 0, 0, 16, 0x0a, 0x7f]).is_none());
    }

    #[test]
    fn api_ports_never_overflow() {
        assert_eq!(api_ports(9995), Some((9998, 9999)));
        assert_eq!(api_ports(65533), None);
    }

    #[test]
    fn xray_api_uses_the_dedicated_api_listener() {
        let mut config = serde_json::json!({
            "inbounds": [{"tag": "socks-in", "listen": "127.0.0.1", "port": 9995}]
        });
        attach_xray_stats(&mut config, 9998).unwrap();
        assert_eq!(config["api"]["listen"], "127.0.0.1:9998");
        assert_eq!(config["api"]["services"][0], "StatsService");
        assert!(config["stats"].is_object());
        // No extra inbound and no routing rule are required any more.
        assert_eq!(config["inbounds"].as_array().unwrap().len(), 1);
        assert!(config.get("routing").is_none());
    }

    #[test]
    fn xray_api_listens_on_the_inbound_address() {
        let mut config = serde_json::json!({
            "inbounds": [{"tag": "socks-in", "listen": "0.0.0.0", "port": 9995}]
        });
        attach_xray_stats(&mut config, 9998).unwrap();
        assert_eq!(config["api"]["listen"], "0.0.0.0:9998");
    }
}
