use std::env;
use std::fs;
use std::path::PathBuf;

const PLACEHOLDER_INDEX_HTML: &str = r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <title>CliSwitch</title>
  </head>
  <body>
    <h1>CliSwitch</h1>
    <p>UI 尚未构建（当前为占位页面）。</p>
    <p>
      开发：先构建 <code>ui</code>（生成 <code>ui/dist</code>），再启动后端；或直接使用
      Vite dev server。
    </p>
    <p>健康检查：<a href="/api/health">/api/health</a></p>
  </body>
</html>
"#;

fn main() {
    maybe_write_placeholder_ui();
    maybe_embed_windows_icon();
}

fn maybe_write_placeholder_ui() {
    if env::var_os("CARGO_FEATURE_EMBED_UI").is_none() {
        return;
    }

    let manifest_dir = match env::var("CARGO_MANIFEST_DIR") {
        Ok(v) => PathBuf::from(v),
        Err(_) => return,
    };

    let dist_dir = manifest_dir.join("ui").join("dist");
    let index = dist_dir.join("index.html");

    if index.is_file() {
        return;
    }

    if let Err(e) = fs::create_dir_all(&dist_dir) {
        println!("cargo:warning=failed to create ui/dist: {e}");
        return;
    }

    if let Err(e) = fs::write(&index, PLACEHOLDER_INDEX_HTML.as_bytes()) {
        println!("cargo:warning=failed to write ui/dist/index.html: {e}");
    }
}

#[cfg(windows)]
fn maybe_embed_windows_icon() {
    // Embed application icon into the Windows executable (Explorer/Taskbar icon).
    println!("cargo:rerun-if-changed=assets/logo.ico");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/logo.ico");

    if let Err(e) = res.compile() {
        // Don't fail builds if the Windows resource toolchain is missing/misconfigured.
        println!("cargo:warning=failed to embed Windows icon: {e}");
    }
}

#[cfg(not(windows))]
fn maybe_embed_windows_icon() {}
