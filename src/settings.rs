use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::os::windows::ffi::OsStrExt;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use windows::{
    Win32::Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
    core::PCWSTR,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub tabs: Vec<SavedTab>,
    pub active: usize,
    pub theme: String,
    pub colors: [String; 2],
    pub custom_colors: BTreeMap<String, String>,
    pub zoom: i32,
    pub window: WindowState,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            tabs: vec![],
            active: 0,
            theme: "light".into(),
            colors: ["#ffffff".into(), "#2864b4".into()],
            custom_colors: BTreeMap::new(),
            zoom: 100,
            window: WindowState::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedTab {
    pub path: Vec<u16>,
    pub scroll: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
    pub dpi_scale: f64,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            x: 100,
            y: 100,
            width: 1100,
            height: 760,
            maximized: false,
            dpi_scale: 1.0,
        }
    }
}

pub fn directory() -> Result<PathBuf> {
    let path = PathBuf::from(std::env::var_os("APPDATA").context("APPDATA is unavailable")?)
        .join("MdViewer");
    fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn load(dir: &Path) -> Result<Settings> {
    let path = dir.join("settings.json");
    match fs::read(&path) {
        Ok(bytes) => {
            let result = serde_json::from_slice::<Settings>(&bytes)
                .context("Invalid settings JSON")
                .and_then(|s| {
                    anyhow::ensure!(s.version == 1, "Unsupported settings version");
                    Ok(s)
                });
            match result {
                Ok(mut s) => {
                    s.zoom = s.zoom.clamp(50, 300) / 10 * 10;
                    if !["light", "dark", "sepia", "custom"].contains(&s.theme.as_str()) {
                        s.theme = "light".into();
                    }
                    for color in &mut s.colors {
                        if color.len() != 7
                            || !color.starts_with('#')
                            || !color[1..].bytes().all(|c| c.is_ascii_hexdigit())
                        {
                            *color = "#2864b4".into();
                        }
                        s.custom_colors.retain(|name, color| {
                            [
                                "base",
                                "text",
                                "surface",
                                "accent",
                                "link",
                                "border",
                                "code",
                                "stripe",
                                "selection",
                                "selection-text",
                                "muted",
                                "syntax",
                                "number",
                                "comment",
                            ]
                            .contains(&name.as_str())
                                && color.len() == 7
                                && color.starts_with('#')
                                && color[1..].bytes().all(|c| c.is_ascii_hexdigit())
                        });
                    }
                    Ok(s)
                }
                Err(error) => {
                    let stamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_secs();
                    fs::rename(&path, dir.join(format!("settings.invalid-{stamp}.json")))?;
                    Err(error)
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(e.into()),
    }
}

pub fn save(dir: &Path, settings: &Settings) -> Result<()> {
    let temp = dir.join("settings.json.tmp");
    let destination = dir.join("settings.json");
    let bytes = serde_json::to_vec_pretty(settings)?;
    let mut file = fs::File::create(&temp)?;
    use std::io::Write;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    let a: Vec<_> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
    let b: Vec<_> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    unsafe {
        MoveFileExW(
            PCWSTR(a.as_ptr()),
            PCWSTR(b.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )?;
    }
    Ok(())
}

static LOG: Mutex<()> = Mutex::new(());
pub fn log(dir: &Path, message: &str) {
    let Ok(_lock) = LOG.lock() else {
        eprintln!("{message}");
        return;
    };
    let result = (|| -> Result<()> {
        let path = dir.join("mdviewer.log");
        if fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024) {
            let old = dir.join("mdviewer.2.log");
            if old.exists() {
                fs::remove_file(&old)?;
            }
            let previous = dir.join("mdviewer.1.log");
            if previous.exists() {
                fs::rename(previous, old)?;
            }
            fs::rename(&path, dir.join("mdviewer.1.log"))?;
        }
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        writeln!(file, "{stamp} {}", message.replace('\n', " "))?;
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("Log error: {e}; {message}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unusual_windows_paths_round_trip() {
        let tab = SavedTab {
            path: vec![68, 58, 92, 0xd800, 46, 109, 100],
            scroll: 123.5,
        };
        let bytes = serde_json::to_vec(&tab).unwrap();
        let restored: SavedTab = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(tab.path, restored.path);
        assert_eq!(tab.scroll, restored.scroll);
    }

    #[test]
    fn older_settings_default_to_no_custom_color_overrides() {
        let settings: Settings = serde_json::from_value(serde_json::json!({
            "version": 1,
            "tabs": [],
            "active": 0,
            "theme": "light",
            "colors": ["#ffffff", "#2864b4"],
            "zoom": 100
        }))
        .unwrap();
        assert!(settings.custom_colors.is_empty());
    }

    #[test]
    fn atomic_replacement_and_corruption_backup() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mdviewer-settings-test-{stamp}"));
        fs::create_dir(&dir).unwrap();
        let mut prefs = Settings::default();
        save(&dir, &prefs).unwrap();
        prefs.zoom = 150;
        save(&dir, &prefs).unwrap();
        assert_eq!(load(&dir).unwrap().zoom, 150);
        assert!(!dir.join("settings.json.tmp").exists());
        fs::write(dir.join("settings.json"), b"broken json").unwrap();
        assert!(load(&dir).is_err());
        assert!(!dir.join("settings.json").exists());
        assert!(fs::read_dir(&dir).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("settings.invalid-")
        }));
        fs::remove_dir_all(dir).unwrap();
    }
}
