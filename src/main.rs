// Im Release-Build unter Windows kein zusätzliches Konsolenfenster öffnen
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod ext;
mod islands;
mod store;

use islands::TabIslands;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Command;
use store::Store;
use tao::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    keyboard::{KeyCode, ModifiersState},
    window::{Fullscreen, Window, WindowBuilder},
};
#[cfg(target_os = "windows")]
use wry::WebViewBuilderExtWindows;
use wry::{
    dpi::{LogicalPosition, LogicalSize},
    PageLoadEvent, Rect, WebView, WebViewBuilder,
};

const TITLEBAR_HEIGHT: f64 = 28.0;
const CHROME_HEIGHT: f64 = 116.0; // Tab-Leiste (40) + Werkzeugleiste (44) + Lesezeichenleiste (32)
const UI_HEIGHT: f64 = TITLEBAR_HEIGHT + CHROME_HEIGHT;
const SIDEBAR_W: f64 = 52.0; // schmale Seitenleiste links
const DEFAULT_PANEL_W: f64 = 380.0; // Standardbreite des App-Panels
const SP_MIN: f64 = 280.0; // kleinste Panelbreite
const SP_STEP: f64 = 60.0; // Schrittweite der ＋/－-Knöpfe
const SPLIT_W: f64 = 8.0; // Breite des Ziehgriffs
const MIN_CONTENT: f64 = 320.0; // so viel Platz bleibt mindestens für die Seite
const MAX_TABS: usize = 12; // Tab-Limiter (RAK hält sich schlank)

// Läuft auch in externen Webseiten. Die IPC-Schnittstelle akzeptiert aus
// Browser-Tabs ausschließlich diese vier whitelisted Shortcut-Nachrichten.
const GLOBAL_SHORTCUT_SCRIPT: &str = r#"
(function () {
  if (window.__rakShortcutsInstalled) return;
  window.__rakShortcutsInstalled = true;
  window.addEventListener('keydown', function (event) {
    if (event.repeat) return;
    var command = null;
    var control = event.ctrlKey || event.metaKey;
    if (event.key === 'F11') command = 'f11';
    else if (control && !event.shiftKey && event.key.toLowerCase() === 't') command = 'ctrl-t';
    else if (control && !event.shiftKey && event.key.toLowerCase() === 'w') command = 'ctrl-w';
    else if (control && event.shiftKey && event.key.toLowerCase() === 'n') command = 'ctrl-shift-n';
    if (command && window.ipc && window.ipc.postMessage) {
      event.preventDefault();
      event.stopImmediatePropagation();
      window.ipc.postMessage('shortcut:' + command);
    }
  }, true);
})();
"#;

/// Alle Apps, die in der Seitenleiste stehen können: (id, Adresse)
const APPS: [(&str, &str); 3] = [
    ("spotify", "https://open.spotify.com"),
    ("instagram", "https://www.instagram.com"),
    ("x", "https://x.com"),
];

#[derive(Debug)]
enum UserEvent {
    /// Kurzbefehle von der Oberfläche, der Seitenleiste und dem Ziehgriff ("befehl:argument")
    Ipc(String),
    /// JSON-Nachrichten der internen Seiten (Verlauf, Lesezeichen)
    Page(String),
    Title(u32, String),
    Started(u32, String),
    Loaded(u32, String),
    /// Popup / target=_blank einer Webseite -> neuer Tab
    OpenUrl {
        url: String,
        source: Option<u32>,
    },
    /// Ergebnis einer Erweiterungs-Installation (JSON), kommt aus einem Hintergrund-Thread
    Ext(String),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Web,
    Home,
    History,
    Bookmarks,
    Extensions,
    Settings,
}

struct Tab {
    id: u32,
    title: String,
    url: String,
    view: WebView,
    private: bool,
    kind: Kind,
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(w.max(1.0), h.max(1.0)).into(),
    }
}

fn window_size(window: &Window) -> (f64, f64) {
    let s = window.inner_size();
    let sf = window.scale_factor();
    (s.width as f64 / sf, s.height as f64 / sf)
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Gemeinsamer Startpunkt für alle WebViews. WebView2 verlangt, dass alle WebViews
/// desselben Profils den Erweiterungs-Schalter gleich setzen.
fn new_builder<'a>() -> WebViewBuilder<'a> {
    let b = WebViewBuilder::new();
    #[cfg(target_os = "windows")]
    let b = b.with_browser_extensions_enabled(true);
    b
}

/// Lädt die installierten Erweiterungen (nur Windows; nur an EINER WebView, sonst würden
/// sie bei jedem neuen Tab neu installiert).
fn with_extensions<'a>(b: WebViewBuilder<'a>) -> WebViewBuilder<'a> {
    #[cfg(target_os = "windows")]
    {
        b.with_extensions_path(ext::ext_dir())
    }
    #[cfg(not(target_os = "windows"))]
    {
        b
    }
}

fn is_http(u: &str) -> bool {
    u.starts_with("http://") || u.starts_with("https://")
}

fn normalize_with_engine(input: &str, engine: &str) -> String {
    let t = input.trim();
    if is_http(t) {
        t.to_string()
    } else if t.contains('.') && !t.contains(' ') {
        format!("https://{t}")
    } else {
        let q = percent_encode(t);
        match engine {
            "google" => format!("https://www.google.com/search?q={q}"),
            "ecosia" => format!("https://www.ecosia.org/search?q={q}"),
            "bing" => format!("https://www.bing.com/search?q={q}"),
            _ => format!("https://duckduckgo.com/?q={q}"),
        }
    }
}

