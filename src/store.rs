//! Persistente Daten von RAK: Lesezeichen, Verlauf, Einstellungen.
//! Alles liegt im Konfigurationsordner des Benutzers, z.B. unter Windows in
//! %APPDATA%\RAK. Private Tabs schreiben nie in den Verlauf (das regelt main.rs).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const HISTORY_MAX: usize = 5000; // so viele Einträge bleiben gespeichert
const HISTORY_PAGE: usize = 1500; // so viele zeigt die Verlaufsseite an

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Bookmark {
    pub id: u32,
    pub title: String,
    pub url: String,
    pub folder: Option<u32>, // None = direkt in der Leiste
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Folder {
    pub id: u32,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryEntry {
    pub ts: u64,
    pub title: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// Welche Apps in der Seitenleiste angezeigt werden
    pub sidebar: Vec<String>,
    pub panel_width: f64,
    pub search_engine: String,
    pub auto_translate: bool,
    pub translate_lang: String,
    pub colors: ThemeColors,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ThemeColors {
    pub background: String,
    pub tabs: String,
    pub toolbar: String,
    pub bookmarks: String,
    pub sidebar: String,
    pub accent: String,
    pub text: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            background: "#0d0d12".into(),
            tabs: "#16161d".into(),
            toolbar: "#1e1e28".into(),
            bookmarks: "#0d0d12".into(),
            sidebar: "#0d0d12".into(),
            accent: "#19d3ff".into(),
            text: "#e8e8f0".into(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            sidebar: vec!["spotify".into(), "instagram".into(), "x".into()],
            panel_width: 380.0,
            search_engine: "duckduckgo".into(),
            auto_translate: false,
            translate_lang: "de".into(),
            colors: ThemeColors::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct BookmarkFile {
    bookmarks: Vec<Bookmark>,
    folders: Vec<Folder>,
    next_id: u32,
}

pub struct Store {
    dir: PathBuf,
    pub bookmarks: Vec<Bookmark>,
    pub folders: Vec<Folder>,
    next_id: u32,
    pub history: Vec<HistoryEntry>,
    pub settings: Settings,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// JSON, das sicher in ein <script> eingebettet werden kann ("<" kommt nur in Texten vor)
pub fn script_safe(v: &Value) -> String {
    v.to_string().replace('<', "\\u003c")
}

/// Ordner, in dem RAK alle Daten ablegt (unter Windows %APPDATA%\\RAK)
pub fn data_dir() -> PathBuf {
    let dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("RAK");
    let _ = fs::create_dir_all(&dir);
    dir
}

impl Store {
    pub fn load() -> Self {
        let dir = data_dir();

        let file: BookmarkFile = fs::read_to_string(dir.join("bookmarks.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let settings: Settings = fs::read_to_string(dir.join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let mut history: Vec<HistoryEntry> = fs::read_to_string(dir.join("history.jsonl"))
            .map(|s| s.lines().filter_map(|l| serde_json::from_str(l).ok()).collect())
            .unwrap_or_default();

        let max_id = file
            .bookmarks
            .iter()
            .map(|b| b.id)
            .chain(file.folders.iter().map(|f| f.id))
            .max()
            .unwrap_or(0);
        let mut store = Store {
            dir,
            bookmarks: file.bookmarks,
            folders: file.folders,
            next_id: file.next_id.max(max_id + 1).max(1),
            history: Vec::new(),
            settings,
        };
        if history.len() > HISTORY_MAX {
            history.drain(..history.len() - HISTORY_MAX);
            store.history = history;
            store.rewrite_history();
        } else {
            store.history = history;
        }
        store
    }

    fn write_json<T: Serialize>(&self, name: &str, value: &T) {
        if let Ok(s) = serde_json::to_string_pretty(value) {
            let _ = fs::write(self.dir.join(name), s);
        }
    }

    fn save_bookmarks(&self) {
        let file = BookmarkFile {
            bookmarks: self.bookmarks.clone(),
            folders: self.folders.clone(),
            next_id: self.next_id,
        };
        self.write_json("bookmarks.json", &file);
    }

    pub fn save_settings(&self) {
        self.write_json("settings.json", &self.settings);
    }

    // ---------- Verlauf ----------

    pub fn add_history(&mut self, url: &str, title: &str) {
        if self.history.last().map_or(false, |h| h.url == url) {
            return; // gleiche Seite direkt nacheinander nur einmal
        }
        let entry = HistoryEntry { ts: now(), title: title.to_string(), url: url.to_string() };
        if let Ok(line) = serde_json::to_string(&entry) {
            if let Ok(mut f) =
                OpenOptions::new().create(true).append(true).open(self.dir.join("history.jsonl"))
            {
                let _ = writeln!(f, "{line}");
            }
        }
        self.history.push(entry);
        if self.history.len() > HISTORY_MAX + 500 {
            let cut = self.history.len() - HISTORY_MAX;
            self.history.drain(..cut);
            self.rewrite_history();
        }
    }

    /// Trägt den Seitentitel nach, falls er erst nach dem Laden bekannt wurde
    pub fn retitle_last(&mut self, url: &str, title: &str) {
        if title.is_empty() {
            return;
        }
        if let Some(last) = self.history.last_mut() {
            if last.url == url && (last.title.is_empty() || last.title == last.url) && last.title != title {
                last.title = title.to_string();
                self.rewrite_history();
            }
        }
    }

    fn rewrite_history(&self) {
        let mut out = String::new();
        for e in &self.history {
            if let Ok(l) = serde_json::to_string(e) {
                out.push_str(&l);
                out.push('\n');
            }
        }
        let _ = fs::write(self.dir.join("history.jsonl"), out);
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        let _ = fs::write(self.dir.join("history.jsonl"), "");
    }

    pub fn delete_history_entry(&mut self, ts: u64, url: &str) {
        self.history.retain(|e| !(e.ts == ts && e.url == url));
        self.rewrite_history();
    }

    // ---------- Lesezeichen ----------

    fn new_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_bookmark(&mut self, title: &str, url: &str, folder: Option<u32>) {
        let folder = folder.filter(|f| self.folders.iter().any(|x| x.id == *f));
        let title = if title.trim().is_empty() { url } else { title.trim() };
        let id = self.new_id();
        self.bookmarks.push(Bookmark { id, title: title.to_string(), url: url.to_string(), folder });
        self.save_bookmarks();
    }

    pub fn delete_bookmark(&mut self, id: u32) {
        self.bookmarks.retain(|b| b.id != id);
        self.save_bookmarks();
    }

    pub fn delete_bookmarks_by_url(&mut self, url: &str) {
        self.bookmarks.retain(|b| b.url != url);
        self.save_bookmarks();
    }

    pub fn move_bookmark(&mut self, id: u32, folder: Option<u32>) {
        let folder = folder.filter(|f| self.folders.iter().any(|x| x.id == *f));
        if let Some(b) = self.bookmarks.iter_mut().find(|b| b.id == id) {
            b.folder = folder;
        }
        self.save_bookmarks();
    }

    pub fn add_folder(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let id = self.new_id();
        self.folders.push(Folder { id, name: name.to_string() });
        self.save_bookmarks();
    }

    pub fn rename_folder(&mut self, id: u32, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(f) = self.folders.iter_mut().find(|f| f.id == id) {
            f.name = name.to_string();
        }
        self.save_bookmarks();
    }

    /// Löscht den Ordner; seine Lesezeichen wandern zurück in die Leiste (nichts geht verloren).
    pub fn delete_folder(&mut self, id: u32) {
        self.folders.retain(|f| f.id != id);
        for b in self.bookmarks.iter_mut().filter(|b| b.folder == Some(id)) {
            b.folder = None;
        }
        self.save_bookmarks();
    }

    // ---------- JSON für die Oberfläche ----------

    pub fn bookmarks_json(&self) -> Value {
        json!({ "bookmarks": self.bookmarks, "folders": self.folders })
    }

    pub fn history_json(&self) -> Value {
        let list: Vec<&HistoryEntry> = self.history.iter().rev().take(HISTORY_PAGE).collect();
        json!({ "history": list })
    }

    pub fn history_page(&self) -> String {
        include_str!("history.html").replace("__DATA__", &script_safe(&self.history_json()))
    }

    pub fn bookmarks_page(&self) -> String {
        include_str!("bookmarks.html").replace("__DATA__", &script_safe(&self.bookmarks_json()))
    }
}
