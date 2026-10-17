use eframe::egui::{
    self, Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, Stroke, Vec2,
};
use npga::{
    config::{self, Store},
    links,
    model::{Node, RouteKind, Settings, Subscription},
    modes::{self, Mode},
    platform,
    runtime::{CommandMessage, Event, RuntimePaths, ToolKind, Worker},
};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

const BLUE: Color32 = Color32::from_rgb(45, 109, 246);
const INK: Color32 = Color32::from_rgb(29, 41, 61);
const MUTED: Color32 = Color32::from_rgb(133, 145, 164);
const LINE: Color32 = Color32::from_rgb(232, 237, 245);
const PALE: Color32 = Color32::from_rgb(245, 248, 254);
const GREEN: Color32 = Color32::from_rgb(39, 173, 123);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Connect,
    Nodes,
    Subscriptions,
    Logs,
    Settings,
    About,
}
impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Connect => "加速连接",
            Self::Nodes => "我的节点",
            Self::Subscriptions => "订阅管理",
            Self::Logs => "运行日志",
            Self::Settings => "高级设置",
            Self::About => "关于 NPGA",
        }
    }
}
#[derive(PartialEq)]
enum Connection {
    Idle,
    Starting,
    Active,
    Stopping,
}

pub struct App {
    paths: RuntimePaths,
    store: Store,
    settings: Settings,
    draft: Settings,
    load_error: bool,
    worker: Worker,
    modes: Vec<Mode>,
    mode_errors: Vec<String>,
    page: Page,
    connection: Connection,
    active_core: String,
    since: Option<Instant>,
    notice: Option<(String, bool)>,
    logs: VecDeque<String>,
    search: String,
    latencies: Vec<Option<(Option<u128>, &'static str)>>,
    probes_left: usize,
    busy: bool,
    import_open: bool,
    import_text: String,
    import_path: String,
    editor: Option<(Option<usize>, Node, String)>,
    subscription_editor: Option<(Option<usize>, Subscription)>,
    up: u64,
    down: u64,
    up_rate: f64,
    down_rate: f64,
    traffic_at: Instant,
    /// Diagnostic tool currently running, if any.
    tool_running: Option<ToolKind>,
    /// Last update check result: (message, is_error) for the About page.
    update_result: Option<(String, bool)>,
    screenshot: Option<PathBuf>,
    screenshot_requested: bool,
    frames: u32,
    exit: bool,
    #[cfg(windows)]
    tray: Option<tray_icon::TrayIcon>,
    #[cfg(windows)]
    tray_ids: Option<(
        tray_icon::menu::MenuId,
        tray_icon::menu::MenuId,
        tray_icon::menu::MenuId,
    )>,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        paths: RuntimePaths,
        store: Store,
        loaded: anyhow::Result<(Settings, Option<String>)>,
        screenshot: Option<PathBuf>,
        view: &str,
    ) -> Self {
        theme(&cc.egui_ctx);
        let (settings, notice, load_error) = match loaded {
            Ok((s, w)) => (s, w.map(|s| (s, true)), false),
            Err(e) => (
                Settings::default(),
                Some((format!("{e:#}。修复配置后重新启动，或导入其他配置。"), true)),
                true,
            ),
        };
        let (modes, mode_errors) = modes::scan(&paths.modes());
        let worker = Worker::new(paths.clone());
        #[cfg(windows)]
        let (tray, tray_ids) = if screenshot.is_none() {
            tray()
                .map(|(t, ids)| (Some(t), Some(ids)))
                .unwrap_or((None, None))
        } else {
            (None, None)
        };
        Self {
            draft: settings.clone(),
            latencies: vec![None; settings.nodes.len()],
            paths,
            store,
            settings,
            load_error,
            worker,
            modes,
            mode_errors,
            page: match view {
                "nodes" => Page::Nodes,
                "subscriptions" => Page::Subscriptions,
                "logs" => Page::Logs,
                "settings" => Page::Settings,
                "about" => Page::About,
                _ => Page::Connect,
            },
            connection: Connection::Idle,
            active_core: String::new(),
            since: None,
            notice,
            logs: VecDeque::new(),
            search: String::new(),
            probes_left: 0,
            busy: false,
            import_open: false,
            import_text: String::new(),
            import_path: String::new(),
            editor: if view == "add-node" {
                Some((None, Node::default(), field_template("VLESS")))
            } else {
                None
            },
            subscription_editor: None,
            up: 0,
            down: 0,
            up_rate: 0.0,
            down_rate: 0.0,
            traffic_at: Instant::now(),
            tool_running: None,
            update_result: None,
            screenshot,
            screenshot_requested: false,
            frames: 0,
            exit: false,
            #[cfg(windows)]
            tray,
            #[cfg(windows)]
            tray_ids,
        }
    }
    fn editable(&self) -> bool {
        self.connection == Connection::Idle
            && !self.busy
            && self.probes_left == 0
            && !self.load_error
    }
    fn commit(&mut self, next: Settings) -> bool {
        match self.store.save(&next) {
            Ok(()) => {
                self.settings = next;
                self.draft = self.settings.clone();
                self.latencies.resize(self.settings.nodes.len(), None);
                self.load_error = false;
                true
            }
            Err(e) => {
                self.error(format!("保存失败：{e:#}"));
                false
            }
        }
    }
    fn error(&mut self, message: String) {
        self.logs.push_back(message.clone());
        self.notice = Some((message, true));
    }
    fn info(&mut self, message: impl Into<String>) {
        self.notice = Some((message.into(), false));
    }
    fn send(&mut self, command: CommandMessage) {
        if let Err(e) = self.worker.send(command) {
            self.error(e.to_string());
        }
    }
    fn selected_mode(&self) -> Option<Mode> {
        self.modes
            .iter()
            .find(|m| m.file.to_string_lossy() == self.settings.mode_file)
            .cloned()
    }
    fn start_stop(&mut self) {
        match self.connection {
            Connection::Idle if self.editable() => {
                if self.settings.nodes.is_empty() {
                    self.import_open = true;
                    return;
                }
                self.worker.cancel.store(false, Ordering::Relaxed);
                self.connection = Connection::Starting;
                self.up = 0;
                self.down = 0;
                self.up_rate = 0.0;
                self.down_rate = 0.0;
                self.send(CommandMessage::Start {
                    settings: Box::new(self.settings.clone()),
                    mode: self.selected_mode(),
                });
            }
            Connection::Active | Connection::Starting => {
                self.worker.cancel.store(true, Ordering::Relaxed);
                self.connection = Connection::Stopping;
                self.send(CommandMessage::Stop);
            }
            _ => {}
        }
    }
    fn probe(&mut self) {
        if self.settings.nodes.is_empty() {
            self.info("请先添加节点");
            return;
        }
        self.probes_left = self.settings.nodes.len();
        self.latencies = vec![None; self.probes_left];
        self.send(CommandMessage::Probe(self.settings.nodes.clone()));
    }
    fn run_tool(&mut self, kind: ToolKind) {
        if self.tool_running.is_some() {
            return;
        }
        self.tool_running = Some(kind);
        self.busy = true;
        if kind != ToolKind::Update {
            self.info(format!("正在执行{}…", kind.label()));
        }
        self.send(CommandMessage::Tool {
            kind,
            settings: Box::new(self.settings.clone()),
        });
    }
    fn events(&mut self) {
        while let Ok(event) = self.worker.events.try_recv() {
            match event {
                Event::Starting => {}
                Event::Connected { core } => {
                    self.active_core = core;
                    self.connection = Connection::Active;
                    self.since = Some(Instant::now());
                    self.info("连接已建立");
                }
                Event::Stopped => {
                    self.connection = Connection::Idle;
                    self.since = None;
                    self.up_rate = 0.0;
                    self.down_rate = 0.0;
                    self.info("连接已停止");
                }
                Event::Failed(message) => {
                    self.connection = Connection::Idle;
                    self.since = None;
                    self.error(message);
                }
                Event::OperationFailed(message) => {
                    self.busy = false;
                    self.error(message);
                }
                Event::Log(line) => {
                    if !line.is_empty() {
                        self.logs.push_back(line);
                    }
                }
                Event::Checked(message) => {
                    self.busy = false;
                    self.info(message);
                }
                Event::Latency {
                    index,
                    millis,
                    method,
                } => {
                    if let Some(slot) = self.latencies.get_mut(index) {
                        *slot = Some((millis, method));
                    }
                    self.probes_left = self.probes_left.saturating_sub(1);
                }
                Event::Refreshed(next) => {
                    self.busy = false;
                    let count = next.nodes.len();
                    if self.commit(*next) {
                        self.info(format!("订阅已更新，当前共 {count} 个节点"));
                    }
                }
                Event::Traffic { up, down } => {
                    let elapsed = self.traffic_at.elapsed().as_secs_f64().max(0.001);
                    self.up_rate = up.saturating_sub(self.up) as f64 / elapsed;
                    self.down_rate = down.saturating_sub(self.down) as f64 / elapsed;
                    self.up = up;
                    self.down = down;
                    self.traffic_at = Instant::now();
                }
                Event::ToolResult {
                    kind,
                    text,
                    is_error,
                } => {
                    self.busy = false;
                    self.tool_running = None;
                    if kind == ToolKind::Update {
                        self.update_result = Some((text.clone(), is_error));
                    }
                    if is_error {
                        self.error(format!("{}失败：{text}", kind.label()));
                    } else {
                        self.info(format!("{}：{text}", kind.label()));
                    }
                }
                Event::Selection { text } => self.info(text),
                Event::Switched {
                    index,
                    millis,
                    reason,
                } => {
                    if let Some(slot) = self.latencies.get_mut(index) {
                        *slot = Some((millis, "自动选路"));
                    }
                    self.info(format!("自动选路：{reason}"));
                }
            }
            while self.logs.len() > 1500 {
                self.logs.pop_front();
            }
        }
    }
    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("navigation")
            .exact_width(208.0)
            .resizable(false)
            .frame(
                Frame::new()
                    .fill(PALE)
                    .inner_margin(Margin::symmetric(18, 24)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    logo(ui, 40.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("NPGA").size(27.0).strong().color(INK));
                        ui.label(RichText::new("让连接快一步").size(11.0).color(MUTED));
                    });
                });
                ui.add_space(38.0);
                for page in [Page::Connect, Page::Nodes, Page::Subscriptions, Page::Logs] {
                    self.nav(ui, page);
                    ui.add_space(8.0);
                }
                ui.add_space(28.0);
                ui.label(RichText::new("  应用").size(11.0).color(MUTED));
                ui.add_space(12.0);
                self.nav(ui, Page::Settings);
                ui.add_space(8.0);
                self.nav(ui, Page::About);
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.label(
                        RichText::new("Netch.Plus Game Accelerator")
                            .size(10.0)
                            .color(MUTED),
                    );
                    ui.label(
                        RichText::new("v3.0.0  ·  Rust Edition")
                            .size(11.0)
                            .color(MUTED),
                    );
                    ui.add_space(20.0);
                    Frame::new()
                        .fill(Color32::WHITE)
                        .corner_radius(10)
                        .inner_margin(14)
                        .show(ui, |ui| {
                            ui.set_width(142.0);
                            ui.label(RichText::new("本地工作空间").color(INK).strong());
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(format!(
                                    "{} 个节点 · {} 个订阅",
                                    self.settings.nodes.len(),
                                    self.settings.subscriptions.len()
                                ))
                                .size(11.0)
                                .color(MUTED),
                            );
                        });
                });
            });
    }
    fn nav(&mut self, ui: &mut egui::Ui, page: Page) {
        let selected = self.page == page;
        let color = if selected {
            BLUE
        } else {
            Color32::from_rgb(102, 115, 137)
        };
        let text = RichText::new(format!("      {}", page.title()))
            .size(15.0)
            .color(if selected {
                BLUE
            } else {
                Color32::from_rgb(102, 115, 137)
            });
        let response = ui.add_sized(
            [172.0, 44.0],
            egui::Button::new(text)
                .fill(if selected {
                    Color32::from_rgb(230, 238, 255)
                } else {
                    Color32::TRANSPARENT
                })
                .corner_radius(8)
                .frame(false),
        );
        nav_icon(
            ui,
            page,
            response.rect.left_center() + Vec2::new(24.0, 0.0),
            color,
        );
        if response.clicked() {
            self.page = page;
        }
    }
    fn header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .exact_height(72.0)
            .frame(
                Frame::new()
                    .fill(Color32::WHITE)
                    .inner_margin(Margin::symmetric(28, 16)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.page.title())
                            .size(21.0)
                            .strong()
                            .color(INK),
                    );
                    let response = ui.interact(
                        ui.available_rect_before_wrap(),
                        egui::Id::new("drag-window"),
                        egui::Sense::drag(),
                    );
                    if response.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(RichText::new("×").size(23.0).color(MUTED))
                                    .frame(false),
                            )
                            .clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui
                            .add(
                                egui::Button::new(RichText::new("□").size(16.0).color(MUTED))
                                    .frame(false),
                            )
                            .clicked()
                        {
                            let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                        }
                        if ui
                            .add(
                                egui::Button::new(RichText::new("−").size(22.0).color(MUTED))
                                    .frame(false),
                            )
                            .clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        ui.add_space(16.0);
                        pill(ui, "Windows · 原生客户端", PALE, MUTED);
                    });
                });
            });
    }
    fn footer(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("footer")
            .exact_height(38.0)
            .frame(
                Frame::new()
                    .fill(Color32::WHITE)
                    .stroke(Stroke::new(1.0_f32, LINE))
                    .inner_margin(Margin::symmetric(26, 9)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let active = self.connection == Connection::Active;
                    ui.label(RichText::new("●").size(10.0).color(if active {
                        GREEN
                    } else {
                        MUTED
                    }));
                    ui.label(
                        RichText::new(if active {
                            "加速服务运行中"
                        } else {
                            "就绪，等待连接"
                        })
                        .size(11.0)
                        .color(MUTED),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "SOCKS :{}  /  HTTP :{}",
                                self.settings.socks_port, self.settings.http_port
                            ))
                            .size(11.0)
                            .color(MUTED),
                        );
                    });
                });
            });
    }
    fn connection_page(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("每一次连接，都更畅快")
                        .size(24.0)
                        .strong()
                        .color(INK),
                );
                ui.add_space(7.0);
                ui.label(
                    RichText::new("选择你的节点，开启游戏与应用加速")
                        .size(13.0)
                        .color(MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                pill(
                    ui,
                    self.settings.route_kind.label(),
                    Color32::from_rgb(236, 242, 255),
                    BLUE,
                );
            });
        });
        ui.add_space(24.0);
        let total = ui.available_width();
        let left = (total - 22.0) * 0.63;
        let right = total - left - 22.0;
        ui.horizontal_top(|ui| {
            card(ui, left, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("加速连接").size(17.0).strong().color(INK));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        pill(
                            ui,
                            match self.connection {
                                Connection::Active => "已连接",
                                Connection::Starting => "连接中",
                                Connection::Stopping => "正在停止",
                                Connection::Idle => "未连接",
                            },
                            if self.connection == Connection::Active {
                                Color32::from_rgb(229, 248, 239)
                            } else {
                                PALE
                            },
                            if self.connection == Connection::Active {
                                GREEN
                            } else {
                                MUTED
                            },
                        );
                    });
                });
                ui.add_space(14.0);
                network_art(ui, self.connection == Connection::Active);
                ui.add_space(14.0);
                ui.label(RichText::new("选择加速节点").size(12.0).color(MUTED));
                ui.add_space(8.0);
                let mut next = self.settings.clone();
                let editable = self.editable();
                ui.add_enabled_ui(editable, |ui| {
                    egui::ComboBox::from_id_salt("node-select")
                        .width(ui.available_width())
                        .height(260.0)
                        .selected_text(
                            self.settings
                                .nodes
                                .get(self.settings.selected_node)
                                .map(Node::name)
                                .unwrap_or("还没有节点，点击导入".into()),
                        )
                        .show_ui(ui, |ui| {
                            for (index, node) in self.settings.nodes.iter().enumerate() {
                                ui.selectable_value(
                                    &mut next.selected_node,
                                    index,
                                    format!("{}  ·  {}", node.name(), node.protocol),
                                );
                            }
                        });
                    ui.add_space(18.0);
                    ui.label(RichText::new("加速方式").size(12.0).color(MUTED));
                    ui.add_space(8.0);
                    egui::ComboBox::from_id_salt("route-select")
                        .width(ui.available_width())
                        .selected_text(next.route_kind.label())
                        .show_ui(ui, |ui| {
                            for kind in [
                                RouteKind::Local,
                                RouteKind::System,
                                RouteKind::Process,
                                RouteKind::Tun,
                                RouteKind::Share,
                            ] {
                                let label = kind.label();
                                ui.selectable_value(&mut next.route_kind, kind, label);
                            }
                        });
                    if matches!(
                        next.route_kind,
                        RouteKind::Process | RouteKind::Tun | RouteKind::Share
                    ) {
                        ui.add_space(12.0);
                        let title = self
                            .modes
                            .iter()
                            .find(|m| {
                                m.file.to_string_lossy() == next.mode_file
                                    && m.kind == next.route_kind
                            })
                            .map(|m| m.title.as_str())
                            .unwrap_or("选择路由规则 / 游戏");
                        egui::ComboBox::from_id_salt("mode-select")
                            .width(ui.available_width())
                            .selected_text(title)
                            .show_ui(ui, |ui| {
                                for mode in self.modes.iter().filter(|m| m.kind == next.route_kind)
                                {
                                    ui.selectable_value(
                                        &mut next.mode_file,
                                        mode.file.to_string_lossy().into_owned(),
                                        &mode.title,
                                    );
                                }
                            });
                    }
                });
                if next.selected_node != self.settings.selected_node
                    || next.route_kind != self.settings.route_kind
                    || next.mode_file != self.settings.mode_file
                {
                    self.commit(next);
                }
                ui.add_space(24.0);
                let (label, enabled) = match self.connection {
                    Connection::Idle => (
                        if self.settings.nodes.is_empty() {
                            "＋  添加第一个节点"
                        } else {
                            "开启加速"
                        },
                        editable,
                    ),
                    Connection::Starting => ("取消连接", true),
                    Connection::Active => ("停止加速", true),
                    Connection::Stopping => ("正在停止…", false),
                };
                if primary(ui, label, [ui.available_width(), 48.0], enabled) {
                    self.start_stop();
                }
                ui.add_space(10.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(match self.settings.route_kind {
                            RouteKind::Local => "连接后可通过本地 SOCKS / HTTP 端口使用代理",
                            RouteKind::System => "自动设置系统代理，停止后恢复原设置",
                            RouteKind::Process => "按游戏或应用进程分流，需要管理员权限",
                            RouteKind::Tun => "通过虚拟网卡转发流量，需要管理员权限",
                            RouteKind::Share => "通过 Npcap 将其他设备流量转入代理",
                        })
                        .size(11.0)
                        .color(MUTED),
                    );
                });
            });
            ui.add_space(14.0);
            ui.vertical(|ui| {
                card(ui, right, |ui| {
                    ui.label(RichText::new("连接概览").size(16.0).strong().color(INK));
                    ui.add_space(12.0);
                    let latency = self
                        .latencies
                        .get(self.settings.selected_node)
                        .copied()
                        .flatten();
                    ui.label(RichText::new("节点延迟").size(12.0).color(MUTED));
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(
                                latency
                                    .and_then(|(n, _)| n)
                                    .map(|n| n.to_string())
                                    .unwrap_or("—".into()),
                            )
                            .size(32.0)
                            .strong()
                            .color(INK),
                        );
                        ui.label(RichText::new("ms").size(13.0).color(MUTED));
                    });
                    ui.label(
                        RichText::new(
                            latency
                                .map(|(_, method)| format!("{method} 连通性测试"))
                                .unwrap_or("尚未测试".into()),
                        )
                        .size(11.0)
                        .color(MUTED),
                    );
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(8.0);
                    let monitor = self.connection == Connection::Active;
                    metric(
                        ui,
                        "↓  下载速率",
                        if monitor {
                            format!("{}/s", bytes(self.down_rate))
                        } else {
                            "—".into()
                        },
                    );
                    ui.add_space(8.0);
                    metric(
                        ui,
                        "↑  上传速率",
                        if monitor {
                            format!("{}/s", bytes(self.up_rate))
                        } else {
                            "—".into()
                        },
                    );
                    ui.add_space(8.0);
                    metric(
                        ui,
                        "连接时长",
                        self.since
                            .map(|s| duration(s.elapsed()))
                            .unwrap_or("—".into()),
                    );
                    ui.add_space(8.0);
                    metric(
                        ui,
                        "代理核心",
                        if self.connection == Connection::Active {
                            self.active_core.clone()
                        } else {
                            self.settings
                                .nodes
                                .get(self.settings.selected_node)
                                .map(|n| n.core().into())
                                .unwrap_or("未选择".into())
                        },
                    );
                    ui.add_space(8.0);
                    metric(
                        ui,
                        "自动选路",
                        if self.settings.auto_select {
                            format!("开启 · {} 个节点参与探测", self.settings.nodes.len())
                        } else {
                            "关闭".into()
                        },
                    );
                });
                ui.add_space(16.0);
                card(ui, right, |ui| {
                    ui.label(RichText::new("快捷操作").size(15.0).strong().color(INK));
                    ui.add_space(13.0);
                    if secondary(ui, "＋  导入节点", self.editable()) {
                        self.import_open = true;
                    }
                    ui.add_space(8.0);
                    if secondary(
                        ui,
                        if self.probes_left > 0 {
                            "◌  正在测试…"
                        } else {
                            "↻  测试节点延迟"
                        },
                        self.probes_left == 0 && !self.busy,
                    ) {
                        self.probe();
                    }
                    ui.add_space(8.0);
                    if secondary(ui, "≡  查看运行日志", true) {
                        self.page = Page::Logs;
                    }
                    ui.add_space(16.0);
                    ui.label(RichText::new("连接诊断").size(15.0).strong().color(INK));
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("以下工具经当前加速链路探测，需要先连接")
                            .size(11.0)
                            .color(MUTED),
                    );
                    ui.add_space(10.0);
                    let ready =
                        self.connection == Connection::Active && self.tool_running.is_none();
                    for (kind, label) in [
                        (ToolKind::Nat, "◎  测试 NAT 类型"),
                        (ToolKind::Geo, "⌖  出口 IP 归属地"),
                        (ToolKind::Speed, "⇅  网络测速"),
                    ] {
                        let text = if self.tool_running == Some(kind) {
                            format!("◌  {} 进行中…", kind.label())
                        } else {
                            label.to_owned()
                        };
                        if secondary(ui, &text, ready) {
                            self.run_tool(kind);
                        }
                        ui.add_space(8.0);
                    }
                });
            });
        });
    }
    fn nodes_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("搜索节点名称、协议或分组")
                    .desired_width(300.0),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary(ui, "＋  导入节点", [120.0, 36.0], self.editable()) {
                    self.import_open = true;
                }
                if ui
                    .add_enabled(self.editable(), egui::Button::new("手动添加"))
                    .clicked()
                {
                    let n = Node::default();
                    self.editor = Some((None, n, "{}".into()));
                }
                if ui
                    .add_enabled(
                        self.probes_left == 0 && !self.busy,
                        egui::Button::new("测试全部"),
                    )
                    .clicked()
                {
                    self.probe();
                }
            });
        });
        ui.add_space(22.0);
        if self.settings.nodes.is_empty() {
            empty(ui, "还没有加速节点", "导入分享链接或订阅，开始第一次连接");
            return;
        }
        let query = self.search.to_lowercase();
        let mut selected = None;
        let mut edit = None;
        let mut delete = None;
        let editable = self.editable();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (index, node) in self.settings.nodes.iter().enumerate().filter(|(_, n)| {
                format!("{} {} {} {}", n.name(), n.protocol, n.group, n.hostname)
                    .to_lowercase()
                    .contains(&query)
            }) {
                Frame::new()
                    .fill(if index == self.settings.selected_node {
                        Color32::from_rgb(242, 247, 255)
                    } else {
                        Color32::WHITE
                    })
                    .stroke(Stroke::new(
                        1.0_f32,
                        if index == self.settings.selected_node {
                            Color32::from_rgb(167, 195, 251)
                        } else {
                            LINE
                        },
                    ))
                    .corner_radius(10)
                    .inner_margin(16)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    editable,
                                    egui::RadioButton::new(
                                        index == self.settings.selected_node,
                                        "",
                                    ),
                                )
                                .clicked()
                            {
                                selected = Some(index);
                            }
                            ui.vertical(|ui| {
                                ui.label(RichText::new(node.name()).size(15.0).strong().color(INK));
                                ui.add_space(5.0);
                                ui.label(
                                    RichText::new(format!(
                                        "{}  ·  {}:{}",
                                        node.group, node.hostname, node.port
                                    ))
                                    .size(11.0)
                                    .color(MUTED),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .add_enabled(editable, egui::Button::new("删除").frame(false))
                                    .clicked()
                                {
                                    delete = Some(index);
                                }
                                if ui
                                    .add_enabled(editable, egui::Button::new("编辑").frame(false))
                                    .clicked()
                                {
                                    edit = Some(index);
                                }
                                if ui.button("分享").clicked() {
                                    match links::export(node) {
                                        Ok(link) => ui.ctx().copy_text(link),
                                        Err(e) => {
                                            let _ = e;
                                        }
                                    }
                                }
                                let delay = self.latencies.get(index).copied().flatten();
                                ui.label(
                                    RichText::new(match delay {
                                        Some((Some(ms), method)) => format!("{ms} ms · {method}"),
                                        Some((None, _)) => "不可达".into(),
                                        None => "未测试".into(),
                                    })
                                    .size(12.0)
                                    .color(match delay {
                                        Some((Some(ms), _)) if ms < 80 => GREEN,
                                        Some((Some(ms), _)) if ms < 200 => {
                                            Color32::from_rgb(218, 155, 47)
                                        }
                                        _ => MUTED,
                                    }),
                                );
                                pill(ui, &node.protocol, PALE, BLUE);
                            });
                        });
                    });
                ui.add_space(10.0);
            }
        });
        if let Some(index) = selected {
            let mut next = self.settings.clone();
            next.selected_node = index;
            self.commit(next);
        }
        if let Some(index) = edit {
            let node = self.settings.nodes[index].clone();
            let fields = serde_json::to_string_pretty(&node.fields).unwrap_or_default();
            self.editor = Some((Some(index), node, fields));
        }
        if let Some(index) = delete {
            let mut next = self.settings.clone();
            next.nodes.remove(index);
            next.selected_node = next
                .selected_node
                .saturating_sub(usize::from(index < next.selected_node))
                .min(next.nodes.len().saturating_sub(1));
            self.commit(next);
            self.latencies = vec![None; self.settings.nodes.len()];
        }
    }
    fn subscriptions_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("自动同步你的节点资源").color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary(ui, "＋  添加订阅", [130.0, 36.0], self.editable()) {
                    self.subscription_editor = Some((None, Subscription::default()));
                }
            });
        });
        ui.add_space(22.0);
        if self.settings.subscriptions.is_empty() {
            empty(ui, "尚未添加订阅", "添加订阅地址后，点击更新同步节点");
        }
        let mut action = None;
        for (index, subscription) in self.settings.subscriptions.iter().enumerate() {
            card(ui, ui.available_width(), |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(&subscription.remark)
                                .size(16.0)
                                .strong()
                                .color(INK),
                        );
                        ui.add_space(7.0);
                        ui.label(
                            RichText::new(
                                url::Url::parse(&subscription.link)
                                    .ok()
                                    .and_then(|u| u.host_str().map(str::to_owned))
                                    .unwrap_or("订阅地址无效".into()),
                            )
                            .size(12.0)
                            .color(MUTED),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_enabled(self.editable(), egui::Button::new("删除"))
                            .clicked()
                        {
                            action = Some((index, 2));
                        }
                        if ui
                            .add_enabled(self.editable(), egui::Button::new("编辑"))
                            .clicked()
                        {
                            action = Some((index, 1));
                        }
                        if ui
                            .add_enabled(self.editable(), egui::Button::new("更新").fill(BLUE))
                            .clicked()
                        {
                            action = Some((index, 0));
                        }
                    });
                });
            });
            ui.add_space(12.0);
        }
        if let Some((index, action)) = action {
            match action {
                0 => {
                    self.busy = true;
                    self.send(CommandMessage::Refresh {
                        settings: Box::new(self.settings.clone()),
                        id: self.settings.subscriptions[index].id.clone(),
                    });
                    self.info("正在下载并校验订阅…");
                }
                1 => {
                    self.subscription_editor =
                        Some((Some(index), self.settings.subscriptions[index].clone()))
                }
                _ => {
                    let mut next = self.settings.clone();
                    let subscription = next.subscriptions.remove(index);
                    next.nodes
                        .retain(|n| n.text("SubscriptionId") != subscription.id);
                    next.selected_node = next.selected_node.min(next.nodes.len().saturating_sub(1));
                    self.commit(next);
                }
            }
        }
        ui.add_space(18.0);
        ui.label(
            RichText::new("仅在完整解析成功后替换此订阅的节点；手动节点独立保存。")
                .size(12.0)
                .color(MUTED),
        );
    }
    fn logs_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("最近 {} 行", self.logs.len())).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("清空").clicked() {
                    self.logs.clear();
                }
                if ui.button("复制日志").clicked() {
                    ui.ctx()
                        .copy_text(self.logs.iter().cloned().collect::<Vec<_>>().join("\n"));
                }
            });
        });
        ui.add_space(18.0);
        Frame::new()
            .fill(PALE)
            .corner_radius(10)
            .inner_margin(18)
            .show(ui, |ui| {
                ui.set_min_height(ui.available_height() - 40.0);
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for line in &self.logs {
                            ui.label(RichText::new(line).size(12.0).monospace().color(INK));
                        }
                        if self.logs.is_empty() {
                            ui.label(
                                RichText::new("暂无日志。启动连接或检查配置后，日志会显示在这里。")
                                    .color(MUTED),
                            );
                        }
                    });
            });
    }
    fn settings_page(&mut self, ui: &mut egui::Ui) {
        let enabled = self.editable();
        card(ui, ui.available_width(), |ui| {
            ui.label(RichText::new("代理与应用").size(17.0).strong().color(INK));
            ui.add_space(20.0);
            ui.add_enabled_ui(enabled, |ui| {
                egui::Grid::new("settings-form")
                    .num_columns(2)
                    .spacing([34.0, 16.0])
                    .show(ui, |ui| {
                        ui.label("本地监听地址");
                        ui.text_edit_singleline(&mut self.draft.local_address);
                        ui.end_row();
                        ui.label("SOCKS5 端口");
                        ui.add(egui::DragValue::new(&mut self.draft.socks_port).range(1..=65535));
                        ui.end_row();
                        ui.label("HTTP 端口");
                        ui.add(egui::DragValue::new(&mut self.draft.http_port).range(1..=65535));
                        ui.end_row();
                        ui.label("订阅超时 / 毫秒");
                        ui.add(
                            egui::DragValue::new(&mut self.draft.request_timeout)
                                .range(1000..=60000),
                        );
                        ui.end_row();
                        ui.label("Npcap 网卡设备名");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.draft.share_interface)
                                .hint_text("\\Device\\NPF_{网卡 GUID}"),
                        );
                        ui.end_row();
                        ui.label("关闭主窗口");
                        ui.checkbox(&mut self.draft.minimize_to_tray, "最小化到系统托盘");
                        ui.end_row();
                        ui.label("开机启动");
                        ui.checkbox(&mut self.draft.startup, "登录 Windows 后启动");
                        ui.end_row();
                        ui.label("进程模式后端");
                        ui.label("sing-box TUN（进程名路由，无需 netfilter2 驱动）");
                        ui.end_row();
                    });
            });
            ui.add_space(24.0);
            if primary(ui, "保存设置", [130.0, 38.0], enabled) {
                let next = self.draft.clone();
                let previous = self.settings.clone();
                let startup_changed = next.startup != previous.startup;
                if self.commit(next) {
                    if startup_changed
                        && let Err(e) = platform::startup(self.settings.startup, &self.paths.data)
                    {
                        let restored = self.commit(previous);
                        self.error(format!(
                            "开机启动设置失败：{e:#}；{}",
                            if restored {
                                "已恢复原配置"
                            } else {
                                "配置回滚失败，请检查数据目录写入权限"
                            }
                        ));
                    } else {
                        self.info("设置已保存");
                    }
                }
            }
        });
        ui.add_space(18.0);
        card(ui, ui.available_width(), |ui| {
            ui.label(RichText::new("自动选路").size(17.0).strong().color(INK));
            ui.add_space(6.0);
            ui.label(
                RichText::new("连接前并行探测所有节点，选择延迟最低者；连接中持续监测并按需切换")
                    .size(11.0)
                    .color(MUTED),
            );
            ui.add_space(14.0);
            ui.checkbox(
                &mut self.draft.auto_select,
                "启用自动选路（仅在有多个节点时生效）",
            );
            ui.add_space(10.0);
            ui.add_enabled_ui(self.draft.auto_select, |ui| {
                ui.checkbox(
                    &mut self.draft.auto_select_degradable,
                    "延迟劣化时也切换（游戏可能因 IP 变化重连，默认仅故障时切换）",
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label("劣化阈值");
                    ui.add(
                        egui::DragValue::new(&mut self.draft.degrade_ms)
                            .range(50..=5000)
                            .suffix(" ms"),
                    );
                    ui.add_space(18.0);
                    ui.label("切换冷却");
                    ui.add(
                        egui::DragValue::new(&mut self.draft.switch_cooldown_ms)
                            .range(5000..=300000)
                            .speed(1000.0)
                            .suffix(" ms"),
                    );
                });
            });
        });
        ui.add_space(18.0);
        card(ui, ui.available_width(), |ui| {
            ui.label(
                RichText::new("数据迁移与诊断")
                    .size(17.0)
                    .strong()
                    .color(INK),
            );
            ui.add_space(12.0);
            ui.label(
                RichText::new(format!("数据：{}", self.paths.data.display()))
                    .size(12.0)
                    .color(MUTED),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("核心：{}", self.paths.root.display()))
                    .size(12.0)
                    .color(MUTED),
            );
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        self.connection == Connection::Idle && !self.busy,
                        egui::Button::new("导入旧版 settings.json"),
                    )
                    .clicked()
                {
                    self.import_open = true;
                }
                if ui
                    .add_enabled(
                        enabled && !self.settings.nodes.is_empty(),
                        egui::Button::new("校验当前配置"),
                    )
                    .clicked()
                {
                    self.busy = true;
                    self.worker.cancel.store(false, Ordering::Relaxed);
                    self.send(CommandMessage::Check {
                        settings: Box::new(self.settings.clone()),
                        mode: self.selected_mode(),
                    });
                }
                if ui
                    .add_enabled(
                        self.connection == Connection::Idle,
                        egui::Button::new("以管理员重启"),
                    )
                    .clicked()
                {
                    self.exit = true;
                    match platform::elevate(&self.paths.data, &self.paths.root) {
                        Ok(()) => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                        Err(e) => {
                            self.exit = false;
                            self.error(e.to_string());
                        }
                    }
                }
            });
        });
    }
    fn about_page(&mut self, ui: &mut egui::Ui) {
        card(ui, ui.available_width(), |ui| {
            ui.horizontal(|ui| {
                logo(ui, 56.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("NPGA Rust").size(26.0).strong().color(INK));
                    ui.label(RichText::new("Netch.Plus Game Accelerator · v3.0.0").color(MUTED));
                });
            });
            ui.add_space(22.0);
            ui.label("基于 Netch 的 Windows 网络加速客户端。主界面、配置、订阅与连接控制使用 Rust 原生编译。");
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.hyperlink_to("项目主页", "https://github.com/OffroadOps/Hatch");
                ui.label("· GPL-3.0 · 致谢 Netch / Xray / sing-box");
            });
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("当前版本 v{}", npga::VERSION))
                        .size(12.0)
                        .color(MUTED),
                );
                ui.add_space(12.0);
                let ready = self.tool_running.is_none() && !self.busy;
                let label = if self.tool_running == Some(ToolKind::Update) {
                    "◌  正在检查…"
                } else {
                    "↻  检查更新"
                };
                let clicked = ui
                    .add_enabled(
                        ready,
                        egui::Button::new(RichText::new(label).size(12.0).color(INK))
                            .fill(Color32::WHITE)
                            .stroke(Stroke::new(1.0_f32, LINE))
                            .corner_radius(7)
                            .min_size(Vec2::new(126.0, 34.0)),
                    )
                    .clicked();
                if clicked {
                    self.run_tool(ToolKind::Update);
                }
            });
            if let Some((message, is_error)) = &self.update_result {
                ui.add_space(10.0);
                for line in message.lines() {
                    if line.starts_with("http://") || line.starts_with("https://") {
                        ui.hyperlink_to(line, line);
                    } else {
                        ui.label(RichText::new(line).size(12.0).color(if *is_error {
                            Color32::from_rgb(220, 94, 82)
                        } else {
                            GREEN
                        }));
                    }
                }
            }
        });
        ui.add_space(18.0);
        card(ui, ui.available_width(), |ui| {
            ui.label(RichText::new("运行组件").size(17.0).strong().color(INK));
            ui.add_space(16.0);
            egui::Grid::new("components")
                .num_columns(2)
                .spacing([160.0, 12.0])
                .show(ui, |ui| {
                    for (name, exists) in self.paths.diagnostics() {
                        ui.label(name);
                        ui.label(RichText::new(if exists { "已找到" } else { "缺失" }).color(
                            if exists {
                                GREEN
                            } else {
                                Color32::from_rgb(220, 94, 82)
                            },
                        ));
                        ui.end_row();
                    }
                });
            ui.add_space(16.0);
            ui.label(RichText::new(format!("已加载 {} 个路由模式", self.modes.len())).color(MUTED));
            if !self.mode_errors.is_empty() {
                egui::CollapsingHeader::new(format!("{} 个模式读取失败", self.mode_errors.len()))
                    .show(ui, |ui| {
                        for error in &self.mode_errors {
                            ui.label(error);
                        }
                    });
            }
        });
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
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
        if let Some((index, mut node, mut advanced)) = self.editor.take() {
            if node.fields.is_empty() {
                node.fields = serde_json::from_str(&advanced).unwrap_or_default();
            }
            let mut open = true;
            let mut save = false;
            egui::Window::new(if index.is_some() {
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
                            .selected_text(&node.protocol)
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
                                    ui.selectable_value(&mut node.protocol, p.into(), p);
                                }
                            });
                        ui.end_row();
                        ui.label("名称");
                        ui.text_edit_singleline(&mut node.remark);
                        ui.end_row();
                        ui.label("分组");
                        ui.text_edit_singleline(&mut node.group);
                        ui.end_row();
                        ui.label("服务器地址");
                        ui.text_edit_singleline(&mut node.hostname);
                        ui.end_row();
                        ui.label("端口");
                        ui.add(egui::DragValue::new(&mut node.port).range(1..=65535));
                        ui.end_row();
                    });
                ui.add_space(18.0);
                let before = node.fields.clone();
                node_fields(ui, &mut node);
                if before != node.fields {
                    advanced = serde_json::to_string_pretty(&node.fields).unwrap_or_default();
                }
                ui.add_space(12.0);
                egui::CollapsingHeader::new("高级协议参数").show(ui, |ui| {
                    ui.label(
                        RichText::new("可编辑原版协议字段，此处包含明文凭据。")
                            .size(11.0)
                            .color(MUTED),
                    );
                    ui.add(
                        egui::TextEdit::multiline(&mut advanced)
                            .desired_width(f32::INFINITY)
                            .desired_rows(8)
                            .code_editor(),
                    );
                });
                ui.add_space(12.0);
                save = primary(ui, "保存节点", [140.0, 38.0], true);
            });
            if save {
                let parsed = serde_json::from_str(&advanced)
                    .map_err(anyhow::Error::from)
                    .and_then(|fields| {
                        node.fields = fields;
                        node.validate()
                    });
                match parsed {
                    Ok(()) => {
                        let mut next = self.settings.clone();
                        if let Some(index) = index {
                            next.nodes[index] = node.clone();
                        } else {
                            next.nodes.push(node.clone());
                        }
                        if !self.commit(next) {
                            self.editor = Some((index, node, advanced));
                        }
                    }
                    Err(e) => {
                        self.error(e.to_string());
                        self.editor = Some((index, node, advanced));
                    }
                }
            } else if open {
                self.editor = Some((index, node, advanced));
            }
        }
        if let Some((index, mut subscription)) = self.subscription_editor.take() {
            let mut open = true;
            let mut save = false;
            egui::Window::new("订阅设置")
                .open(&mut open)
                .collapsible(false)
                .default_width(520.0)
                .show(ctx, |ui| {
                    ui.label("订阅名称");
                    ui.text_edit_singleline(&mut subscription.remark);
                    ui.add_space(12.0);
                    ui.label("订阅地址");
                    ui.add(
                        egui::TextEdit::singleline(&mut subscription.link)
                            .password(true)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(12.0);
                    ui.label("User-Agent");
                    ui.text_edit_singleline(&mut subscription.user_agent);
                    ui.add_space(16.0);
                    save = primary(ui, "保存订阅", [130.0, 36.0], true);
                });
            if save {
                if subscription.remark.trim().is_empty()
                    || !url::Url::parse(&subscription.link)
                        .is_ok_and(|u| matches!(u.scheme(), "http" | "https"))
                {
                    self.error("订阅名称或 HTTP/HTTPS 地址无效".into());
                    self.subscription_editor = Some((index, subscription));
                } else {
                    let mut next = self.settings.clone();
                    if let Some(index) = index {
                        next.subscriptions[index] = subscription.clone();
                    } else {
                        next.subscriptions.push(subscription.clone());
                    }
                    if !self.commit(next) {
                        self.subscription_editor = Some((index, subscription));
                    }
                }
            } else if open {
                self.subscription_editor = Some((index, subscription));
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.frames += 1;
        self.events();
        #[cfg(windows)]
        {
            if let Some((show, stop, quit)) = &self.tray_ids.clone() {
                while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                    if event.id == show {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    } else if event.id == stop {
                        if self.connection != Connection::Idle {
                            self.start_stop();
                        }
                    } else if event.id == quit {
                        self.exit = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
            while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
                if matches!(event, tray_icon::TrayIconEvent::DoubleClick { .. }) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.exit && self.screenshot.is_none()
        {
            #[cfg(windows)]
            if self.settings.minimize_to_tray && self.tray.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }
        self.sidebar(ctx);
        self.header(ctx);
        self.footer(ctx);
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(Color32::WHITE)
                    .inner_margin(Margin::symmetric(28, 20)),
            )
            .show(ctx, |ui| {
                if let Some((message, error)) = &self.notice.clone() {
                    Frame::new()
                        .fill(if *error {
                            Color32::from_rgb(255, 242, 240)
                        } else {
                            Color32::from_rgb(236, 248, 243)
                        })
                        .corner_radius(8)
                        .inner_margin(12)
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new(message).size(12.0).color(if *error {
                                    Color32::from_rgb(177, 76, 63)
                                } else {
                                    GREEN
                                }));
                                if ui.small_button("×").clicked() {
                                    self.notice = None;
                                }
                            });
                        });
                    ui.add_space(14.0);
                }
                egui::ScrollArea::vertical()
                    .id_salt("page-scroll")
                    .show(ui, |ui| match self.page {
                        Page::Connect => self.connection_page(ui),
                        Page::Nodes => self.nodes_page(ui),
                        Page::Subscriptions => self.subscriptions_page(ui),
                        Page::Logs => self.logs_page(ui),
                        Page::Settings => self.settings_page(ui),
                        Page::About => self.about_page(ui),
                    });
            });
        self.dialogs(ctx);
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if self.connection == Connection::Idle && !self.busy {
            for file in dropped {
                if let Some(path) = file.path {
                    if path.extension().is_some_and(|e| e == "json") {
                        self.import_path = path.to_string_lossy().into_owned();
                        self.import_open = true;
                    } else if let Ok(text) = std::fs::read_to_string(path) {
                        self.import_text = text;
                        self.import_open = true;
                    }
                }
            }
        }
        if let Some(path) = &self.screenshot {
            if self.frames >= 5 && !self.screenshot_requested {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                self.screenshot_requested = true;
            }
            for event in ctx.input(|i| i.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let result = image::save_buffer(
                        path,
                        &rgba,
                        image.size[0] as u32,
                        image.size[1] as u32,
                        image::ColorType::Rgba8,
                    );
                    if let Err(e) = result {
                        eprintln!("截图保存失败：{e}");
                    }
                    self.exit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

fn theme(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for path in ["C:/Windows/Fonts/msyh.ttc", "C:/Windows/Fonts/simhei.ttf"] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                "Chinese".into(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "Chinese".into());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("Chinese".into());
            break;
        }
    }
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(INK);
    style.visuals.panel_fill = Color32::WHITE;
    style.visuals.window_fill = Color32::WHITE;
    style.visuals.selection.bg_fill = BLUE;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    style.visuals.widgets.inactive.bg_fill = PALE;
    style.visuals.widgets.inactive.weak_bg_fill = PALE;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, LINE);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(7);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(232, 239, 255);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(7);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(7);
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 9.0);
    style.spacing.interact_size.y = 34.0;
    ctx.set_style(style);
}
fn primary(ui: &mut egui::Ui, label: &str, size: [f32; 2], enabled: bool) -> bool {
    ui.add_enabled(
        enabled,
        egui::Button::new(
            RichText::new(label)
                .size(14.0)
                .strong()
                .color(Color32::WHITE),
        )
        .fill(BLUE)
        .corner_radius(8)
        .min_size(Vec2::from(size)),
    )
    .clicked()
}
fn secondary(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).size(12.0).color(INK))
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1.0_f32, LINE))
            .corner_radius(7)
            .min_size(Vec2::new(ui.available_width(), 34.0)),
    )
    .clicked()
}
fn card(ui: &mut egui::Ui, width: f32, content: impl FnOnce(&mut egui::Ui)) {
    Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(12)
        .inner_margin(20)
        .show(ui, |ui| {
            ui.set_width((width - 40.0).max(40.0));
            ui.with_layout(Layout::top_down(Align::Min), content);
        });
}
fn nav_icon(ui: &egui::Ui, page: Page, c: egui::Pos2, color: Color32) {
    let p = ui.painter();
    let stroke = Stroke::new(1.6_f32, color);
    match page {
        Page::Connect => {
            for sign in [-1.0, 1.0] {
                let y = sign * 4.0;
                let x = sign * 7.0;
                p.line_segment([c + Vec2::new(-7.0, y), c + Vec2::new(7.0, y)], stroke);
                p.line_segment(
                    [c + Vec2::new(x, y), c + Vec2::new(x - sign * 4.0, y - 4.0)],
                    stroke,
                );
            }
        }
        Page::Nodes => {
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    p.rect_stroke(
                        egui::Rect::from_center_size(
                            c + Vec2::new(x * 4.5, y * 4.5),
                            Vec2::splat(6.0),
                        ),
                        1,
                        stroke,
                        egui::StrokeKind::Middle,
                    );
                }
            }
        }
        Page::Subscriptions => {
            p.circle_stroke(c, 7.0, stroke);
            p.line_segment([c + Vec2::new(7.0, -2.0), c + Vec2::new(4.0, -6.0)], stroke);
            p.line_segment(
                [c + Vec2::new(7.0, -2.0), c + Vec2::new(10.0, -6.0)],
                stroke,
            );
        }
        Page::Logs => {
            for y in [-5.0, 0.0, 5.0] {
                p.line_segment(
                    [
                        c + Vec2::new(-7.0, y),
                        c + Vec2::new(if y == 5.0 { 3.0 } else { 7.0 }, y),
                    ],
                    stroke,
                );
            }
        }
        Page::Settings => {
            p.circle_stroke(c, 5.0, stroke);
            p.circle_stroke(c, 1.5, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let v = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + v * 6.0, c + v * 8.5], stroke);
            }
        }
        Page::About => {
            p.circle_stroke(c, 8.0, stroke);
            p.line_segment([c + Vec2::new(0.0, -1.0), c + Vec2::new(0.0, 4.0)], stroke);
            p.circle_filled(c + Vec2::new(0.0, -4.0), 1.0, color);
        }
    }
}
fn pill(ui: &mut egui::Ui, label: &str, bg: Color32, color: Color32) {
    let font = egui::FontId::proportional(10.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), font.clone(), color);
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(galley.size().x + 18.0, 22.0),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(rect, 5, bg);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font,
        color,
    );
}
fn metric(ui: &mut egui::Ui, label: &str, value: String) {
    let previous = ui.spacing().interact_size.y;
    ui.spacing_mut().interact_size.y = 20.0;
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.0).color(MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).size(13.0).strong().color(INK));
        });
    });
    ui.spacing_mut().interact_size.y = previous;
}
fn empty(ui: &mut egui::Ui, title: &str, detail: &str) {
    ui.add_space(90.0);
    ui.vertical_centered(|ui| {
        logo(ui, 60.0);
        ui.add_space(20.0);
        ui.label(RichText::new(title).size(21.0).strong().color(INK));
        ui.add_space(8.0);
        ui.label(RichText::new(detail).size(13.0).color(MUTED));
    });
}
fn logo(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same((size / 4.0) as u8), BLUE);
    let c = rect.center();
    let points = vec![
        c + Vec2::new(-size * 0.22, size * 0.04),
        c + Vec2::new(size * 0.08, -size * 0.27),
        c + Vec2::new(size * 0.01, -size * 0.02),
        c + Vec2::new(size * 0.22, -size * 0.04),
        c + Vec2::new(-size * 0.08, size * 0.27),
        c + Vec2::new(-size * 0.01, size * 0.02),
    ];
    painter.add(egui::Shape::convex_polygon(
        points,
        Color32::WHITE,
        Stroke::NONE,
    ));
}
fn network_art(ui: &mut egui::Ui, active: bool) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 88.0), egui::Sense::hover());
    let p = ui.painter();
    let c = rect.center();
    p.circle_filled(c, 51.0, Color32::from_rgb(247, 250, 255));
    p.circle_stroke(
        c,
        41.0,
        Stroke::new(1.0_f32, Color32::from_rgb(212, 227, 255)),
    );
    p.circle_stroke(
        c,
        28.0,
        Stroke::new(1.0_f32, Color32::from_rgb(212, 227, 255)),
    );
    p.line_segment(
        [c + Vec2::new(-41.0, 0.0), c + Vec2::new(41.0, 0.0)],
        Stroke::new(1.0_f32, Color32::from_rgb(212, 227, 255)),
    );
    for x in [-1.0, 1.0] {
        let node = c + Vec2::new(x * 105.0, 0.0);
        p.line_segment(
            [c + Vec2::new(x * 50.0, 0.0), node],
            Stroke::new(1.5_f32, Color32::from_rgb(212, 227, 255)),
        );
        p.circle_filled(node, 18.0, Color32::from_rgb(237, 244, 255));
        p.circle_filled(node, 5.0, BLUE);
    }
    p.circle_filled(c, 18.0, BLUE);
    p.text(
        c,
        egui::Align2::CENTER_CENTER,
        if active { "✓" } else { "↗" },
        egui::FontId::proportional(24.0),
        Color32::WHITE,
    );
    p.circle_filled(
        c + Vec2::new(31.0, -27.0),
        5.0,
        if active {
            GREEN
        } else {
            Color32::from_rgb(137, 175, 250)
        },
    );
}
fn bytes(n: f64) -> String {
    if n >= 1048576.0 {
        format!("{:.1} MB", n / 1048576.0)
    } else if n >= 1024.0 {
        format!("{:.1} KB", n / 1024.0)
    } else {
        format!("{n:.0} B")
    }
}
fn duration(d: Duration) -> String {
    let n = d.as_secs();
    format!("{:02}:{:02}:{:02}", n / 3600, (n / 60) % 60, n % 60)
}
fn field_template(protocol: &str) -> String {
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
fn text_parameter(ui: &mut egui::Ui, node: &mut Node, label: &str, key: &str, secret: bool) {
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
fn choice_parameter(
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
fn node_fields(ui: &mut egui::Ui, node: &mut Node) {
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
#[cfg(windows)]
fn tray() -> anyhow::Result<(
    tray_icon::TrayIcon,
    (
        tray_icon::menu::MenuId,
        tray_icon::menu::MenuId,
        tray_icon::menu::MenuId,
    ),
)> {
    use tray_icon::{
        Icon, TrayIconBuilder,
        menu::{Menu, MenuItem},
    };
    let menu = Menu::new();
    let show = MenuItem::new("显示主界面", true, None);
    let stop = MenuItem::new("停止加速", true, None);
    let quit = MenuItem::new("退出 NPGA", true, None);
    menu.append_items(&[&show, &stop, &quit])?;
    let rgba = native_icon().rgba.clone();
    let tray = TrayIconBuilder::new()
        .with_tooltip("NPGA Rust · 游戏加速")
        .with_icon(Icon::from_rgba(rgba, 32, 32)?)
        .with_menu(Box::new(menu))
        .build()?;
    Ok((
        tray,
        (show.id().clone(), stop.id().clone(), quit.id().clone()),
    ))
}

pub fn native_icon() -> std::sync::Arc<egui::IconData> {
    let polygon = [
        (-0.22, 0.04),
        (0.08, -0.27),
        (0.01, -0.02),
        (0.22, -0.04),
        (-0.08, 0.27),
        (-0.01, 0.02),
    ];
    let mut rgba = vec![0; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let px = (x as f32 + 0.5) / 32.0 - 0.5;
            let py = (y as f32 + 0.5) / 32.0 - 0.5;
            let mut white = false;
            for i in 0..polygon.len() {
                let (ax, ay) = polygon[i];
                let (bx, by) = polygon[(i + 1) % polygon.len()];
                if (ay > py) != (by > py) && px < (bx - ax) * (py - ay) / (by - ay) + ax {
                    white = !white;
                }
            }
            let i = (y * 32 + x) * 4;
            let alpha = if (px.abs() - 0.25).max(0.0).hypot((py.abs() - 0.25).max(0.0)) > 0.25 {
                0
            } else {
                255
            };
            let color = if white {
                [255, 255, 255, alpha]
            } else {
                [45, 109, 246, alpha]
            };
            rgba[i..i + 4].copy_from_slice(&color);
        }
    }
    std::sync::Arc::new(egui::IconData {
        rgba,
        width: 32,
        height: 32,
    })
}
