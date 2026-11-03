use crate::{
    model::{Node, RouteKind, Settings},
    modes::Mode,
};
use anyhow::{Result, bail};
use serde_json::{Value, json};

pub fn xray(node: &Node, settings: &Settings) -> Result<Value> {
    node.validate()?;
    settings.validate()?;
    let host = &node.hostname;
    let port = node.port;
    let mut outbound = json!({"tag":"proxy"});
    match node.protocol.as_str() {
        "SOCKS" | "Socks5" => {
            outbound["protocol"] = json!("socks");
            let mut server = json!({"address":host,"port":port});
            if !node.text("Username").is_empty() || !node.text("Password").is_empty() {
                server["users"] =
                    json!([{"user":node.text("Username"),"pass":node.text("Password")}]);
            }
            outbound["settings"] = json!({"servers":[server]});
        }
        "SS" => {
            // Xray does not implement SIP003 plugins. Fail explicitly rather than silently dropping them.
            if !node.text("Plugin").is_empty() {
                bail!("此 SS 节点使用 SIP003 插件，请先移除插件或使用独立插件客户端");
            }
            outbound["protocol"] = json!("shadowsocks");
            outbound["settings"] = json!({"servers":[{"address":host,"port":port,"method":node.string("EncryptMethod","aes-128-gcm"),"password":node.text("Password")}]});
        }
        "SSR" => bail!("Xray 不支持原生 SSR。已保留节点；请转换为 SS 或使用支持 SSR 的核心"),
        "VLESS" | "VMess" => {
            outbound["protocol"] = json!(node.protocol.to_lowercase());
            let user = if node.protocol == "VLESS" {
                json!({"id":node.text("UserID"),"encryption":node.string("EncryptMethod","none"),"flow":node.text("Flow")})
            } else {
                json!({"id":node.text("UserID"),"alterId":node.number("AlterID",0),"security":node.string("EncryptMethod","auto")})
            };
            outbound["settings"] = json!({"vnext":[{"address":host,"port":port,"users":[user]}]});
            outbound["streamSettings"] = transport(node)?;
            if node.flag("UseMux")
                && node.text("Flow").is_empty()
                && node.text("TLSSecureType") != "reality"
            {
                outbound["mux"] = json!({"enabled":true,"concurrency":8});
            }
        }
        "Trojan" => {
            outbound["protocol"] = json!("trojan");
            outbound["settings"] =
                json!({"servers":[{"address":host,"port":port,"password":node.text("Password")}]});
            outbound["streamSettings"] = transport(node)?;
        }
        "WireGuard" => {
            outbound["protocol"] = json!("wireguard");
            let addresses: Vec<_> = node
                .string("LocalAddresses", "172.16.0.2/32")
                .split([',', ';', ' '])
                .filter(|s| !s.is_empty())
                .map(|s| {
                    if s.contains('/') {
                        s.to_owned()
                    } else {
                        format!("{s}/32")
                    }
                })
                .collect();
            let endpoint = if host.contains(':') {
                format!("[{host}]:{port}")
            } else {
                format!("{host}:{port}")
            };
            outbound["settings"] = json!({"secretKey":node.text("PrivateKey"),"address":addresses,"mtu":node.number("MTU",1420),"peers":[{"publicKey":node.text("PeerPublicKey"),"preSharedKey":node.text("PreSharedKey"),"endpoint":endpoint,"allowedIPs":["0.0.0.0/0","::/0"]}]});
        }
        other => bail!("Xray 不支持 {other}"),
    }
    Ok(json!({"log":{"loglevel":"warning"},"inbounds":[
        {"tag":"socks-in","listen":settings.local_address,"port":settings.socks_port,"protocol":"socks","settings":{"auth":"noauth","udp":true}},
        {"tag":"http-in","listen":settings.local_address,"port":settings.http_port,"protocol":"http","settings":{}}
    ],"outbounds":[outbound,{"tag":"direct","protocol":"freedom"}]}))
}

