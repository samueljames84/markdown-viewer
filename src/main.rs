#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod documents;
mod settings;
mod watcher;
mod windows;

#[derive(Debug)]
enum Event {
    Ipc(String),
    Menu(String),
    Open(Vec<std::path::PathBuf>, bool),
    Changed(Vec<std::path::PathBuf>),
    Error(String),
    PdfFinished(Result<String, String>),
}

fn main() {
    if let Err(error) = app::run() {
        let text = format!("{error:#}");
        if let Ok(dir) = settings::directory() {
            settings::log(&dir, &text);
        }
        rfd::MessageDialog::new()
            .set_title("MdViewer")
            .set_level(rfd::MessageLevel::Error)
            .set_description(&text)
            .show();
        std::process::exit(1);
    }
}
