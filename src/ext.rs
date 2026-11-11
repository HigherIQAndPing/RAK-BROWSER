//! Erweiterungen aus dem Chrome Web Store.
//!
//! WebView2 (Windows) lädt Erweiterungen nur als entpackte Ordner. Dieses Modul holt eine
//! Erweiterung als .crx-Datei, entpackt sie nach %APPDATA%\RAK\extensions\<id>\ und
//! verwaltet die Liste. Geladen werden die Ordner beim Start (siehe main.rs).
//!
//! Wichtig: In `extensions\` dürfen NUR gültige Erweiterungsordner liegen, denn wry
//! übergibt jeden Eintrag an WebView2. `sanitize()` räumt Reste beim Start auf.

use serde::Serialize;
use serde_json::{json, Value};
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::Command,
};

const MAX_DOWNLOAD: u64 = 100 * 1024 * 1024; // 100 MB
const MAX_UNPACKED: u64 = 300 * 1024 * 1024; // 300 MB entpackt
const MAX_FILES: usize = 20_000;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Installed {
    pub id: String,
    pub name: String,
    pub version: String,
    pub manifest_version: u64,
}

pub fn ext_dir() -> PathBuf {
    let d = crate::store::data_dir().join("extensions");
    let _ = fs::create_dir_all(&d);
    d
}

fn tmp_dir() -> PathBuf {
    crate::store::data_dir().join("ext-tmp")
}

/// Chrome-Erweiterungs-IDs bestehen aus genau 32 Buchstaben von a bis p
pub fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b))
}

/// Nimmt eine Store-Adresse oder eine nackte ID und liefert die ID.
/// Beispiel: https://chromewebstore.google.com/detail/ublock-origin-lite/ddkjiahejlhfcafbddmgiahcphecmpfh?hl=de
pub fn extract_id(input: &str) -> Option<String> {
    let t = input.trim();
    let t = t.split(['?', '#']).next().unwrap_or(t);
    t.split(|c: char| !c.is_ascii_alphanumeric())
        .rev()
        .find(|tok| valid_id(tok))
        .map(|s| s.to_string())
}

/// Wo beginnt das ZIP in einer .crx-Datei? (CRX2 und CRX3; reine ZIPs gehen auch)
pub fn crx_zip_offset(b: &[u8]) -> Result<usize, String> {
    let u32le = |at: usize| -> Option<usize> {
        b.get(at..at + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]) as usize)
    };
    if b.starts_with(b"PK\x03\x04") {
        return Ok(0);
    }
    if !b.starts_with(b"Cr24") {
        return Err("Die heruntergeladene Datei ist keine Erweiterung (.crx)".into());
    }
    let off = match u32le(4) {
        Some(3) => 12usize.checked_add(u32le(8).ok_or("Beschädigte .crx-Datei")?),
        Some(2) => {
            let key = u32le(8).ok_or("Beschädigte .crx-Datei")?;
            let sig = u32le(12).ok_or("Beschädigte .crx-Datei")?;
            16usize.checked_add(key).and_then(|x| x.checked_add(sig))
        }
        _ => return Err("Unbekannte .crx-Version".into()),
    };
    match off {
        Some(o) if o < b.len() && b[o..].starts_with(b"PK") => Ok(o),
        _ => Err("Beschädigte .crx-Datei".into()),
    }
}

/// Entpackt das ZIP sicher nach `dest` (keine Pfade nach außen, Größenlimits,
/// reservierte "_"-Ordner wie _metadata werden ausgelassen, weil WebView2 sie ablehnt).
pub fn unpack(zip_bytes: &[u8], dest: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(io::Cursor::new(zip_bytes))
        .map_err(|e| format!("ZIP nicht lesbar: {e}"))?;
    if zip.len() > MAX_FILES {
        return Err("Zu viele Dateien in der Erweiterung".into());
    }
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let mut total: u64 = 0;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let rel = f.enclosed_name().ok_or("Unsicherer Dateipfad in der Erweiterung")?;
        let first = rel
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .unwrap_or_default();
        if first.starts_with('_') && first != "_locales" {
            continue;
        }
        let out = dest.join(&rel);
        if f.is_dir() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut file = fs::File::create(&out).map_err(|e| e.to_string())?;
        let mut limited = (&mut f).take(MAX_UNPACKED - total + 1);
        total += io::copy(&mut limited, &mut file).map_err(|e| e.to_string())?;
        if total > MAX_UNPACKED {
            return Err("Die Erweiterung ist entpackt zu groß".into());
        }
    }
    Ok(())
}

