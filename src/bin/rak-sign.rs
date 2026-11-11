//! Release-Werkzeug: rak-sign <RAK.exe> <version> <ausgabeordner>
//! Privater Schlüssel (64 Hex-Zeichen) kommt aus der Umgebungsvariable RAK_SIGNING_KEY.
//! Erzeugt update.json (Version + SHA-256) und update.json.sig (Ed25519).
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::{env, fs, process::exit};

fn unhex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    (s.len() % 2 == 0).then_some(())?;
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

fn main() {
    let a: Vec<String> = env::args().collect();
    if a.len() != 4 {
        eprintln!("Aufruf: rak-sign <RAK.exe> <version> <ausgabeordner>");
        exit(2);
    }
    let key = env::var("RAK_SIGNING_KEY").ok().and_then(|k| unhex(&k)).and_then(|k| <[u8; 32]>::try_from(k).ok());
    let Some(key) = key else {
        eprintln!("RAK_SIGNING_KEY fehlt oder ist kein 64-stelliger Hex-Wert");
        exit(1);
    };
    let bytes = fs::read(&a[1]).expect("EXE nicht lesbar");
    let sha: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    let version = a[2].trim_start_matches('v');
    let manifest = format!("{{\"version\":\"{version}\",\"sha256\":\"{sha}\"}}");
    let sig = SigningKey::from_bytes(&key).sign(manifest.as_bytes());
    fs::create_dir_all(&a[3]).unwrap();
    fs::write(format!("{}/update.json", a[3]), &manifest).unwrap();
    fs::write(format!("{}/update.json.sig", a[3]), sig.to_bytes()).unwrap();
    println!("signiert: {version} {sha}");
}
