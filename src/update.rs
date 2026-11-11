//! Automatische, signierte Updates.
//!
//! Ablauf: update.json (Version + SHA-256 der EXE) und update.json.sig werden vom Release geladen.
//! Nur wenn die Ed25519-Signatur zum fest eingebauten öffentlichen Schlüssel passt und die Version
//! NEUER ist (Downgrade-Schutz), wird RAK.exe geladen, per SHA-256 geprüft und ausgetauscht.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, process::Command};

/// Wo die Releases liegen (öffentliches Repo). Bei Umzug hier ändern.
pub const REPO: &str = "HigherIQAndPing/RAK-BROWSER";
/// Öffentlicher Schlüssel (der private liegt nur bei dir / als GitHub-Secret RAK_SIGNING_KEY).
const PUBKEY_HEX: &str = "e911bb46b0cff1ebe1025496221513a78c19a6b86efaf8e30c003f3e51b125e4";
const MAX_EXE: u64 = 300 * 1024 * 1024;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize)]
struct Manifest {
    version: String,
    sha256: String,
}

fn base_url() -> String {
    #[cfg(debug_assertions)]
    if let Ok(u) = std::env::var("RAK_UPDATE_URL") {
        return u; // nur in Testbuilds
    }
    format!("https://github.com/{REPO}/releases/latest/download")
}