/// Liest Name/Version aus der manifest.json (löst __MSG_...__ über _locales auf)
pub fn read_info(dir: &Path, id: &str) -> Option<Installed> {
    let raw = fs::read_to_string(dir.join("manifest.json")).ok()?;
    let m: Value = serde_json::from_str(raw.trim_start_matches('\u{feff}')).ok()?;
    let name = m["name"].as_str().map(|n| localized(dir, &m, n)).filter(|n| !n.trim().is_empty());
    Some(Installed {
        id: id.to_string(),
        name: name.unwrap_or_else(|| id.to_string()),
        version: m["version"].as_str().unwrap_or("?").to_string(),
        manifest_version: m["manifest_version"].as_u64().unwrap_or(0),
    })
}

fn localized(dir: &Path, manifest: &Value, text: &str) -> String {
    let key = match text.strip_prefix("__MSG_").and_then(|t| t.strip_suffix("__")) {
        Some(k) => k.to_lowercase(),
        None => return text.to_string(),
    };
    let mut langs: Vec<String> = Vec::new();
    for l in ["de", "en", "en_US"] {
        langs.push(l.to_string());
    }
    if let Some(d) = manifest["default_locale"].as_str() {
        langs.insert(0, d.to_string());
    }
    for l in langs {
        let p = dir.join("_locales").join(&l).join("messages.json");
        let Ok(raw) = fs::read_to_string(p) else { continue };
        let Ok(v) = serde_json::from_str::<Value>(raw.trim_start_matches('\u{feff}')) else { continue };
        if let Some(obj) = v.as_object() {
            for (k, val) in obj {
                if k.to_lowercase() == key {
                    if let Some(msg) = val["message"].as_str() {
                        return msg.to_string();
                    }
                }
            }
        }
    }
    text.to_string()
}

