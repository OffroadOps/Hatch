use base64::{Engine, engine::general_purpose::STANDARD};
use npga::{
    core_config,
    model::{Node, RouteKind, Settings},
    runtime::{CommandMessage, Event, RuntimePaths, Worker},
};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

fn paths() -> RuntimePaths {
    let root = std::env::var_os("NPGA_RUNTIME_ROOT")
        .map(PathBuf::from)
        .expect("设置 NPGA_RUNTIME_ROOT 指向带 bin/xray.exe、bin/sing-box.exe 的运行目录");
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/integration")
        .join(uuid::Uuid::new_v4().to_string());
    RuntimePaths { root, data }
}
fn free_ports() -> (u16, u16) {
    let a = TcpListener::bind("127.0.0.1:0").unwrap();
    let b = TcpListener::bind("127.0.0.1:0").unwrap();
    (
        a.local_addr().unwrap().port(),
        b.local_addr().unwrap().port(),
    )
}
fn settings(node: Node) -> Settings {
    let (socks_port, http_port) = free_ports();
    Settings {
        nodes: vec![node],
        socks_port,
        http_port,
        ..Settings::default()
    }
}
fn wait(worker: &Worker, check: impl Fn(&Event) -> bool) {
    let start = Instant::now();
    loop {
        let event = worker
            .events
            .recv_timeout(Duration::from_secs(12))
            .expect("控制线程超时");
        if let Event::Failed(message) = &event {
            panic!("{message}");
        }
        if check(&event) {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(15));
    }
}

