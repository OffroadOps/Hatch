mod dialogs;
mod node_form;
mod theme;
mod tray;

use eframe::egui::{self, Align, Color32, Frame, Layout, Margin, RichText, Stroke, Vec2};
use node_form::field_template;
use npga::{
    config::{Store, StoreWriter},
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
use theme::{
    BLUE, GREEN, INK, LINE, MUTED, PALE, bytes, card, duration, empty, logo, metric, nav_icon,
    network_art, pill, primary, secondary, theme,
};
pub use tray::native_icon;
#[cfg(windows)]
use tray::tray;
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

/// The node editor dialog state.
struct NodeEditor {
    /// Index of the edited node, `None` while adding a new one.
    index: Option<usize>,
    /// Identity of the node when the editor was opened. The node list can
    /// change while the dialog is open (a delete, a subscription refresh) and
    /// the index alone would then address a different node.
    identity: Option<String>,
    node: Node,
    /// Raw protocol parameters shown in the advanced section.
    advanced: String,
}

/// The subscription editor dialog state.
struct SubscriptionEditor {
    index: Option<usize>,
    /// Subscription id when the editor was opened.
    id: Option<String>,
    subscription: Subscription,
}

/// Editor content that waits for the configuration write to be confirmed, so a
/// failed write can restore the user's input instead of losing it.
enum PendingSave {
    Node(NodeEditor),
    Subscription(SubscriptionEditor),
}

pub struct App {
    paths: RuntimePaths,
    /// Writes the configuration on a background thread; the UI never waits for
    /// DPAPI encryption or disk I/O.
    writer: StoreWriter,
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
    editor: Option<NodeEditor>,
    subscription_editor: Option<SubscriptionEditor>,
    /// Sequence number of the most recent queued configuration save.
    save_sequence: u64,
    /// Editor content waiting for the pending write to be confirmed.
    pending_save: Option<PendingSave>,
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
        let writer = StoreWriter::new(store.clone());
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
            writer,
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
                Some(NodeEditor {
                    index: None,
                    identity: None,
                    node: Node::default(),
                    advanced: field_template("VLESS"),
                })
            } else {
                None
            },
            subscription_editor: None,
            save_sequence: 0,
            pending_save: None,
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
    /// Applies a settings change and queues it for writing.
    ///
    /// The in-memory state is updated immediately so the UI never blocks on
    /// DPAPI encryption or disk I/O; the write itself happens on the
    /// background writer thread and is confirmed through `events()`.
    fn commit(&mut self, next: Settings) -> bool {
        // Validate before touching the in-memory state: the background writer
        // would only report the problem after the change was already applied.
        if let Err(e) = next.validate() {
            self.error(format!("配置无效：{e:#}"));
            return false;
        }
        self.settings = next.clone();
        self.draft = next.clone();
        self.latencies.resize(self.settings.nodes.len(), None);
        self.load_error = false;
        match self.writer.save(next) {
            Ok(sequence) => {
                self.save_sequence = sequence;
                true
            }
            Err(e) => {
                self.error(format!("保存失败：{e:#}"));
                false
            }
        }
    }
    /// Keeps the displayed selection in sync with the node the runtime uses.
    fn select_node(&mut self, index: usize) {
        if index == self.settings.selected_node || index >= self.settings.nodes.len() {
            return;
        }
        let mut next = self.settings.clone();
        next.selected_node = index;
        self.commit(next);
    }
    /// Closes editors whose target disappeared from the lists.
    ///
    /// egui windows are not modal, so the node list can be edited while the
    /// editor is open. Without this check the editor would write to a stale
    /// index and either corrupt another node or panic on an out-of-range
    /// access.
    fn drop_stale_editors(&mut self) {
        let stale_node = match &self.editor {
            Some(editor) => match editor.index {
                Some(index) => {
                    index >= self.settings.nodes.len()
                        || self.settings.nodes[index].identity()
                            != editor.identity.clone().unwrap_or_default()
                }
                None => false,
            },
            None => false,
        };
        if stale_node {
            self.editor = None;
        }
        let stale_subscription = match &self.subscription_editor {
            Some(editor) => match editor.index {
                Some(index) => {
                    index >= self.settings.subscriptions.len()
                        || self.settings.subscriptions[index].id
                            != editor.id.clone().unwrap_or_default()
                }
                None => false,
            },
            None => false,
        };
        if stale_subscription {
            self.subscription_editor = None;
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
        // A mode file belongs to exactly one route kind; a file left over from
        // another kind would be rejected by the core when the session starts.
        self.modes
            .iter()
            .find(|m| {
                m.file.to_string_lossy() == self.settings.mode_file
                    && m.kind == self.settings.route_kind
            })
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
        // Configuration writes complete on a background thread.
        let outcomes = self.writer.poll();
        for outcome in outcomes {
            if outcome.error.is_none() {
                self.pending_save = None;
                continue;
            }
            // A newer save is already queued; it carries the same content and
            // will report its own outcome, so this failure is stale.
            if outcome.sequence != self.save_sequence {
                continue;
            }
            let message = outcome.error.unwrap_or_default();
            self.error(format!("保存失败：{message}（更改仅保留在内存中）"));
            // Restore the editor so the user's input is not lost.
            match self.pending_save.take() {
                Some(PendingSave::Node(editor)) => self.editor = Some(editor),
                Some(PendingSave::Subscription(editor)) => self.subscription_editor = Some(editor),
                None => {}
            }
        }
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
                    // A failed session must not leave the UI in a busy state.
                    self.busy = false;
                    self.tool_running = None;
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
                Event::Refreshed { id, nodes } => {
                    self.busy = false;
                    // Merge into the live configuration: a whole-snapshot
                    // replacement would discard the edits the user made while
                    // the download was running.
                    let mut next = self.settings.clone();
                    next.nodes.retain(|n| n.text("SubscriptionId") != id);
                    next.nodes.extend(nodes);
                    let selected = self
                        .settings
                        .nodes
                        .get(self.settings.selected_node)
                        .map(|n| n.identity());
                    next.selected_node = selected
                        .and_then(|identity| {
                            next.nodes.iter().position(|n| n.identity() == identity)
                        })
                        .unwrap_or(0);
                    let count = next.nodes.len();
                    if self.commit(next) {
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
                Event::Selection { index, text } => {
                    // Automatic selection may pick another node; keep the
                    // displayed selection aligned with the running session.
                    self.select_node(index);
                    self.info(text);
                }
                Event::Switched {
                    index,
                    millis,
                    reason,
                } => {
                    self.select_node(index);
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
                    // A mode file belongs to one route kind. Switching the
                    // acceleration mode invalidates the previous selection, so
                    // fall back to the first mode that fits the new kind.
                    if next.route_kind != self.settings.route_kind {
                        next.mode_file = self
                            .modes
                            .iter()
                            .find(|m| m.kind == next.route_kind)
                            .map(|m| m.file.to_string_lossy().into_owned())
                            .unwrap_or_default();
                    }
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
                    self.editor = Some(NodeEditor {
                        index: None,
                        identity: None,
                        node: Node::default(),
                        advanced: "{}".into(),
                    });
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
            self.editor = Some(NodeEditor {
                index: Some(index),
                identity: Some(node.identity()),
                node,
                advanced: fields,
            });
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
            // The node that was being edited may have just been deleted.
            self.drop_stale_editors();
        }
    }
    fn subscriptions_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("自动同步你的节点资源").color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary(ui, "＋  添加订阅", [130.0, 36.0], self.editable()) {
                    self.subscription_editor = Some(SubscriptionEditor {
                        index: None,
                        id: None,
                        subscription: Subscription::default(),
                    });
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
                    let subscription = self.settings.subscriptions[index].clone();
                    self.subscription_editor = Some(SubscriptionEditor {
                        index: Some(index),
                        id: Some(subscription.id.clone()),
                        subscription,
                    });
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
                // The log buffer can hold 1500 lines; rendering them all every
                // frame is wasteful, so only the visible rows are drawn.
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show_rows(ui, 18.0, self.logs.len(), |ui, rows| {
                        for row in rows {
                            let Some(line) = self.logs.get(row) else {
                                continue;
                            };
                            ui.add(
                                egui::Label::new(
                                    RichText::new(line).size(12.0).monospace().color(INK),
                                )
                                .truncate(),
                            );
                        }
                    });
                if self.logs.is_empty() {
                    ui.label(
                        RichText::new("暂无日志。启动连接或检查配置后，日志会显示在这里。")
                            .color(MUTED),
                    );
                }
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