fn pubkey() -> Result<VerifyingKey, String> {
    #[cfg(debug_assertions)]
    let hex = std::env::var("RAK_UPDATE_PUBKEY").unwrap_or_else(|_| PUBKEY_HEX.to_string());
    #[cfg(not(debug_assertions))]
    let hex = PUBKEY_HEX.to_string();
    let bytes = unhex(&hex).ok_or("Schlüssel ungültig")?;
    let arr: [u8; 32] = bytes.try_into().map_err(|_| "Schlüssel ungültig")?;
    VerifyingKey::from_bytes(&arr).map_err(|e| e.to_string())
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// "1.2.3" -> (1,2,3); alles andere ist ungültig
fn parse_ver(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let mut it = v.split('.');
    let r = (it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?);
    if it.next().is_some() {
        return None;
    }
    Some(r)
}

fn fetch(url: &str, dest: &PathBuf, max: u64) -> Result<(), String> {
    let mut cmd = Command::new(if cfg!(windows) { "curl.exe" } else { "curl" });
    cmd.args(["-sS", "-L", "-f", "--proto", "=https,http", "--max-time", "300", "--max-filesize"])
        .arg(max.to_string())
        .arg("-o")
        .arg(dest)
        .arg(url);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| format!("curl: {e}"))?;
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

/// Prüft Manifest + Signatur. Gibt die neue Version zurück, falls eine neuere existiert.
pub fn verify_manifest(manifest: &[u8], sig: &[u8], key: &VerifyingKey) -> Result<Option<(String, String)>, String> {
    let sig: [u8; 64] = sig.try_into().map_err(|_| "Signatur hat falsche Länge")?;
    key.verify(manifest, &Signature::from_bytes(&sig)).map_err(|_| "Signatur ungültig – Update abgelehnt")?;
    let m: Manifest = serde_json::from_slice(manifest).map_err(|e| e.to_string())?;
    let new = parse_ver(&m.version).ok_or("Versionsangabe ungültig")?;
    let cur = parse_ver(VERSION).ok_or("eigene Version ungültig")?;
    if new <= cur {
        return Ok(None);
    }
    if m.sha256.len() != 64 || unhex(&m.sha256).is_none() {
        return Err("Prüfsumme ungültig".into());
    }
    Ok(Some((m.version, m.sha256.to_lowercase())))
}

fn exe_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

/// Räumt die Reste eines früheren Updates auf (beim Start aufrufen).
pub fn cleanup() {
    if let Ok(e) = exe_path() {
        let _ = fs::remove_file(e.with_extension("old"));
        let _ = fs::remove_file(e.with_extension("new"));
    }
}

/// Sucht, lädt, prüft und installiert ein Update. Blockiert -> eigener Thread.
/// Ok(None) = aktuell, Ok(Some(version)) = installiert (greift nach Neustart).
pub fn check_and_install() -> Result<Option<String>, String> {
    let key = pubkey()?;
    let exe = exe_path()?;
    let tmp = std::env::temp_dir().join(format!("rak-upd-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let r = (|| {
        let base = base_url();
        let (mp, sp) = (tmp.join("update.json"), tmp.join("update.json.sig"));
        fetch(&format!("{base}/update.json"), &mp, 64 * 1024)?;
        fetch(&format!("{base}/update.json.sig"), &sp, 1024)?;
        let manifest = fs::read(&mp).map_err(|e| e.to_string())?;
        let sig = fs::read(&sp).map_err(|e| e.to_string())?;
        let Some((version, sha)) = verify_manifest(&manifest, &sig, &key)? else { return Ok(None) };

        let newf = exe.with_extension("new");
        let _ = fs::remove_file(&newf);
        fetch(&format!("{base}/RAK.exe"), &newf, MAX_EXE).map_err(|e| format!("Download: {e}"))?;
        let bytes = fs::read(&newf).map_err(|e| e.to_string())?;
        if hex(&Sha256::digest(&bytes)) != sha {
            let _ = fs::remove_file(&newf);
            return Err("Prüfsumme stimmt nicht – Update verworfen".into());
        }
        // Austausch: laufende Datei umbenennen (geht unter Windows), neue an ihren Platz
        let old = exe.with_extension("old");
        let _ = fs::remove_file(&old);
        fs::rename(&exe, &old).map_err(|e| format!("Austausch nicht möglich ({e}). Liegt RAK in einem geschützten Ordner?"))?;
        if let Err(e) = fs::rename(&newf, &exe) {
            let _ = fs::rename(&old, &exe); // zurückrollen
            return Err(format!("Austausch fehlgeschlagen: {e}"));
        }
        Ok(Some(version))
    })();
    let _ = fs::remove_dir_all(&tmp);
    r
}

/// Startet die (neue) EXE und beendet danach die alte (das erledigt der Aufrufer).
pub fn spawn_self() {
    if let Ok(e) = exe_path() {
        let _ = Command::new(e).spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    #[test]
    fn accepts_valid_newer() {
        let m = br#"{"version":"99.0.0","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}"#;
        let s = key().sign(m).to_bytes();
        let r = verify_manifest(m, &s, &key().verifying_key()).unwrap();
        assert_eq!(r.unwrap().0, "99.0.0");
    }

    #[test]
    fn rejects_tampered_and_wrong_key() {
        let m = br#"{"version":"99.0.0","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}"#;
        let s = key().sign(m).to_bytes();
        let t = br#"{"version":"99.0.1","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}"#;
        assert!(verify_manifest(t, &s, &key().verifying_key()).is_err());
        let other = SigningKey::from_bytes(&[8u8; 32]).verifying_key();
        assert!(verify_manifest(m, &s, &other).is_err());
        assert!(verify_manifest(m, &s[..10], &key().verifying_key()).is_err());
    }

    #[test]
    fn rejects_downgrade_and_same() {
        for v in ["0.0.1", VERSION] {
            let m = format!(r#"{{"version":"{v}","sha256":"{}"}}"#, "0".repeat(64));
            let s = key().sign(m.as_bytes()).to_bytes();
            assert!(verify_manifest(m.as_bytes(), &s, &key().verifying_key()).unwrap().is_none());
        }
    }

    #[test]
    fn version_parse() {
        assert_eq!(parse_ver("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_ver("1.2"), None);
        assert_eq!(parse_ver("1.2.3.4"), None);
        assert!(parse_ver("0.10.0") > parse_ver("0.9.0"));
    }
}
