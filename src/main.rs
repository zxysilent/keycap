// Hide console on Windows
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod capture;
mod config;
mod mode;
mod tray;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};
use winit::dpi::PhysicalSize;

use config::Config;
use mode::{DisplayMode, KeyLabel};

struct FileLogger(std::sync::Mutex<std::fs::File>);

impl log::Log for FileLogger {
    fn enabled(&self, m: &log::Metadata) -> bool { m.level() <= log::max_level() }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            use std::io::Write;
            let _ = writeln!(self.0.lock().unwrap(), "{} {}:{} {}", r.level(),
                r.file().unwrap_or("?"), r.line().unwrap_or(0), r.args());
        }
    }
    fn flush(&self) { use std::io::Write; let _ = self.0.lock().unwrap().flush(); }
}

struct App {
    window: Option<Window>,
    mode: Arc<Mutex<Box<dyn DisplayMode>>>,
    rx: crossbeam::channel::Receiver<capture::KeyEvent>,
    tray_rx: crossbeam::channel::Receiver<tray::TrayAction>,
    display_cfg: config::DisplayConfig,
    tray_hidden: Arc<Mutex<bool>>,
    showing: bool,
    needs_hide: bool,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = WindowAttributes::default()
            .with_decorations(false)
            .with_inner_size(PhysicalSize::new(640, 60));
        self.window = Some(event_loop.create_window(attrs).unwrap());
        if let Some(win) = &self.window { hide_from_taskbar(win); }
        event_loop.set_control_flow(ControlFlow::Poll);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Some(win) = &self.window {
                    redraw(win, &self.mode, &self.display_cfg);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        while let Ok(e) = self.rx.try_recv() {
            self.mode.lock().unwrap().push_event(e);
        }
        while let Ok(a) = self.tray_rx.try_recv() {
            use tray::TrayAction;
            match a {
                TrayAction::ShowHide => { *self.tray_hidden.lock().unwrap() ^= true; }
                TrayAction::ToggleMode => {},
                TrayAction::Quit => event_loop.exit(),
            }
        }

        if *self.tray_hidden.lock().unwrap() {
            if self.showing {
                if let Some(win) = &self.window { win.set_visible(false); }
                self.showing = false;
                self.needs_hide = false;
            }
            return;
        }

        let labels = self.mode.lock().unwrap().render(Instant::now());
        let has_content = labels.iter().any(|l| l.opacity > 0.02);

        if has_content {
            self.needs_hide = false;
            if !self.showing {
                if let Some(win) = &self.window { win.set_visible(true); }
                self.showing = true;
            }
            if let Some(win) = &self.window { win.request_redraw(); }
        } else if self.showing {
            // Redraw one frame for fade-out, then hide
            if self.needs_hide {
                if let Some(win) = &self.window { win.set_visible(false); }
                self.showing = false;
                self.needs_hide = false;
            } else {
                self.needs_hide = true;
                if let Some(win) = &self.window { win.request_redraw(); }
            }
        }
    }
}

fn redraw(window: &Window, mode: &Arc<Mutex<Box<dyn DisplayMode>>>, cfg: &config::DisplayConfig) {
    let labels = mode.lock().unwrap().render(Instant::now());
    let size = window.inner_size();
    let w = size.width.max(1);
    let h = size.height.max(1);

    let mut pm = tiny_skia::Pixmap::new(w, h).unwrap();

    // Dark semi-transparent panel background
    let panel_color = parse_color(cfg.bg_color.as_str(), 0.88);
    pm.fill(panel_color);

    let pills: Vec<_> = labels.into_iter().filter(|l| l.opacity > 0.02).collect();
    if !pills.is_empty() {
        draw_pills(&mut pm, &pills, cfg);
    }

    use raw_window_handle::{HasWindowHandle, HasDisplayHandle};
    let wh = window.window_handle().unwrap();
    let dh = window.display_handle().unwrap();
    if let Ok(ctx) = softbuffer::Context::new(dh) {
        if let Ok(mut surface) = softbuffer::Surface::new(&ctx, wh) {
            if let Ok(mut buf) = surface.buffer_mut() {
                let bw = buf.width().get();
                let bh = buf.height().get();
                for y in 0..h.min(bh) {
                    for x in 0..w.min(bw) {
                        let si = ((y * w + x) * 4) as usize;
                        let di = (y * bw + x) as usize;
                        let r = pm.data()[si];
                        let g = pm.data()[si + 1];
                        let b = pm.data()[si + 2];
                        buf[di] = u32::from_ne_bytes([b, g, r, 0]);
                    }
                }
                buf.present().ok();
            }
        }
    }
}

