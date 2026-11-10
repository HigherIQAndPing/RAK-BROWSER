// Der WebView2-Loader braucht Registry- und ETW-Funktionen aus advapi32.lib.
// Je nach wry-/Rust-Version wird die Bibliothek nicht automatisch mitgelinkt.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-lib=advapi32");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
