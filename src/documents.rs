use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::os::windows::ffi::OsStrExt;
use std::{
    fs,
    path::{Path, PathBuf},
};
use windows::{
    Win32::Storage::FileSystem::{GetDriveTypeW, GetVolumePathNameW},
    core::PCWSTR,
};

#[derive(Clone, Serialize)]
pub struct Tab {
    pub id: u64,
    #[serde(skip)]
    pub path: PathBuf,
    pub name: String,
    pub display: String,
    pub scroll: f64,
}

#[derive(Default)]
pub struct Documents {
    pub tabs: Vec<Tab>,
    pub active: Option<u64>,
    next: u64,
}

impl Documents {
    pub fn open(&mut self, path: &Path, scroll: f64) -> Result<u64> {
        let path =
            fs::canonicalize(path).with_context(|| format!("Cannot open {}", path.display()))?;
        ensure_local(&path)?;
        anyhow::ensure!(path.is_file(), "Not a file: {}", path.display());
        for tab in &self.tabs {
            let same =
                tab.path == path || same_file::is_same_file(&tab.path, &path).unwrap_or(false);
            if same {
                self.active = Some(tab.id);
                return Ok(tab.id);
            }
        }
        self.next += 1;
        let id = self.next;
        self.tabs.push(Tab {
            id,
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            display: path.to_string_lossy().trim_start_matches("\\\\?\\").into(),
            path,
            scroll: if scroll.is_finite() {
                scroll.max(0.0)
            } else {
                0.0
            },
        });
        self.active = Some(id);
        Ok(id)
    }

    pub fn close(&mut self, id: u64) {
        if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(index);
            if self.active == Some(id) {
                self.active = self
                    .tabs
                    .get(index)
                    .or_else(|| self.tabs.last())
                    .map(|t| t.id);
            }
        }
    }
}

pub fn ensure_local(path: &Path) -> Result<()> {
    let s = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let text = path.to_string_lossy();
    anyhow::ensure!(
        !text.starts_with("\\\\") || text.starts_with("\\\\?\\"),
        "Network paths are not supported"
    );
    anyhow::ensure!(
        !text.to_ascii_lowercase().starts_with("\\\\?\\unc\\"),
        "Network paths are not supported"
    );
    let mut root = [0u16; 32768];
    unsafe {
        GetVolumePathNameW(PCWSTR(s.as_ptr()), &mut root)?;
        anyhow::ensure!(
            GetDriveTypeW(PCWSTR(root.as_ptr())) != 4,
            "Network drives are not supported"
        );
    }
    Ok(())
}

pub fn decode(bytes: &[u8]) -> (String, bool) {
    if let Some(bytes) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return match std::str::from_utf8(bytes) {
            Ok(text) => (text.into(), false),
            Err(_) => (String::from_utf8_lossy(bytes).into(), true),
        };
    }
    if let Some((encoding, skip)) = encoding_rs::Encoding::for_bom(bytes) {
        let (text, _, errors) = encoding.decode(&bytes[skip..]);
        return (text.into(), errors);
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return (text.into(), false);
    }
    let (text, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
    (text.into(), true)
}

pub fn image_path(base: &Path, requested: &str) -> Result<PathBuf> {
    let decoded = percent_encoding::percent_decode_str(requested).decode_utf8()?;
    anyhow::ensure!(
        !decoded.contains([':', '\0']) && !decoded.starts_with(['/', '\\']),
        "Invalid image path"
    );
    let root = fs::canonicalize(base)?;
    let resolved = fs::canonicalize(root.join(decoded.replace('/', "\\")))?;
    anyhow::ensure!(
        resolved.starts_with(&root),
        "Image is outside the document folder"
    );
    ensure_local(&resolved)?;
    match resolved
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "avif" => Ok(resolved),
        _ => bail!("Unsupported image format"),
    }
}

include!(concat!(env!("OUT_DIR"), "\\assets.rs"));
pub fn asset(name: &str) -> Result<Option<Vec<u8>>> {
    if name == "favicon.ico" {
        return Ok(Some(include_bytes!("..\\resources\\app.ico").to_vec()));
    }
    if let Some((_, compressed)) = ASSETS.iter().find(|(n, _)| *n == name) {
        let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(compressed)
            .map_err(|e| anyhow::anyhow!("Embedded asset is invalid: {e:?}"))?;
        Ok(Some(bytes))
    } else {
        Ok(None)
    }
}

pub fn mime(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decoding() {
        assert_eq!(decode(b"\xef\xbb\xbf# Hello"), ("# Hello".into(), false));
        assert_eq!(decode(&[0xff, 0xfe, 65, 0]), ("A".into(), false));
        assert_eq!(
            decode(&[0x93, 65, 0x94]),
            ("\u{201c}A\u{201d}".into(), true)
        );
        assert_eq!(decode(&[0xfe, 0xff, 0, 65]), ("A".into(), false));
        assert!(decode(&[0xef, 0xbb, 0xbf, 0xff]).1);
    }
    #[test]
    fn rejects_dangerous_image_names() {
        for path in [
            "https://evil/a.png",
            "C:\\a.png",
            "//host/a.png",
            "%00a.png",
            "x:secret.png",
        ] {
            assert!(image_path(Path::new("."), path).is_err(), "{path}");
        }
    }

    #[test]
    fn document_identity_and_image_containment() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mdviewer-doc-test-{stamp}"));
        let root = dir.join("documents");
        fs::create_dir_all(&root).unwrap();
        let document = root.join("file.md");
        fs::write(&document, "# Test").unwrap();
        fs::write(root.join("image space.png"), b"image").unwrap();
        fs::write(dir.join("outside.png"), b"outside").unwrap();
        fs::hard_link(&document, root.join("alias.md")).unwrap();
        let mut documents = Documents::default();
        let id = documents.open(&document, 3.0).unwrap();
        assert_eq!(documents.open(&root.join("alias.md"), 0.0).unwrap(), id);
        assert_eq!(documents.tabs.len(), 1);
        assert!(
            image_path(&root, "image%20space.png")
                .unwrap()
                .ends_with("image space.png")
        );
        for path in ["../outside.png", "%2e%2e%2foutside.png", "..\\outside.png"] {
            assert!(image_path(&root, path).is_err());
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
