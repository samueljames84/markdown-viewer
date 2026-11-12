use crate::{
    Event,
    documents::{self, Documents},
    settings::{self, SavedTab, Settings},
    watcher::Watches,
    windows,
};
use ::windows::{
    Win32::{
        System::Com::{CoTaskMemFree, IStream},
        UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, HACCEL, MSG, TranslateAcceleratorW},
    },
    core::{Interface, PWSTR, w},
};
use anyhow::{Context, Result};
use muda::{
    ContextMenu, Menu, MenuItem, Submenu,
    accelerator::{Accelerator, Code, Modifiers},
};
use serde::Deserialize;
use serde_json::json;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use tao::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{Event as TaoEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::windows::{EventLoopBuilderExtWindows, WindowExtWindows},
    window::WindowBuilder,
};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, WebResourceRequestedEventHandler};
use wry::{WebView, WebViewBuilder, WebViewBuilderExtWindows, WebViewExtWindows};

const ORIGIN: &str = "https://mdv.localhost";
const CSP: &str = "default-src 'none'; script-src 'self'; worker-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; font-src 'self'; connect-src 'self'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'";

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Ipc {
    Ready,
    Command {
        command: String,
    },
    Activate {
        id: u64,
    },
    Close {
        id: u64,
    },
    Context {
        id: u64,
    },
    Scroll {
        id: u64,
        scroll: f64,
    },
    Reorder {
        ids: Vec<u64>,
    },
    Preferences {
        theme: String,
        colors: [String; 2],
        zoom: i32,
    },
    Link {
        id: u64,
        href: String,
    },
    PickColor {
        field: String,
        value: String,
    },
    Diagnostic {
        message: String,
    },
    Metric {
        stage: String,
    },
    ExitReady,
}

fn menus() -> Result<Menu> {
    let menu = Menu::new();
    let file = Submenu::new("&File", true);
    let view = Submenu::new("&View", true);
    let search = Submenu::new("&Search", true);
    let help = Submenu::new("&Help", true);
    let ctrl = Modifiers::CONTROL;
    for (id, label, key) in [
        ("open", "&Open File", Some(Code::KeyO)),
        ("open-new", "&New Tab", Some(Code::KeyT)),
        ("close", "&Close Tab", Some(Code::KeyW)),
        ("close-all", "Close All Tabs", None),
        ("associate", "Set as Default for .md...", None),
        ("unassociate", "Remove Association", None),
        ("exit", "E&xit", None),
    ] {
        file.append(&MenuItem::with_id(
            id,
            label,
            true,
            key.map(|k| Accelerator::new(Some(ctrl), k)),
        ))?;
    }
    for (id, label, key, mods) in [
        ("zoom-in", "Zoom In", Code::Equal, ctrl),
        ("zoom-out", "Zoom Out", Code::Minus, ctrl),
        ("zoom-reset", "Reset Zoom", Code::Digit0, ctrl),
    ] {
        view.append(&MenuItem::with_id(
            id,
            label,
            true,
            Some(Accelerator::new(Some(mods), key)),
        ))?;
    }
    let theme = Submenu::new("Theme", true);
    for (id, label) in [
        ("light", "Light"),
        ("dark", "Dark"),
        ("custom", "Custom..."),
    ] {
        theme.append(&MenuItem::with_id(id, label, true, None))?;
    }
    view.append(&theme)?;
    view.append(&MenuItem::with_id(
        "reload",
        "Reload",
        true,
        Some(Accelerator::new(None, Code::F5)),
    ))?;
    for (id, label, mods, code) in [
        ("find", "Find", Some(ctrl), Code::KeyF),
        ("find-next", "Find Next", None, Code::F3),
        (
            "find-previous",
            "Find Previous",
            Some(Modifiers::SHIFT),
            Code::F3,
        ),
    ] {
        search.append(&MenuItem::with_id(
            id,
            label,
            true,
            Some(Accelerator::new(mods, code)),
        ))?;
    }
    help.append(&MenuItem::with_id("about", "About", true, None))?;
    menu.append_items(&[&file, &view, &search, &help])?;
    Ok(menu)
}

