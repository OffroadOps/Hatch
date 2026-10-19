//! Modal dialogs: node import, node editor and subscription editor.

use super::node_form::node_fields;
use super::theme::{MUTED, primary};
use super::{App, PendingSave};
use eframe::egui::{self, RichText};
use npga::{config, links};
use std::path::PathBuf;

impl App {
    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        if self.import_open {
            let mut open = true;
            let mut action = 0;
            egui::Window::new("导入节点与配置").open(&mut open).collapsible(false).resizable(true).default_width(560.0).show(ctx,|ui|{
                ui.label(RichText::new("粘贴 NPGA / VLESS / VMess / SS / SSR / Trojan / Hysteria2 / SOCKS 分享链接或 Base64 订阅内容").size(12.0).color(MUTED));ui.add_space(12.0);
                ui.add(egui::TextEdit::multiline(&mut self.import_text).desired_rows(7).desired_width(f32::INFINITY).hint_text("vless://…"));ui.add_space(12.0);
                if primary(ui,"导入分享链接",[150.0,36.0],!self.import_text.trim().is_empty()&&!self.busy){action=1;}
                ui.add_space(16.0);ui.separator();ui.add_space(16.0);ui.label("或导入旧版配置文件（先备份当前 Rust 配置）");ui.add_space(8.0);
                ui.add(egui::TextEdit::singleline(&mut self.import_path).desired_width(f32::INFINITY).hint_text("D:\\NPGA\\data\\settings.json"));
                ui.add_space(12.0);if ui.button("导入 settings.json").clicked(){action=2;}
                ui.add_space(8.0);ui.label(RichText::new("也可以将 settings.json 或节点文本文件拖到窗口。").size(11.0).color(MUTED));
            });
            self.import_open = open;
            if action == 1 {
                match links::parse(&self.import_text) {
                    Ok(parsed) => {
                        let mut next = self.settings.clone();
                        let mut count = 0;
                        for node in parsed.nodes {
                            if !next.nodes.iter().any(|n| n.identity() == node.identity()) {
                                next.nodes.push(node);
                                count += 1;
                            }
                        }
                        if self.commit(next) {
                            self.import_open = false;
                            self.import_text.clear();
                            self.info(format!(
                                "已导入 {count} 个节点，忽略 {} 个无效条目",
                                parsed.rejected
                            ));
                        }
                    }
                    Err(e) => self.error(format!("{e:#}")),
                }
            }
            if action == 2 {
                match config::import(&PathBuf::from(self.import_path.trim())) {
                    Ok(next) => {
                        let count = next.nodes.len();
                        if self.commit(next) {
                            self.import_open = false;
                            self.info(format!("已迁移 {count} 个节点，原配置文件保留"));
                        }
                    }
                    Err(e) => self.error(format!("导入失败：{e:#}")),
                }
            }
        }
        // The lists may have changed since the last frame (a node was
        // deleted, a subscription was refreshed); drop editors whose target is
        // gone before they can write to a stale slot.
        self.drop_stale_editors();
        if let Some(mut editor) = self.editor.take() {
            if editor.node.fields.is_empty() {
                editor.node.fields = serde_json::from_str(&editor.advanced).unwrap_or_default();
            }
            let mut open = true;
            let mut save = false;
            egui::Window::new(if editor.index.is_some() {
                "编辑节点"
            } else {
                "添加节点"
            })
            .open(&mut open)
            .collapsible(false)
            .default_width(580.0)
            .max_height(600.0)
            .vscroll(true)
            .show(ctx, |ui| {
                egui::Grid::new("node-form")
                    .num_columns(2)
                    .min_col_width(120.0)
                    .spacing([26.0, 12.0])
                    .show(ui, |ui| {
                        ui.label("协议");
                        egui::ComboBox::from_id_salt("protocol-editor")
                            .selected_text(&editor.node.protocol)
                            .show_ui(ui, |ui| {
                                for p in [
                                    "VLESS",
                                    "VMess",
                                    "Trojan",
                                    "Hysteria2",
                                    "SS",
                                    "SOCKS",
                                    "WireGuard",
                                    "SSR",
                                ] {
                                    ui.selectable_value(&mut editor.node.protocol, p.into(), p);
                                }
                            });
                        ui.end_row();
                        ui.label("名称");
                        ui.text_edit_singleline(&mut editor.node.remark);
                        ui.end_row();
                        ui.label("分组");
                        ui.text_edit_singleline(&mut editor.node.group);
                        ui.end_row();
                        ui.label("服务器地址");
                        ui.text_edit_singleline(&mut editor.node.hostname);
                        ui.end_row();
                        ui.label("端口");
                        ui.add(egui::DragValue::new(&mut editor.node.port).range(1..=65535));
                        ui.end_row();
                    });
                ui.add_space(18.0);
                let before = editor.node.fields.clone();
                node_fields(ui, &mut editor.node);
                if before != editor.node.fields {
                    editor.advanced =
                        serde_json::to_string_pretty(&editor.node.fields).unwrap_or_default();
                }
                ui.add_space(12.0);
                egui::CollapsingHeader::new("高级协议参数").show(ui, |ui| {
                    ui.label(
                        RichText::new("可编辑原版协议字段，此处包含明文凭据。")
                            .size(11.0)
                            .color(MUTED),
                    );
                    ui.add(
                        egui::TextEdit::multiline(&mut editor.advanced)
                            .desired_width(f32::INFINITY)
                            .desired_rows(8)
                            .code_editor(),
                    );
                });
                ui.add_space(12.0);
                save = primary(ui, "保存节点", [140.0, 38.0], true);
            });
            if save {
                let parsed = serde_json::from_str(&editor.advanced)
                    .map_err(anyhow::Error::from)
                    .and_then(|fields| {
                        editor.node.fields = fields;
                        editor.node.validate()
                    });
                match parsed {
                    Ok(()) => {
                        let mut next = self.settings.clone();
                        match editor.index {
                            Some(index) => {
                                // The list may have changed while the dialog
                                // was open. Writing to the stale slot would
                                // overwrite an unrelated node, so refuse.
                                let still_there = next.nodes.get(index).is_some_and(|current| {
                                    Some(current.identity()) == editor.identity
                                });
                                if !still_there {
                                    self.error("节点列表已变化，请关闭后重新打开编辑器".into());
                                    self.editor = Some(editor);
                                    return;
                                }
                                next.nodes[index] = editor.node.clone();
                            }
                            None => next.nodes.push(editor.node.clone()),
                        }
                        self.pending_save = Some(PendingSave::Node(editor));
                        self.commit(next);
                    }
                    Err(e) => {
                        self.error(e.to_string());
                        self.editor = Some(editor);
                    }
                }
            } else if open {
                self.editor = Some(editor);
            }
        }
        if let Some(mut editor) = self.subscription_editor.take() {
            let mut open = true;
            let mut save = false;
            egui::Window::new("订阅设置")
                .open(&mut open)
                .collapsible(false)
                .default_width(520.0)
                .show(ctx, |ui| {
                    ui.label("订阅名称");
                    ui.text_edit_singleline(&mut editor.subscription.remark);
                    ui.add_space(12.0);
                    ui.label("订阅地址");
                    ui.add(
                        egui::TextEdit::singleline(&mut editor.subscription.link)
                            .password(true)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(12.0);
                    ui.label("User-Agent");
                    ui.text_edit_singleline(&mut editor.subscription.user_agent);
                    ui.add_space(16.0);
                    save = primary(ui, "保存订阅", [130.0, 36.0], true);
                });
            if save {
                if editor.subscription.remark.trim().is_empty()
                    || !url::Url::parse(&editor.subscription.link)
                        .is_ok_and(|u| matches!(u.scheme(), "http" | "https"))
                {
                    self.error("订阅名称或 HTTP/HTTPS 地址无效".into());
                    self.subscription_editor = Some(editor);
                } else {
                    let mut next = self.settings.clone();
                    match editor.index {
                        Some(index) => {
                            // Refuse to write to a slot that now holds a
                            // different subscription.
                            let still_there = next
                                .subscriptions
                                .get(index)
                                .is_some_and(|current| Some(current.id.clone()) == editor.id);
                            if !still_there {
                                self.error("订阅列表已变化，请关闭后重新打开编辑器".into());
                                self.subscription_editor = Some(editor);
                                return;
                            }
                            next.subscriptions[index] = editor.subscription.clone();
                        }
                        None => next.subscriptions.push(editor.subscription.clone()),
                    }
                    self.pending_save = Some(PendingSave::Subscription(editor));
                    self.commit(next);
                }
            } else if open {
                self.subscription_editor = Some(editor);
            }
        }
    }
}