fn search_home(engine: &str) -> &'static str {
    match engine {
        "google" => "https://www.google.com",
        "ecosia" => "https://www.ecosia.org",
        "bing" => "https://www.bing.com",
        _ => "https://duckduckgo.com",
    }
}

fn translate_url(url: &str, language: &str) -> String {
    format!(
        "https://translate.google.com/translate?sl=auto&tl={}&u={}",
        percent_encode(language),
        percent_encode(url)
    )
}

/// Panelbreite zwischen Minimum und dem, was das Fenster hergibt
fn clamp_width(width: f64, win_w: f64) -> f64 {
    let max = (win_w - SIDEBAR_W - SPLIT_W - MIN_CONTENT).max(SP_MIN);
    width.clamp(SP_MIN, max)
}

#[cfg(target_os = "linux")]
type Host = gtk::Fixed;
#[cfg(not(target_os = "linux"))]
type Host = ();

/// Hängt eine WebView ins Fenster: unter Linux in einen GTK-Container
/// (klappt auch unter Wayland), sonst als Child-Fenster (Windows/macOS).
#[cfg(target_os = "linux")]
fn attach(b: WebViewBuilder, _window: &Window, host: &Host) -> wry::Result<WebView> {
    use wry::WebViewBuilderExtUnix;
    b.build_gtk(host)
}
#[cfg(not(target_os = "linux"))]
fn attach(b: WebViewBuilder, window: &Window, _host: &Host) -> wry::Result<WebView> {
    b.build_as_child(window)
}

/// Setzt Position und Größe einer WebView. Unter Linux zusätzlich fest im GTK-Container,
/// sonst setzt GTK beim nächsten Neuaufbau die alte Startposition wieder ein.
#[cfg(target_os = "linux")]
fn place(host: &Host, view: &WebView, x: f64, y: f64, w: f64, h: f64) {
    use gtk::prelude::*;
    use wry::WebViewExtUnix;
    let _ = view.set_bounds(rect(x, y, w, h));
    let wv = view.webview();
    host.move_(&wv, x as i32, y as i32);
    wv.set_size_request(w.max(1.0) as i32, h.max(1.0) as i32);
}
#[cfg(not(target_os = "linux"))]
fn place(_host: &Host, view: &WebView, x: f64, y: f64, w: f64, h: f64) {
    let _ = view.set_bounds(rect(x, y, w, h));
}

#[cfg(target_os = "linux")]
fn make_host(window: &Window) -> Host {
    use gtk::prelude::*;
    use tao::platform::unix::WindowExtUnix;
    let vbox = window.default_vbox().expect("keine GTK-VBox");
    let fixed = gtk::Fixed::new();
    vbox.pack_start(&fixed, true, true, 0);
    fixed.show_all();
    fixed
}
#[cfg(not(target_os = "linux"))]
fn make_host(_window: &Window) -> Host {}

struct State {
    host: Host,
    proxy: EventLoopProxy<UserEvent>,
    titlebar: WebView,
    ui: WebView,
    side: WebView,
    split: Option<WebView>,
    panels: HashMap<String, WebView>,
    open: Option<String>,
    panel_w: f64,
    drag_start_w: f64,
    tabs: Vec<Tab>,
    islands: TabIslands,
    next_id: u32,
    active: u32,
    store: Store,
}

impl State {
    // ---------- Anordnung ----------

    fn content_x(&self, win_w: f64) -> f64 {
        let mut x = SIDEBAR_W;
        if self.open.is_some() {
            x += clamp_width(self.panel_w, win_w) + SPLIT_W;
        }
        x
    }

    /// Ordnet alle Bereiche neu an: oben Tabs/Adressleiste/Lesezeichen, links die Seitenleiste,
    /// daneben (optional) das App-Panel mit Ziehgriff, rechts davon die Seiten-Tabs.
    fn layout(&self, window: &Window) {
        let (w, h) = window_size(window);
        let body_h = h - UI_HEIGHT;
        place(&self.host, &self.titlebar, 0.0, 0.0, w, TITLEBAR_HEIGHT);
        place(&self.host, &self.ui, 0.0, TITLEBAR_HEIGHT, w, CHROME_HEIGHT);
        place(&self.host, &self.side, 0.0, UI_HEIGHT, SIDEBAR_W, body_h);
        let pw = clamp_width(self.panel_w, w);
        for (id, view) in &self.panels {
            let shown = self.open.as_deref() == Some(id.as_str());
            place(&self.host, view, SIDEBAR_W, UI_HEIGHT, pw, body_h);
            let _ = view.set_visible(shown);
        }
        if let Some(split) = &self.split {
            place(
                &self.host,
                split,
                SIDEBAR_W + pw,
                UI_HEIGHT,
                SPLIT_W,
                body_h,
            );
            let _ = split.set_visible(self.open.is_some());
        }
        let x = self.content_x(w);
        for t in &self.tabs {
            place(&self.host, &t.view, x, UI_HEIGHT, w - x, body_h);
        }
    }

    fn show_only(&self) {
        for t in &self.tabs {
            let _ = t.view.set_visible(t.id == self.active);
        }
    }

    // ---------- Daten an die Oberfläche schicken ----------

