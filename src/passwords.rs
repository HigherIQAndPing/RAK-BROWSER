//! Lokaler Passworttresor. Zugangsdaten werden nie als Klartext gespeichert.
use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use keyring::Entry;
use rand::{rngs::OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{path::Path, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};
use url::Url;
use zeroize::Zeroizing;

const SERVICE: &str = "RAK Browser";
const ACCOUNT: &str = "vault-dek";
const AUTO_LOCK: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CredentialView { pub id: i64, pub origin: String, pub username: String, pub created: u64 }
#[derive(Serialize, Deserialize)]
struct Credential { origin: String, username: String, password: String, created: u64 }

pub struct Vault { db: Connection, dek: Option<Zeroizing<Vec<u8>>>, last_used: Option<Instant>, master: bool }

fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }
fn entry() -> Result<Entry, String> { Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string()) }
fn random_key() -> Vec<u8> { let mut k = vec![0u8; 32]; OsRng.fill_bytes(&mut k); k }
fn nonce() -> [u8; 12] { let mut n = [0u8; 12]; OsRng.fill_bytes(&mut n); n }
fn derive(password: &str, salt: &[u8]) -> Result<Vec<u8>, String> {
    let params = Params::new(19 * 1024, 3, 1, Some(32)).map_err(|e| e.to_string())?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = vec![0u8; 32]; argon.hash_password_into(password.as_bytes(), salt, &mut out).map_err(|e| e.to_string())?; Ok(out)
}
fn seal(key: &[u8], plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let n = nonce(); let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    Ok((n.to_vec(), cipher.encrypt(Nonce::from_slice(&n), plaintext).map_err(|e| e.to_string())?))
}
fn open(key: &[u8], n: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    if n.len() != 12 { return Err("Ungültige Tresor-Nonce".into()); }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    cipher.decrypt(Nonce::from_slice(n), ciphertext).map_err(|_| "Tresor konnte nicht entschlüsselt werden".into())
}
fn origin_of(raw: &str) -> Option<String> {
    let u = Url::parse(raw).ok()?; if u.scheme() != "https" && u.scheme() != "http" { return None; }
    Some(u.origin().ascii_serialization())
}