fn emit(view: &WebView, value: serde_json::Value) -> Result<()> {
    view.evaluate_script(&format!(
        "window.receive({});",
        serde_json::to_string(&value)?
    ))?;
    Ok(())
}

fn tabs(view: &WebView, docs: &Documents, prefs: &Settings) -> Result<()> {
    emit(
        view,
        json!({"type":"tabs","tabs":docs.tabs,"active":docs.active,"theme":prefs.theme,"colors":prefs.colors,"zoom":prefs.zoom}),
    )
}

fn snapshot(docs: &Documents, prefs: &Settings) -> Settings {
    let mut result = prefs.clone();
    result.tabs = docs
        .tabs
        .iter()
        .map(|t| SavedTab {
            path: t.path.as_os_str().encode_wide().collect(),
            scroll: t.scroll,
        })
        .collect();
    result.active = docs
        .tabs
        .iter()
        .position(|t| Some(t.id) == docs.active)
        .unwrap_or(0);
    result
}

fn protocol(
    request: wry::http::Request<Vec<u8>>,
    docs: &Mutex<Documents>,
) -> Result<wry::http::Response<Vec<u8>>> {
    use wry::http::{HeaderValue, Response, StatusCode};
    anyhow::ensure!(request.method() == "GET", "Only GET is allowed");
    let path = request.uri().path().trim_start_matches('/');
    let (bytes, mime, status) = if let Some(id) = path.strip_prefix("document/") {
        let id: u64 = id.parse()?;
        let file = docs
            .lock()
            .map_err(|_| anyhow::anyhow!("Document lock failed"))?
            .tabs
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.path.clone())
            .context("Document is closed")?;
        match fs::read(file) {
            Ok(bytes) => {
                let (source, legacy) = documents::decode(&bytes);
                (
                    serde_json::to_vec(
                        &json!({"source":source,"warning":if legacy {Some("Decoded using a legacy encoding or replacement characters")} else {None}}),
                    )?,
                    "application/json",
                    StatusCode::OK,
                )
            }
            Err(error) => {
                let missing = error.kind() == std::io::ErrorKind::NotFound;
                (
                    serde_json::to_vec(
                        &json!({"missing":missing,"error":if missing {None} else {Some(error.to_string())}}),
                    )?,
                    "application/json",
                    StatusCode::NOT_FOUND,
                )
            }
        }
    } else if let Some(rest) = path.strip_prefix("image/") {
        let (id, image) = rest.split_once('/').context("Invalid image URL")?;
        let id: u64 = id.parse()?;
        let parent = docs
            .lock()
            .map_err(|_| anyhow::anyhow!("Document lock failed"))?
            .tabs
            .iter()
            .find(|t| t.id == id)
            .and_then(|t| t.path.parent())
            .map(Path::to_path_buf)
            .context("Document is closed")?;
        let file = documents::image_path(&parent, image)?;
        let mime = documents::mime(file.to_str().context("Unsupported image name")?);
        (fs::read(file)?, mime, StatusCode::OK)
    } else {
        let name = if path.is_empty() { "index.html" } else { path };
        (
            documents::asset(name)?.context("Unknown embedded asset")?,
            documents::mime(name),
            StatusCode::OK,
        )
    };
    let mut response = Response::new(bytes);
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert("content-type", HeaderValue::from_static(mime));
    headers.insert("content-security-policy", HeaderValue::from_static(CSP));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("cache-control", HeaderValue::from_static("no-store"));
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    Ok(response)
}