    fn push_tabs(&self) {
        let list: Vec<_> = self
            .tabs
            .iter()
            .map(|t| json!({ "id": t.id, "title": t.title, "url": t.url, "private": t.private, "island": self.islands.island_for(t.id) }))
            .collect();
        let islands: Vec<_> = self
            .islands
            .islands
            .iter()
            .map(|g| {
                json!({
                    "id": g.id, "name": g.name, "color": g.color,
                    "collapsed": g.collapsed, "private": g.private
                })
            })
            .collect();
        let js = format!(
            "window.rak && window.rak.setTabs({}, {}, {});",
            json!(list),
            self.active,
            json!(islands)
        );
        let _ = self.ui.evaluate_script(&js);
    }

    fn push_bookmarks(&self) {
        let js = format!(
            "window.rak && window.rak.setBookmarks({});",
            self.store.bookmarks_json()
        );
        let _ = self.ui.evaluate_script(&js);
    }

    fn push_side(&self) {
        let apps: Vec<_> = APPS
            .iter()
            .map(|(id, _)| json!({ "id": id, "enabled": self.store.settings.sidebar.iter().any(|s| s == id) }))
            .collect();
        let js = format!(
            "window.side && window.side.setState({});",
            json!({ "apps": apps, "open": self.open, "colors": self.store.settings.colors })
        );
        let _ = self.side.evaluate_script(&js);
    }

    fn push_settings(&self) {
        let data = json!({ "settings": self.store.settings });
        let _ = self
            .ui
            .evaluate_script(&format!("window.rak && window.rak.setSettings({});", data));
        for t in self.tabs.iter().filter(|t| t.kind == Kind::Settings) {
            let _ = t
                .view
                .evaluate_script(&format!("window.setData({});", data));
        }
        self.push_side();
    }

    fn push_internal(&self) {
        for t in self.tabs.iter().filter(|t| t.kind != Kind::Web) {
            let data = match t.kind {
                Kind::Home => {
                    json!({ "bookmarks": self.store.bookmarks.len(), "version": env!("CARGO_PKG_VERSION") })
                }
                Kind::History => self.store.history_json(),
                Kind::Extensions => ext::data_json(),
                Kind::Settings => json!({ "settings": self.store.settings }),
                _ => self.store.bookmarks_json(),
            };
            let _ = t
                .view
                .evaluate_script(&format!("window.setData({});", data));
        }
    }

    // ---------- Tabs ----------

    fn make_web_tab(
        &self,
        window: &Window,
        id: u32,
        url: &str,
        private: bool,
    ) -> wry::Result<WebView> {
        let (w, h) = window_size(window);
        let x = self.content_x(w);
        let p_title = self.proxy.clone();
        let p_load = self.proxy.clone();
        let p_new = self.proxy.clone();
        let p_shortcut = self.proxy.clone();
        let builder = new_builder()
            .with_bounds(rect(x, UI_HEIGHT, w - x, h - UI_HEIGHT))
            .with_url(url)
            .with_initialization_script(GLOBAL_SHORTCUT_SCRIPT)
            // Privater Tab: eigener, flüchtiger Speicher (keine Cookies/Cache danach)
            .with_incognito(private)
            .with_document_title_changed_handler(move |t| {
                let _ = p_title.send_event(UserEvent::Title(id, t));
            })
            .with_on_page_load_handler(move |ev, u| {
                let e = match ev {
                    PageLoadEvent::Started => UserEvent::Started(id, u),
                    PageLoadEvent::Finished => UserEvent::Loaded(id, u),
                };
                let _ = p_load.send_event(e);
            })
            .with_new_window_req_handler(move |u| {
                let _ = p_new.send_event(UserEvent::OpenUrl {
                    url: u,
                    source: Some(id),
                });
                false // kein eigenes Fenster, die Adresse öffnet in einem neuen Tab
            })
            .with_ipc_handler(move |req| {
                let body = req.body();
                if body == "shortcut:f11"
                    || body == "shortcut:ctrl-t"
                    || body == "shortcut:ctrl-w"
                    || body == "shortcut:ctrl-shift-n"
                {
                    let _ = p_shortcut.send_event(UserEvent::Ipc(body.to_string()));
                }
            });
        // Externe Seiten dürfen nur die vier oben whitelisted Shortcut-Befehle senden.
        attach(builder, window, &self.host)
    }

    /// Interne Seiten (Verlauf, Lesezeichen) – nur diese dürfen mit dem Browser sprechen.
    fn make_internal(&self, window: &Window, kind: Kind) -> wry::Result<WebView> {
        let (w, h) = window_size(window);
        let x = self.content_x(w);
        let html = match kind {
            Kind::Home => include_str!("home.html").to_string(),
            Kind::History => self.store.history_page(),
            Kind::Extensions => ext::page(),
            Kind::Settings => include_str!("settings.html").replace(
                "__DATA__",
                &store::script_safe(&json!({ "settings": self.store.settings })),
            ),
            _ => self.store.bookmarks_page(),
        };
        let p_ipc = self.proxy.clone();
        let p_nav = self.proxy.clone();
        let builder = new_builder()
            .with_bounds(rect(x, UI_HEIGHT, w - x, h - UI_HEIGHT))
            .with_html(html)
            .with_initialization_script(GLOBAL_SHORTCUT_SCRIPT)
            .with_ipc_handler(move |req| {
                let body = req.body();
                if body.starts_with("shortcut:") {
                    let _ = p_ipc.send_event(UserEvent::Ipc(body.to_string()));
                } else {
                    let _ = p_ipc.send_event(UserEvent::Page(body.to_string()));
                }
            })
            // Eine interne Seite darf nie zu einer Webseite werden (sonst hätte diese IPC-Zugriff)
            .with_navigation_handler(move |u| {
                if is_http(&u) {
                    let _ = p_nav.send_event(UserEvent::OpenUrl {
                        url: u,
                        source: None,
                    });
                    false
                } else {
                    true
                }
            });
        attach(builder, window, &self.host)
    }