fn transport(node: &Node) -> Result<Value> {
    let network = node.string("TransferProtocol", "tcp");
    let security = node.string(
        "TLSSecureType",
        if node.protocol == "Trojan" {
            "tls"
        } else {
            "none"
        },
    );
    let name = node.string("ServerName", &node.string("Host", &node.hostname));
    let mut stream = json!({"network":network,"security":security});
    match security.as_str() {
        "tls" => {
            stream["tlsSettings"] =
                json!({"serverName":name,"allowInsecure":node.flag("AllowInsecure")});
        }
        "reality" => {
            if node.text("RealityPublicKey").is_empty() {
                bail!("Reality 节点缺少公钥");
            }
            stream["realitySettings"] = json!({"serverName":name,"fingerprint":node.string("RealityFingerprint","chrome"),"publicKey":node.text("RealityPublicKey"),"shortId":node.text("RealityShortId"),"spiderX":node.text("RealitySpiderX")});
        }
        "none" => {}
        _ => bail!("旧 XTLS 配置已移除，请使用 TLS + xtls-rprx-vision"),
    }
    match network.as_str() {
        "tcp" | "raw" => {
            let fake = node.string("FakeType", "none");
            let mut header = json!({"type":fake});
            if fake == "http" {
                header["request"] = json!({"path":[node.string("Path","/")],"headers":{"Host":[node.string("Host", &node.hostname)]}});
            }
            stream["tcpSettings"] = json!({"header":header});
        }
        "ws" => {
            stream["wsSettings"] =
                json!({"path":node.string("Path","/"),"headers":{"Host":node.text("Host")}});
        }
        "grpc" => {
            stream["grpcSettings"] = json!({"serviceName":node.text("Path"),"multiMode":node.text("FakeType") == "multi"});
        }
        "xhttp" | "splithttp" => {
            stream["xhttpSettings"] = json!({"path":node.string("Path","/"),"host":node.text("Host"),"mode":node.string("XHTTPMode","auto")});
        }
        "kcp" => {
            stream["kcpSettings"] = json!({"mtu":1350,"tti":50,"uplinkCapacity":12,"downlinkCapacity":100,"congestion":false,"readBufferSize":2,"writeBufferSize":2,"header":{"type":node.string("FakeType","none")},"seed":node.text("Path")});
        }
        _ => bail!("此核心不支持旧传输 {network}，请更新节点传输配置"),
    }
    Ok(stream)
}

pub fn sing_box(node: &Node, settings: &Settings) -> Result<Value> {
    node.validate()?;
    settings.validate()?;
    if node.protocol != "Hysteria2" {
        bail!("sing-box 直连配置仅用于 Hysteria2");
    }
    let mut outbound = json!({"type":"hysteria2","tag":"proxy","server":node.hostname,"server_port":node.port,"password":node.text("Password"),
        "up_mbps":node.number("UploadBandwidth",100),"down_mbps":node.number("DownloadBandwidth",100),
        "tls":{"enabled":true,"server_name":node.string("ServerName", &node.hostname),"insecure":node.flag("TLSInsecure"),"alpn":node.fields.get("ALPN").filter(|v| !v.is_null()).cloned().unwrap_or(json!(["h3"]))}});
    if !node.text("ObfuscationType").is_empty() {
        outbound["obfs"] = json!({"type":node.text("ObfuscationType"),"password":node.text("ObfuscationPassword")});
    }
    if !node.text("CertificateFingerprint").is_empty() {
        bail!("此版本尚未接入证书指纹校验，请保留指纹并使用原客户端");
    }
    if !node.text("PortHoppingRange").is_empty() {
        outbound["server_ports"] = json!([node.text("PortHoppingRange").replace('-', ":")]);
        outbound.as_object_mut().unwrap().remove("server_port");
        if !node.text("HopInterval").is_empty() {
            outbound["hop_interval"] = json!(node.text("HopInterval"));
        }
    }
    Ok(json!({"log":{"level":"warn","timestamp":true},"inbounds":[
        {"type":"socks","tag":"socks-in","listen":settings.local_address,"listen_port":settings.socks_port},
        {"type":"http","tag":"http-in","listen":settings.local_address,"listen_port":settings.http_port}
    ],"outbounds":[outbound,{"type":"direct","tag":"direct"}],"route":{"rules":[{"action":"sniff"}],"final":"proxy"}}))
}

