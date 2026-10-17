use crate::model::Node;
use anyhow::{Context, Result, bail};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use url::Url;

pub fn decode(text: &str) -> Result<String> {
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = engine.decode(&compact) {
            return String::from_utf8(bytes).context("Base64 内容不是 UTF-8");
        }
    }
    bail!("Base64 格式无效")
}
fn unescape(text: &str) -> String {
    // form_urlencoded also maps '+' to space; keep literal plus in URI credentials/fragments.
    url::form_urlencoded::parse(format!("v={}", text.replace('+', "%2B")).as_bytes())
        .next()
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default()
}
pub fn export(node: &Node) -> Result<String> {
    node.validate()?;
    let mut node = node.clone();
    node.fields.remove("SubscriptionId");
    Ok(format!(
        "NPGA://{}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&node)?)
    ))
}

#[derive(Default)]
pub struct Parsed {
    pub nodes: Vec<Node>,
    pub rejected: usize,
}
pub fn parse(text: &str) -> Result<Parsed> {
    let decoded;
    let text = text.trim().trim_start_matches('\u{feff}');
    let text = if !text.contains("://") && !text.starts_with(['[', '{']) {
        decoded = decode(text)?;
        decoded.as_str()
    } else {
        text
    };
    let mut parsed = Parsed::default();
    let mut candidates = vec![];
    if text.starts_with(['[', '{']) {
        let v: Value = serde_json::from_str(text).context("节点 JSON 无效")?;
        let values = v.as_array().cloned().unwrap_or_else(|| vec![v]);
        for value in values {
            if value.get("Type").is_some() {
                candidates.push(serde_json::from_value(value).context("NPGA 节点格式无效")?);
            } else if value.get("server").is_some() {
                let mut n = Node {
                    protocol: "SS".into(),
                    hostname: value["server"].as_str().unwrap_or_default().into(),
                    port: value["server_port"]
                        .as_u64()
                        .and_then(|p| p.try_into().ok())
                        .unwrap_or(0),
                    remark: value["remarks"].as_str().unwrap_or_default().into(),
                    ..Node::default()
                };
                for (source, target) in [
                    ("method", "EncryptMethod"),
                    ("password", "Password"),
                    ("plugin", "Plugin"),
                    ("plugin_opts", "PluginOption"),
                ] {
                    if let Some(v) = value.get(source) {
                        n.set(target, v.clone());
                    }
                }
                candidates.push(n);
            } else {
                parsed.rejected += 1;
            }
        }
    } else {
        for line in text
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.starts_with('#'))
        {
            match parse_uri(line) {
                Ok(node) => candidates.push(node),
                Err(_) => parsed.rejected += 1,
            }
        }
    }
    let mut identities = HashSet::new();
    for node in candidates {
        if node.validate().is_ok() {
            if identities.insert(node.identity()) {
                parsed.nodes.push(node);
            }
        } else {
            parsed.rejected += 1;
        }
    }
    if parsed.nodes.is_empty() {
        bail!("未识别到有效节点，已保留原节点");
    }
    Ok(parsed)
}

