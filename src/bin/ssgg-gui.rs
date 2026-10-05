//! SteelSeries GG desktop app: a native window around the control panel.
//!
//! Makes sure the background service is running (starting it through systemd, or directly when
//! no user unit is installed), then shows the panel in an embedded web view (WebKitGTK on Linux,
//! WebView2 on Windows). Closing the window leaves the service running, like GG on Windows.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

use steelseries_gg::engine::control::{self, ControlInfo};

/// How long to wait for a freshly started service to open its control API.
const START_TIMEOUT: Duration = Duration::from_secs(10);

fn main() {
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: called first thing in `main`, before any other thread exists, so no other
        // code can be reading the environment concurrently.
        unsafe {
            // WebKitGTK's DMA-BUF renderer shows a blank window on some NVIDIA and older Mesa
            // drivers; the fallback renderer is reliable everywhere.
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    let target = match connect_or_start() {
        Ok((info, ticket)) => Page::Url(info.ticket_url(&ticket)),
        Err(message) => Page::Error(message),
    };

    if let Err(e) = run_window(target) {
        eprintln!("ssgg-gui: {e}");
        std::process::exit(1);
    }
}

enum Page {
    Url(String),
    Error(String),
}

/// Get a one-time panel ticket from the running service, starting the service if needed.
fn connect_or_start() -> Result<(ControlInfo, String), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("could not start the async runtime: {e}"))?;
    let ticket = || runtime.block_on(control::issue_ticket()).ok().flatten();

    if let Some(found) = ticket() {
        return Ok(found);
    }
    start_service()?;
    let deadline = Instant::now() + START_TIMEOUT;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(250));
        if let Some(found) = ticket() {
            return Ok(found);
        }
    }
    Err(
        "The SteelSeries GG service did not start. Run `ssgg daemon` in a terminal to see why, \
         or check `journalctl --user -u ssgg`."
            .to_string(),
    )
}

/// Start the background service: the systemd user unit when installed, else `ssgg daemon`.
fn start_service() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let started = Command::new("systemctl")
            .args(["--user", "start", "ssgg.service"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if started {
            return Ok(());
        }
    }
    let daemon = sibling_binary("ssgg");
    Command::new(&daemon)
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not start {}: {e}", daemon.display()))
}

/// `name` next to this executable, falling back to a `PATH` lookup.
fn sibling_binary(name: &str) -> PathBuf {
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(&file)))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(file))
}

fn error_html(message: &str) -> String {
    let escaped = message.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    format!(
        "<!doctype html><html><head><meta charset=utf-8><style>\
         body{{background:#121214;color:#ececf1;font:15px system-ui,sans-serif;display:grid;place-items:center;height:100vh;margin:0}}\
         div{{max-width:560px;padding:28px;border:1px solid #2e2e35;border-radius:12px;background:#1b1b1f}}\
         h1{{font-size:20px;margin:0 0 12px}}code{{background:#232328;padding:2px 6px;border-radius:5px}}\
         </style></head><body><div><h1>SteelSeries GG</h1><p>{escaped}</p></div></body></html>"
    )
}

fn run_window(page: Page) -> Result<(), String> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("SteelSeries GG")
        .with_inner_size(LogicalSize::new(1180.0, 820.0))
        .with_min_inner_size(LogicalSize::new(820.0, 560.0))
        .build(&event_loop)
        .map_err(|e| format!("could not open a window: {e}"))?;

    let builder = match &page {
        Page::Url(url) => WebViewBuilder::new().with_url(url),
        Page::Error(message) => WebViewBuilder::new().with_html(error_html(message)),
    };

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let webview = builder.build(&window);
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window
            .default_vbox()
            .ok_or_else(|| "the window has no GTK container".to_string())?;
        builder.build_gtk(vbox)
    };
    let webview = webview.map_err(|e| format!("could not create the web view: {e}"))?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        // Keep the web view alive for the lifetime of the loop.
        let _ = &webview;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}