    fn new_web_tab(&mut self, window: &Window, url: &str, private: bool) {
        if self.tabs.len() >= MAX_TABS {
            return;
        }
        let id = self.next_id;
        if let Ok(view) = self.make_web_tab(window, id, url, private) {
            self.tabs.push(Tab {
                id,
                title: String::new(),
                url: url.to_string(),
                view,
                private,
                kind: Kind::Web,
            });
            self.next_id += 1;
            self.active = id;
            self.show_only();
            self.layout(window);
        }
    }

    fn tab_metadata(&self) -> Vec<(u32, bool)> {
        self.tabs.iter().map(|t| (t.id, t.private)).collect()
    }

    fn arrange_islands(&mut self) {
        self.islands.arrange(&mut self.tabs, |t| t.id);
    }

    fn new_link_tab(&mut self, window: &Window, url: &str, source: Option<u32>) {
        // Eine verspätete Anfrage eines bereits geschlossenen Tabs wird verworfen.
        let private = match source {
            Some(id) => match self.tabs.iter().find(|t| t.id == id) {
                Some(tab) => tab.private,
                None => return,
            },
            None => false,
        };
        let child = self.next_id;
        self.new_web_tab(window, url, private);
        if self.tabs.iter().any(|t| t.id == child) {
            if let Some(source) = source {
                self.islands.link(source, child, &self.tab_metadata());
                self.islands.expand_tab(child);
                self.arrange_islands();
            }
        }
    }

    fn new_island_tab(&mut self, window: &Window, island: u32) {
        let Some(private) = self.islands.get(island).map(|g| g.private) else {
            return;
        };
        let id = self.next_id;
        self.new_web_tab(
            window,
            search_home(&self.store.settings.search_engine),
            private,
        );
        if self.tabs.iter().any(|t| t.id == id) {
            self.islands.add(island, &[id], &self.tab_metadata());
            self.islands.expand_tab(id);
            self.arrange_islands();
        }
    }

    /// Auch beim Gruppenschließen bleiben sichtbare Seite und aktiver Tab konsistent.
    fn close_tabs(&mut self, window: &Window, ids: &[u32]) -> bool {
        let old_index = self
            .tabs
            .iter()
            .position(|t| t.id == self.active)
            .unwrap_or(0);
        self.tabs.retain(|t| !ids.contains(&t.id));
        self.islands.remove(ids);
        // Der Browser bleibt bei null Tabs geöffnet und zeigt die interne Startseite.
        if self.tabs.is_empty() {
            self.open_internal(window, Kind::Home);
            return false;
        }
        if !self.tabs.iter().any(|t| t.id == self.active) {
            self.active = self.tabs[old_index.min(self.tabs.len() - 1)].id;
            self.islands.expand_tab(self.active);
        }
        self.show_only();
        self.push_internal();
        false
    }

    fn open_internal(&mut self, window: &Window, kind: Kind) {
        if let Some(t) = self.tabs.iter().find(|t| t.kind == kind) {
            self.active = t.id;
            self.islands.expand_tab(self.active);
            self.show_only();
            self.push_internal();
            return;
        }
        if self.tabs.len() >= MAX_TABS {
            return;
        }
        let id = self.next_id;
        if let Ok(view) = self.make_internal(window, kind) {
            let (title, url) = match kind {
                Kind::Home => ("RAK Startseite", "rak://start"),
                Kind::History => ("Verlauf", "rak://verlauf"),
                Kind::Extensions => ("Erweiterungen", "rak://erweiterungen"),
                Kind::Settings => ("Einstellungen", "rak://einstellungen"),
                _ => ("Lesezeichen", "rak://lesezeichen"),
            };
            self.tabs.push(Tab {
                id,
                title: title.into(),
                url: url.into(),
                view,
                private: false,
                kind,
            });
            self.next_id += 1;
            self.active = id;
            self.show_only();
            self.layout(window);
        }
    }

    /// Lädt eine Adresse im aktiven Tab. Ist der aktive Tab eine interne Seite, wird seine
    /// WebView durch eine frische ersetzt, damit die Webseite keinen IPC-Zugang erbt.
    fn navigate_active(&mut self, window: &Window, url: &str) {
        let Some(idx) = self.tabs.iter().position(|t| t.id == self.active) else {
            return;
        };
        if self.tabs[idx].kind == Kind::Web {
            self.tabs[idx].url = url.to_string();
            let _ = self.tabs[idx].view.load_url(url);
        } else {
            let id = self.tabs[idx].id;
            if let Ok(view) = self.make_web_tab(window, id, url, false) {
                let t = &mut self.tabs[idx];
                t.view = view; // die alte interne WebView wird hier freigegeben
                t.kind = Kind::Web;
                t.url = url.to_string();
                t.title = String::new();
                self.layout(window);
            }
        }
    }