fn draw_pills(pm: &mut tiny_skia::Pixmap, pills: &[KeyLabel], cfg: &config::DisplayConfig) {
    let fs = cfg.font_size as f32;
    let pad_x = fs * 0.55;
    let pad_y = fs * 0.18;
    let radius = fs * 0.42;
    let gap = cfg.key_spacing as f32;
    let margin = cfg.padding as f32;
    let pill_h = fs + pad_y * 2.0;
    let mut x = margin;

    for pill in pills {
        let tw = pill.text.len() as f32 * fs * 0.6;
        let pw = tw + pad_x * 2.0;
        if x + pw > pm.width() as f32 - margin { break; }
        let bg = parse_color(&cfg.bg_color, pill.opacity);
        draw_round_rect(pm, x, margin, pw, pill_h, radius, bg);
        let fg = parse_color(&cfg.text_color, pill.opacity);
        let cw = fs * 0.6;
        for ci in 0..pill.text.len() {
            let cx = x + pad_x + ci as f32 * cw + cw * 0.35;
            let mut pb = tiny_skia::PathBuilder::new();
            pb.move_to(cx, margin + pad_y + fs * 0.2);
            pb.line_to(cx, margin + pad_y + fs * 0.8);
            if let Some(path) = pb.finish() {
                let mut paint = tiny_skia::Paint::default();
                paint.set_color(fg);
                paint.anti_alias = true;
                pm.stroke_path(&path, &paint, &tiny_skia::Stroke::default(), tiny_skia::Transform::identity(), None);
            }
        }
        x += pw + gap;
    }
}

fn draw_round_rect(pm: &mut tiny_skia::Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: tiny_skia::Color) {
    let mut paint = tiny_skia::Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    pm.fill_rect(tiny_skia::Rect::from_xywh(x + r, y, w - 2.0 * r, h).unwrap(), &paint, tiny_skia::Transform::identity(), None);
    pm.fill_rect(tiny_skia::Rect::from_xywh(x, y + r, w, h - 2.0 * r).unwrap(), &paint, tiny_skia::Transform::identity(), None);
    for &(cx, cy) in &[(x + r, y + r), (x + w - r, y + r), (x + r, y + h - r), (x + w - r, y + h - r)] {
        if let Some(circle) = tiny_skia::PathBuilder::from_circle(cx, cy, r) {
            pm.fill_path(&circle, &paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
        }
    }
}

fn parse_color(s: &str, alpha: f32) -> tiny_skia::Color {
    let h = s.trim_start_matches('#');
    let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(255);
    tiny_skia::Color::from_rgba8(r, g, b, (alpha * 255.0) as u8)
}

fn hide_from_taskbar(window: &Window) {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(handle) = window.window_handle() {
            if let RawWindowHandle::Win32(h) = handle.as_raw() {
                unsafe {
                    extern "system" {
                        fn GetWindowLongW(hwnd: isize, index: i32) -> i32;
                        fn SetWindowLongW(hwnd: isize, index: i32, value: i32) -> i32;
                    }
                    const GWL_EXSTYLE: i32 = -20;
                    const WS_EX_TOOLWINDOW: i32 = 0x80;
                    const WS_EX_APPWINDOW: i32 = 0x40000;
                    let hwnd = h.hwnd.get();
                    let ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
                    SetWindowLongW(hwnd, GWL_EXSTYLE, (ex | WS_EX_TOOLWINDOW) & !WS_EX_APPWINDOW);
                }
            }
        }
    }
}

fn main() {
    let mut config = Config::load();
    config.validate();

    let lf = std::fs::OpenOptions::new().create(true).append(true)
        .open(Config::log_path()).expect("log");
    log::set_max_level(log::LevelFilter::Info);
    let _ = log::set_boxed_logger(Box::new(FileLogger(std::sync::Mutex::new(lf))));
    log::info!("keycap starting, mode={}", config.global.mode);

    let rx = capture::start_capture();
    let mode = Arc::new(Mutex::new(mode::create_mode(&config)));
    let display_cfg = config.display.clone();

    let tray_mgr = tray::TrayManager::setup();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App {
        window: None,
        mode: mode.clone(),
        rx: rx.clone(),
        tray_rx: tray_mgr.action_rx.clone(),
        display_cfg,
        tray_hidden: Arc::new(Mutex::new(false)),
        showing: false,
        needs_hide: false,
    };

    event_loop.run_app(&mut app).unwrap();
}
