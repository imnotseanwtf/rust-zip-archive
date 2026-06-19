use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use eframe::egui;
use rust_zip_archive::archive::{self, EntryInfo, Progress};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "rza — Archive Utility",
        options,
        Box::new(|_cc| Ok(Box::new(RzaApp::default()))),
    )
}

/// A row in the browse table.
struct Row {
    info: EntryInfo,
    selected: bool,
}

/// Messages sent from a worker thread back to the UI.
enum JobMsg {
    Progress(Progress),
    Done(Result<(), String>),
}

#[derive(Default)]
struct RzaApp {
    /// Currently opened archive (Browse mode).
    archive_path: Option<PathBuf>,
    rows: Vec<Row>,
    /// In-flight job channel + last progress.
    job: Option<Receiver<JobMsg>>,
    progress: Option<Progress>,
    status: String,
}

impl RzaApp {
    fn open_archive(&mut self, path: PathBuf) {
        match archive::list(&path) {
            Ok(entries) => {
                self.rows = entries
                    .into_iter()
                    .map(|info| Row {
                        info,
                        selected: true,
                    })
                    .collect();
                self.archive_path = Some(path);
                self.status = format!("Opened {} entries", self.rows.len());
            }
            Err(e) => self.status = format!("Error: {e:#}"),
        }
    }

    /// Spawn a worker thread, wiring its progress channel into the UI.
    fn spawn_job<F>(&mut self, ctx: &egui::Context, work: F)
    where
        F: FnOnce(Sender<JobMsg>) + Send + 'static,
    {
        let (tx, rx) = std::sync::mpsc::channel();
        self.job = Some(rx);
        self.progress = None;
        self.status = "Working…".into();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let tx_for_work = tx.clone();
            work(tx_for_work);
            ctx.request_repaint();
        });
    }

    fn start_extract(&mut self, ctx: &egui::Context, selected_only: bool) {
        let Some(archive_path) = self.archive_path.clone() else {
            return;
        };
        let Some(dest) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        let names: Vec<String> = if selected_only {
            self.rows
                .iter()
                .filter(|r| r.selected && !r.info.is_dir)
                .map(|r| r.info.name.clone())
                .collect()
        } else {
            Vec::new()
        };
        let ctx2 = ctx.clone();
        self.spawn_job(ctx, move |tx| {
            let send_progress = {
                let tx = tx.clone();
                let ctx2 = ctx2.clone();
                move |p: Progress| {
                    let _ = tx.send(JobMsg::Progress(p));
                    ctx2.request_repaint();
                }
            };
            let result = if selected_only {
                archive::extract_selected(&archive_path, &dest, &names, true, send_progress)
            } else {
                archive::extract(&archive_path, &dest, true, send_progress)
            };
            let _ = tx.send(JobMsg::Done(result.map_err(|e| format!("{e:#}"))));
        });
    }

    /// Drain worker messages once per frame.
    fn poll_job(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.job {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    JobMsg::Progress(p) => self.progress = Some(p),
                    JobMsg::Done(res) => {
                        self.status = match res {
                            Ok(()) => "Done.".into(),
                            Err(e) => format!("Error: {e}"),
                        };
                        finished = true;
                    }
                }
            }
        }
        if finished {
            self.job = None;
            self.progress = None;
        }
    }

    fn busy(&self) -> bool {
        self.job.is_some()
    }
}

impl eframe::App for RzaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_job();

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_enabled_ui(!self.busy(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Open Archive…").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("zip", &["zip"])
                            .pick_file()
                        {
                            self.open_archive(path);
                        }
                    }
                    if let Some(p) = &self.archive_path {
                        ui.label(format!("Open: {}", p.display()));
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("actions").show(ctx, |ui| {
            ui.add_enabled_ui(!self.busy() && self.archive_path.is_some(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Extract Selected…").clicked() {
                        self.start_extract(ctx, true);
                    }
                    if ui.button("Extract All…").clicked() {
                        self.start_extract(ctx, false);
                    }
                });
            });
            if let Some(p) = &self.progress {
                let frac = if p.total == 0 {
                    0.0
                } else {
                    p.current as f32 / p.total as f32
                };
                ui.add(egui::ProgressBar::new(frac).text(p.message.clone()));
            }
            ui.label(&self.status);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.rows.is_empty() {
                ui.label("Open a .zip to see its contents.");
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("entries").striped(true).show(ui, |ui| {
                    ui.label("");
                    ui.label("Name");
                    ui.label("Size");
                    ui.label("Compressed");
                    ui.end_row();
                    for row in &mut self.rows {
                        ui.add_enabled(
                            !row.info.is_dir,
                            egui::Checkbox::without_text(&mut row.selected),
                        );
                        ui.label(&row.info.name);
                        ui.label(row.info.size.to_string());
                        ui.label(row.info.compressed.to_string());
                        ui.end_row();
                    }
                });
            });
        });
    }
}