pub fn list() -> Vec<Installed> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(ext_dir()) {
        for e in rd.flatten() {
            let id = e.file_name().to_string_lossy().to_string();
            if valid_id(&id) && e.path().is_dir() {
                if let Some(i) = read_info(&e.path(), &id) {
                    out.push(i);
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Entfernt alles aus `extensions\`, was kein vollständiger Erweiterungsordner ist.
/// Ein kaputter Eintrag würde sonst das Starten des Browsers verhindern.
pub fn sanitize() {
    let _ = fs::remove_dir_all(tmp_dir());
    if let Ok(rd) = fs::read_dir(ext_dir()) {
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            let ok = p.is_dir() && !name.starts_with('_') && p.join("manifest.json").is_file();
            if !ok {
                let _ = if p.is_dir() { fs::remove_dir_all(&p) } else { fs::remove_file(&p) };
            }
        }
    }
}

pub fn remove(id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("Ungültige ID".into());
    }
    let p = ext_dir().join(id);
    if p.exists() {
        fs::remove_dir_all(&p).map_err(|e| format!("Konnte den Ordner nicht löschen: {e}"))?;
    }
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    copy_dir_depth(src, dst, 0)
}

/// Folgt keinen Symlinks/Junctions (Schleifen, Ausbruch aus dem Profil) und ist tiefenbegrenzt.
fn copy_dir_depth(src: &Path, dst: &Path, depth: u32) -> Result<(), String> {
    if depth > 12 {
        return Err("Ordnerstruktur zu tief".into());
    }
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if kind.is_symlink() { continue; }
        if kind.is_dir() { copy_dir_depth(&from, &to, depth + 1)?; }
        else if kind.is_file() { fs::copy(&from, &to).map_err(|e| e.to_string())?; }
    }
    Ok(())
}

fn profile_roots(browser: &str) -> Vec<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let roaming = std::env::var_os("APPDATA").map(PathBuf::from);
    match browser {
        "chrome" => local.map(|p| vec![p.join("Google/Chrome/User Data")]).unwrap_or_default(),
        "brave" => local.map(|p| vec![p.join("BraveSoftware/Brave-Browser/User Data")]).unwrap_or_default(),
        "edge" => local.map(|p| vec![p.join("Microsoft/Edge/User Data")]).unwrap_or_default(),
        "opera" => roaming.map(|p| vec![p.join("Opera Software/Opera Stable")]).unwrap_or_default(),
        "firefox" => roaming.map(|p| vec![p.join("Mozilla/Firefox/Profiles")]).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn find_extension_dirs(dir: &Path, out: &mut Vec<PathBuf>, budget: &mut usize) {
    if *budget == 0 { return; }
    *budget -= 1;
    let Ok(entries) = fs::read_dir(dir) else { return; };
    for entry in entries.flatten() {
        let p = entry.path();
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if p.join("manifest.json").is_file() { out.push(p); }
            else { find_extension_dirs(&p, out, budget); }
        }
    }
}

/// Importiert Chromium-Erweiterungen aus dem lokalen Browserprofil.
/// Firefox-Erweiterungen werden erkannt, aber wegen inkompatibler XPI-IDs nicht
/// blind als WebView2-Erweiterungen installiert.
pub fn import_browser(browser: &str) -> Result<Value, String> {
    if !matches!(browser, "chrome" | "brave" | "edge" | "opera" | "firefox") {
        return Err("Unbekannter Browser".into());
    }
    let mut candidates = Vec::new();
    for root in profile_roots(browser) {
        if root.exists() { find_extension_dirs(&root, &mut candidates, &mut 40_000); }
    }
    let mut imported = Vec::new();
    let mut skipped = 0usize;
    for source in candidates {
        let id = source.ancestors().find_map(|p| p.file_name().and_then(|n| {
            let s = n.to_string_lossy(); valid_id(&s).then_some(s.to_string())
        }));
        let Some(id) = id else { skipped += 1; continue; };
        let target = ext_dir().join(&id);
        let _ = fs::remove_dir_all(&target);
        if copy_dir(&source, &target).is_ok() {
            if let Some(info) = read_info(&target, &id) {
                imported.push(json!({"id":info.id,"name":info.name,"version":info.version}));
            }
        }
    }
    Ok(json!({"ok":true,"browser":browser,"imported":imported,"skipped":skipped}))
}

#[cfg(debug_assertions)]
fn crx_url(id: &str) -> String {
    // Nur für Tests: eigene Adresse mit {id} als Platzhalter
    match std::env::var("RAK_CRX_URL") {
        Ok(t) => t.replace("{id}", id),
        Err(_) => store_url(id),
    }
}
#[cfg(not(debug_assertions))]
fn crx_url(id: &str) -> String {
    store_url(id)
}

fn store_url(id: &str) -> String {
    format!(
        "https://clients2.google.com/service/update2/crx?response=redirect&os=win&arch=x64&os_arch=x86_64\
         &nacl_arch=x86-64&prod=chromecrx&prodchannel=&prodversion=140.0.0.0&lang=de&acceptformat=crx2,crx3\
         &x=id%3D{id}%26installsource%3Dondemand%26uc"
    )
}

/// Lädt die .crx mit curl (seit Windows 10 mitgeliefert, nutzt die Windows-Zertifikate).
fn download(id: &str, dest: &Path) -> Result<(), String> {
    let mut cmd = Command::new(if cfg!(windows) { "curl.exe" } else { "curl" });
    cmd.args(["-sS", "-L", "-f", "--max-time", "120", "--max-filesize"])
        .arg(MAX_DOWNLOAD.to_string())
        .args(["-A", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36"])
        .arg("-o")
        .arg(dest)
        .arg(crx_url(id));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: kein Konsolenfenster aufblitzen lassen
    }
    let out = cmd.output().map_err(|e| format!("curl konnte nicht gestartet werden ({e}). Ab Windows 10 ist curl eigentlich dabei."))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        return Err(format!("Download fehlgeschlagen: {}", if err.is_empty() { "unbekannter Fehler" } else { err }));
    }
    Ok(())
}

/// Lädt und installiert eine Erweiterung. Blockiert, daher in einem eigenen Thread aufrufen.
pub fn install(input: &str) -> Result<Installed, String> {
    let id = extract_id(input)
        .ok_or("Keine gültige Erweiterungs-ID oder Store-Adresse erkannt (die ID hat 32 Buchstaben von a bis p).")?;
    let work = tmp_dir().join(&id);
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let result = install_in(&id, &work);
    let _ = fs::remove_dir_all(&work);
    result
}

fn install_in(id: &str, work: &Path) -> Result<Installed, String> {
    let crx = work.join("download.crx");
    download(id, &crx)?;
    let bytes = fs::read(&crx).map_err(|e| e.to_string())?;
    if bytes.len() < 100 {
        return Err("Der Store hat keine Datei geliefert. Die Erweiterung gibt es nicht (mehr) oder sie ist nicht herunterladbar.".into());
    }
    let off = crx_zip_offset(&bytes)?;
    let unpacked = work.join("x");
    unpack(&bytes[off..], &unpacked)?;
    let info = read_info(&unpacked, id).ok_or("Die Erweiterung enthält keine gültige manifest.json")?;

    let target = ext_dir().join(id);
    if target.exists() {
        fs::remove_dir_all(&target)
            .map_err(|e| format!("Die alte Version konnte nicht ersetzt werden ({e}). Starte RAK neu und versuche es erneut."))?;
    }
    fs::rename(&unpacked, &target).map_err(|e| format!("Installation fehlgeschlagen: {e}"))?;
    Ok(info)
}

pub fn data_json() -> Value {
    json!({ "supported": cfg!(windows), "installed": list() })
}

pub fn page() -> String {
    include_str!("extensions.html").replace("__DATA__", &crate::store::script_safe(&data_json()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const ID: &str = "ddkjiahejlhfcafbddmgiahcphecmpfh";

    fn make_zip(files: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(io::Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default();
            for (name, content) in files {
                w.start_file(*name, opts).unwrap();
                w.write_all(content.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        buf
    }

    fn crx3(zip: &[u8]) -> Vec<u8> {
        let header = vec![7u8; 50]; // Platzhalter für die Protobuf-Signatur
        let mut v = b"Cr24".to_vec();
        v.extend(3u32.to_le_bytes());
        v.extend((header.len() as u32).to_le_bytes());
        v.extend(&header);
        v.extend(zip);
        v
    }

    fn crx2(zip: &[u8]) -> Vec<u8> {
        let (key, sig) = (vec![1u8; 30], vec![2u8; 20]);
        let mut v = b"Cr24".to_vec();
        v.extend(2u32.to_le_bytes());
        v.extend((key.len() as u32).to_le_bytes());
        v.extend((sig.len() as u32).to_le_bytes());
        v.extend(&key);
        v.extend(&sig);
        v.extend(zip);
        v
    }

    #[test]
    fn id_aus_adressen() {
        assert_eq!(extract_id(ID).as_deref(), Some(ID));
        assert_eq!(extract_id(&format!("  {ID}  ")).as_deref(), Some(ID));
        let neu = format!("https://chromewebstore.google.com/detail/ublock-origin-lite/{ID}?hl=de&utm_source=x");
        assert_eq!(extract_id(&neu).as_deref(), Some(ID));
        let alt = format!("https://chrome.google.com/webstore/detail/{ID}");
        assert_eq!(extract_id(&alt).as_deref(), Some(ID));
        assert_eq!(extract_id("https://example.com/hallo"), None);
        assert_eq!(extract_id("ddkjiahejlhfcafbddmgiahcphecmpf"), None); // 31 Zeichen
        assert_eq!(extract_id("zzkjiahejlhfcafbddmgiahcphecmpfh"), None); // z ist nicht erlaubt
        assert_eq!(extract_id(""), None);
    }

    #[test]
    fn crx_header_werden_uebersprungen() {
        let z = make_zip(&[("manifest.json", "{}")]);
        for crx in [crx3(&z), crx2(&z), z.clone()] {
            let off = crx_zip_offset(&crx).unwrap();
            assert!(crx[off..].starts_with(b"PK\x03\x04"));
        }
        assert!(crx_zip_offset(b"HTTP/1.1 404 Not Found").is_err());
        assert!(crx_zip_offset(b"Cr24\x03\x00\x00\x00\xff\xff\xff\xff").is_err()); // Header länger als Datei
        assert!(crx_zip_offset(b"Cr24").is_err());
        assert!(crx_zip_offset(b"").is_err());
    }

    #[test]
    fn entpacken_und_manifest_lesen() {
        let z = make_zip(&[
            ("manifest.json", r#"{"manifest_version":3,"name":"__MSG_appName__","version":"1.2.3","default_locale":"en"}"#),
            ("_locales/en/messages.json", r#"{"AppName":{"message":"Super Blocker"}}"#),
            ("_metadata/verified_contents.json", "geheim"),
            ("js/content.js", "console.log(1)"),
        ]);
        let dir = std::env::temp_dir().join(format!("rak-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        unpack(&z, &dir).unwrap();
        assert!(dir.join("js/content.js").is_file());
        assert!(dir.join("_locales/en/messages.json").is_file());
        assert!(!dir.join("_metadata").exists(), "_metadata muss fehlen (WebView2 lehnt es ab)");
        let info = read_info(&dir, ID).unwrap();
        assert_eq!(info.name, "Super Blocker");
        assert_eq!(info.version, "1.2.3");
        assert_eq!(info.manifest_version, 3);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pfad_ausbruch_wird_abgelehnt() {
        let z = make_zip(&[("manifest.json", "{}"), ("../ausbruch.txt", "boese")]);
        let dir = std::env::temp_dir().join(format!("rak-test-evil-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(unpack(&z, &dir).is_err());
        assert!(!dir.parent().unwrap().join("ausbruch.txt").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn name_ohne_uebersetzung_und_ohne_manifest() {
        let dir = std::env::temp_dir().join(format!("rak-test-name-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert!(read_info(&dir, ID).is_none());
        fs::write(dir.join("manifest.json"), "\u{feff}{\"name\":\"__MSG_fehlt__\",\"version\":\"2\"}").unwrap();
        assert_eq!(read_info(&dir, ID).unwrap().name, "__MSG_fehlt__"); // lieber unübersetzt als leer
        let _ = fs::remove_dir_all(&dir);
    }
}
