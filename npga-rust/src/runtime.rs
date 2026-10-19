use crate::{
    core_config,
    geoip::GeoIp,
    model::{Node, RouteKind, Settings},
    modes::Mode,
    nat,
    platform::{self, Job, SystemProxy},
    selector, speedtest,
    stats::{self, StatsPoller},
    subscription, update,
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct RuntimePaths {
    pub root: PathBuf,
    pub data: PathBuf,
}
impl RuntimePaths {
    pub fn resolve(
        executable: &Path,
        override_root: Option<PathBuf>,
        data: Option<PathBuf>,
    ) -> Self {
        let exe_dir = executable.parent().unwrap_or(Path::new("."));
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_owned();
        let root = override_root.unwrap_or_else(|| {
            // Only Rust-era runtime layouts; the C# package directories moved
            // to the external archive (refactor/DECISIONS.md ADR-007).
            [
                exe_dir.to_owned(),
                workspace.join("release-rust/NPGA-Rust"),
                workspace.join("release-npga/NPGA"),
                workspace.join(".release-runtime"),
                workspace.clone(),
            ]
            .into_iter()
            .find(|p| p.join("bin/xray.exe").exists())
            .unwrap_or(exe_dir.to_owned())
        });
        let root = std::path::absolute(root).unwrap_or_else(|_| exe_dir.to_owned());
        let data = std::path::absolute(data.unwrap_or_else(|| exe_dir.join("data/rust")))
            .unwrap_or_else(|_| exe_dir.join("data/rust"));
        Self { data, root }
    }
    pub fn bin(&self, name: &str) -> Result<PathBuf> {
        let path = self.root.join("bin").join(name);
        if !path.is_file() {
            bail!("缺少核心组件：{}", path.display());
        }
        Ok(path)
    }
    pub fn modes(&self) -> PathBuf {
        let runtime = self.root.join("mode");
        if runtime.is_dir() {
            runtime
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("Storage/mode")
        }
    }
    pub fn diagnostics(&self) -> Vec<(String, bool)> {
        [
            "bin/xray.exe",
            "bin/sing-box.exe",
            "bin/wintun.dll",
            "bin/pcap2socks.exe",
        ]
        .into_iter()
        .map(|name| (name.into(), self.root.join(name).is_file()))
        .collect()
    }
}

pub enum CommandMessage {
    Start {
        settings: Box<Settings>,
        mode: Option<Mode>,
    },
    Stop,
    Probe(Vec<Node>),
    Refresh {
        settings: Box<Settings>,
        id: String,
    },
    Check {
        settings: Box<Settings>,
        mode: Option<Mode>,
    },
    /// On-demand diagnostic tool (NAT type, exit country, speed, update check).
    Tool {
        kind: ToolKind,
        settings: Box<Settings>,
    },
    Shutdown,
}
/// What an automatic selection cycle is for: picking the initial node or
/// monitoring a live connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectPurpose {
    Connect,
    Monitor,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Update,
    Nat,
    Geo,
    Speed,
}
impl ToolKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Update => "检查更新",
            Self::Nat => "NAT 类型",
            Self::Geo => "出口归属地",
            Self::Speed => "网络测速",
        }
    }
}
#[derive(Debug)]
pub enum Event {
    Starting,
    Connected {
        core: String,
    },
    Stopped,
    Failed(String),
    OperationFailed(String),
    Log(String),
    Latency {
        index: usize,
        millis: Option<u128>,
        method: &'static str,
    },
    /// A subscription finished downloading. Only the nodes owned by `id` are
    /// reported; the caller merges them into the live configuration so edits
    /// made while the download was in flight are not lost.
    Refreshed {
        id: String,
        nodes: Vec<Node>,
    },
    Checked(String),
    Traffic {
        up: u64,
        down: u64,
    },
    ToolResult {
        kind: ToolKind,
        text: String,
        is_error: bool,
    },
    /// Automatic selection picked the node used for this connection.
    Selection {
        index: usize,
        text: String,
    },
    /// A live connection moved to another node (index is the new node).
    Switched {
        index: usize,
        millis: Option<u128>,
        reason: String,
    },
}

/// Probe sample routed to the background worker (not the UI channel).
enum Sample {
    Latency { index: usize, millis: Option<u128> },
}

/// How often a live connection is re-probed for automatic switching.
const MONITOR_INTERVAL: Duration = Duration::from_secs(20);

/// Minimum interval between two core traffic samples.
const TRAFFIC_INTERVAL: Duration = Duration::from_secs(1);

