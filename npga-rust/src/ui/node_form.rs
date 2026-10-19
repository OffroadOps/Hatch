//! The protocol specific parameter editor used by the node dialog.

use eframe::egui;
use npga::model::Node;

pub(super) fn field_template(protocol: &str) -> String {
    let value = match protocol {
        "VLESS" => {
            serde_json::json!({"UserID":"","EncryptMethod":"none","TransferProtocol":"tcp","TLSSecureType":"tls","ServerName":"","Flow":""})
        }
        "VMess" => {
            serde_json::json!({"UserID":"","EncryptMethod":"auto","AlterID":0,"TransferProtocol":"ws","TLSSecureType":"tls","Host":"","Path":"/","ServerName":""})
        }
        "Trojan" => {
            serde_json::json!({"Password":"","TransferProtocol":"tcp","TLSSecureType":"tls","ServerName":""})
        }
        "Hysteria2" => {
            serde_json::json!({"Password":"","ServerName":"","TLSInsecure":false,"UploadBandwidth":100,"DownloadBandwidth":100})
        }
        "SS" => serde_json::json!({"Password":"","EncryptMethod":"aes-128-gcm"}),
        "SSR" => {
            serde_json::json!({"Password":"","EncryptMethod":"aes-128-cfb","Protocol":"origin","OBFS":"plain"})
        }
        "WireGuard" => {
            serde_json::json!({"PrivateKey":"","PeerPublicKey":"","PreSharedKey":"","LocalAddresses":"172.16.0.2/32","MTU":1420})
        }
        _ => serde_json::json!({"Username":"","Password":"","Version":"5"}),
    };
    serde_json::to_string_pretty(&value).unwrap_or_default()
}
pub(super) fn text_parameter(
    ui: &mut egui::Ui,
    node: &mut Node,
    label: &str,
    key: &str,
    secret: bool,
) {
    ui.label(label);
    let mut value = node.text(key).to_owned();
    if ui
        .add(
            egui::TextEdit::singleline(&mut value)
                .password(secret)
                .desired_width(330.0),
        )
        .changed()
    {
        node.set(key, value);
    }
    ui.end_row();
}
pub(super) fn choice_parameter(
    ui: &mut egui::Ui,
    node: &mut Node,
    label: &str,
    key: &str,
    fallback: &str,
    values: &[&str],
) {
    ui.label(label);
    let mut value = node.string(key, fallback);
    let previous = value.clone();
    egui::ComboBox::from_id_salt(key)
        .selected_text(&value)
        .width(200.0)
        .show_ui(ui, |ui| {
            for option in values {
                ui.selectable_value(&mut value, (*option).into(), *option);
            }
        });
    if value != previous {
        node.set(key, value);
    }
    ui.end_row();
}
pub(super) fn node_fields(ui: &mut egui::Ui, node: &mut Node) {
    egui::Grid::new("protocol-fields")
        .num_columns(2)
        .min_col_width(120.0)
        .spacing([26.0, 12.0])
        .show(ui, |ui| {
            match node.protocol.as_str() {
                "VLESS" | "VMess" => text_parameter(ui, node, "用户 UUID", "UserID", true),
                "WireGuard" => {
                    text_parameter(ui, node, "私钥", "PrivateKey", true);
                    text_parameter(ui, node, "对端公钥", "PeerPublicKey", false);
                    text_parameter(ui, node, "预共享密钥", "PreSharedKey", true);
                    text_parameter(ui, node, "本地地址 / CIDR", "LocalAddresses", false);
                }
                "SOCKS" | "Socks5" => {
                    text_parameter(ui, node, "用户名（可留空）", "Username", false);
                    text_parameter(ui, node, "密码（可留空）", "Password", true);
                }
                _ => text_parameter(ui, node, "密码", "Password", true),
            }
            if node.protocol == "SS" || node.protocol == "SSR" {
                choice_parameter(
                    ui,
                    node,
                    "加密方式",
                    "EncryptMethod",
                    "aes-128-gcm",
                    &[
                        "aes-128-gcm",
                        "aes-256-gcm",
                        "chacha20-ietf-poly1305",
                        "2022-blake3-aes-128-gcm",
                        "2022-blake3-aes-256-gcm",
                    ],
                );
            }
            if matches!(node.protocol.as_str(), "VMess" | "VLESS" | "Trojan") {
                choice_parameter(
                    ui,
                    node,
                    "传输方式",
                    "TransferProtocol",
                    "tcp",
                    &["tcp", "ws", "grpc", "xhttp", "kcp"],
                );
                let fallback = if node.protocol == "Trojan" {
                    "tls"
                } else {
                    "none"
                };
                choice_parameter(
                    ui,
                    node,
                    "传输安全",
                    "TLSSecureType",
                    fallback,
                    &["none", "tls", "reality"],
                );
                text_parameter(ui, node, "TLS 服务器名称", "ServerName", false);
                if matches!(node.text("TransferProtocol"), "ws" | "grpc" | "xhttp") {
                    text_parameter(ui, node, "Host", "Host", false);
                    text_parameter(ui, node, "路径 / ServiceName", "Path", false);
                }
                if node.protocol == "VLESS" {
                    text_parameter(ui, node, "Flow（可留空）", "Flow", false);
                }
                if node.text("TLSSecureType") == "reality" {
                    text_parameter(ui, node, "Reality 公钥", "RealityPublicKey", false);
                    text_parameter(ui, node, "Reality ShortId", "RealityShortId", false);
                }
            }
            if node.protocol == "Hysteria2" {
                text_parameter(ui, node, "TLS 服务器名称", "ServerName", false);
                ui.label("证书校验");
                let mut insecure = node.flag("TLSInsecure");
                if ui.checkbox(&mut insecure, "跳过证书验证").changed() {
                    node.set("TLSInsecure", insecure);
                }
                ui.end_row();
                for (label, key) in [
                    ("上行带宽 / Mbps", "UploadBandwidth"),
                    ("下行带宽 / Mbps", "DownloadBandwidth"),
                ] {
                    ui.label(label);
                    let mut n = node.number(key, 100);
                    if ui
                        .add(egui::DragValue::new(&mut n).range(1..=10000))
                        .changed()
                    {
                        node.set(key, serde_json::json!(n));
                    }
                    ui.end_row();
                }
            }
        });
}
