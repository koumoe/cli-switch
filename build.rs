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

