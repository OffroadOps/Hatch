use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use npga::{
    config::{self, Store},
    core_config, links,
    model::{Node, RouteKind, Settings, Subscription},
    modes, platform,
};
use serde_json::json;
use std::{fs, path::PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("npga-rust-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn vless() -> Node {
    let mut n = Node {
        hostname: "example.com".into(),
        remark: "测试节点".into(),
        ..Node::default()
    };
    n.set("UserID", "550e8400-e29b-41d4-a716-446655440000");
    n
}

#[test]
fn legacy_numeric_and_string_ports() {
    for port in [json!(443), json!("443")] {
        let node:Node=serde_json::from_value(json!({"Type":"VLESS","Hostname":"example.com","Port":port,"UserID":"550e8400-e29b-41d4-a716-446655440000"})).unwrap();
        assert_eq!(node.port, 443);
        node.validate().unwrap();
    }
    assert!(serde_json::from_value::<Node>(json!({"Port":65536})).is_err());
}
#[test]
fn custom_share_roundtrip_preserves_reality() {
    let mut n = vless();
    n.set("TLSSecureType", "reality");
    n.set("RealityPublicKey", "public");
    n.set("RealityShortId", "12345678");
    n.set("Flow", "xtls-rprx-vision");
    let link = links::export(&n).unwrap();
    let parsed = links::parse(&link).unwrap();
    assert_eq!(parsed.nodes[0].identity(), n.identity());
    assert_eq!(parsed.nodes[0].remark, "测试节点");
    assert_eq!(
        links::parse(&link.replacen("NPGA://", "Hatch://", 1))
            .unwrap()
            .nodes[0]
            .port,
        443
    );
}
#[test]
fn base64_subscription_and_dedup() {
    let link = links::export(&vless()).unwrap();
    let parsed = links::parse(&STANDARD.encode(format!("{link}\r\n{link}"))).unwrap();
    assert_eq!(parsed.nodes.len(), 1);
    assert!(links::parse("<html>upstream error</html>").is_err());
}
#[test]
fn percent_encoded_plus_and_ipv6() {
    let node = links::parse("trojan://p%2Ba%3Ass@[::1]:8443?sni=example.com#%E6%B5%8B%E8%AF%95")
        .unwrap()
        .nodes
        .remove(0);
    assert_eq!(node.hostname, "::1");
    assert_eq!(node.text("Password"), "p+a:ss");
    assert_eq!(node.remark, "测试");
}
#[test]
fn sip002_ss_and_legacy_ss() {
    let auth = URL_SAFE_NO_PAD.encode("aes-128-gcm:a+b:c");
    let n = links::parse(&format!("ss://{auth}@example.com:1234#test"))
        .unwrap()
        .nodes
        .remove(0);
    assert_eq!(n.text("Password"), "a+b:c");
    let body = STANDARD.encode("aes-128-gcm:a+b:c@example.com:1234");
    let n = links::parse(&format!("ss://{body}#test"))
        .unwrap()
        .nodes
        .remove(0);
    assert_eq!(n.port, 1234);
    assert_eq!(n.text("Password"), "a+b:c");
}
#[test]
fn hysteria_auth_parameters() {
    let n=links::parse("hy2://u:p%2Ba@example.com:443?sni=cdn.example.com&obfs=salamander&obfs-password=secret&insecure=1").unwrap().nodes.remove(0);
    assert_eq!(n.text("Password"), "u:p+a");
    assert!(n.flag("TLSInsecure"));
    let c = core_config::sing_box(&n, &Settings::default()).unwrap();
    assert_eq!(c["outbounds"][0]["obfs"]["password"], "secret");
    assert!(c["inbounds"][0].get("sniff").is_none());
}
#[test]
fn vmess_string_numbers() {
    let body=STANDARD.encode(serde_json::to_vec(&json!({"add":"example.com","port":"443","aid":"0","id":"550e8400-e29b-41d4-a716-446655440000","net":"ws","tls":"tls","path":"/proxy","ps":"VMess"})).unwrap());
    let n = links::parse(&format!("vmess://{body}"))
        .unwrap()
        .nodes
        .remove(0);
    assert_eq!(n.number("AlterID", 100), 0);
    assert_eq!(
        core_config::xray(&n, &Settings::default()).unwrap()["outbounds"][0]["streamSettings"]["wsSettings"]
            ["path"],
        "/proxy"
    );
}
#[test]
fn socks_legacy_type_is_supported() {
    let n:Node=serde_json::from_value(json!({"Type":"SOCKS","Hostname":"127.0.0.1","Port":1080,"Username":"user","Password":"pass"})).unwrap();
    let c = core_config::xray(&n, &Settings::default()).unwrap();
    assert_eq!(
        c["outbounds"][0]["settings"]["servers"][0]["users"][0]["pass"],
        "pass"
    );
}
#[test]
fn invalid_subscription_never_replaces_nodes() {
    let mut settings = Settings::default();
    settings.nodes.push(vless());
    let sub = Subscription {
        remark: "测试节点".into(),
        ..Subscription::default()
    };
    assert!(npga::subscription::replace(&settings, &sub, "error").is_err());
    let link = links::export(&vless()).unwrap();
    assert!(npga::subscription::replace(&settings, &sub, &format!("{link}\ninvalid")).is_err());
    assert_eq!(settings.nodes.len(), 1);
}
#[test]
fn subscription_ownership_does_not_delete_manual_group() {
    let mut settings = Settings::default();
    let sub = Subscription {
        remark: "手动添加".into(),
        ..Subscription::default()
    };
    settings.nodes.push(vless());
    let mut owned = vless();
    owned.set("SubscriptionId", sub.id.clone());
    settings.nodes.push(owned);
    let next =
        npga::subscription::replace(&settings, &sub, &links::export(&vless()).unwrap()).unwrap();
    assert_eq!(next.nodes.len(), 2);
    assert_eq!(
        next.nodes
            .iter()
            .filter(|n| n.text("SubscriptionId").is_empty())
            .count(),
        1
    );
}
#[cfg(windows)]
#[test]
fn dpapi_and_config_backup_roundtrip() {
    let dir = Temp::new();
    let store = Store::new(dir.0.clone());
    let mut s = Settings::default();
    s.nodes.push(vless());
    s.nodes[0].set("PrivateKey", "private-secret");
    s.subscriptions.push(Subscription {
        link: "https://example.com/sub?token=secret-token".into(),
        ..Subscription::default()
    });
    s.legacy
        .insert("FutureSetting".into(), json!({"preserve":true}));
    store.save(&s).unwrap();
    let text = fs::read_to_string(store.file()).unwrap();
    assert!(text.contains("DPAPI:"));
    assert!(!text.contains("private-secret"));
    assert!(!text.contains("secret-token"));
    let loaded = store.load().unwrap().0;
    assert_eq!(loaded.nodes[0].text("PrivateKey"), "private-secret");
    assert_eq!(loaded.legacy["FutureSetting"]["preserve"], true);
    store.save(&s).unwrap();
    fs::write(store.file(), "broken").unwrap();
    let (loaded, warning) = store.load().unwrap();
    assert_eq!(loaded.nodes.len(), 1);
    assert!(warning.is_some());
}
#[cfg(windows)]
#[test]
fn decrypt_rejects_broken_dpapi_without_data_loss() {
    assert!(platform::unprotect("DPAPI:broken!").is_err());
    assert_eq!(platform::unprotect("plain").unwrap(), "plain");
    let s = platform::protect("密码+secret").unwrap();
    assert_eq!(platform::unprotect(&s).unwrap(), "密码+secret");
}
#[test]
fn migration_keeps_original_file() {
    let dir = Temp::new();
    let path = dir.0.join("legacy.json");
    let text=serde_json::to_string(&json!({"Server":[vless()],"ServerComboBoxSelectedIndex":0,"TUNTAP":{"Address":"10.0.236.10"},"Socks5LocalPort":9995,"HTTPLocalPort":2802})).unwrap();
    fs::write(&path, &text).unwrap();
    let settings = config::import(&path).unwrap();
    assert_eq!(settings.nodes.len(), 1);
    assert_eq!(settings.legacy["TUNTAP"]["Address"], "10.0.236.10");
    assert_eq!(fs::read_to_string(path).unwrap(), text);
}
#[test]
fn mode_include_cycle_and_boundary() {
    let dir = Temp::new();
    fs::write(dir.0.join("base.txt"), "# Base,0\nfoo.exe\n!bar.exe\n").unwrap();
    fs::write(
        dir.0.join("game.txt"),
        "# Game,0\n#include <base.h>\nGame.exe\n",
    )
    .unwrap();
    let (modes, errors) = modes::scan(&dir.0);
    assert!(errors.is_empty());
    let game = modes.iter().find(|m| m.title == "Game").unwrap();
    assert_eq!(game.handle.len(), 2);
    assert_eq!(game.bypass[0], "bar.exe");
    fs::write(dir.0.join("cycle.txt"), "# Cycle,0\n#include <cycle.h>\n").unwrap();
    fs::write(
        dir.0.join("escape.txt"),
        "# Escape,0\n#include <../escape.txt>\n",
    )
    .unwrap();
    let (_, errors) = modes::scan(&dir.0);
    assert_eq!(errors.len(), 2);
}
#[test]
fn legacy_tun_bypass_and_server_exclusion() {
    let dir = Temp::new();
    fs::write(
        dir.0.join("bypass.txt"),
        "# Bypass LAN,2\n10.0.0.0/8\n192.168.0.0/16\n",
    )
    .unwrap();
    let (modes, errors) = modes::scan(&dir.0);
    assert!(errors.is_empty());
    assert_eq!(modes[0].kind, RouteKind::Tun);
    assert_eq!(modes[0].handle, ["0.0.0.0/0"]);
    let config = core_config::tun(&Settings::default(), Some(&modes[0]), "203.0.113.1").unwrap();
    assert!(
        config["inbounds"][0]["route_exclude_address"]
            .as_array()
            .unwrap()
            .contains(&json!("203.0.113.1/32"))
    );
    assert_eq!(config["route"]["rules"][1]["action"], "hijack-dns");
}
#[test]
fn unsupported_protocols_fail_explicitly() {
    let mut n = vless();
    n.protocol = "SSR".into();
    n.set("Password", "secret");
    assert!(core_config::xray(&n, &Settings::default()).is_err());
    n.protocol = "SS".into();
    n.set("Plugin", "v2ray-plugin");
    assert!(core_config::xray(&n, &Settings::default()).is_err());
}
#[test]
fn port_conflict_settings_are_rejected() {
    let s = Settings {
        socks_port: 2802,
        ..Settings::default()
    };
    assert!(s.validate().is_err());
}

#[cfg(windows)]
#[test]
fn new_selection_survives_legacy_index_on_reload() {
    let dir = Temp::new();
    let store = Store::new(dir.0.clone());
    let mut first = vless();
    first.remark = "A".into();
    let mut second = vless();
    second.remark = "B".into();
    let mut settings = Settings {
        nodes: vec![first, second],
        selected_node: 1,
        ..Settings::default()
    };
    settings
        .legacy
        .insert("ServerComboBoxSelectedIndex".into(), json!(0));
    store.save(&settings).unwrap();
    assert_eq!(store.load().unwrap().0.selected_node, 1);
}