pub fn parse_uri(text: &str) -> Result<Node> {
    let (scheme, body) = text.split_once("://").context("节点链接缺少协议")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme == "npga" || scheme == "hatch" {
        return serde_json::from_str(&decode(body)?).context("NPGA 分享链接无效");
    }
    if scheme == "vmess" {
        let v: Value = serde_json::from_str(&decode(body)?)?;
        let mut node = Node {
            protocol: "VMess".into(),
            hostname: v["add"].as_str().unwrap_or_default().into(),
            port: v["port"]
                .as_u64()
                .or_else(|| v["port"].as_str().and_then(|s| s.parse().ok()))
                .and_then(|p| p.try_into().ok())
                .unwrap_or(0),
            remark: v["ps"].as_str().unwrap_or_default().into(),
            ..Node::default()
        };
        for (a, b) in [
            ("id", "UserID"),
            ("net", "TransferProtocol"),
            ("type", "FakeType"),
            ("host", "Host"),
            ("path", "Path"),
            ("tls", "TLSSecureType"),
            ("sni", "ServerName"),
            ("scy", "EncryptMethod"),
        ] {
            if let Some(value) = v.get(a) {
                node.set(b, value.clone());
            }
        }
        node.set(
            "AlterID",
            json!(
                v["aid"]
                    .as_u64()
                    .or_else(|| v["aid"].as_str().and_then(|s| s.parse().ok()))
                    .unwrap_or(0)
            ),
        );
        return Ok(node);
    }
    if scheme == "ssr" {
        let full = decode(body)?;
        let (endpoint, query) = full.split_once("/?").unwrap_or((&full, ""));
        let pieces: Vec<_> = endpoint.rsplitn(6, ':').collect();
        if pieces.len() != 6 {
            bail!("SSR 格式无效");
        }
        let mut n = Node {
            protocol: "SSR".into(),
            hostname: pieces[5].trim_matches(['[', ']']).into(),
            port: pieces[4].parse()?,
            ..Node::default()
        };
        n.set("Protocol", pieces[3]);
        n.set("EncryptMethod", pieces[2]);
        n.set("OBFS", pieces[1]);
        n.set("Password", decode(pieces[0])?);
        for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
            match k.as_ref() {
                "remarks" => n.remark = decode(&v)?,
                "group" => n.group = decode(&v)?,
                "obfsparam" => n.set("OBFSParam", decode(&v)?),
                "protoparam" => n.set("ProtocolParam", decode(&v)?),
                _ => {}
            }
        }
        return Ok(n);
    }
    let normalized;
    let text = if scheme == "ss"
        && !body
            .split(['?', '#'])
            .next()
            .unwrap_or_default()
            .contains('@')
    {
        let (encoded, suffix) = body.split_once('#').unwrap_or((body, ""));
        normalized = format!("ss://{}#{}", decode(encoded.trim_end_matches('/'))?, suffix);
        normalized.as_str()
    } else {
        text
    };
    let url = Url::parse(text).context("节点 URI 无效")?;
    let q: BTreeMap<_, _> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let mut node = Node {
        hostname: url
            .host_str()
            .unwrap_or_default()
            .trim_matches(['[', ']'])
            .into(),
        port: url.port().unwrap_or(443),
        remark: unescape(url.fragment().unwrap_or_default()),
        ..Node::default()
    };
    let get = |key: &str| q.get(key).cloned().unwrap_or_default();
    match scheme.as_str() {
        "ss" => {
            node.protocol = "SS".into();
            let auth = if let Some(password) = url.password() {
                format!("{}:{}", unescape(url.username()), unescape(password))
            } else {
                decode(&unescape(url.username()))?
            };
            let (method, password) = auth.split_once(':').context("SS 缺少密码")?;
            node.set("EncryptMethod", method);
            node.set("Password", password);
            if let Some(plugin) = q.get("plugin") {
                let (name, options) = plugin.split_once(';').unwrap_or((plugin, ""));
                node.set("Plugin", name);
                node.set("PluginOption", options);
            }
        }
        "vless" | "trojan" => {
            node.protocol = if scheme == "vless" { "VLESS" } else { "Trojan" }.into();
            node.set(
                if scheme == "vless" {
                    "UserID"
                } else {
                    "Password"
                },
                unescape(url.username()),
            );
            node.set(
                "TransferProtocol",
                q.get("type").map(String::as_str).unwrap_or("tcp"),
            );
            node.set(
                "TLSSecureType",
                q.get("security")
                    .map(String::as_str)
                    .unwrap_or(if scheme == "trojan" { "tls" } else { "none" }),
            );
            for (a, b) in [
                ("sni", "ServerName"),
                ("host", "Host"),
                ("path", "Path"),
                ("serviceName", "Path"),
                ("pbk", "RealityPublicKey"),
                ("sid", "RealityShortId"),
                ("spx", "RealitySpiderX"),
                ("fp", "RealityFingerprint"),
                ("flow", "Flow"),
                ("headerType", "FakeType"),
            ] {
                if let Some(value) = q.get(a) {
                    node.set(b, value.as_str());
                }
            }
            node.set(
                "AllowInsecure",
                get("allowInsecure") == "1" || get("allowInsecure") == "true",
            );
        }
        "hy2" | "hysteria2" => {
            node.protocol = "Hysteria2".into();
            let pass = if let Some(p) = url.password() {
                format!("{}:{}", unescape(url.username()), unescape(p))
            } else {
                unescape(url.username())
            };
            node.set("Password", pass);
            node.set("ServerName", get("sni"));
            node.set(
                "TLSInsecure",
                get("insecure") == "1" || get("insecure") == "true",
            );
            node.set("ObfuscationType", get("obfs"));
            node.set("ObfuscationPassword", get("obfs-password"));
            for (a, b) in [
                ("upmbps", "UploadBandwidth"),
                ("downmbps", "DownloadBandwidth"),
            ] {
                if let Some(v) = q.get(a) {
                    node.set(b, json!(v.parse::<u32>()?));
                }
            }
            if !get("mport").is_empty() {
                node.set("PortHoppingRange", get("mport"));
            }
        }
        "socks" | "socks5" => {
            node.protocol = "SOCKS".into();
            node.port = url.port().unwrap_or(1080);
            node.set("Username", unescape(url.username()));
            node.set("Password", unescape(url.password().unwrap_or_default()));
        }
        "tg" | "https"
            if text.starts_with("tg://socks?") || text.starts_with("https://t.me/socks?") =>
        {
            node.protocol = "SOCKS".into();
            node.hostname = get("server");
            node.port = get("port").parse()?;
            node.set("Username", get("user"));
            node.set("Password", get("pass"));
        }
        _ => bail!("不支持的分享协议"),
    }
    Ok(node)
}