fn restrict_requests(view: &WebView) -> Result<()> {
    unsafe {
        let core = view.webview();
        let environment = view.environment();
        let settings = core.Settings()?;
        settings.SetAreDevToolsEnabled(false)?;
        settings.SetIsZoomControlEnabled(false)?;
        settings.SetIsStatusBarEnabled(false)?;
        if let Ok(settings) = settings.cast::<ICoreWebView2Settings4>() {
            settings.SetIsGeneralAutofillEnabled(false)?;
            settings.SetIsPasswordAutosaveEnabled(false)?;
        }
        core.AddWebResourceRequestedFilter(w!("*"), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL)?;
        let mut token = 0;
        core.add_WebResourceRequested(
            &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let mut uri = PWSTR::null();
                    args.Request()?.Uri(&mut uri)?;
                    let address = uri.to_string()?;
                    CoTaskMemFree(Some(uri.0.cast()));
                    if !address.starts_with(&format!("{ORIGIN}/")) {
                        let response = environment.CreateWebResourceResponse(
                            None::<&IStream>,
                            403,
                            w!("Forbidden"),
                            w!("Content-Type: text/plain"),
                        )?;
                        args.SetResponse(&response)?;
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;
    }
    Ok(())
}

enum Save {
    Later(Settings),
    Now(Settings, mpsc::Sender<std::result::Result<(), String>>),
}

pub fn run() -> Result<()> {
    let started = std::time::Instant::now();
    let dir = settings::directory()?;
    let menu = menus()?;
    let mut builder = EventLoopBuilder::<Event>::with_user_event();
    let accelerator = menu.clone();
    builder.with_msg_hook(move |msg| unsafe {
        let msg = &*(msg as *const MSG);
        TranslateAcceleratorW(
            GetAncestor(msg.hwnd, GA_ROOT),
            HACCEL(accelerator.haccel() as _),
            msg,
        ) != 0
    });
    let event_loop = builder.build();
    let proxy = event_loop.create_proxy();
    let working_directory = std::env::current_dir()?;
    let args: Vec<PathBuf> = std::env::args_os()
        .skip(1)
        .map(|arg| {
            let p = PathBuf::from(arg);
            if p.is_absolute() {
                p
            } else {
                working_directory.join(p)
            }
        })
        .collect();
    let Some(instance) = windows::instance(args.clone(), proxy.clone())? else {
        return Ok(());
    };
    let mut prefs = match settings::load(&dir) {
        Ok(s) => s,
        Err(e) => {
            settings::log(&dir, &format!("{e:#}"));
            rfd::MessageDialog::new()
                .set_title("Settings could not be restored")
                .set_description(format!(
                    "{e:#}\nStarting with default settings. Invalid JSON, if any, was backed up."
                ))
                .show();
            Settings::default()
        }
    };
    let version = match wry::webview_version() {
        Ok(v) => v,
        Err(e) => {
            let result = rfd::MessageDialog::new().set_title("WebView2 runtime is required")
                .set_description(format!("MdViewer uses the Microsoft Edge WebView2 Runtime, which could not be found.\nDownload it from:\nhttps://developer.microsoft.com/microsoft-edge/webview2/\n\n{e}\n\nOpen the download page?"))
                .set_buttons(rfd::MessageButtons::YesNo).show();
            if result == rfd::MessageDialogResult::Yes {
                windows::launch("https://developer.microsoft.com/microsoft-edge/webview2/")?;
            }
            return Ok(());
        }
    };
    let mut documents = Documents::default();
    let mut selected = None;
    for (i, tab) in prefs.tabs.iter().enumerate() {
        let path = PathBuf::from(std::ffi::OsString::from_wide(&tab.path));
        if !path.exists() {
            continue;
        }
        match documents.open(&path, tab.scroll) {
            Ok(id) => {
                if i <= prefs.active || selected.is_none() {
                    selected = Some(id);
                }
            }
            Err(e) => {
                settings::log(&dir, &format!("Restore: {e:#}"));
            }
        }
    }
    documents.active = selected;
    for path in args {
        if let Err(e) = documents.open(&path, 0.0) {
            settings::log(&dir, &format!("{e:#}"));
            rfd::MessageDialog::new()
                .set_title("Cannot open file")
                .set_description(format!("{e:#}"))
                .show();
        }
    }
    let docs = Arc::new(Mutex::new(documents));
    let window = WindowBuilder::new()
        .with_title("MdViewer")
        .with_inner_size(PhysicalSize::new(
            prefs.window.width.clamp(400, 16384),
            prefs.window.height.clamp(250, 16384),
        ))
        .with_min_inner_size(PhysicalSize::new(400, 250))
        .build(&event_loop)?;
    unsafe {
        menu.init_for_hwnd(window.hwnd() as _)?;
    }
    let mut position = PhysicalPosition::new(prefs.window.x, prefs.window.y);
    let visible = window.available_monitors().any(|m| {
        let p = m.position();
        let s = m.size();
        position.x >= p.x
            && position.y >= p.y
            && position.x < p.x + s.width as i32 - 60
            && position.y < p.y + s.height as i32 - 40
    });
    if !visible && let Some(m) = window.primary_monitor() {
        let p = m.position();
        position = PhysicalPosition::new(p.x + 40, p.y + 40);
    }
    window.set_outer_position(position);
    windows::decorate(window.hwnd())?;
    let (left, top, work_width, work_height) = windows::work_area(window.hwnd())?;
    let frame_width = window
        .outer_size()
        .width
        .saturating_sub(window.inner_size().width);
    let frame_height = window
        .outer_size()
        .height
        .saturating_sub(window.inner_size().height);
    let width = work_width.saturating_sub(frame_width).max(1);
    let height = work_height.saturating_sub(frame_height).max(1);
    let scale = if prefs.window.dpi_scale.is_finite() && prefs.window.dpi_scale > 0.0 {
        window.scale_factor() / prefs.window.dpi_scale
    } else {
        1.0
    };
    let restored_width = ((prefs.window.width as f64 * scale) as u32).clamp(400.min(width), width);
    let restored_height =
        ((prefs.window.height as f64 * scale) as u32).clamp(250.min(height), height);
    window.set_inner_size(PhysicalSize::new(restored_width, restored_height));
    window.set_outer_position(PhysicalPosition::new(
        position
            .x
            .clamp(left, left + width.saturating_sub(restored_width) as i32),
        position
            .y
            .clamp(top, top + height.saturating_sub(restored_height) as i32),
    ));
    window.set_maximized(prefs.window.maximized);
    let menu_proxy = proxy.clone();
    muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
        let _ = menu_proxy.send_event(Event::Menu(event.id.0));
    }));
    let protocol_docs = docs.clone();
    let protocol_dir = dir.clone();
    let (requests, receive) =
        mpsc::channel::<(wry::http::Request<Vec<u8>>, wry::RequestAsyncResponder)>();
    let receive = Arc::new(Mutex::new(receive));
    for _ in 0..2 {
        let receive = receive.clone();
        let docs = protocol_docs.clone();
        let log = protocol_dir.clone();
        std::thread::spawn(move || {
            loop {
                let next = match receive.lock() {
                    Ok(rx) => rx.recv(),
                    Err(_) => break,
                };
                let Ok((request, responder)) = next else {
                    break;
                };
                let response = match protocol(request, &docs) {
                    Ok(r) => r,
                    Err(e) => {
                        settings::log(&log, &format!("Resource request failed: {e:#}"));
                        let mut r = wry::http::Response::new(
                            serde_json::to_vec(&json!({"error":format!("{e:#}")}))
                                .unwrap_or_default(),
                        );
                        *r.status_mut() = wry::http::StatusCode::FORBIDDEN;
                        r
                    }
                };
                responder.respond(response);
            }
        });
    }
    let ipc_proxy = proxy.clone();
    let drop_proxy = proxy.clone();
    let profile =
        PathBuf::from(std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA unavailable")?)
            .join("MdViewer\\WebView2");
    let mut context = wry::WebContext::new(Some(profile));
    let mut browser_args = "--disable-background-networking --disable-component-update --disable-sync --no-first-run --disable-features=msSmartScreenProtection,msWebOOUI,msPdfOOUI".to_string();
    if cfg!(debug_assertions)
        && let Ok(port) = std::env::var("MDVIEWER_DEBUG_PORT")
    {
        let port: u16 = port.parse().context("Invalid test debugging port")?;
        browser_args.push_str(&format!(
            " --remote-debugging-port={port} --remote-debugging-address=127.0.0.1"
        ));
    }
    let view = WebViewBuilder::new_with_web_context(&mut context)
        .with_asynchronous_custom_protocol("mdv".into(), move |_, request, responder| {
            let _ = requests.send((request, responder));
        })
        .with_https_scheme(true)
        .with_browser_accelerator_keys(false)
        .with_default_context_menus(false)
        .with_devtools(false)
        .with_additional_browser_args(browser_args)
        .with_navigation_handler(|url| url == format!("{ORIGIN}/index.html"))
        .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
        .with_permission_handler(|_| wry::PermissionResponse::Deny)
        .with_download_started_handler(|_, _| false)
        .with_ipc_handler(move |request| {
            if request.uri().to_string().starts_with(&format!("{ORIGIN}/"))
                && request.body().len() <= 128 * 1024
            {
                let _ = ipc_proxy.send_event(Event::Ipc(request.into_body()));
            }
        })
        .with_drag_drop_handler(move |event| {
            if let wry::DragDropEvent::Drop { paths, .. } = event {
                let files = paths.into_iter().filter(|p| supported(p)).collect();
                let _ = drop_proxy.send_event(Event::Open(files, true));
            }
            true
        })
        .with_url(format!("{ORIGIN}/index.html"))
        .build(&window)?;
    restrict_requests(&view)?;
    let watches = Watches::new(proxy.clone(), dir.clone());
    watches.update(
        docs.lock()
            .map_err(|_| anyhow::anyhow!("Document lock failed"))?
            .tabs
            .iter()
            .map(|t| t.path.clone())
            .collect(),
    );
    let (save_tx, save_rx) = mpsc::channel::<Save>();
    let save_dir = dir.clone();
    let save_proxy = proxy.clone();
    std::thread::spawn(move || {
        let mut pending = None;
        let mut last_error = None;
        loop {
            let message = if pending.is_some() {
                match save_rx.recv_timeout(Duration::from_millis(400)) {
                    Ok(m) => Some(m),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(_) => break,
                }
            } else {
                match save_rx.recv() {
                    Ok(m) => Some(m),
                    Err(_) => break,
                }
            };
            let ack = match message {
                Some(Save::Later(s)) => {
                    pending = Some(s);
                    continue;
                }
                Some(Save::Now(s, ack)) => {
                    pending = Some(s);
                    Some(ack)
                }
                None => None,
            };
            let result = if let Some(s) = pending.take() {
                settings::save(&save_dir, &s)
                    .map_err(|e| format!("Settings could not be saved: {e:#}"))
            } else {
                Ok(())
            };
            if let Err(error) = &result {
                settings::log(&save_dir, error);
                if ack.is_none() && last_error.as_ref() != Some(error) {
                    let _ = save_proxy.send_event(Event::Error(error.clone()));
                }
                last_error = Some(error.clone());
            } else {
                last_error = None;
            }
            if let Some(ack) = ack {
                let _ = ack.send(result);
            }
        }
    });
    settings::log(
        &dir,
        &format!(
            "MdViewer {} / WebView2 {version}",
            env!("CARGO_PKG_VERSION")
        ),
    );
    let mut ready = false;
    let mut context_tab = None;
    let mut exiting = false;
    event_loop.run(move |event, _, control| {
        let _keep_alive = (&instance, &menu, &context);
        *control = ControlFlow::Wait;
        let result = (|| -> Result<()> {
            let mut docs = docs.lock().map_err(|_| anyhow::anyhow!("Document lock failed"))?;
            let mut changed = false;
            match event {
                TaoEvent::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    if ready { emit(&view, json!({"type":"command","command":"exit"}))?; }
                    else { exiting = true; }
                }
                TaoEvent::WindowEvent { event: WindowEvent::Moved(p), .. } => {
                    prefs.window.maximized = window.is_maximized();
                    prefs.window.dpi_scale = window.scale_factor();
                    if !window.is_maximized() && !window.is_minimized() { prefs.window.x = p.x; prefs.window.y = p.y; }
                    changed = true;
                }
                TaoEvent::WindowEvent { event: WindowEvent::Resized(size), .. } => {
                    prefs.window.maximized = window.is_maximized();
                    prefs.window.dpi_scale = window.scale_factor();
                    if !window.is_maximized() && !window.is_minimized() { prefs.window.width = size.width; prefs.window.height = size.height; }
                    changed = true;
                }
                TaoEvent::UserEvent(Event::Error(error)) => {
                    settings::log(&dir, &error);
                    rfd::MessageDialog::new().set_title("MdViewer").set_level(rfd::MessageLevel::Error).set_description(error).show();
                }
                TaoEvent::UserEvent(Event::Changed(paths)) => {
                    if ready {
                        let ids: Vec<_> = docs.tabs.iter().filter(|t| paths.contains(&t.path)).map(|t| t.id).collect();
                        let missing: Vec<_> = docs.tabs.iter().filter(|t| paths.contains(&t.path) && matches!(t.path.try_exists(), Ok(false))).map(|t| t.id).collect();
                        emit(&view, json!({"type":"invalidate","ids":ids,"missing":missing}))?;
                    }
                }
                TaoEvent::UserEvent(Event::Open(paths, focus)) => {
                    for path in paths { if let Err(e) = docs.open(&path, 0.0) { let _ = proxy.send_event(Event::Error(format!("{e:#}"))); } }
                    if focus { window.set_minimized(false); window.set_focus(); windows::attention(window.hwnd()); }
                    if ready { tabs(&view, &docs, &prefs)?; }
                    changed = true;
                }
                TaoEvent::UserEvent(Event::Ipc(text)) => {
                    match serde_json::from_str::<Ipc>(&text).context("Invalid application message")? {
                        Ipc::Ready => { ready = true; tabs(&view, &docs, &prefs)?; changed = true; }
                        Ipc::Command { command } => { let _ = proxy.send_event(Event::Menu(command)); }
                        Ipc::Activate { id } => { if docs.tabs.iter().any(|t| t.id == id) { docs.active = Some(id); tabs(&view, &docs, &prefs)?; changed = true; } }
                        Ipc::Close { id } => { docs.close(id); tabs(&view, &docs, &prefs)?; changed = true; }
                        Ipc::Scroll { id, scroll } => { if scroll.is_finite() && let Some(tab) = docs.tabs.iter_mut().find(|t| t.id == id) { tab.scroll = scroll.max(0.0); changed = true; } }
                        Ipc::Reorder { ids } => {
                            let existing: std::collections::HashSet<_> = docs.tabs.iter().map(|t| t.id).collect();
                            anyhow::ensure!(ids.len() == existing.len() && ids.iter().copied().collect::<std::collections::HashSet<_>>() == existing, "Invalid tab order");
                            docs.tabs.sort_by_key(|t| ids.iter().position(|id| *id == t.id));
                            tabs(&view, &docs, &prefs)?; changed = true;
                        }
                        Ipc::Preferences { theme, colors, zoom } => {
                            anyhow::ensure!(["light","dark","custom"].contains(&theme.as_str()) && (50..=300).contains(&zoom) && zoom % 10 == 0, "Invalid preferences");
                            anyhow::ensure!(colors.iter().all(|c| c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|b| b.is_ascii_hexdigit())), "Invalid colors");
                            prefs.theme = theme; prefs.colors = colors; prefs.zoom = zoom; changed = true;
                        }
                        Ipc::Context { id } => {
                            if docs.tabs.iter().any(|t| t.id == id) {
                                context_tab = Some(id);
                                let popup = Menu::new();
                                for (id, label) in [("context-close","Close"),("context-others","Close Others"),("context-right","Close Tabs to the Right"),("context-copy","Copy Full Path"),("context-folder","Open Containing Folder")] {
                                    popup.append(&MenuItem::with_id(id,label,true,None))?;
                                }
                                unsafe { popup.show_context_menu_for_hwnd(window.hwnd() as _, None); }
                            }
                        }
                        Ipc::Link { id, href } => {
                            if href.to_ascii_lowercase().starts_with("https://") || href.to_ascii_lowercase().starts_with("http://") {
                                let url = url::Url::parse(&href)?; windows::launch(url.as_str())?;
                            } else {
                                anyhow::ensure!(!href.contains(':') && !href.starts_with(['/', '\\']), "Unsupported link");
                                let (file, anchor) = href.split_once('#').unwrap_or((&href, ""));
                                let decoded = percent_encoding::percent_decode_str(file).decode_utf8()?;
                                let base = docs.tabs.iter().find(|t| t.id == id).and_then(|t| t.path.parent()).context("Document is closed")?.to_path_buf();
                                let path = base.join(decoded.replace('/', "\\"));
                                anyhow::ensure!(supported(&path), "Only Markdown/text file links can be opened");
                                let opened = docs.open(&path, 0.0)?; tabs(&view,&docs,&prefs)?;
                                emit(&view, json!({"type":"anchor","id":opened,"anchor":anchor}))?; changed = true;
                            }
                        }
                        Ipc::PickColor { field, value } => {
                            if let Some(color) = windows::color(window.hwnd(), &value)? { emit(&view, json!({"type":"color","field":field,"value":color}))?; }
                        }
                        Ipc::Diagnostic { message } => { settings::log(&dir,&format!("Renderer: {message}")); }
                        Ipc::Metric { stage } => { settings::log(&dir,&format!("METRIC {stage} {} ms",started.elapsed().as_millis())); }
                        Ipc::ExitReady => exiting = true,
                    }
                }
                TaoEvent::UserEvent(Event::Menu(command)) => {
                    match command.as_str() {
                        "open" | "open-new" => {
                            if let Some(files) = rfd::FileDialog::new().set_title("Open Markdown files").add_filter("Markdown and text", &["md","markdown","mdown","txt"]).add_filter("All files",&["*"]).pick_files() {
                                let _ = proxy.send_event(Event::Open(files, true));
                            }
                        }
                        "about" => { rfd::MessageDialog::new().set_title("About MdViewer").set_description(format!("MdViewer {}\nView-only, offline Markdown viewer\nWebView2 {version}",env!("CARGO_PKG_VERSION"))).show(); }
                        "associate" => windows::association(true)?,
                        "unassociate" => windows::association(false)?,
                        "context-close" | "context-others" | "context-right" => {
                            if let Some(id) = context_tab.take() {
                                let index = docs.tabs.iter().position(|t| t.id == id);
                                let close: Vec<_> = docs.tabs.iter().enumerate().filter(|(i,t)| match command.as_str() {
                                    "context-close" => t.id == id, "context-others" => t.id != id, _ => index.is_some_and(|n| *i > n),
                                }).map(|(_,t)| t.id).collect();
                                for id in close { docs.close(id); }
                                tabs(&view,&docs,&prefs)?; changed = true;
                            }
                        }
                        "context-copy" => { if let Some(t) = docs.tabs.iter().find(|t| Some(t.id) == context_tab) { windows::clipboard(window.hwnd(), &t.display)?; } }
                        "context-folder" => { if let Some(t) = docs.tabs.iter().find(|t| Some(t.id) == context_tab) { windows::launch(t.path.parent().context("No containing folder")?)?; } }
                        _ => {
                            anyhow::ensure!(["close","close-all","exit","zoom-in","zoom-out","zoom-reset","light","dark","custom","reload","find","find-next","find-previous"].contains(&command.as_str()),"Unknown command");
                            if ready { emit(&view,json!({"type":"command","command":command}))?; }
                        }
                    }
                }
                _ => {}
            }
            if changed {
                watches.update(docs.tabs.iter().map(|t| t.path.clone()).collect());
                save_tx.send(Save::Later(snapshot(&docs,&prefs)))?;
            }
            if exiting {
                let (tx,rx) = mpsc::channel();
                save_tx.send(Save::Now(snapshot(&docs,&prefs),tx))?;
                let saved = rx.recv_timeout(Duration::from_secs(5)).map_err(|e| format!("Settings did not finish saving: {e}")).and_then(|r| r);
                let exit = match saved {
                    Ok(()) => true,
                    Err(error) => {
                        settings::log(&dir, &error);
                        rfd::MessageDialog::new().set_title("Settings not saved")
                            .set_description(format!("{error}\n\nExit without saving the session?"))
                            .set_buttons(rfd::MessageButtons::YesNo).show() == rfd::MessageDialogResult::Yes
                    }
                };
                exiting = false;
                if exit { *control = ControlFlow::Exit; }
            }
            Ok(())
        })();
        if let Err(error) = result { let _ = proxy.send_event(Event::Error(format!("{error:#}"))); }
    });
}

fn supported(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        ["md", "markdown", "mdown", "txt"].contains(&s.to_ascii_lowercase().as_str())
    })
}
