#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod tree;

use std::path::PathBuf;

use app::RzaApp;
use eframe::egui;

/// What to do with the paths the app was launched with.
pub enum Launch {
    Open(PathBuf),
    Stage(Vec<PathBuf>),
    Empty,
}

/// A single existing, recognizable archive -> open it; one or more other existing
/// paths -> stage them for a new archive; nothing usable -> empty.
pub fn launch_intent(args: &[String]) -> Launch {
    let existing: Vec<PathBuf> = args
        .iter()
        .skip(1)
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect();
    match existing.as_slice() {
        [] => Launch::Empty,
        [one]
            if one.is_file() && rust_zip_archive::archive::format::detect_for_read(one).is_ok() =>
        {
            Launch::Open(one.clone())
        }
        _ => Launch::Stage(existing),
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let intent = launch_intent(&args);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "rza — Archive Utility",
        options,
        Box::new(move |_cc| {
            let mut app = RzaApp::default();
            match intent {
                Launch::Open(p) => app.open_archive(p),
                Launch::Stage(paths) => app.stage_paths(paths),
                Launch::Empty => {}
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::{launch_intent, Launch};

    #[test]
    fn single_archive_opens() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.zip");
        // Minimal valid empty zip (PK end-of-central-directory record).
        std::fs::write(
            &f,
            [
                0x50, 0x4B, 0x05, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        )
        .unwrap();
        let args = vec!["rza-gui".to_string(), f.to_string_lossy().to_string()];
        assert!(matches!(launch_intent(&args), Launch::Open(_)));
    }

    #[test]
    fn multiple_paths_stage() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        std::fs::write(&a, "a").unwrap();
        let b = dir.path().join("b.txt");
        std::fs::write(&b, "b").unwrap();
        let args = vec![
            "rza-gui".into(),
            a.to_string_lossy().into(),
            b.to_string_lossy().into(),
        ];
        match launch_intent(&args) {
            Launch::Stage(v) => assert_eq!(v.len(), 2),
            _ => panic!("expected Stage"),
        }
    }

    #[test]
    fn no_args_is_empty() {
        assert!(matches!(
            launch_intent(&["rza-gui".to_string()]),
            Launch::Empty
        ));
    }
}