impl Vault {
    pub fn open(dir: &Path) -> Result<Self, String> {
        let db = Connection::open(dir.join("passwords.sqlite3")).map_err(|e| e.to_string())?;
        db.execute_batch("PRAGMA secure_delete=ON; PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS vault_meta (key TEXT PRIMARY KEY, value BLOB NOT NULL); CREATE TABLE IF NOT EXISTS credentials (id INTEGER PRIMARY KEY, created INTEGER NOT NULL, nonce BLOB NOT NULL, ciphertext BLOB NOT NULL);").map_err(|e| e.to_string())?;
        let master = db.query_row("SELECT value FROM vault_meta WHERE key='master'", [], |r| r.get::<_, Vec<u8>>(0)).optional().map_err(|e| e.to_string())?.map(|v| v == b"1").unwrap_or(false);
        let mut v = Self { db, dek: None, last_used: None, master };
        if !master { let _ = v.unlock_os(); }
        Ok(v)
    }
    fn meta(&self, key: &str) -> Result<Option<Vec<u8>>, String> { self.db.query_row("SELECT value FROM vault_meta WHERE key=?1", [key], |r| r.get(0)).optional().map_err(|e| e.to_string()) }
    fn set_meta(&self, key: &str, value: &[u8]) -> Result<(), String> { self.db.execute("INSERT INTO vault_meta(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value]).map(|_|()).map_err(|e| e.to_string()) }
    fn touch(&mut self) { self.last_used = Some(Instant::now()); }
    pub fn tick(&mut self) { if self.master && self.last_used.is_some_and(|t| t.elapsed() >= AUTO_LOCK) { self.lock(); } }
    pub fn is_locked(&self) -> bool { self.dek.is_none() }
    pub fn status(&self) -> Value { json!({"locked":self.is_locked(),"master":self.master,"auto_lock_minutes":10}) }
    fn unlock_os(&mut self) -> Result<(), String> {
        let e = entry()?; let key = match e.get_secret() { Ok(k) if k.len() == 32 => k, _ => { let k=random_key(); e.set_secret(&k).map_err(|x| x.to_string())?; k } };
        self.dek = Some(Zeroizing::new(key)); self.touch(); Ok(())
    }
    pub fn unlock(&mut self, password: Option<&str>) -> Result<(), String> {
        if !self.master { return self.unlock_os(); }
        let p = password.ok_or_else(|| "Master-Passwort erforderlich".to_string())?;
        let salt = self.meta("salt")?.ok_or_else(|| "Tresor-Salt fehlt".to_string())?;
        let wrapped_nonce = self.meta("wrapped_nonce")?.ok_or_else(|| "Tresor-Nonce fehlt".to_string())?;
        let wrapped = self.meta("wrapped_dek")?.ok_or_else(|| "Verschlüsselter DEK fehlt".to_string())?;
        let kek = derive(p, &salt)?; let key = open(&kek, &wrapped_nonce, &wrapped)?;
        if key.len() != 32 { return Err("Ungültiger Tresorschlüssel".into()); }
        self.dek = Some(Zeroizing::new(key)); self.touch(); Ok(())
    }
    pub fn lock(&mut self) { self.dek = None; self.last_used = None; }
    pub fn enable_master(&mut self, password: &str) -> Result<(), String> {
        if password.chars().count() < 12 { return Err("Das Master-Passwort muss mindestens 12 Zeichen haben".into()); }
        let dek = self.dek.as_ref().ok_or_else(|| "Tresor zuerst entsperren".to_string())?;
        let mut salt=vec![0u8;16]; OsRng.fill_bytes(&mut salt); let kek=derive(password,&salt)?; let (n,w)=seal(&kek,dek)?;
        self.set_meta("salt",&salt)?; self.set_meta("wrapped_nonce",&n)?; self.set_meta("wrapped_dek",&w)?; self.set_meta("master",b"1")?;
        let _ = entry().map(|e| e.delete_credential()); self.master=true; Ok(())
    }
    pub fn disable_master(&mut self, password: &str) -> Result<(), String> {
        self.unlock(Some(password))?; let dek=self.dek.as_ref().ok_or_else(|| "Tresor gesperrt".to_string())?; entry()?.set_secret(dek).map_err(|e|e.to_string())?;
        self.set_meta("master",b"0")?; self.master=false; for k in ["salt","wrapped_nonce","wrapped_dek"] { self.db.execute("DELETE FROM vault_meta WHERE key=?1",[k]).map_err(|e|e.to_string())?; } Ok(())
    }
    pub fn list(&self) -> Result<Vec<CredentialView>, String> {
        let key=self.dek.as_ref().ok_or_else(|| "Tresor gesperrt".to_string())?; let mut st=self.db.prepare("SELECT id,created,nonce,ciphertext FROM credentials ORDER BY created DESC").map_err(|e|e.to_string())?; let rows=st.query_map([], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,u64>(1)?,r.get::<_,Vec<u8>>(2)?,r.get::<_,Vec<u8>>(3)?))).map_err(|e|e.to_string())?;
        let mut out=Vec::new(); for row in rows { let (id,created,n,c)=row.map_err(|e|e.to_string())?; let x:Credential=serde_json::from_slice(&open(key,&n,&c)?).map_err(|e|e.to_string())?; out.push(CredentialView{id,origin:x.origin,username:x.username,created}); } Ok(out)
    }
    pub fn matching(&mut self, page_url: &str) -> Result<Vec<CredentialView>, String> { let origin=origin_of(page_url).ok_or_else(|| "Keine gültige HTTP-Origin".to_string())?; Ok(self.list()?.into_iter().filter(|x|x.origin==origin).collect()) }
    pub fn secret_for(&mut self, page_url: &str, id: i64) -> Result<(String, String), String> {
        let origin = origin_of(page_url).ok_or_else(|| "Keine gültige HTTP-Origin".to_string())?;
        self.tick(); let key=self.dek.as_ref().ok_or_else(|| "Tresor gesperrt".to_string())?;
        let row: Option<(Vec<u8>,Vec<u8>)> = self.db.query_row("SELECT nonce,ciphertext FROM credentials WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
        let (n,c)=row.ok_or_else(|| "Zugangsdaten nicht gefunden".to_string())?;
        let x:Credential=serde_json::from_slice(&open(key,&n,&c)?).map_err(|e|e.to_string())?;
        if x.origin != origin { return Err("Origin stimmt nicht überein".into()); }
        self.touch(); Ok((x.username,x.password))
    }
    pub fn save(&mut self, page_url:&str, username:&str, password:&str) -> Result<(),String> { let origin=origin_of(page_url).ok_or_else(||"Nur HTTP(S)-Origins erlaubt".to_string())?; if username.is_empty()||password.is_empty(){return Err("Benutzername und Passwort dürfen nicht leer sein".into())}; let key=self.dek.as_ref().ok_or_else(||"Tresor gesperrt".to_string())?; let x=Credential{origin,username:username.into(),password:password.into(),created:now()}; let (n,c)=seal(key,&serde_json::to_vec(&x).map_err(|e|e.to_string())?)?; self.db.execute("INSERT INTO credentials(created,nonce,ciphertext) VALUES(?1,?2,?3)",params![x.created,n,c]).map_err(|e|e.to_string())?; self.touch(); Ok(()) }
    pub fn delete(&mut self,id:i64)->Result<(),String>{self.db.execute("DELETE FROM credentials WHERE id=?1",[id]).map(|_|()).map_err(|e|e.to_string())}
}

#[cfg(test)] mod tests { use super::*; #[test] fn origins_are_exact(){assert_eq!(origin_of("https://example.com/login").as_deref(),Some("https://example.com"));assert_ne!(origin_of("https://sub.example.com").as_deref(),Some("https://example.com"));} #[test] fn crypto_roundtrip(){let k=random_key();let(n,c)=seal(&k,b"secret").unwrap();assert_eq!(open(&k,&n,&c).unwrap(),b"secret");} }
