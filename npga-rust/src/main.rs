#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ui;
use npga::{config::Store, runtime::RuntimePaths};
use std::path::PathBuf;

fn run() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let mut data = None;
    let mut root = None;
    let mut screenshot = None;
    let mut diagnostics = false;
    let mut parent = None;
    let mut view = "connect".to_owned();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-dir" => {
                data = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--data-dir 需要路径"))?,
                ))
            }
            "--runtime-root" => {
                root = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--runtime-root 需要路径"))?,
                ))
            }
            "--screenshot" => {
                screenshot =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        anyhow::anyhow!("--screenshot 需要 PNG 路径")
                    })?))
            }
            "--diagnostics" => diagnostics = true,
            "--view" => {
                view = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--view 需要页面名称"))?
            }
            "--wait-for-parent" => {
                parent = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("缺少父进程 PID"))?
                        .parse::<u32>()?,
                )
            }
            "--help" => {
                println!(
                    "NPGA-Rust [--data-dir PATH] [--runtime-root PATH] [--screenshot PNG] [--diagnostics]"
                );
                return Ok(());
            }
            _ => anyhow::bail!("未知参数：{arg}"),
        }
    }
    if let Some(pid) = parent {
        npga::platform::wait_for_parent(pid)?;
    }
    let paths = RuntimePaths::resolve(&std::env::current_exe()?, root, data);
    if diagnostics {
        println!(
            "NPGA Rust 3.0.0\n运行目录：{}\n数据目录：{}",
            paths.root.display(),
            paths.data.display()
        );
        for (name, ok) in paths.diagnostics() {
            println!("{} {name}", if ok { "OK" } else { "MISSING" });
        }
        let (modes, errors) = npga::modes::scan(&paths.modes());
        println!("模式：{}，读取错误：{}", modes.len(), errors.len());
        return Ok(());
    }
    #[cfg(windows)]
    let _instance = npga::platform::SingleInstance::acquire()?;
    let store = Store::new(paths.data.clone());
    let loaded = npga::platform::recover_proxy(&paths.data.join("system-proxy.dpapi")).and_then(
        |recovered| {
            let (settings, warning) = store.load()?;
            Ok((
                settings,
                warning.or_else(|| recovered.then(|| "已恢复上次异常退出前的系统代理设置".into())),
            ))
        },
    );
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1080.0, 780.0])
            .with_min_inner_size([980.0, 680.0])
            .with_decorations(false)
            .with_icon(ui::native_icon())
            .with_title("NPGA · Netch.Plus Game Accelerator"),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "NPGA",
        options,
        Box::new(move |cc| {
            Ok(Box::new(ui::App::new(
                cc, paths, store, loaded, screenshot, &view,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("窗口启动失败：{e}"))
}
fn main() {
    if let Err(error) = run() {
        let message = format!("{error:#}");
        eprintln!("{message}");
        #[cfg(windows)]
        unsafe {
            #[link(name = "user32")]
            unsafe extern "system" {
                fn MessageBoxW(
                    window: *mut std::ffi::c_void,
                    text: *const u16,
                    title: *const u16,
                    kind: u32,
                ) -> i32;
            }
            let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
            let title: Vec<u16> = "NPGA Rust".encode_utf16().chain(Some(0)).collect();
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), 0x10);
        }
        std::process::exit(1);
    }
}
