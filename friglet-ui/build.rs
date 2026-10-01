//! Build script: embeds the web assets and records the friglet pin.
//!
//! Assets come from `ui/` when it holds an `index.html` (written by the web-UI track),
//! otherwise from `ui-placeholder/`, so the crate always builds. Every file is embedded with
//! `include_bytes!` through a generated table in `$OUT_DIR/assets.rs`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));

    // The whole crate directory is watched so that creating `ui/` later re-runs this script.
    println!("cargo:rerun-if-changed=.");
    println!("cargo:rerun-if-changed=../versions.env");
    println!("cargo:rerun-if-env-changed=BLINDBIT_RS_SRC");

    // friglet pin from versions.env (single source of truth).
    let versions = fs::read_to_string(manifest.join("../versions.env")).unwrap_or_default();
    let rev = versions
        .lines()
        .filter_map(|l| l.strip_prefix("BLINDBIT_RS_REV="))
        .map(|v| v.trim().to_string())
        .next()
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=FRIGLET_UI_FRIGLET_REV={rev}");
    let dev = env::var_os("BLINDBIT_RS_SRC").is_some_and(|v| !v.is_empty());
    println!(
        "cargo:rustc-env=FRIGLET_UI_DEV_BUILD={}",
        if dev { "1" } else { "0" }
    );

    let ui = manifest.join("ui");
    let (source, dir) = if ui.join("index.html").is_file() {
        ("ui", ui)
    } else {
        ("ui-placeholder", manifest.join("ui-placeholder"))
    };
    let mut files = Vec::new();
    collect(&dir, &dir, &mut files);
    files.sort();

    let mut table = String::new();
    table.push_str(&format!("pub const ASSETS_SOURCE: &str = {source:?};\n"));
    table.push_str("pub static ASSETS: &[(&str, &[u8])] = &[\n");
    for (rel, abs) in &files {
        table.push_str(&format!(
            "    ({rel:?}, include_bytes!({:?})),\n",
            abs.to_string_lossy()
        ));
    }
    table.push_str("];\n");
    fs::write(out.join("assets.rs"), table).expect("write assets.rs");
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, out);
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .expect("under root")
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.push((rel, path));
        }
    }
}
