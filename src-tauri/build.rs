//! Build script — runs Tauri's codegen, then ensures a matching Pdfium
//! shared library is available so the PDF parser can dynamically bind
//! to it at runtime.
//!
//! The library is downloaded once from <https://github.com/bblanchon/pdfium-binaries>
//! and cached under `target/.../out/pdfium/`. The absolute path is
//! propagated to the compiled binary via the `PDFIUM_LIBRARY_PATH`
//! compile-time env var (read in `src/parser/pdf.rs` via `env!()`).
//!
//! Network is touched on the first build only; subsequent builds reuse
//! the cached archive. If the download fails (offline build, restricted
//! network) the env var is still set but the file may not exist —
//! `pdfium-render` will then fail to bind at runtime and `parse_pdf`
//! returns a clean error instead of panicking.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const PDFIUM_RELEASE_TAG: &str = "chromium/7834";

fn main() {
    tauri_build::build();

    if let Err(e) = ensure_pdfium() {
        // Don't fail the whole build for a Pdfium fetch error — emit a
        // warning so CI logs surface it but keep the rest of the crate
        // compiling. PDF parsing will return a runtime error instead.
        println!("cargo:warning=pdfium download skipped: {e}");
    }
}

fn ensure_pdfium() -> Result<(), String> {
    let (asset, lib_name) = pdfium_asset_for_host()?;

    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|e| e.to_string())?);
    let pdfium_dir = out_dir.join("pdfium");
    let lib_path = pdfium_dir.join(&lib_name);

    if !lib_path.exists() {
        fs::create_dir_all(&pdfium_dir).map_err(|e| e.to_string())?;
        let archive_path = pdfium_dir.join(asset);
        download(
            &format!(
                "https://github.com/bblanchon/pdfium-binaries/releases/download/{PDFIUM_RELEASE_TAG}/{asset}"
            ),
            &archive_path,
        )?;
        extract_tgz(&archive_path, &pdfium_dir)?;
        flatten_lib(&pdfium_dir, &lib_name)?;
    }

    println!("cargo:rustc-env=PDFIUM_LIBRARY_PATH={}", lib_path.display());
    println!("cargo:rerun-if-changed=build.rs");
    Ok(())
}

fn pdfium_asset_for_host() -> Result<(&'static str, String), String> {
    let target_os = env::var("CARGO_CFG_TARGET_OS").map_err(|e| e.to_string())?;
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").map_err(|e| e.to_string())?;
    let asset = match (target_os.as_str(), target_arch.as_str()) {
        ("linux", "x86_64") => "pdfium-linux-x64.tgz",
        ("linux", "aarch64") => "pdfium-linux-arm64.tgz",
        ("windows", "x86_64") => "pdfium-win-x64.tgz",
        ("windows", "aarch64") => "pdfium-win-arm64.tgz",
        ("macos", "x86_64") => "pdfium-mac-x64.tgz",
        ("macos", "aarch64") => "pdfium-mac-arm64.tgz",
        other => return Err(format!("no Pdfium binary mapping for target {other:?}")),
    };
    let lib_name = match target_os.as_str() {
        "windows" => "pdfium.dll".to_string(),
        "macos" => "libpdfium.dylib".to_string(),
        _ => "libpdfium.so".to_string(),
    };
    Ok((asset, lib_name))
}

fn download(url: &str, dest: &Path) -> Result<(), String> {
    let status = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--location",
            "--output",
        ])
        .arg(dest)
        .arg(url)
        .status()
        .map_err(|e| format!("failed to spawn curl: {e}"))?;
    if !status.success() {
        return Err(format!("curl exited with {status} downloading {url}"));
    }
    Ok(())
}

fn extract_tgz(archive: &Path, dest: &Path) -> Result<(), String> {
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .status()
        .map_err(|e| format!("failed to spawn tar: {e}"))?;
    if !status.success() {
        return Err(format!(
            "tar exited with {status} extracting {}",
            archive.display()
        ));
    }
    Ok(())
}

/// pdfium-binaries archives unpack to `lib/libpdfium.so` (or
/// `bin/pdfium.dll`); collapse that into the top of `pdfium_dir` so
/// callers only need to know `pdfium/<lib_name>`.
fn flatten_lib(pdfium_dir: &Path, lib_name: &str) -> Result<(), String> {
    let candidates = [
        pdfium_dir.join("lib").join(lib_name),
        pdfium_dir.join("bin").join(lib_name),
    ];
    for c in &candidates {
        if c.exists() {
            fs::rename(c, pdfium_dir.join(lib_name)).map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    Err(format!(
        "{lib_name} not found in extracted archive under lib/ or bin/"
    ))
}