#[test]
#[ignore = "requires pinned bundled proxy cores; run with NPGA_RUNTIME_ROOT"]
fn configs_validate_against_real_pinned_cores() {
    let paths = paths();
    fs::create_dir_all(&paths.data).unwrap();
    for protocol in [
        "SOCKS",
        "SS",
        "VMess",
        "VLESS",
        "Trojan",
        "WireGuard",
        "Hysteria2",
    ] {
        let mut node = Node {
            protocol: protocol.into(),
            hostname: "127.0.0.1".into(),
            port: 443,
            ..Node::default()
        };
        node.set("UserID", "550e8400-e29b-41d4-a716-446655440000");
        node.set("Password", "test-only-secret");
        node.set("PrivateKey", STANDARD.encode([1u8; 32]));
        node.set("PeerPublicKey", STANDARD.encode([2u8; 32]));
        let settings = settings(node.clone());
        let sing = protocol == "Hysteria2";
        let config = if sing {
            core_config::sing_box(&node, &settings).unwrap()
        } else {
            core_config::xray(&node, &settings).unwrap()
        };
        let file = paths.data.join(format!("{protocol}.json"));
        fs::write(&file, serde_json::to_vec(&config).unwrap()).unwrap();
        let mut command = Command::new(paths.root.join(if sing {
            "bin/sing-box.exe"
        } else {
            "bin/xray.exe"
        }));
        npga::platform::hidden(&mut command);
        command
            .args(if sing {
                vec!["check", "-c"]
            } else {
                vec!["run", "-test", "-config"]
            })
            .arg(&file);
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{protocol}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let settings = Settings {
        route_kind: RouteKind::Tun,
        ..Settings::default()
    };
    let file = paths.data.join("tun.json");
    fs::write(
        &file,
        serde_json::to_vec(&core_config::tun(&settings, None, "127.0.0.1").unwrap()).unwrap(),
    )
    .unwrap();
    let output = Command::new(paths.root.join("bin/sing-box.exe"))
        .args(["check", "-c"])
        .arg(file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "TUN: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Only known fixture files in the unique test directory are removed.
    for entry in fs::read_dir(&paths.data).unwrap().flatten() {
        if entry.path().is_file() {
            fs::remove_file(entry.path()).unwrap();
        }
    }
    fs::remove_dir(&paths.data).unwrap();
}

struct MockSocks {
    port: u16,
    quit: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl MockSocks {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let quit = Arc::new(AtomicBool::new(false));
        let q = quit.clone();
        let thread = thread::spawn(move || {
            while !q.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        thread::spawn(move || {
                            stream.set_nonblocking(false).unwrap();
                            stream
                                .set_read_timeout(Some(Duration::from_secs(4)))
                                .unwrap();
                            stream
                                .set_write_timeout(Some(Duration::from_secs(4)))
                                .unwrap();
                            let mut head = [0u8; 2];
                            if stream.read_exact(&mut head).is_err() {
                                return;
                            }
                            assert_eq!(head[0], 5);
                            let mut methods = vec![0; head[1] as usize];
                            stream.read_exact(&mut methods).unwrap();
                            stream.write_all(&[5, 0]).unwrap();
                            let mut req = [0; 4];
                            stream.read_exact(&mut req).unwrap();
                            assert_eq!(req[1], 1);
                            let mut address = vec![
                                0;
                                match req[3] {
                                    1 => 4,
                                    4 => 16,
                                    3 => {
                                        let mut len = [0];
                                        stream.read_exact(&mut len).unwrap();
                                        len[0] as usize
                                    }
                                    _ => panic!("bad address"),
                                } + 2
                            ];
                            stream.read_exact(&mut address).unwrap();
                            stream.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0]).unwrap();
                            let mut data = [0; 256];
                            while let Ok(n) = stream.read(&mut data) {
                                if n == 0 {
                                    break;
                                }
                                if stream.write_all(&data[..n]).is_err() {
                                    break;
                                }
                            }
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20))
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            port,
            quit,
            thread: Some(thread),
        }
    }
}
impl Drop for MockSocks {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

#[test]
#[ignore = "requires bundled xray; no external network or routing changes"]
fn real_xray_socks_and_http_echo_then_stop_twice() {
    let upstream = MockSocks::start();
    let node = Node {
        protocol: "SOCKS".into(),
        hostname: "127.0.0.1".into(),
        port: upstream.port,
        ..Node::default()
    };
    let settings = settings(node);
    let paths = paths();
    let worker = Worker::new(paths.clone());
    worker
        .send(CommandMessage::Start {
            settings: Box::new(settings.clone()),
            mode: None,
        })
        .unwrap();
    wait(&worker, |e| matches!(e, Event::Connected { .. }));
    let mut socks = TcpStream::connect(("127.0.0.1", settings.socks_port)).unwrap();
    socks
        .set_read_timeout(Some(Duration::from_secs(4)))
        .unwrap();
    socks.write_all(&[5, 1, 0]).unwrap();
    let mut greeting = [0; 2];
    socks.read_exact(&mut greeting).unwrap();
    assert_eq!(greeting, [5, 0]);
    socks
        .write_all(&[5, 1, 0, 1, 203, 0, 113, 5, 1, 187])
        .unwrap();
    let mut response = [0; 10];
    socks.read_exact(&mut response).unwrap();
    assert_eq!(response[1], 0);
    socks.write_all(b"npga-e2e-socks").unwrap();
    let mut echo = [0; 14];
    socks.read_exact(&mut echo).unwrap();
    assert_eq!(&echo, b"npga-e2e-socks");
    drop(socks);
    let mut http = TcpStream::connect(("127.0.0.1", settings.http_port)).unwrap();
    http.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
    http.write_all(b"CONNECT 203.0.113.5:443 HTTP/1.1\r\nHost: 203.0.113.5:443\r\n\r\n")
        .unwrap();
    let mut header = vec![];
    loop {
        let mut byte = [0];
        http.read_exact(&mut byte).unwrap();
        header.push(byte[0]);
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
        assert!(header.len() < 4096);
    }
    assert!(String::from_utf8_lossy(&header).contains("200"));
    http.write_all(b"npga-http").unwrap();
    let mut echo = [0; 9];
    http.read_exact(&mut echo).unwrap();
    assert_eq!(&echo, b"npga-http");
    drop(http);
    worker.send(CommandMessage::Stop).unwrap();
    wait(&worker, |e| matches!(e, Event::Stopped));
    worker.send(CommandMessage::Stop).unwrap();
    wait(&worker, |e| matches!(e, Event::Stopped));
    drop(worker);
    assert!(TcpListener::bind(("127.0.0.1", settings.socks_port)).is_ok());
    assert!(TcpListener::bind(("127.0.0.1", settings.http_port)).is_ok());
    assert!(
        fs::read_dir(paths.data.join("runtime"))
            .unwrap()
            .next()
            .is_none()
    );
}

/// Probes whether a local HTTP filter intercepts cleartext HTTP/2 requests.
///
/// AdGuard and similar products answer loopback HTTP/2 requests with an HTML
/// block page instead of passing them through. A plain HTTP/1.1 probe would
/// not notice that, so the control request uses the same h2c stack as the
/// stats query.
fn h2c_is_intercepted(port: u16) -> bool {
    let client = reqwest::blocking::Client::builder()
        .http2_prior_knowledge()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    match client
        .post(format!("http://127.0.0.1:{port}/"))
        .header("content-type", "application/grpc")
        .body(Vec::new())
        .send()
    {
        Ok(response) => {
            let body = response.text().unwrap_or_default();
            body.contains("<html") || body.contains("AdGuard")
        }
        Err(_) => false,
    }
}

#[test]
#[ignore = "requires bundled xray; no external network or routing changes"]
fn xray_stats_are_queried_over_grpc_without_spawning_the_cli() {
    let upstream = MockSocks::start();
    let node = Node {
        protocol: "SOCKS".into(),
        hostname: "127.0.0.1".into(),
        port: upstream.port,
        ..Node::default()
    };
    let settings = settings(node);
    let paths = paths();
    let worker = Worker::new(paths.clone());
    worker
        .send(CommandMessage::Start {
            settings: Box::new(settings.clone()),
            mode: None,
        })
        .unwrap();
    wait(&worker, |e| matches!(e, Event::Connected { .. }));

    // The core exposes its stats service on the API port derived from the
    // SOCKS port. The query must work over gRPC so the traffic display does
    // not have to spawn `xray api statsquery` once per second.
    let (xray_api, _) = npga::stats::api_ports(settings.socks_port).unwrap();
    let poller = npga::stats::StatsPoller::xray(xray_api, paths.root.join("bin/xray.exe"));

    // Push some traffic through the proxy so the counters are non-zero.
    let mut socks = TcpStream::connect(("127.0.0.1", settings.socks_port)).unwrap();
    socks
        .set_read_timeout(Some(Duration::from_secs(4)))
        .unwrap();
    socks.write_all(&[5, 1, 0]).unwrap();
    let mut greeting = [0; 2];
    socks.read_exact(&mut greeting).unwrap();
    assert_eq!(greeting, [5, 0]);
    socks
        .write_all(&[5, 1, 0, 1, 203, 0, 113, 5, 1, 187])
        .unwrap();
    let mut response = [0; 10];
    socks.read_exact(&mut response).unwrap();
    assert_eq!(response[1], 0);
    let payload = vec![0x5Au8; 4096];
    socks.write_all(&payload).unwrap();
    let mut echoed = vec![0u8; payload.len()];
    socks.read_exact(&mut echoed).unwrap();
    assert_eq!(echoed, payload);
    drop(socks);

    // The counters may lag behind until the core flushes them.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut counted = None;
    let mut last_error = String::new();
    while Instant::now() < deadline {
        match poller.xray_stats_over_grpc(xray_api) {
            Ok(value) => {
                counted = Some(value);
                if value.0 > 0 || value.1 > 0 {
                    break;
                }
            }
            Err(e) => last_error = format!("{e:#}"),
        }
        thread::sleep(Duration::from_millis(250));
    }

    if counted.is_none() {
        // Distinguish a broken client from an environment that filters
        // cleartext HTTP/2 traffic (AdGuard and similar products answer
        // loopback h2c requests with a block page).
        if h2c_is_intercepted(xray_api) {
            println!(
                "跳过：本机的过滤软件拦截了回环 h2c 请求，无法验证 Xray gRPC 统计查询。原始错误：{last_error}"
            );
            worker.send(CommandMessage::Stop).unwrap();
            wait(&worker, |e| matches!(e, Event::Stopped));
            drop(worker);
            return;
        }
        panic!("Xray gRPC 统计查询失败：{last_error}");
    }
    let (up, down) = counted.unwrap();
    assert!(
        up > 0 || down > 0,
        "gRPC 统计应反映已传输的流量，收到 up={up} down={down}"
    );

    worker.send(CommandMessage::Stop).unwrap();
    wait(&worker, |e| matches!(e, Event::Stopped));
    drop(worker);
}

#[test]
#[ignore = "requires bundled xray; no external network or routing changes"]
fn occupied_port_fails_without_killing_other_processes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let node = Node {
        protocol: "SOCKS".into(),
        hostname: "127.0.0.1".into(),
        port: 1080,
        ..Node::default()
    };
    let mut settings = settings(node);
    settings.socks_port = listener.local_addr().unwrap().port();
    let worker = Worker::new(paths());
    worker
        .send(CommandMessage::Start {
            settings: Box::new(settings),
            mode: None,
        })
        .unwrap();
    loop {
        let event = worker.events.recv_timeout(Duration::from_secs(12)).unwrap();
        if let Event::Failed(message) = event {
            assert!(message.contains("端口已被占用"));
            break;
        }
    }
    assert!(listener.local_addr().is_ok());
}

/// The stats API attachments (Xray `api`/`stats` + dokodemo-door, sing-box
/// `experimental.clash_api`) must produce configurations the pinned cores
/// accept, otherwise traffic monitoring would break connection startup.
#[test]
#[ignore = "requires pinned bundled proxy cores; run with NPGA_RUNTIME_ROOT"]
fn stats_api_attachments_validate_against_real_cores() {
    let paths = paths();
    fs::create_dir_all(&paths.data).unwrap();

    // Xray: VLESS node plus the StatsService API listener.
    let mut xray_node = Node {
        protocol: "VLESS".into(),
        hostname: "127.0.0.1".into(),
        port: 443,
        ..Node::default()
    };
    xray_node.set("UserID", "550e8400-e29b-41d4-a716-446655440000");
    let xray_settings = settings(xray_node.clone());
    let (xray_api, sing_api) = npga::stats::api_ports(xray_settings.socks_port).unwrap();
    let mut config = core_config::xray(&xray_node, &xray_settings).unwrap();
    let inbounds_before = config["inbounds"].as_array().unwrap().len();
    npga::stats::attach_xray_stats(&mut config, xray_api).unwrap();
    // The core hosts the API itself, so no extra inbound and no routing rule
    // are needed any more.
    assert_eq!(config["api"]["tag"], "api");
    assert_eq!(config["api"]["listen"], format!("127.0.0.1:{xray_api}"));
    assert_eq!(config["api"]["services"][0], "StatsService");
    assert!(config["stats"].is_object());
    assert_eq!(
        config["inbounds"].as_array().unwrap().len(),
        inbounds_before,
        "统计接口不应再额外增加入站"
    );
    let file = paths.data.join("xray-stats.json");
    fs::write(&file, serde_json::to_vec(&config).unwrap()).unwrap();
    let mut command = Command::new(paths.root.join("bin/xray.exe"));
    npga::platform::hidden(&mut command);
    let output = command
        .args(["run", "-test", "-config"])
        .arg(&file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "xray stats: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // sing-box: Hysteria2 node plus the Clash API external controller.
    let mut sing_node = Node {
        protocol: "Hysteria2".into(),
        hostname: "127.0.0.1".into(),
        port: 443,
        ..Node::default()
    };
    sing_node.set("Password", "test-only-secret");
    let sing_settings = settings(sing_node.clone());
    let mut config = core_config::sing_box(&sing_node, &sing_settings).unwrap();
    npga::stats::attach_sing_box_stats(&mut config, sing_api);
    assert_eq!(
        config["experimental"]["clash_api"]["external_controller"],
        format!("127.0.0.1:{sing_api}")
    );
    let file = paths.data.join("sing-stats.json");
    fs::write(&file, serde_json::to_vec(&config).unwrap()).unwrap();
    let mut command = Command::new(paths.root.join("bin/sing-box.exe"));
    npga::platform::hidden(&mut command);
    let output = command.args(["check", "-c"]).arg(&file).output().unwrap();
    assert!(
        output.status.success(),
        "sing-box stats: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for entry in fs::read_dir(&paths.data).unwrap().flatten() {
        if entry.path().is_file() {
            fs::remove_file(entry.path()).unwrap();
        }
    }
    fs::remove_dir(&paths.data).unwrap();
}

/// ADR-002 preparation: the process-mode TUN configuration (process_name
/// routing + scoped DNS hijack) must be accepted by the pinned sing-box.
#[test]
#[ignore = "requires pinned bundled proxy cores; run with NPGA_RUNTIME_ROOT"]
fn process_tun_config_validates_against_real_sing_box() {
    let paths = paths();
    fs::create_dir_all(&paths.data).unwrap();
    let mode = npga::modes::Mode {
        file: "process-mode.json".into(),
        title: "进程模式验证".into(),
        kind: RouteKind::Process,
        handle: vec![
            "^game\\.exe$".into(),
            "^launcher\\.exe$".into(),
            "^.*无法转换的正则.*$".into(),
        ],
        bypass: vec!["^browser\\.exe$".into(), "10.9.0.0/16".into()],
        options: serde_json::Value::Null,
        argument: String::new(),
    };
    let settings = settings(Node {
        protocol: "SOCKS".into(),
        hostname: "127.0.0.1".into(),
        port: 1080,
        ..Node::default()
    });
    let config = core_config::tun_process(&settings, &mode, "203.0.113.7").unwrap();
    let file = paths.data.join("tun-process.json");
    fs::write(&file, serde_json::to_vec(&config).unwrap()).unwrap();
    let mut command = Command::new(paths.root.join("bin/sing-box.exe"));
    npga::platform::hidden(&mut command);
    let output = command.args(["check", "-c"]).arg(&file).output().unwrap();
    assert!(
        output.status.success(),
        "process tun: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    for entry in fs::read_dir(&paths.data).unwrap().flatten() {
        if entry.path().is_file() {
            fs::remove_file(entry.path()).unwrap();
        }
    }
    fs::remove_dir(&paths.data).unwrap();
}

/// Automatic selection must ignore unreachable candidates and connect through
/// the working one, instead of following the user's (broken) selection.
#[test]
#[ignore = "requires bundled xray; no external network or routing changes"]
fn auto_select_skips_failed_candidates() {
    let upstream = MockSocks::start();
    // A closed port fails the probe quickly; the mock SOCKS port answers.
    let dead = TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_port = dead.local_addr().unwrap().port();
    drop(dead);
    let broken = Node {
        protocol: "SOCKS".into(),
        hostname: "127.0.0.1".into(),
        port: dead_port,
        remark: "坏节点".into(),
        ..Node::default()
    };
    let working = Node {
        protocol: "SOCKS".into(),
        hostname: "127.0.0.1".into(),
        port: upstream.port,
        remark: "可用节点".into(),
        ..Node::default()
    };
    let mut settings = Settings {
        nodes: vec![broken, working],
        auto_select: true,
        ..Settings::default()
    };
    settings.selected_node = 0;
    let paths = paths();
    let worker = Worker::new(paths.clone());
    worker
        .send(CommandMessage::Start {
            settings: Box::new(settings.clone()),
            mode: None,
        })
        .unwrap();
    let mut saw_selection = false;
    let deadline = Instant::now() + Duration::from_secs(25);
    while Instant::now() < deadline {
        let event = worker
            .events
            .recv_timeout(Duration::from_secs(25))
            .expect("控制线程超时");
        match event {
            Event::Selection { index, text } => {
                assert!(text.contains("可用节点"), "应选中可用节点: {text}");
                assert!(!text.contains("坏节点"), "不应选中坏节点: {text}");
                // The reported index must address the node that was picked.
                assert_eq!(
                    settings.nodes.get(index).map(|n| n.remark.as_str()),
                    Some("可用节点"),
                    "自动选路应汇报可用节点的下标"
                );
                saw_selection = true;
            }
            Event::Connected { .. } => break,
            Event::Failed(message) => panic!("连接失败: {message}"),
            _ => {}
        }
    }
    assert!(saw_selection, "自动选路应汇报选中的节点");
    // The connection must actually work through the selected node.
    let mut socks = TcpStream::connect(("127.0.0.1", settings.socks_port)).unwrap();
    socks
        .set_read_timeout(Some(Duration::from_secs(4)))
        .unwrap();
    socks.write_all(&[5, 1, 0]).unwrap();
    let mut greeting = [0; 2];
    socks.read_exact(&mut greeting).unwrap();
    assert_eq!(greeting, [5, 0]);
    socks
        .write_all(&[5, 1, 0, 1, 203, 0, 113, 5, 1, 187])
        .unwrap();
    let mut response = [0; 10];
    socks.read_exact(&mut response).unwrap();
    assert_eq!(response[1], 0);
    socks.write_all(b"npga-selector").unwrap();
    let mut echo = [0; 13];
    socks.read_exact(&mut echo).unwrap();
    assert_eq!(&echo, b"npga-selector");
    drop(socks);
    worker.send(CommandMessage::Stop).unwrap();
    wait(&worker, |e| matches!(e, Event::Stopped));
}