    // ---------- App-Panel (Spotify, Instagram, X) ----------

    fn ensure_split(&mut self, window: &Window) {
        if self.split.is_some() {
            return;
        }
        let (_, h) = window_size(window);
        let p = self.proxy.clone();
        let b = new_builder()
            .with_bounds(rect(SIDEBAR_W, UI_HEIGHT, SPLIT_W, h - UI_HEIGHT))
            .with_html(include_str!("split.html"))
            .with_ipc_handler(move |req| {
                let _ = p.send_event(UserEvent::Ipc(req.body().to_string()));
            });
        if let Ok(v) = attach(b, window, &self.host) {
            self.split = Some(v);
        }
    }

    fn toggle_app(&mut self, window: &Window, app: &str) {
        let Some((_, url)) = APPS.iter().find(|(id, _)| *id == app) else {
            return;
        };
        if !self.store.settings.sidebar.iter().any(|s| s == app) {
            return;
        }
        if self.open.as_deref() == Some(app) {
            self.open = None;
        } else {
            if !self.panels.contains_key(app) {
                let (w, h) = window_size(window);
                let b = new_builder()
                    .with_bounds(rect(
                        SIDEBAR_W,
                        UI_HEIGHT,
                        clamp_width(self.panel_w, w),
                        h - UI_HEIGHT,
                    ))
                    .with_url(*url);
                if let Ok(v) = attach(b, window, &self.host) {
                    self.panels.insert(app.to_string(), v);
                } else {
                    return;
                }
            }
            self.ensure_split(window);
            self.open = Some(app.to_string());
        }
        self.layout(window);
        self.push_side();
    }

    fn set_app_enabled(&mut self, window: &Window, app: &str, enabled: bool) {
        if !APPS.iter().any(|(id, _)| *id == app) {
            return;
        }
        let list = &mut self.store.settings.sidebar;
        list.retain(|s| s != app);
        if enabled {
            list.push(app.to_string());
            list.sort_by_key(|s| {
                APPS.iter()
                    .position(|(id, _)| id == s)
                    .unwrap_or(usize::MAX)
            });
        } else {
            // Entfernt = Panel schließen und die Seite beenden (Musik stoppt, Speicher wird frei)
            if self.open.as_deref() == Some(app) {
                self.open = None;
            }
            self.panels.remove(app);
        }
        self.store.save_settings();
        self.layout(window);
        self.push_side();
    }

    fn resize_panel(&mut self, window: &Window, target: f64, save: bool) {
        let (w, _) = window_size(window);
        self.panel_w = clamp_width(target, w);
        if save {
            self.store.settings.panel_width = self.panel_w;
            self.store.save_settings();
        }
        self.layout(window);
    }

    // ---------- Ereignisse ----------

    /// Gibt true zurück, wenn das Programm beendet werden soll.
    fn handle(&mut self, window: &Window, ev: UserEvent) -> bool {
        match ev {
            UserEvent::Title(id, title) => {
                if let Some(t) = self
                    .tabs
                    .iter_mut()
                    .find(|t| t.id == id && t.kind == Kind::Web)
                {
                    t.title = title.clone();
                    if !t.private && is_http(&t.url) {
                        let url = t.url.clone();
                        self.store.retitle_last(&url, &title);
                    }
                }
            }
            UserEvent::Started(id, url) => {
                if let Some(t) = self
                    .tabs
                    .iter_mut()
                    .find(|t| t.id == id && t.kind == Kind::Web)
                {
                    t.url = url;
                    t.title.clear();
                }
            }
            UserEvent::Loaded(id, url) => {
                if self.store.settings.auto_translate
                    && is_http(&url)
                    && !url.starts_with("https://translate.google.com/")
                    && !url.starts_with("http://translate.google.com/")
                {
                    if let Some(t) = self.tabs.iter().find(|t| t.id == id && t.kind == Kind::Web) {
                        let translated = translate_url(&url, &self.store.settings.translate_lang);
                        let _ = t.view.load_url(&translated);
                    }
                }
                if let Some(t) = self
                    .tabs
                    .iter_mut()
                    .find(|t| t.id == id && t.kind == Kind::Web)
                {
                    t.url = url.clone();
                    // Private Tabs kommen nie in den Verlauf
                    if !t.private && is_http(&url) {
                        let title = t.title.clone();
                        self.store.add_history(&url, &title);
                    }
                }
            }
            UserEvent::OpenUrl { url, source } => {
                if is_http(&url) {
                    self.new_link_tab(window, &url, source);
                }
            }
            UserEvent::Page(msg) => {
                if self.page_message(window, &msg) {
                    return true;
                }
            }
            UserEvent::Ext(json) => self.ext_result(&json),
            UserEvent::Ipc(msg) => {
                if self.ipc(window, &msg) {
                    return true;
                }
            }
        }
        self.push_tabs();
        false
    }

