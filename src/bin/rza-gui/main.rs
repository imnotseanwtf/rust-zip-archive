#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use app::RzaApp;
use eframe::egui;

/// The first CLI argument (after the program name) that names an existing file.
/// File associations launch the app as `rza-gui <path>`; this is how we find it.
fn first_existing_file(args: &[String]) -> Option<std::path::PathBuf> {
    args.iter()
        .skip(1)
        .map(std::path::PathBuf::from)
        .find(|p| p.is_file())
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let initial = first_existing_file(&args);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "rza — Archive Utility",
        options,
        Box::new(move |_cc| {
            let mut app = RzaApp::default();
            if let Some(path) = initial {
                app.open_archive(path);
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::first_existing_file;
    use std::path::PathBuf;

    #[test]
    fn picks_first_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.zip");
        std::fs::write(&f, b"x").unwrap();
        let args = vec!["rza-gui".to_string(), f.to_string_lossy().to_string()];
        assert_eq!(first_existing_file(&args), Some(PathBuf::from(&f)));
    }

    #[test]
    fn none_when_arg_missing_or_absent() {
        let args = vec!["rza-gui".to_string(), "/no/such/file.zip".to_string()];
        assert_eq!(first_existing_file(&args), None);
        let only_prog = vec!["rza-gui".to_string()];
        assert_eq!(first_existing_file(&only_prog), None);
    }
}