/// Latency probe pool; results flow to the selector over the private channel.
fn spawn_probe_pool(nodes: Vec<Node>, samples: Sender<Sample>) {
    let nodes = Arc::new(nodes);
    let next = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut pool = vec![];
    for _ in 0..4 {
        let nodes = nodes.clone();
        let next = next.clone();
        let samples = samples.clone();
        pool.push(thread::spawn(move || {
            loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(node) = nodes.get(index) else {
                    break;
                };
                let (millis, _) = latency(node);
                let _ = samples.send(Sample::Latency { index, millis });
            }
        }));
    }
    for thread in pool {
        let _ = thread.join();
    }
}

/// A running selection cycle: probes all candidates, then starts or switches.
struct PendingSelection {
    settings: Box<Settings>,
    mode: Option<Mode>,
    purpose: SelectPurpose,
    results: Vec<Option<u128>>,
    deadline: Instant,
}
pub struct Worker {
    pub commands: Sender<CommandMessage>,
    pub events: Receiver<Event>,
    pub cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Worker {
    pub fn new(paths: RuntimePaths) -> Self {
        let (commands, incoming) = mpsc::channel();
        let (outgoing, events) = mpsc::channel();
        let (sample_tx, sample_rx) = mpsc::channel::<Sample>();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop_flag = cancel.clone();
        let thread = thread::spawn(move || {
            let mut session: Option<Session> = None;
            let mut pending: Option<PendingSelection> = None;
            // Remember the parameters of the current/last connection so the
            // selector can restart it on another node.
            let mut last_start: Option<(Settings, Option<Mode>)> = None;
            let mut last_switch: Option<Instant> = None;
            let mut last_probe: Option<Instant> = None;
            loop {
                // Drain selector samples first so a completed cycle lands in the
                // same tick that reports it.
                while let Ok(sample) = sample_rx.try_recv() {
                    if let Sample::Latency { index, millis } = sample
                        && let Some(cycle) = pending.as_mut()
                        && let Some(slot) = cycle.results.get_mut(index)
                    {
                        *slot = millis;
                    }
                }
                match incoming.recv_timeout(Duration::from_millis(250)) {
                    Ok(CommandMessage::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                    Ok(CommandMessage::Stop) => {
                        // Drop any in-flight selection cycle so a pending
                        // automatic pick cannot restart the session afterwards.
                        pending = None;
                        last_start = None;
                        session.take();
                        let _ = outgoing.send(Event::Stopped);
                    }
                    Ok(CommandMessage::Start { settings, mode }) => {
                        if session.is_some() || pending.is_some() {
                            let _ = outgoing.send(Event::Failed("连接已经启动".into()));
                            continue;
                        }
                        let _ = outgoing.send(Event::Starting);
                        if settings.auto_select && settings.nodes.len() > 1 {
                            // Probe every candidate first; the session starts when
                            // the cycle completes (or the deadline passes).
                            let expected = settings.nodes.len();
                            pending = Some(PendingSelection {
                                settings,
                                mode,
                                purpose: SelectPurpose::Connect,
                                results: vec![None; expected],
                                deadline: Instant::now() + Duration::from_millis(2500),
                            });
                            let nodes = pending
                                .as_ref()
                                .map(|cycle| cycle.settings.nodes.clone())
                                .unwrap_or_default();
                            let samples = sample_tx.clone();
                            thread::spawn(move || spawn_probe_pool(nodes, samples));
                            let _ = outgoing.send(Event::Log("自动选路：正在探测节点延迟…".into()));
                        } else {
                            last_start = Some(((*settings).clone(), mode.clone()));
                            match Session::start(
                                &paths,
                                &settings,
                                mode.as_ref(),
                                &outgoing,
                                &stop_flag,
                            ) {
                                Ok(s) => {
                                    let core = s.core.clone();
                                    session = Some(s);
                                    let _ = outgoing.send(Event::Connected { core });
                                }
                                Err(e) => {
                                    last_start = None;
                                    let _ = outgoing.send(Event::Failed(format!("{e:#}")));
                                }
                            }
                        }
                    }
                    Ok(CommandMessage::Probe(nodes)) => {
                        // A separate bounded probe pool leaves stop/shutdown responsive.
                        let outgoing = outgoing.clone();
                        thread::spawn(move || {
                            let nodes = Arc::new(nodes);
                            let next = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                            let mut pool = vec![];
                            for _ in 0..4 {
                                let nodes = nodes.clone();
                                let next = next.clone();
                                let out = outgoing.clone();
                                pool.push(thread::spawn(move || {
                                    loop {
                                        let index = next.fetch_add(1, Ordering::Relaxed);
                                        let Some(node) = nodes.get(index) else {
                                            break;
                                        };
                                        let (millis, method) = latency(node);
                                        let _ = out.send(Event::Latency {
                                            index,
                                            millis,
                                            method,
                                        });
                                    }
                                }));
                            }
                            for thread in pool {
                                let _ = thread.join();
                            }
                        });
                    }
                    Ok(CommandMessage::Refresh { settings, id }) => {
                        let out = outgoing.clone();
                        let subscription_id = id.clone();
                        thread::spawn(move || {
                            // A panicking worker must still report back, or the
                            // UI would stay stuck in its busy state.
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    settings
                                        .subscriptions
                                        .iter()
                                        .find(|s| s.id == id)
                                        .context("订阅不存在")
                                        .and_then(|s| subscription::refresh(&settings, s))
                                }))
                                .unwrap_or_else(|_| Err(anyhow::anyhow!("订阅刷新任务异常终止")));
                            let _ = out.send(match result {
                                Ok(nodes) => Event::Refreshed {
                                    id: subscription_id,
                                    nodes,
                                },
                                Err(e) => Event::OperationFailed(format!("{e:#}")),
                            });
                        });
                    }
                    Ok(CommandMessage::Check { settings, mode }) => {
                        let result =
                            check_all(&paths, &settings, mode.as_ref(), &outgoing, &stop_flag);
                        let _ = outgoing.send(match result {
                            Ok(()) => Event::Checked("当前节点及模式配置通过核心校验".into()),
                            Err(e) => Event::OperationFailed(format!("{e:#}")),
                        });
                    }
                    Ok(CommandMessage::Tool { kind, settings }) => {
                        let outgoing = outgoing.clone();
                        let paths = paths.clone();
                        thread::spawn(move || {
                            // A panicking tool must still report back, or the
                            // diagnostic buttons would stay disabled forever.
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    run_tool(kind, &paths, &settings)
                                }))
                                .unwrap_or_else(|_| {
                                    Err(anyhow::anyhow!("{}工具执行异常终止", kind.label()))
                                });
                            let _ = outgoing.send(match result {
                                Ok(text) => Event::ToolResult {
                                    kind,
                                    text,
                                    is_error: false,
                                },
                                Err(e) => Event::ToolResult {
                                    kind,
                                    text: format!("{e:#}"),
                                    is_error: true,
                                },
                            });
                        });
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                // Complete or expire the in-flight selection cycle.
                let cycle_done = pending
                    .as_ref()
                    .map(|cycle| {
                        cycle.results.iter().all(|r| r.is_some())
                            || Instant::now() >= cycle.deadline
                    })
                    .unwrap_or(false);
                if cycle_done && let Some(cycle) = pending.take() {
                    match cycle.purpose {
                        SelectPurpose::Connect => {
                            let (mut settings, mode) = (*cycle.settings, cycle.mode);
                            let results: Vec<_> = cycle
                                .results
                                .iter()
                                .enumerate()
                                .map(|(index, millis)| (index, *millis))
                                .collect();
                            let best = selector::select_best(&results);
                            let index = best
                                .map(|(index, _)| index)
                                .unwrap_or(settings.selected_node);
                            let name = settings
                                .nodes
                                .get(index)
                                .map(|n| n.name())
                                .unwrap_or_else(|| "未知节点".into());
                            match best {
                                Some((_, millis)) => {
                                    let _ = outgoing.send(Event::Selection {
                                        index,
                                        text: format!("自动选路：选中「{name}」（{millis}ms）"),
                                    });
                                }
                                None => {
                                    let _ = outgoing.send(Event::Log(format!(
                                        "自动选路：所有节点探测失败，使用手动选择的节点「{name}」"
                                    )));
                                }
                            }
                            settings.selected_node = index;
                            last_start = Some((settings.clone(), mode.clone()));
                            match Session::start(
                                &paths,
                                &settings,
                                mode.as_ref(),
                                &outgoing,
                                &stop_flag,
                            ) {
                                Ok(s) => {
                                    let core = s.core.clone();
                                    session = Some(s);
                                    last_probe = Some(Instant::now());
                                    let _ = outgoing.send(Event::Connected { core });
                                }
                                Err(e) => {
                                    last_start = None;
                                    let _ = outgoing.send(Event::Failed(format!("{e:#}")));
                                }
                            }
                        }
                        SelectPurpose::Monitor => {
                            let Some((settings, mode)) = last_start.clone() else {
                                continue;
                            };
                            let Some(current) = session.as_ref().map(|s| s.node_index) else {
                                continue;
                            };
                            let results: Vec<_> = cycle
                                .results
                                .iter()
                                .enumerate()
                                .map(|(index, millis)| (index, *millis))
                                .collect();
                            let current_ms = cycle.results.get(current).copied().flatten();
                            let config = selector::SelectorConfig {
                                enabled: settings.auto_select,
                                switch_on_degradation: settings.auto_select_degradable,
                                degrade_ms: settings.degrade_ms as u128,
                                cooldown: Duration::from_millis(settings.switch_cooldown_ms),
                                probe_interval: MONITOR_INTERVAL,
                            };
                            match selector::decide_switch(
                                current,
                                current_ms,
                                &results,
                                &config,
                                last_switch.map(|t| t.elapsed()),
                            ) {
                                selector::Outcome::Keep => {}
                                selector::Outcome::Blocked { reason } => {
                                    let _ =
                                        outgoing.send(Event::Log(format!("自动选路：{reason}")));
                                }
                                selector::Outcome::Switch { to, millis, reason } => {
                                    session.take();
                                    let mut next = settings.clone();
                                    next.selected_node = to;
                                    let name = next
                                        .nodes
                                        .get(to)
                                        .map(|n| n.name())
                                        .unwrap_or_else(|| "未知节点".into());
                                    last_start = Some((next.clone(), mode.clone()));
                                    match Session::start(
                                        &paths,
                                        &next,
                                        mode.as_ref(),
                                        &outgoing,
                                        &stop_flag,
                                    ) {
                                        Ok(s) => {
                                            session = Some(s);
                                            last_switch = Some(Instant::now());
                                            let _ = outgoing.send(Event::Switched {
                                                index: to,
                                                millis: Some(millis),
                                                reason: format!("{reason}，已切换至「{name}」"),
                                            });
                                        }
                                        Err(e) => {
                                            last_start = None;
                                            let _ = outgoing.send(Event::Failed(format!(
                                                "切换节点后启动失败：{e:#}"
                                            )));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(s) = &mut session {
                    if s.exited() {
                        session.take();
                        let _ = outgoing.send(Event::Failed("代理核心意外退出，连接已停止".into()));
                    } else if let Some((up, down)) = s.traffic() {
                        let _ = outgoing.send(Event::Traffic { up, down });
                    }
                } // Schedule the next monitoring probe.
                if session.is_some() && pending.is_none() {
                    let due = last_probe
                        .map(|at| at.elapsed() >= MONITOR_INTERVAL)
                        .unwrap_or(false);
                    if due
                        && let Some((settings, _)) = &last_start
                        && settings.auto_select
                        && settings.nodes.len() > 1
                    {
                        let nodes = settings.nodes.clone();
                        let samples = sample_tx.clone();
                        thread::spawn(move || spawn_probe_pool(nodes, samples));
                        pending = Some(PendingSelection {
                            settings: Box::new(settings.clone()),
                            mode: None,
                            purpose: SelectPurpose::Monitor,
                            results: vec![None; settings.nodes.len()],
                            deadline: Instant::now() + Duration::from_millis(2500),
                        });
                        last_probe = Some(Instant::now());
                    }
                }
            }
            drop(session);
        });
        Self {
            commands,
            events,
            cancel,
            thread: Some(thread),
        }
    }
    pub fn send(&self, command: CommandMessage) -> Result<()> {
        self.commands.send(command).context("后台控制线程已停止")
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        let _ = self.commands.send(CommandMessage::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn scrub(line: &str, node: &Node) -> String {
    let mut output = line.to_owned();
    for key in [
        "Password",
        "Username",
        "UserID",
        "QUICSecret",
        "PrivateKey",
        "PreSharedKey",
        "ObfuscationPassword",
    ] {
        let secret = node.text(key);
        if !secret.is_empty() {
            output = output.replace(secret, "******");
        }
    }
    output
}

struct ManagedChild {
    child: Child,
    readers: Vec<JoinHandle<()>>,
    ready: Arc<AtomicBool>,
    _job: Job,
}
impl ManagedChild {
    fn spawn(
        executable: &Path,
        args: &[String],
        cwd: &Path,
        events: &Sender<Event>,
        node: &Node,
    ) -> Result<Self> {
        let job = Job::new()?;
        let mut command = Command::new(executable);
        command
            .args(args)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        platform::hidden(&mut command);
        let mut child = command.spawn().with_context(|| {
            format!(
                "启动 {} 失败",
                executable.file_name().unwrap_or_default().to_string_lossy()
            )
        })?;
        if let Err(e) = job.attach(&child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
        let ready = Arc::new(AtomicBool::new(false));
        let mut readers = vec![];
        let pipes: Vec<Box<dyn Read + Send>> = vec![
            Box::new(child.stdout.take().unwrap()),
            Box::new(child.stderr.take().unwrap()),
        ];
        for pipe in pipes {
            let out = events.clone();
            let node = node.clone();
            let ready = ready.clone();
            readers.push(thread::spawn(move || {
                let mut reader = BufReader::new(pipe);
                let mut bytes = vec![];
                loop {
                    bytes.clear();
                    if reader
                        .by_ref()
                        .take(16384)
                        .read_until(b'\n', &mut bytes)
                        .unwrap_or(0)
                        == 0
                    {
                        break;
                    }
                    let line = scrub(String::from_utf8_lossy(&bytes).trim(), &node);
                    if line.contains("sing-box started") {
                        ready.store(true, Ordering::Relaxed);
                    }
                    let _ = out.send(Event::Log(line));
                }
            }));
        }
        Ok(Self {
            child,
            readers,
            ready,
            _job: job,
        })
    }
    fn listening(&mut self, address: &str, port: u16, cancel: &AtomicBool) -> Result<()> {
        let target: SocketAddr = format!(
            "{}:{port}",
            if address.contains(':') {
                format!("[{address}]")
            } else {
                address.into()
            }
        )
        .parse()?;
        let begin = Instant::now();
        loop {
            if cancel.load(Ordering::Relaxed) {
                bail!("连接已取消");
            }
            if self.child.try_wait()?.is_some() {
                bail!("核心在启动期间退出，请查看运行日志");
            }
            if TcpStream::connect_timeout(&target, Duration::from_millis(100)).is_ok() {
                return Ok(());
            }
            if begin.elapsed() > Duration::from_secs(8) {
                bail!("核心启动超时，请查看运行日志");
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
    fn started(&mut self, cancel: &AtomicBool) -> Result<()> {
        let begin = Instant::now();
        loop {
            if cancel.load(Ordering::Relaxed) {
                bail!("连接已取消");
            }
            if self.child.try_wait()?.is_some() {
                bail!("TUN 核心启动失败，请检查管理员权限和运行日志");
            }
            if self.ready.load(Ordering::Relaxed) {
                return Ok(());
            }
            if begin.elapsed() > Duration::from_secs(8) {
                bail!("TUN 启动超时");
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        self._job.terminate();
        let _ = self.child.kill();
        let _ = self.child.wait();
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
    }
}

struct TemporaryConfigs {
    directory: PathBuf,
}
impl TemporaryConfigs {
    fn new(paths: &RuntimePaths) -> Result<Self> {
        let directory = paths
            .data
            .join("runtime")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&directory)?;
        Ok(Self { directory })
    }
    fn write(&self, name: &str, config: &Value) -> Result<PathBuf> {
        let file = self.directory.join(name);
        let mut out = fs::File::create(&file)?;
        out.write_all(&serde_json::to_vec_pretty(config)?)?;
        out.sync_all()?;
        Ok(file)
    }
}
impl Drop for TemporaryConfigs {
    fn drop(&mut self) {
        // This directory is generated by this session only; never recursively remove user paths.
        if let Ok(entries) = fs::read_dir(&self.directory) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        let _ = fs::remove_dir(&self.directory);
    }
}

struct Session {
    core: String,
    /// Index of the node this session uses (may differ from the user's choice
    /// when automatic selection picked another node).
    node_index: usize,
    proxy: Option<SystemProxy>,
    /// Core stats backend (Clash API / Xray gRPC) for the traffic display.
    stats: Option<StatsPoller>,
    /// Last time the core stats API was queried. The worker loop ticks up to
    /// four times a second and a stats query is not free, so it is throttled.
    last_traffic: Option<Instant>,
    children: Vec<ManagedChild>,
    _configs: TemporaryConfigs,
    events: Sender<Event>,
}
impl Session {
    fn start(
        paths: &RuntimePaths,
        settings: &Settings,
        mode: Option<&Mode>,
        events: &Sender<Event>,
        cancel: &AtomicBool,
    ) -> Result<Self> {
        settings.validate()?;
        let node = settings
            .nodes
            .get(settings.selected_node)
            .context("请先选择节点")?;
        node.validate()?;
        let addresses: Vec<_> = (node.hostname.as_str(), node.port)
            .to_socket_addrs()
            .context("节点 DNS 解析失败")?
            .collect();
        let server = addresses.first().context("节点没有可用 IP")?;
        let core_name = if node.protocol == "Hysteria2" {
            "sing-box.exe"
        } else {
            "xray.exe"
        };
        let executable = paths.bin(core_name)?;
        let mut config = if node.protocol == "Hysteria2" {
            core_config::sing_box(node, settings)?
        } else {
            core_config::xray(node, settings)?
        };
        // Pin the upstream address so TUN does not change the core's DNS route.
        let ip = server.ip().to_string();
        if node.protocol == "Hysteria2" {
            config["outbounds"][0]["server"] = json!(ip);
        } else if node.protocol == "WireGuard" {
            config["outbounds"][0]["settings"]["peers"][0]["endpoint"] = json!(format!(
                "{}:{}",
                if ip.contains(':') {
                    format!("[{ip}]")
                } else {
                    ip
                },
                node.port
            ));
        } else {
            let key = if matches!(node.protocol.as_str(), "VLESS" | "VMess") {
                "vnext"
            } else {
                "servers"
            };
            config["outbounds"][0]["settings"][key][0]["address"] = json!(ip);
        }
        // Attach the core's stats API so every route mode reports traffic.
        let api_ports = stats::api_ports(settings.socks_port);
        if let Some((xray_port, sing_box_port)) = api_ports {
            if node.protocol == "Hysteria2" {
                stats::attach_sing_box_stats(&mut config, sing_box_port);
            } else {
                stats::attach_xray_stats(&mut config, xray_port)?;
            }
        }
        let configs = TemporaryConfigs::new(paths)?;
        let file = configs.write("core.json", &config)?;
        check_core(&executable, &file, paths, events, node, cancel)?;
        let socks_reserve =
            TcpListener::bind((settings.local_address.as_str(), settings.socks_port))
                .context("SOCKS 端口已被占用，请修改端口")?;
        let http_reserve = TcpListener::bind((settings.local_address.as_str(), settings.http_port))
            .context("HTTP 端口已被占用，请修改端口")?;
        // Reserve the stats ports early so a conflict fails fast with a clear message.
        let stats_reserve = api_ports.and_then(|(xray_port, sing_box_port)| {
            let port = if node.protocol == "Hysteria2" {
                sing_box_port
            } else {
                xray_port
            };
            TcpListener::bind((settings.local_address.as_str(), port))
                .ok()
                .map(|listener| (listener, port))
        });
        let args = run_args(core_name, &file);
        drop((socks_reserve, http_reserve, stats_reserve));
        let child = ManagedChild::spawn(&executable, &args, &paths.root, events, node)?;
        let stats = api_ports.map(|(xray_port, sing_box_port)| {
            if node.protocol == "Hysteria2" {
                StatsPoller::sing_box(sing_box_port)
            } else {
                StatsPoller::xray(xray_port, executable.clone())
            }
        });
        let mut session = Self {
            core: node.core().into(),
            node_index: settings.selected_node,
            proxy: None,
            stats,
            last_traffic: None,
            children: vec![child],
            _configs: configs,
            events: events.clone(),
        };
        session.children[0].listening(&settings.local_address, settings.socks_port, cancel)?;
        session.children[0].listening(&settings.local_address, settings.http_port, cancel)?;
        match settings.route_kind {
            RouteKind::Local => {}
            RouteKind::System => {
                session.proxy = Some(SystemProxy::enable(
                    &settings.local_address,
                    settings.http_port,
                    &paths.data.join("system-proxy.dpapi"),
                )?);
            }
            RouteKind::Process => {
                let mode = mode
                    .filter(|m| m.kind == RouteKind::Process)
                    .context("请选择进程模式")?;
                // ADR-002: sing-box TUN with process_name routing replaced the
                // Redirector + netfilter2 path (Phase 2 removal, 2026-10-17).
                let (handle, unconvertible) = core_config::process_names(&mode.handle);
                if !unconvertible.is_empty() {
                    let _ = events.send(Event::Log(format!(
                        "[进程模式] {} 条规则无法转换为精确进程名，已跳过：{}",
                        unconvertible.len(),
                        unconvertible.join("、")
                    )));
                }
                if handle.is_empty() {
                    bail!(
                        "进程模式只支持精确进程名（如 ^game\\.exe$），当前模式没有可用规则；请修改模式规则"
                    );
                }
                let mut config =
                    core_config::tun_process(settings, mode, &server.ip().to_string())?;
                config["log"]["level"] = json!("info");
                let file = session._configs.write("tun-process.json", &config)?;
                let executable = paths.bin("sing-box.exe")?;
                check_core(&executable, &file, paths, events, node, cancel)?;
                let child = ManagedChild::spawn(
                    &executable,
                    &run_args("sing-box.exe", &file),
                    &paths.root,
                    events,
                    node,
                )?;
                session.children.push(child);
                session.children.last_mut().unwrap().started(cancel)?;
            }
            RouteKind::Tun => {
                let mut config = core_config::tun(settings, mode, &server.ip().to_string())?;
                config["log"]["level"] = json!("info");
                let file = session._configs.write("tun.json", &config)?;
                let executable = paths.bin("sing-box.exe")?;
                check_core(&executable, &file, paths, events, node, cancel)?;
                let child = ManagedChild::spawn(
                    &executable,
                    &run_args("sing-box.exe", &file),
                    &paths.root,
                    events,
                    node,
                )?;
                session.children.push(child);
                session.children.last_mut().unwrap().started(cancel)?;
            }
            RouteKind::Share => {
                let mode = mode
                    .filter(|m| m.kind == RouteKind::Share)
                    .context("请选择网络共享模式")?;
                if settings.share_interface.trim().is_empty() {
                    bail!("请在设置中填写 Npcap 网卡设备名");
                }
                let mut args = vec![
                    "--interface".into(),
                    settings.share_interface.clone(),
                    "--destination".into(),
                    format!("{}:{}", settings.local_address, settings.socks_port),
                ];
                args.extend(mode.argument.split_whitespace().map(str::to_owned));
                let child = ManagedChild::spawn(
                    &paths.bin("pcap2socks.exe")?,
                    &args,
                    &paths.root,
                    events,
                    node,
                )?;
                session.children.push(child);
                thread::sleep(Duration::from_millis(600));
                if session.exited() {
                    bail!("网络共享核心退出，请检查 Npcap 和运行日志");
                }
            }
        }
        if cancel.load(Ordering::Relaxed) {
            bail!("连接已取消");
        }
        Ok(session)
    }
    fn exited(&mut self) -> bool {
        self.children
            .iter_mut()
            .any(|c| c.child.try_wait().is_ok_and(|s| s.is_some()))
    }
    fn traffic(&mut self) -> Option<(u64, u64)> {
        // The worker loop wakes several times per second. Querying the core's
        // stats API that often is wasteful (and the CLI fallback would spawn a
        // process every tick), so sample at most once per interval.
        let due = self
            .last_traffic
            .map(|at| at.elapsed() >= TRAFFIC_INTERVAL)
            .unwrap_or(true);
        if !due {
            return None;
        }
        self.last_traffic = Some(Instant::now());
        self.stats.as_ref().and_then(|stats| stats.traffic())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(proxy) = self.proxy.take()
            && let Err(e) = proxy.restore()
        {
            let _ = self.events.send(Event::OperationFailed(format!(
                "恢复系统代理失败：{e:#}。下次启动会再次恢复。"
            )));
        }
        self.children.clear();
    }
}

fn run_args(core: &str, file: &Path) -> Vec<String> {
    vec![
        "run".into(),
        if core == "sing-box.exe" {
            "-c"
        } else {
            "-config"
        }
        .into(),
        file.to_string_lossy().into_owned(),
    ]
}
fn check_core(
    executable: &Path,
    file: &Path,
    paths: &RuntimePaths,
    events: &Sender<Event>,
    node: &Node,
    cancel: &AtomicBool,
) -> Result<()> {
    let sing = executable.file_name().is_some_and(|s| s == "sing-box.exe");
    let args = if sing {
        vec![
            "check".into(),
            "-c".into(),
            file.to_string_lossy().into_owned(),
        ]
    } else {
        vec![
            "run".into(),
            "-test".into(),
            "-config".into(),
            file.to_string_lossy().into_owned(),
        ]
    };
    let mut child = ManagedChild::spawn(executable, &args, &paths.root, events, node)?;
    let begin = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("配置检查已取消");
        }
        if let Some(status) = child.child.try_wait()? {
            if !status.success() {
                bail!("核心拒绝配置，请查看运行日志");
            }
            return Ok(());
        }
        if begin.elapsed() > Duration::from_secs(10) {
            bail!("核心配置校验超时");
        }
        thread::sleep(Duration::from_millis(50));
    }
}
fn check_all(
    paths: &RuntimePaths,
    settings: &Settings,
    mode: Option<&Mode>,
    events: &Sender<Event>,
    cancel: &AtomicBool,
) -> Result<()> {
    let node = settings
        .nodes
        .get(settings.selected_node)
        .context("请先选择节点")?;
    let configs = TemporaryConfigs::new(paths)?;
    let (name, value) = if node.protocol == "Hysteria2" {
        ("sing-box.exe", core_config::sing_box(node, settings)?)
    } else {
        ("xray.exe", core_config::xray(node, settings)?)
    };
    let file = configs.write("core.json", &value)?;
    check_core(&paths.bin(name)?, &file, paths, events, node, cancel)?;
    if settings.route_kind == RouteKind::Tun {
        let address = (node.hostname.as_str(), node.port)
            .to_socket_addrs()?
            .next()
            .context("节点 DNS 解析失败")?;
        let file = configs.write(
            "tun.json",
            &core_config::tun(settings, mode, &address.ip().to_string())?,
        )?;
        check_core(
            &paths.bin("sing-box.exe")?,
            &file,
            paths,
            events,
            node,
            cancel,
        )?;
    }
    Ok(())
}

/// Runs an on-demand diagnostic tool against the current settings.
fn run_tool(kind: ToolKind, paths: &RuntimePaths, settings: &Settings) -> Result<String> {
    match kind {
        ToolKind::Update => {
            let release = update::check(crate::VERSION, settings.allow_prerelease())?;
            match release {
                Some(release) => {
                    let mut text = format!(
                        "发现新版本 {}（当前 v{}）",
                        release.tag_name,
                        crate::VERSION
                    );
                    let assets = update::checksums(&release, Some("npga.zip"));
                    if let Some((name, _)) = assets.first() {
                        text.push_str(&format!("\n发布包：{name}"));
                    }
                    text.push_str(&format!("\n{}", release.html_url));
                    Ok(text)
                }
                None => Ok(format!("当前已是最新版本 v{}", crate::VERSION)),
            }
        }
        ToolKind::Nat => {
            let (stun_host, stun_port) = settings.stun_server();
            let socks = format!("{}:{}", settings.local_address, settings.socks_port);
            let result = nat::discover_nat_type(&socks, &stun_host, stun_port, "", "")?;
            let mut text = format!(
                "NAT 类型：{}\n映射：{}\n过滤：{}",
                result.result, result.mapping, result.filtering
            );
            if let Some(public) = result.public_end {
                text.push_str(&format!("\n出口端点：{public}"));
            }
            text.push_str("\n（经本地 SOCKS 端口探测，反映加速出口的网络环境）");
            Ok(text)
        }
        ToolKind::Geo => {
            let http_proxy = format!("http://{}:{}", settings.local_address, settings.http_port);
            let address = stats::exit_ip(&http_proxy, settings.request_timeout)?;
            let database = paths.root.join("bin").join("GeoLite2-Country.mmdb");
            if !database.is_file() {
                return Ok(format!(
                    "出口 IP：{address}（未找到 IP 库 {database:?}，无法查询归属地）"
                ));
            }
            let geo = GeoIp::open(&database)?;
            match geo.country_code(address) {
                Some(code) => Ok(format!("出口 IP：{address}　归属地：{code}")),
                None => Ok(format!("出口 IP：{address}　归属地：未知")),
            }
        }
        ToolKind::Speed => {
            let http_proxy = format!("http://{}:{}", settings.local_address, settings.http_port);
            let result = speedtest::measure(&http_proxy, settings.request_timeout.max(15000))?;
            Ok(format!(
                "下载速率：{:.2} Mbps（{:.1} MB / {:.1}s，{}）",
                result.mbps,
                result.bytes as f64 / 1_048_576.0,
                result.seconds,
                result.endpoint
            ))
        }
    }
}

pub fn latency(node: &Node) -> (Option<u128>, &'static str) {
    if node.protocol == "Hysteria2" {
        let mut command = Command::new("ping");
        command.args(["-n", "1", "-w", "1800", &node.hostname]);
        platform::hidden(&mut command);
        let output = command.output();
        let millis = output.ok().filter(|o| o.status.success()).and_then(|o| {
            // Both Chinese and English ping output contain the ASCII ms suffix.
            let text = String::from_utf8_lossy(&o.stdout);
            text.split("ms").find_map(|part| {
                let digits: String = part
                    .chars()
                    .rev()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                digits.chars().rev().collect::<String>().parse().ok()
            })
        });
        return (millis, "ICMP");
    }
    let addresses = (node.hostname.as_str(), node.port).to_socket_addrs();
    let millis = addresses.ok().and_then(|mut addresses| {
        addresses.find_map(|address| {
            let begin = Instant::now();
            TcpStream::connect_timeout(&address, Duration::from_millis(1800))
                .ok()
                .map(|_| begin.elapsed().as_millis())
        })
    });
    (millis, "TCP")
}
