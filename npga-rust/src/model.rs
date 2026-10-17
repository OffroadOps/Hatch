use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn port<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u16, D::Error> {
    let v = Value::deserialize(d)?;
    let n = v
        .as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()));
    n.and_then(|n| u16::try_from(n).ok())
        .ok_or_else(|| serde::de::Error::custom("端口必须在 0–65535 之间"))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Node {
    #[serde(rename = "Type")]
    pub protocol: String,
    pub hostname: String,
    #[serde(deserialize_with = "port")]
    pub port: u16,
    pub remark: String,
    pub group: String,
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}
impl Default for Node {
    fn default() -> Self {
        Self {
            protocol: "VLESS".into(),
            hostname: String::new(),
            port: 443,
            remark: String::new(),
            group: "手动添加".into(),
            fields: BTreeMap::new(),
        }
    }
}
impl Node {
    pub fn text(&self, key: &str) -> &str {
        self.fields.get(key).and_then(Value::as_str).unwrap_or("")
    }
    pub fn string(&self, key: &str, fallback: &str) -> String {
        let s = self.text(key);
        if s.is_empty() {
            fallback.into()
        } else {
            s.into()
        }
    }
    pub fn flag(&self, key: &str) -> bool {
        self.fields
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
    pub fn number(&self, key: &str, fallback: u64) -> u64 {
        self.fields
            .get(key)
            .and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .unwrap_or(fallback)
    }
    pub fn set(&mut self, key: &str, value: impl Into<Value>) {
        self.fields.insert(key.into(), value.into());
    }
    pub fn name(&self) -> String {
        if self.remark.is_empty() {
            format!("{}:{}", self.hostname, self.port)
        } else {
            self.remark.clone()
        }
    }
    pub fn core(&self) -> &'static str {
        if self.protocol == "Hysteria2" {
            "sing-box"
        } else {
            "Xray"
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.hostname.trim().is_empty()
            || self.hostname.contains(['/', '\\', '\0', ' ', '\n', '\r'])
            || self.port == 0
        {
            bail!("节点地址或端口无效");
        }
        match self.protocol.as_str() {
            "SOCKS" | "Socks5" => {}
            "SS" | "SSR" | "Trojan" | "Hysteria2" if self.text("Password").is_empty() => {
                bail!("节点缺少密码")
            }
            "VMess" | "VLESS" => {
                uuid::Uuid::parse_str(self.text("UserID"))
                    .map_err(|_| anyhow::anyhow!("UserID 必须为有效 UUID"))?;
            }
            "WireGuard"
                if self.text("PrivateKey").is_empty() || self.text("PeerPublicKey").is_empty() =>
            {
                bail!("WireGuard 缺少密钥")
            }
            "SS" | "SSR" | "Trojan" | "Hysteria2" | "WireGuard" => {}
            p => bail!("暂不支持节点协议：{p}"),
        }
        Ok(())
    }
    pub fn identity(&self) -> String {
        // Credentials participate in deduplication without being exposed in logs.
        serde_json::to_string(&(
            self.protocol.as_str(),
            self.hostname.as_str(),
            self.port,
            &self.fields,
        ))
        .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Subscription {
    pub id: String,
    pub enable: bool,
    pub link: String,
    pub remark: String,
    pub user_agent: String,
}
impl Default for Subscription {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            enable: true,
            link: String::new(),
            remark: String::new(),
            user_agent: "NPGA-Rust/3.0".into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteKind {
    #[default]
    Local,
    System,
    Process,
    Tun,
    Share,
}
impl RouteKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Local => "本地代理",
            Self::System => "系统代理",
            Self::Process => "进程加速",
            Self::Tun => "全局 TUN",
            Self::Share => "网络共享",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Settings {
    #[serde(rename = "Server")]
    pub nodes: Vec<Node>,
    #[serde(rename = "Subscription")]
    pub subscriptions: Vec<Subscription>,
    #[serde(rename = "Socks5LocalPort")]
    pub socks_port: u16,
    #[serde(rename = "HTTPLocalPort")]
    pub http_port: u16,
    pub local_address: String,
    pub selected_node: usize,
    pub route_kind: RouteKind,
    pub mode_file: String,
    pub minimize_to_tray: bool,
    pub startup: bool,
    pub request_timeout: u64,
    pub share_interface: String,
    /// Automatic node selection (probe all candidates, connect to the best).
    pub auto_select: bool,
    /// Allow switching a working connection on latency degradation.
    pub auto_select_degradable: bool,
    /// Latency (ms) above which a connection counts as degraded.
    pub degrade_ms: u64,
    /// Minimum milliseconds between automatic switches.
    pub switch_cooldown_ms: u64,
    #[serde(flatten)]
    pub legacy: BTreeMap<String, Value>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            nodes: vec![],
            subscriptions: vec![],
            socks_port: 9995,
            http_port: 2802,
            local_address: "127.0.0.1".into(),
            selected_node: 0,
            route_kind: RouteKind::Local,
            mode_file: String::new(),
            minimize_to_tray: true,
            startup: false,
            request_timeout: 10000,
            share_interface: String::new(),
            auto_select: false,
            auto_select_degradable: false,
            degrade_ms: 250,
            switch_cooldown_ms: 30_000,
            legacy: BTreeMap::new(),
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        self.local_address
            .parse::<std::net::IpAddr>()
            .map_err(|_| anyhow::anyhow!("本地监听地址必须为 IP"))?;
        if self.socks_port == 0 || self.http_port == 0 || self.socks_port == self.http_port {
            bail!("SOCKS 和 HTTP 端口必须非零且不同");
        }
        Ok(())
    }
    pub fn defaults_for(&self, section: &str, key: &str, fallback: Value) -> Value {
        self.legacy
            .get(section)
            .and_then(|v| v.get(key))
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or(fallback)
    }
    /// Whether update checks should consider pre-releases (legacy setting).
    pub fn allow_prerelease(&self) -> bool {
        self.legacy
            .get("Update")
            .and_then(|v| v.get("CheckPreRelease"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
    /// Process-mode backend: `"tun"` uses sing-box TUN process routing (ADR-002),
    /// anything else keeps the legacy Redirector + netfilter2 path.
    pub fn process_engine(&self) -> String {
        self.legacy
            .get("ProcessEngine")
            .and_then(Value::as_str)
            .unwrap_or("redirector")
            .to_ascii_lowercase()
    }
    /// STUN server used by NAT type discovery, from legacy settings.
    pub fn stun_server(&self) -> (String, u16) {
        let host = self
            .legacy
            .get("STUN_Server")
            .and_then(Value::as_str)
            .unwrap_or("stun.miwifi.com")
            .to_owned();
        let port = self
            .legacy
            .get("STUN_Server_Port")
            .and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .unwrap_or(3478) as u16;
        (host, port)
    }
}

pub fn sample_node() -> Node {
    let mut node = Node {
        protocol: "SOCKS".into(),
        hostname: "127.0.0.1".into(),
        port: 1080,
        remark: "本地 SOCKS5".into(),
        ..Node::default()
    };
    node.set("Username", json!(""));
    node.set("Password", json!(""));
    node
}