    fn ipc(&mut self, window: &Window, msg: &str) -> bool {
        let (cmd, arg) = msg.split_once(':').unwrap_or((msg, ""));
        match cmd {
            "window" => match arg {
                "minimize" => window.set_minimized(true),
                "maximize" => {
                    window.set_maximized(!window.is_maximized());
                    self.layout(window);
                }
                "drag" => {
                    let _ = window.drag_window();
                }
                "close" => return true,
                _ => {}
            },
            "shortcut" => match arg {
                "f11" => {
                    if window.fullscreen().is_some() {
                        window.set_fullscreen(None);
                    } else {
                        window.set_fullscreen(Some(Fullscreen::Borderless(window.current_monitor())));
                    }
                    self.layout(window);
                }
                "ctrl-t" => self.new_web_tab(
                    window,
                    search_home(&self.store.settings.search_engine),
                    false,
                ),
                "ctrl-shift-n" => self.new_web_tab(
                    window,
                    search_home(&self.store.settings.search_engine),
                    true,
                ),
                "ctrl-w" => {
                    let active = self.active;
                    if self.close_tabs(window, &[active]) {
                        return true;
                    }
                }
                _ => {}
            },
            "ready" => {
                self.push_tabs();
                self.push_bookmarks();
                self.push_side();
                self.push_settings();
            }
            "new" => self.new_web_tab(
                window,
                search_home(&self.store.settings.search_engine),
                false,
            ),
            "newp" => self.new_web_tab(
                window,
                search_home(&self.store.settings.search_engine),
                true,
            ),
            "switch" => {
                if let Ok(id) = arg.parse::<u32>() {
                    if self.tabs.iter().any(|t| t.id == id) {
                        self.active = id;
                        self.islands.expand_tab(id);
                        self.show_only();
                        self.push_internal();
                    }
                }
            }
            "close" => {
                if let Ok(id) = arg.parse::<u32>() {
                    if self.close_tabs(window, &[id]) {
                        return true;
                    }
                }
            }
            "island-create" | "island-add" | "island-remove" => {
                if let Ok(v) = serde_json::from_str::<Value>(arg) {
                    let ids: Vec<u32> = v["tabs"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|id| id.as_u64().and_then(|id| u32::try_from(id).ok()))
                        .collect();
                    let tabs = self.tab_metadata();
                    match cmd {
                        "island-create" => {
                            self.islands.create(&ids, &tabs);
                        }
                        "island-add" => {
                            if let Some(id) =
                                v["island"].as_u64().and_then(|id| u32::try_from(id).ok())
                            {
                                self.islands.add(id, &ids, &tabs);
                            }
                        }
                        _ => self.islands.remove(&ids),
                    }
                    self.arrange_islands();
                }
            }
            "island-toggle" | "island-dissolve" | "island-close" | "island-new" => {
                if let Ok(id) = arg.parse::<u32>() {
                    match cmd {
                        "island-toggle" => self.islands.toggle(id),
                        "island-dissolve" => self.islands.dissolve(id),
                        "island-new" => self.new_island_tab(window, id),
                        _ => {
                            let members = self.islands.members(id);
                            if self.close_tabs(window, &members) {
                                return true;
                            }
                        }
                    }
                }
            }
            "island-rename" => {
                if let Ok(v) = serde_json::from_str::<Value>(arg) {
                    if let Some(id) = v["id"].as_u64().and_then(|id| u32::try_from(id).ok()) {
                        self.islands.rename(
                            id,
                            v["name"].as_str().unwrap_or(""),
                            v["color"].as_str().unwrap_or(""),
                        );
                    }
                }
            }
            "nav" => match arg.trim() {
                "rak://start" => self.open_internal(window, Kind::Home),
                "rak://verlauf" => self.open_internal(window, Kind::History),
                "rak://lesezeichen" => self.open_internal(window, Kind::Bookmarks),
                "rak://erweiterungen" => self.open_internal(window, Kind::Extensions),
                "rak://einstellungen" => self.open_internal(window, Kind::Settings),
                other => {
                    let url = normalize_with_engine(other, &self.store.settings.search_engine);
                    self.navigate_active(window, &url);
                }
            },
            "home" => self.open_internal(window, Kind::Home),
            "settings" => self.open_internal(window, Kind::Settings),
            "translate" => {
                if let Some(url) = self
                    .tabs
                    .iter()
                    .find(|t| t.id == self.active && t.kind == Kind::Web)
                    .map(|t| t.url.clone())
                    .filter(|u| is_http(u) && !u.starts_with("https://translate.google.com/"))
                {
                    let translated = translate_url(&url, &self.store.settings.translate_lang);
                    self.navigate_active(window, &translated);
                }
            }
            "back" | "fwd" | "reload" => {
                let js = match cmd {
                    "back" => "history.back()",
                    "fwd" => "history.forward()",
                    _ => "location.reload()",
                };
                if let Some(t) = self
                    .tabs
                    .iter()
                    .find(|t| t.id == self.active && t.kind == Kind::Web)
                {
                    let _ = t.view.evaluate_script(js);
                }
            }
            "hist" => self.open_internal(window, Kind::History),
            "bmgr" => self.open_internal(window, Kind::Bookmarks),
            "ext" => self.open_internal(window, Kind::Extensions),

            // Lesezeichenleiste
            "bmopen" => {
                if is_http(arg) {
                    self.navigate_active(window, arg);
                }
            }
            "bmadd" => {
                if let Ok(v) = serde_json::from_str::<Value>(arg) {
                    if let Some(url) = v["url"].as_str().filter(|u| is_http(u)) {
                        let title = v["title"].as_str().unwrap_or("");
                        let folder = v["folder"].as_u64().map(|f| f as u32);
                        self.store.add_bookmark(title, url, folder);
                        self.bookmarks_changed();
                    }
                }
            }
            "bmdel" => {
                if let Ok(id) = arg.parse::<u32>() {
                    self.store.delete_bookmark(id);
                    self.bookmarks_changed();
                }
            }
            "bmdelurl" => {
                self.store.delete_bookmarks_by_url(arg);
                self.bookmarks_changed();
            }

            // Seitenleiste
            "app" => self.toggle_app(window, arg),
            "sbrm" => self.set_app_enabled(window, arg, false),
            "sbadd" => self.set_app_enabled(window, arg, true),
            "sp+" => {
                if self.open.is_some() {
                    self.resize_panel(window, self.panel_w + SP_STEP, true);
                }
            }
            "sp-" => {
                if self.open.is_some() {
                    self.resize_panel(window, self.panel_w - SP_STEP, true);
                }
            }
            "sp0" => self.resize_panel(window, DEFAULT_PANEL_W, true),
            // Ziehgriff: rs:start / rs:move:<dx> / rs:end
            "rs" => {
                if arg == "start" {
                    self.drag_start_w = self.panel_w;
                } else if arg == "end" {
                    self.store.settings.panel_width = self.panel_w;
                    self.store.save_settings();
                } else if let Some(dx) = arg.strip_prefix("move:") {
                    if let Ok(dx) = dx.parse::<f64>() {
                        self.resize_panel(window, self.drag_start_w + dx, false);
                    }
                }
            }
            _ => {}
        }
        false
    }

