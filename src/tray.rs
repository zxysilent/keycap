// ── Linux tray (ksni) ─────────────────────────────────

#[cfg(target_os = "linux")]
mod linux_tray {
    use ksni::{self, MenuItem, ToolTip, Tray, TrayService, menu::{Disposition, StandardItem}};
    use log::info;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TrayAction { ShowHide, ToggleMode, Quit }

    pub struct KeycapTray {
        pub visible: Arc<Mutex<bool>>,
        pub action_tx: crossbeam::channel::Sender<TrayAction>,
    }

    impl Tray for KeycapTray {
        fn id(&self) -> String { "keycap".into() }
        fn icon_name(&self) -> String { "input-keyboard".into() }
        fn title(&self) -> String { "Keycap".into() }
        fn tool_tip(&self) -> ToolTip {
            let v = *self.visible.lock().unwrap();
            ToolTip { title: if v { "Keycap — visible" } else { "Keycap — hidden" }.into(), ..Default::default() }
        }
        fn menu(&self) -> Vec<MenuItem<Self>> {
            let v = *self.visible.lock().unwrap();
            let t1 = self.action_tx.clone();
            let t2 = self.action_tx.clone();
            let t3 = self.action_tx.clone();
            vec![
                MenuItem::Standard(StandardItem {
                    label: if v { "Hide" } else { "Show" }.into(),
                    enabled: true, visible: true, icon_name: String::new(),
                    icon_data: vec![], shortcut: vec![], disposition: Disposition::Normal,
                    activate: Box::new(move |_|{let _=t1.send(TrayAction::ShowHide);}),
                }),
                MenuItem::Sepatator,
                MenuItem::Standard(StandardItem {
                    label: "Switch Mode".into(),
                    enabled: true, visible: true, icon_name: String::new(),
                    icon_data: vec![], shortcut: vec![], disposition: Disposition::Normal,
                    activate: Box::new(move |_|{let _=t2.send(TrayAction::ToggleMode);}),
                }),
                MenuItem::Sepatator,
                MenuItem::Standard(StandardItem {
                    label: "Quit".into(),
                    enabled: true, visible: true, icon_name: String::new(),
                    icon_data: vec![], shortcut: vec![], disposition: Disposition::Normal,
                    activate: Box::new(move |_|{let _=t3.send(TrayAction::Quit);}),
                }),
            ]
        }
        fn activate(&mut self, _x: i32, _y: i32) { let _ = self.action_tx.send(TrayAction::ShowHide); }
    }

    pub fn spawn_tray(v: Arc<Mutex<bool>>) -> (ksni::Handle<KeycapTray>, crossbeam::channel::Receiver<TrayAction>) {
        let (tx, rx) = crossbeam::channel::bounded(8);
        let t = KeycapTray { visible: v, action_tx: tx };
        let s = TrayService::new(t);
        let h = s.handle();
        s.spawn();
        info!("tray: ksni started");
        (h, rx)
    }
}

// ── Non-Linux tray (tray-icon on main thread) ──────────

#[cfg(not(target_os = "linux"))]
mod other_tray {
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TrayAction { ShowHide, ToggleMode, Quit }

    fn make_icon() -> tray_icon::Icon {
        let w = 32;
        let mut p = vec![0u8; w * w * 4];
        for y in 0..w {
            for x in 0..w {
                let i = (y * w + x) * 4;
                let body = x >= 3 && x < 29 && y >= 8 && y < 24;
                if body {
                    p[i] = 40; p[i+1] = 44; p[i+2] = 52; p[i+3] = 255;
                }
                let key = body && ((y >= 11 && y < 15) || (y >= 17 && y < 21)) && (x % 4 != 0);
                if key {
                    p[i] = 220; p[i+1] = 220; p[i+2] = 220; p[i+3] = 255;
                }
            }
        }
        tray_icon::Icon::from_rgba(p, w as u32, w as u32)
            .unwrap_or_else(|e| { log::error!("icon creation: {e}"); std::process::exit(1) })
    }

    pub fn spawn_tray(
        _visible: Arc<Mutex<bool>>,
    ) -> (tray_icon::TrayIcon, crossbeam::channel::Receiver<TrayAction>) {
        use tray_icon::{TrayIconBuilder, menu::{Menu, MenuItem, PredefinedMenuItem, MenuEvent}};

        let (tx, rx) = crossbeam::channel::bounded(8);

        let show = MenuItem::new("Show/Hide", true, None);
        let mode = MenuItem::new("Switch Mode", true, None);
        let quit = MenuItem::new("Quit", true, None);

        let show_id = show.id().0.clone();
        let mode_id = mode.id().0.clone();
        let quit_id = quit.id().0.clone();

        let menu = Menu::new();
        menu.append(&show).ok();
        menu.append(&PredefinedMenuItem::separator()).ok();
        menu.append(&mode).ok();
        menu.append(&PredefinedMenuItem::separator()).ok();
        menu.append(&quit).ok();

        // Allow GPUI message pump to fully initialize on Windows
        // before creating the tray icon (tray-icon relies on the pump).
        std::thread::sleep(std::time::Duration::from_millis(800));

        let icon = make_icon();
        let tray = match TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Keycap")
            .with_menu(Box::new(menu))
            .build()
        {
            Ok(t) => t,
            Err(e) => { log::error!("tray build failed: {e}"); std::process::exit(1); }
        };

        // MenuEvent is a global channel — poll from event loop thread
        std::thread::spawn(move || loop {
            if let Ok(ev) = MenuEvent::receiver().recv() {
                let id = &ev.id.0;
                if *id == show_id { let _ = tx.send(TrayAction::ShowHide); }
                else if *id == mode_id { let _ = tx.send(TrayAction::ToggleMode); }
                else if *id == quit_id { let _ = tx.send(TrayAction::Quit); }
            }
        });

        log::info!("tray: tray-icon started");
        (tray, rx)
    }
}

// ── Common ──────────────────────────────────────────────

#[cfg(target_os = "linux")]
pub use linux_tray::TrayAction;
#[cfg(not(target_os = "linux"))]
pub use other_tray::TrayAction;

pub struct TrayManager {
    #[allow(dead_code)]
    pub action_rx: crossbeam::channel::Receiver<TrayAction>,
    #[cfg(target_os = "linux")]
    _svc: ksni::Handle<linux_tray::KeycapTray>,
    #[cfg(not(target_os = "linux"))]
    _tray: tray_icon::TrayIcon,
}

impl TrayManager {
    pub fn setup() -> Self {
        let visible = std::sync::Arc::new(std::sync::Mutex::new(true));
        #[cfg(target_os = "linux")]
        {
            let (h, rx) = linux_tray::spawn_tray(visible);
            Self { action_rx: rx, _svc: h }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let (tray, rx) = other_tray::spawn_tray(visible);
            Self { action_rx: rx, _tray: tray }
        }
    }
}