/// The legacy Netch process rules converted into sing-box TUN rule families.
#[derive(Default)]
pub struct ProcessRules {
    /// Exact image names for `process_name`. Spaces are legal in image names
    /// (e.g. `Battle.net Launcher.exe`) and must not be rejected.
    pub names: Vec<String>,
    /// Regexes for `process_path_regex` (supported by the pinned sing-box):
    /// literal path fragments and anchored legacy regexes, matched
    /// case-insensitively against the full process path.
    pub path_regexes: Vec<String>,
    /// Rules that fit neither family; reported to the user, never silently
    /// dropped from the mode file.
    pub unconvertible: Vec<String>,
}

/// Escapes a literal so it matches verbatim inside `process_path_regex`
/// (the regex crate has no `\Q…\E` quoting). Also used by the mode wizard to
/// build anchored directory rules.
pub(crate) fn regex_escape(literal: &str) -> String {
    let mut out = String::with_capacity(literal.len());
    for c in literal.chars() {
        if "\\.^$|?*+()[]{}".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Converts legacy process rules (C++ regex style, e.g. `^game\.exe$`,
/// `\\Battle.net\\`) into sing-box TUN rules:
/// - a plain image name (spaces allowed) becomes an exact `process_name`;
/// - a rule containing a path separator becomes a literal
///   `process_path_regex` fragment;
/// - an anchored regex (`^…`) becomes a `process_path_regex` matched against
///   the file-name part of the full path.
///
/// Everything else lands in `unconvertible` and is reported, never dropped
/// silently.
pub fn process_rules(rules: &[String]) -> ProcessRules {
    let mut out = ProcessRules::default();
    for rule in rules {
        let trimmed = rule.trim().trim_start_matches('!').trim();
        if trimmed.is_empty() {
            continue;
        }
        // CIDR bypass entries are handled separately via route_exclude_address
        // and are never process rules.
        if rule.trim().parse::<ipnet::IpNet>().is_ok() {
            continue;
        }
        let anchored = trimmed.starts_with('^');
        let ends_dollar = trimmed.ends_with('$');
        let body = trimmed.strip_prefix('^').unwrap_or(trimmed);
        let body = body.strip_suffix('$').unwrap_or(body);
        // Regex escapes inside a plain image name are just the literal dot.
        let candidate = body.replace("\\.", ".");
        let is_path = candidate.contains(['\\', '/']);
        let has_meta = candidate.contains(['*', '?', '+', '|', '(', ')', '[', ']', '{', '}']);
        if is_path {
            // A literal path fragment: match it verbatim anywhere in the full
            // process path (case-insensitive, Windows paths vary in case).
            out.path_regexes.push(format!("(?i){}", regex_escape(body)));
        } else if !has_meta
            && !candidate.is_empty()
            && !candidate.contains("..")
            && candidate.to_ascii_lowercase().ends_with(".exe")
        {
            out.names.push(candidate);
        } else if has_meta && anchored {
            if ends_dollar {
                out.path_regexes.push(format!("(?i)(\\\\|/){body}$"));
            } else {
                out.path_regexes.push(format!("(?i)(\\\\|/){body}"));
            }
        } else {
            out.unconvertible.push(rule.clone());
        }
    }
    out.names.sort();
    out.names.dedup();
    out.path_regexes.sort();
    out.path_regexes.dedup();
    out
}

/// Builds the TUN configuration for process mode (ADR-002): only the listed
/// processes use the proxy, everything else stays direct. The only process-mode
/// backend since the Redirector/netfilter2 path was retired (Phase 2, 2026-10-17).
pub fn tun_process(settings: &Settings, mode: &Mode, server_ip: &str) -> Result<Value> {
    if mode.kind != RouteKind::Process {
        bail!("请选择进程模式");
    }
    let mut handle = process_rules(&mode.handle);
    // Wizard-generated modes carry final `pathRegex` directory rules; they
    // pass through untouched instead of going through the legacy converter.
    handle
        .path_regexes
        .extend(mode.path_regexes.iter().cloned());
    handle.path_regexes.sort();
    handle.path_regexes.dedup();
    if handle.names.is_empty() && handle.path_regexes.is_empty() {
        bail!(
            "该模式的 {} 条进程规则均无法转换为 sing-box 规则（仅支持精确进程名、路径或 ^…$ 正则）",
            handle.unconvertible.len()
        );
    }
    let bypass = process_rules(&mode.bypass);
    let mut inbound = json!({"type":"tun","tag":"tun-in","interface_name":"NPGA-TUN","address":["172.30.255.1/30","fdfe:dcba:9876::1/126"],"mtu":1500,"auto_route":true,"strict_route":true,"stack":"mixed","route_exclude_address":["10.0.0.0/8","172.16.0.0/12","192.168.0.0/16","fc00::/7","fe80::/10"]});
    let mut excludes: Vec<String> =
        serde_json::from_value(inbound["route_exclude_address"].clone())?;
    excludes.push(format!(
        "{server_ip}/{}",
        if server_ip.contains(':') { 128 } else { 32 }
    ));
    // Bypass entries that are CIDRs (not process names) exclude whole ranges.
    for rule in &mode.bypass {
        if let Ok(net) = rule.trim().parse::<ipnet::IpNet>() {
            excludes.push(net.to_string());
        }
    }
    if let Some(values) = settings
        .legacy
        .get("TUNTAP")
        .and_then(|v| v["BypassIPs"].as_array())
    {
        for value in values {
            if let Some(v) = value.as_str() {
                v.parse::<ipnet::IpNet>()?;
                excludes.push(v.into());
            }
        }
    }
    inbound["route_exclude_address"] = json!(excludes);
    let dns = settings
        .legacy
        .get("TUNTAP")
        .and_then(|v| v["DNS"].as_str())
        .unwrap_or("1.1.1.1");
    dns.parse::<std::net::IpAddr>()?;
    // Rules are ordered: bypass first (explicit exclusion), then handled
    // processes to the proxy, then everything else stays direct. Exact names
    // and path regexes are separate rules — conditions inside one sing-box
    // rule combine with AND, so the two families must never be merged.
    let mut rules = vec![json!({"action":"sniff"})];
    if !handle.names.is_empty() {
        rules.push(json!({"protocol":"dns","process_name":handle.names,"action":"hijack-dns"}));
    }
    if !handle.path_regexes.is_empty() {
        rules.push(json!({"protocol":"dns","process_path_regex":handle.path_regexes,"action":"hijack-dns"}));
    }
    rules.push(json!({"protocol":"dns","action":"route","outbound":"direct"}));
    if !bypass.names.is_empty() {
        rules.push(json!({"process_name":bypass.names,"action":"route","outbound":"direct"}));
    }
    if !bypass.path_regexes.is_empty() {
        rules.push(
            json!({"process_path_regex":bypass.path_regexes,"action":"route","outbound":"direct"}),
        );
    }
    if !handle.names.is_empty() {
        rules.push(json!({"process_name":handle.names,"action":"route","outbound":"proxy"}));
    }
    if !handle.path_regexes.is_empty() {
        rules.push(
            json!({"process_path_regex":handle.path_regexes,"action":"route","outbound":"proxy"}),
        );
    }
    rules.push(json!({"ip_is_private":true,"action":"route","outbound":"direct"}));
    Ok(json!({"log":{"level":"warn"},"inbounds":[inbound],
        "dns":{"servers":[{"type":"udp","tag":"remote-dns","server":dns,"detour":"proxy"},{"type":"local","tag":"local-dns"}],"final":"remote-dns"},
        "outbounds":[{"type":"socks","tag":"proxy","server":settings.local_address,"server_port":settings.socks_port},{"type":"direct","tag":"direct"}],
        "route":{"auto_detect_interface":true,"default_domain_resolver":{"server":"local-dns"},"rules":rules,"final":"direct"}}))
}

pub fn tun(settings: &Settings, mode: Option<&Mode>, server_ip: &str) -> Result<Value> {
    let mut inbound = json!({"type":"tun","tag":"tun-in","interface_name":"NPGA-TUN","address":["172.30.255.1/30","fdfe:dcba:9876::1/126"],"mtu":1500,"auto_route":true,"strict_route":true,"stack":"mixed","route_exclude_address":["10.0.0.0/8","172.16.0.0/12","192.168.0.0/16","fc00::/7","fe80::/10"]});
    let mut excludes: Vec<String> =
        serde_json::from_value(inbound["route_exclude_address"].clone())?;
    excludes.push(format!(
        "{server_ip}/{}",
        if server_ip.contains(':') { 128 } else { 32 }
    ));
    if let Some(mode) = mode {
        if mode.kind != RouteKind::Tun {
            bail!("请选择 TUN 模式");
        }
        if !mode.handle.is_empty() {
            inbound["route_address"] = json!(mode.handle);
        }
        excludes.extend(mode.bypass.clone());
    }
    if let Some(values) = settings
        .legacy
        .get("TUNTAP")
        .and_then(|v| v["BypassIPs"].as_array())
    {
        for value in values {
            if let Some(v) = value.as_str() {
                v.parse::<ipnet::IpNet>()?;
                excludes.push(v.into());
            }
        }
    }
    inbound["route_exclude_address"] = json!(excludes);
    let dns = settings
        .legacy
        .get("TUNTAP")
        .and_then(|v| v["DNS"].as_str())
        .unwrap_or("1.1.1.1");
    dns.parse::<std::net::IpAddr>()?;
    Ok(json!({"log":{"level":"warn"},"inbounds":[inbound],
        "dns":{"servers":[{"type":"udp","tag":"remote-dns","server":dns,"detour":"proxy"}],"final":"remote-dns"},
        "outbounds":[{"type":"socks","tag":"proxy","server":settings.local_address,"server_port":settings.socks_port},{"type":"direct","tag":"direct"}],
        "route":{"auto_detect_interface":true,"rules":[{"action":"sniff"},{"protocol":"dns","action":"hijack-dns"}],"final":"proxy"}}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_process_rules_convert_into_sing_box_families() {
        let rules: Vec<String> = [
            "^game\\.exe$",
            "chrome.exe",
            "!^notepad\\.exe$",
            "^.*steam.*$",
            "C://Games//other.exe",
            "helper",
            "^game\\.exe$",
            "Battle.net Launcher.exe",
            "\\\\Battle.net\\\\",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let out = process_rules(&rules);
        assert_eq!(
            out.names,
            // Spaces are legal in image names; they must not be rejected.
            // `^game\.exe$` and `!^notepad\.exe$` carry no regex metacharacters
            // once the anchors are stripped, so they become exact names too.
            vec![
                "Battle.net Launcher.exe",
                "chrome.exe",
                "game.exe",
                "notepad.exe"
            ]
        );
        assert_eq!(
            out.path_regexes,
            vec![
                "(?i)(\\\\|/).*steam.*$",
                // A path fragment is matched literally, anchors stripped.
                "(?i)C://Games//other\\.exe",
                "(?i)\\\\\\\\Battle\\.net\\\\\\\\",
            ]
        );
        assert_eq!(out.unconvertible, vec!["helper"], "无扩展名的裸名应被报告");
    }

    #[test]
    fn process_tun_config_matches_sing_box_rule_shape() {
        let mode = Mode {
            file: "test.json".into(),
            title: "测试模式".into(),
            kind: RouteKind::Process,
            handle: vec!["^.*launcher.*$".into(), "Battle.net Launcher.exe".into()],
            bypass: vec![
                "browser.exe".into(),
                "\\\\Steam\\\\".into(),
                "10.9.0.0/16".into(),
            ],
            path_regexes: Vec::new(),
            options: serde_json::Value::Null,
            argument: String::new(),
        };
        let settings = Settings::default();
        let config = tun_process(&settings, &mode, "203.0.113.7").unwrap();
        let rules = config["route"]["rules"].as_array().unwrap();
        // Bypass rules (exact and path families) must precede the handle rule
        // so exclusion wins.
        let bypass_index = rules
            .iter()
            .position(|r| r["process_name"] == serde_json::json!(["browser.exe"]))
            .expect("缺少 bypass 精确名规则");
        let handle_index = rules
            .iter()
            .position(|r| r["outbound"] == "proxy")
            .expect("缺少 proxy 规则");
        assert!(bypass_index < handle_index);
        // The path-regex family runs as its own rules: conditions inside one
        // sing-box rule combine with AND, so families must not be merged.
        assert!(
            rules.iter().any(|r| r["process_path_regex"]
                == serde_json::json!(["(?i)\\\\\\\\Steam\\\\\\\\"])
                && r["outbound"] == "direct"),
            "缺少 bypass 路径规则"
        );
        // Both families proxy handled processes, each in its own rule.
        let proxy_exact = rules
            .iter()
            .find(|r| r["outbound"] == "proxy")
            .expect("缺少 proxy 规则");
        assert_eq!(
            proxy_exact["process_name"],
            serde_json::json!(["Battle.net Launcher.exe"])
        );
        assert!(
            rules.iter().any(|r| r["outbound"] == "proxy"
                && r["process_path_regex"] == serde_json::json!(["(?i)(\\\\|/).*launcher.*$"])),
            "缺少 proxy 路径规则"
        );
        // DNS hijack is scoped to handled processes: exact names and path
        // regexes each get their own hijack rule.
        let dns_exact = rules
            .iter()
            .find(|r| r["action"] == "hijack-dns" && r.get("process_name").is_some())
            .expect("缺少精确名 DNS 劫持规则");
        assert_eq!(
            dns_exact["process_name"],
            serde_json::json!(["Battle.net Launcher.exe"])
        );
        assert!(
            rules.iter().any(|r| r["action"] == "hijack-dns"
                // Bypass entries never appear here: they are excluded by the
                // earlier direct rules, not hijacked.
                && r["process_path_regex"] == serde_json::json!(["(?i)(\\\\|/).*launcher.*$"])),
            "缺少路径 DNS 劫持规则"
        );
        // Bypass CIDR excludes the range from the TUN.
        let excludes = config["inbounds"][0]["route_exclude_address"]
            .as_array()
            .unwrap();
        assert!(excludes.iter().any(|v| v == "10.9.0.0/16"));
        assert!(excludes.iter().any(|v| v == "203.0.113.7/32"));
        assert_eq!(config["route"]["final"], "direct");
    }

    #[test]
    fn process_tun_rejects_modes_without_convertible_rules() {
        let mode = Mode {
            file: "test.json".into(),
            title: "无法识别的模式".into(),
            kind: RouteKind::Process,
            handle: vec!["helper".into()],
            bypass: vec![],
            path_regexes: Vec::new(),
            options: serde_json::Value::Null,
            argument: String::new(),
        };
        let error = tun_process(&Settings::default(), &mode, "203.0.113.7").unwrap_err();
        assert!(error.to_string().contains("无法转换"));
    }

    #[test]
    fn process_tun_proxies_wizard_directory_rules() {
        // A wizard mode carries no legacy rules at all: one final directory
        // regex must be enough to proxy (and must not trip the "unusable"
        // bail that guards empty conversions).
        let rule = r"(?i)^D:\\Games\\My Game\\";
        let mode = Mode {
            file: "wizard.json".into(),
            title: "向导模式".into(),
            kind: RouteKind::Process,
            handle: vec![],
            bypass: vec![],
            path_regexes: vec![rule.into()],
            options: serde_json::Value::Null,
            argument: String::new(),
        };
        let config = tun_process(&Settings::default(), &mode, "203.0.113.7").unwrap();
        let rules = config["route"]["rules"].as_array().unwrap();
        let proxy = rules
            .iter()
            .find(|r| r["outbound"] == "proxy")
            .expect("缺少 proxy 规则");
        assert_eq!(proxy["process_path_regex"], serde_json::json!([rule]));
        assert!(
            proxy.get("process_name").is_none(),
            "纯目录模式不应有精确名规则"
        );
        // DNS hijack covers the directory family as well.
        assert!(
            rules
                .iter()
                .any(|r| r["action"] == "hijack-dns" && r.get("process_path_regex").is_some()),
            "缺少目录族 DNS 劫持规则"
        );
    }
}