    fn bookmarks_changed(&self) {
        self.push_bookmarks();
        self.push_internal();
    }

    fn update_settings(&mut self, v: &Value) {
        if let Some(engine) = v["search_engine"].as_str() {
            if ["google", "duckduckgo", "ecosia", "bing"].contains(&engine) {
                self.store.settings.search_engine = engine.to_string();
            }
        }
        if let Some(value) = v["auto_translate"].as_bool() {
            self.store.settings.auto_translate = value;
        }
        if let Some(lang) = v["translate_lang"].as_str() {
            if lang.len() <= 12 && lang.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                self.store.settings.translate_lang = lang.to_string();
            }
        }
        if let Some(colors) = v["colors"].as_object() {
            for (key, slot) in [
                ("background", &mut self.store.settings.colors.background),
                ("tabs", &mut self.store.settings.colors.tabs),
                ("toolbar", &mut self.store.settings.colors.toolbar),
                ("bookmarks", &mut self.store.settings.colors.bookmarks),
                ("sidebar", &mut self.store.settings.colors.sidebar),
                ("accent", &mut self.store.settings.colors.accent),
                ("text", &mut self.store.settings.colors.text),
            ] {
                if let Some(color) = colors.get(key).and_then(Value::as_str) {
                    if color.len() == 7
                        && color.starts_with('#')
                        && color[1..].bytes().all(|b| b.is_ascii_hexdigit())
                    {
                        *slot = color.to_ascii_lowercase();
                    }
                }
            }
        }
        self.store.save_settings();
        self.push_settings();
    }

    /// Nachrichten der internen Seiten (JSON)
    /// Gibt true zurück, wenn das Programm beendet werden soll (Neustart).
    fn page_message(&mut self, window: &Window, msg: &str) -> bool {
        let Ok(v) = serde_json::from_str::<Value>(msg) else {
            return false;
        };
        let id = v["id"].as_u64().map(|x| x as u32);
        match v["t"].as_str() {
            Some("settings_save") => {
                self.update_settings(&v);
                return false;
            }
            Some("open") => {
                if let Some(u) = v["url"].as_str().filter(|u| is_http(u)) {
                    self.new_web_tab(window, u, false);
                }
            }
            Some("home_open") => {
                if let Some(input) = v["url"].as_str().filter(|u| !u.trim().is_empty()) {
                    let url = normalize_with_engine(input, &self.store.settings.search_engine);
                    self.navigate_active(window, &url);
                }
            }
            Some("home_command") => match v["command"].as_str() {
                Some("new") => self.new_web_tab(
                    window,
                    search_home(&self.store.settings.search_engine),
                    false,
                ),
                Some("newp") => self.new_web_tab(
                    window,
                    search_home(&self.store.settings.search_engine),
                    true,
                ),
                Some("settings") => self.open_internal(window, Kind::Settings),
                _ => {}
            },
            Some("hist_del") => {
                if let (Some(ts), Some(url)) = (v["ts"].as_u64(), v["url"].as_str()) {
                    self.store.delete_history_entry(ts, url);
                }
            }
            Some("hist_clear") => self.store.clear_history(),
            Some("bm_move") => {
                if let Some(id) = id {
                    self.store
                        .move_bookmark(id, v["folder"].as_u64().map(|f| f as u32));
                }
            }
            Some("bm_del") => {
                if let Some(id) = id {
                    self.store.delete_bookmark(id);
                }
            }
            Some("folder_add") => self.store.add_folder(v["name"].as_str().unwrap_or("")),
            Some("folder_ren") => {
                if let Some(id) = id {
                    self.store
                        .rename_folder(id, v["name"].as_str().unwrap_or(""));
                }
            }
            Some("folder_del") => {
                if let Some(id) = id {
                    self.store.delete_folder(id);
                }
            }
            // ---- Erweiterungen ----
            Some("ext_install") => {
                if let Some(input) = v["input"].as_str() {
                    // Der Download blockiert: in einem eigenen Thread, damit der Browser bedienbar bleibt
                    let p = self.proxy.clone();
                    let input = input.to_string();
                    std::thread::spawn(move || {
                        let j = match ext::install(&input) {
                            Ok(i) => {
                                json!({ "ok": true, "name": i.name, "version": i.version, "id": i.id })
                            }
                            Err(e) => json!({ "ok": false, "error": e }),
                        };
                        let _ = p.send_event(UserEvent::Ext(j.to_string()));
                    });
                }
                return false;
            }
            Some("ext_remove") => {
                if let Some(eid) = v["id"].as_str() {
                    if let Err(e) = ext::remove(eid) {
                        self.ext_result(&json!({ "ok": false, "error": e }).to_string());
                    }
                }
                self.push_internal();
                return false;
            }
            Some("ext_restart") => {
                if let Ok(exe) = std::env::current_exe() {
                    if Command::new(exe).spawn().is_ok() {
                        return true;
                    }
                }
                return false;
            }
            _ => return false,
        }
        self.bookmarks_changed();
        false
    }

    /// Meldet das Ergebnis einer Installation an die Erweiterungsseite und aktualisiert die Liste
    fn ext_result(&self, json: &str) {
        for t in self.tabs.iter().filter(|t| t.kind == Kind::Extensions) {
            let _ = t
                .view
                .evaluate_script(&format!("window.extResult && window.extResult({});", json));
        }
        self.push_internal();
    }
}

