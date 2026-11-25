//! Serving a DevTools frontend: the copy embedded by the `embedded-frontend`
//! feature (see [`blitz_chrome_devtools_frontend`]), or a local build of it.

use std::borrow::Cow;
use std::path::{Component, Path};

/// Environment variable naming a directory to serve the frontend from
/// instead of the embedded copy (e.g. the `dist/` of a local
/// blitz-chrome-devtools-frontend build)
const FRONTEND_DIR_VAR: &str = "PAGEFRAME_DEVTOOLS_FRONTEND_DIR";

/// The path of the frontend's page
const ENTRY_PATH: &str = "devtools_app.html";

/// The URL of a DevTools frontend inspecting the target `target_id` (see
/// [`target_id`](crate::target_id)) of the server listening on `addr`.
///
/// When the `embedded-frontend` feature is enabled (or
/// `PAGEFRAME_DEVTOOLS_FRONTEND_DIR` is set), this is the frontend served by the
/// server itself, which any Chromium-based browser can open. Otherwise it's the
/// frontend bundled with Chrome (a `devtools://` URL, as advertised at
/// `/json/list`), which can only be opened by pasting it into Chrome's address
/// bar.
pub fn frontend_url(addr: std::net::SocketAddr, target_id: &str) -> String {
    let serves_frontend =
        cfg!(feature = "embedded-frontend") || std::env::var_os(FRONTEND_DIR_VAR).is_some();
    let base = if serves_frontend {
        format!("http://{addr}")
    } else {
        "devtools://devtools/bundled".to_string()
    };
    format!("{base}/{ENTRY_PATH}?ws={addr}/devtools/page/{target_id}")
}

/// The `Content-Type` and contents of the frontend file at `path` (relative
/// to the frontend's root), if the server serves a frontend and it has one
pub(crate) fn file(path: &str) -> Option<(&'static str, Cow<'static, [u8]>)> {
    if !is_relative_file(path) {
        return None;
    }
    let body = match std::env::var_os(FRONTEND_DIR_VAR) {
        Some(dir) => Cow::Owned(read_inside(Path::new(&dir), path)?),
        None => Cow::Borrowed(embedded_file(path)?),
    };
    Some((content_type(path), body))
}

fn is_relative_file(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '\0'])
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
        && Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn read_inside(dir: &Path, path: &str) -> Option<Vec<u8>> {
    let base = dir.canonicalize().ok()?;
    let full = base.join(path).canonicalize().ok()?;
    if !full.starts_with(&base) || !full.is_file() {
        return None;
    }
    std::fs::read(full).ok()
}

#[cfg(feature = "embedded-frontend")]
fn embedded_file(path: &str) -> Option<&'static [u8]> {
    use std::collections::HashMap;
    use std::sync::OnceLock;

    // Unpacked on first use (about 11 MB) and kept for the process lifetime
    static FILES: OnceLock<HashMap<String, Vec<u8>>> = OnceLock::new();
    let files = FILES.get_or_init(|| {
        blitz_chrome_devtools_frontend::files().unwrap_or_else(|err| {
            tracing::warn!("CDP: failed to unpack the DevTools frontend: {err}");
            HashMap::new()
        })
    });
    files.get(path).map(Vec::as_slice)
}

#[cfg(not(feature = "embedded-frontend"))]
fn embedded_file(_path: &str) -> Option<&'static [u8]> {
    None
}

/// Browsers refuse to run module scripts not served with a JavaScript MIME type
fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, ext)| ext) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        Some("wasm") => "application/wasm",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{is_relative_file, read_inside};

    #[test]
    fn plain_relative_paths_are_accepted() {
        for path in ["devtools_app.html", "core/sdk/sdk.js", "a.b/c-d_e.json"] {
            assert!(is_relative_file(path), "{path}");
        }
    }

    #[test]
    fn paths_that_could_leave_the_folder_are_refused() {
        for path in [
            "",
            "/etc/passwd",
            "../Cargo.toml",
            "core/../../secret",
            "./devtools_app.html",
            "core//sdk.js",
            r"..\secret",
            r"core\..\..\secret",
            "C:/Windows/win.ini",
            "C:secret",
            r"\\server\share\file",
            "//server/share/file",
            "core/sdk.js\0.png",
        ] {
            assert!(!is_relative_file(path), "{path:?}");
        }
    }

    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reads_stay_inside_the_frontend_folder() {
        let root = std::env::temp_dir().join(format!(
            "pageframe-devtools-frontend-{}",
            std::process::id()
        ));
        let scratch = Scratch(root.clone());
        let frontend = root.join("frontend");
        std::fs::create_dir_all(frontend.join("core")).expect("create the test folder");
        std::fs::write(frontend.join("core").join("app.js"), b"app").expect("write app.js");
        std::fs::write(root.join("secret.txt"), b"secret").expect("write secret.txt");

        assert_eq!(
            read_inside(&frontend, "core/app.js").as_deref(),
            Some(&b"app"[..])
        );
        assert_eq!(read_inside(&frontend, "../secret.txt"), None);
        assert_eq!(read_inside(&frontend, "core"), None);
        assert_eq!(read_inside(&frontend, "missing.js"), None);
        drop(scratch);
    }
}