fn main() -> wry::Result<()> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("RAK")
        .with_decorations(false)
        .with_inner_size(tao::dpi::LogicalSize::new(1280.0, 800.0))
        .build(&event_loop)
        .expect("Fenster konnte nicht erstellt werden");

    let host = make_host(&window);
    let store = Store::load();
    let panel_w = store.settings.panel_width;

    // RAK-Titelleiste liegt außerhalb der Browser-Werkzeugleisten.
    let (w, h) = window_size(&window);
    ext::sanitize(); // kaputte Reste aus \extensions\ entfernen, sonst könnte der Start scheitern
    let title_proxy = proxy.clone();
    let titlebar = attach(
        new_builder()
            .with_bounds(rect(0.0, 0.0, w, TITLEBAR_HEIGHT))
            .with_html(include_str!("titlebar.html"))
            .with_ipc_handler(move |req| {
                let _ = title_proxy.send_event(UserEvent::Ipc(req.body().to_string()));
            }),
        &window,
        &host,
    )?;

    // Oberfläche darunter (Tabs + Adressleiste + Lesezeichenleiste)
    let make_ui = |with_ext: bool| {
        let p = proxy.clone();
        let mut b = new_builder()
            .with_bounds(rect(0.0, TITLEBAR_HEIGHT, w, CHROME_HEIGHT))
            .with_html(include_str!("ui.html"))
            .with_ipc_handler(move |req| {
                let _ = p.send_event(UserEvent::Ipc(req.body().to_string()));
            });
        if with_ext {
            b = with_extensions(b);
        }
        attach(b, &window, &host)
    };
    // Falls eine Erweiterung das Laden verhindert, startet RAK trotzdem (ohne Erweiterungen)
    let ui = match make_ui(true) {
        Ok(v) => v,
        Err(_) => make_ui(false)?,
    };

    // Seitenleiste links
    let side_proxy = proxy.clone();
    let side_builder = new_builder()
        .with_bounds(rect(0.0, UI_HEIGHT, SIDEBAR_W, h - UI_HEIGHT))
        .with_html(include_str!("side.html"))
        .with_ipc_handler(move |req| {
            let _ = side_proxy.send_event(UserEvent::Ipc(req.body().to_string()));
        });
    let side = attach(side_builder, &window, &host)?;

    let mut st = State {
        host,
        proxy,
        titlebar,
        ui,
        side,
        split: None,
        panels: HashMap::new(),
        open: None,
        panel_w,
        drag_start_w: panel_w,
        tabs: Vec::new(),
        islands: TabIslands::default(),
        next_id: 1,
        active: 0,
        store,
    };
    st.open_internal(&window, Kind::Home);

    let mut modifiers = ModifiersState::default();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                st.layout(&window);
            }
            Event::WindowEvent {
                event: WindowEvent::ModifiersChanged(state),
                ..
            } => {
                modifiers = state;
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        event,
                        ..
                    },
                ..
            } if event.state == ElementState::Pressed && !event.repeat => {
                let ctrl = modifiers.control_key();
                let shift = modifiers.shift_key();
                let command = match event.physical_key {
                    KeyCode::F11 => Some("fullscreen".to_string()),
                    KeyCode::KeyT if ctrl && !shift => Some("new".to_string()),
                    KeyCode::KeyW if ctrl && !shift => Some(format!("close:{}", st.active)),
                    KeyCode::KeyN if ctrl && shift => Some("newp".to_string()),
                    _ => None,
                };
                if let Some(command) = command {
                    if command == "fullscreen" {
                        if window.fullscreen().is_some() {
                            window.set_fullscreen(None);
                        } else {
                            window.set_fullscreen(Some(Fullscreen::Borderless(window.current_monitor())));
                        }
                        st.layout(&window);
                    } else if st.ipc(&window, &command) {
                        *control_flow = ControlFlow::Exit;
                    } else {
                        st.push_tabs();
                    }
                }
            }
            Event::UserEvent(ev) => {
                if st.handle(&window, ev) {
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
}
