use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use eframe::egui;
use rust_zip_archive::archive::{self, EntryInfo, Progress};

/// Messages sent from a worker thread back to the UI.
enum JobMsg {
    Progress(Progress),
    Done(Result<(), String>),
}

pub(crate) struct RzaApp {
    /// Currently opened archive (Browse mode).
    archive_path: Option<PathBuf>,
    entries: Vec<EntryInfo>,
    current_dir: String,
    selected: HashSet<String>,
    /// In-flight job channel + last progress.
    job: Option<Receiver<JobMsg>>,
    progress: Option<Progress>,
    status: String,
    /// Files staged for a new archive (Create mode).
    staged: Vec<PathBuf>,
    method: rust_zip_archive::cli::Compression,
}

impl Default for RzaApp {
    fn default() -> Self {
        Self {
            archive_path: None,
            entries: Vec::new(),
            current_dir: String::new(),
            selected: HashSet::new(),
            job: None,
            progress: None,
            status: String::new(),
            staged: Vec::new(),
            method: rust_zip_archive::cli::Compression::Deflate,
        }
    }
}

impl RzaApp {
    pub(crate) fn open_archive(&mut self, path: PathBuf) {
        match archive::list(&path) {
            Ok(entries) => {
                self.status = format!("Opened {} entries", entries.len());
                self.entries = entries;
                self.current_dir.clear();
                self.selected.clear();
                self.archive_path = Some(path);
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
        self.status = "Working\u{2026}".into();
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
            self.selected.iter().cloned().collect()
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

    fn start_create(&mut self, ctx: &egui::Context) {
        if self.staged.is_empty() {
            self.status = "Add files first (drag them in or use New Archive\u{2026}).".into();
            return;
        }
        let Some(output) = rfd::FileDialog::new()
            .add_filter("Archives", &["zip", "tar", "gz", "tgz", "bz2", "xz", "zst"])
            .set_file_name("archive.zip")
            .save_file()
        else {
            return;
        };
        let inputs = self.staged.clone();
        let method = self.method;
        let ctx2 = ctx.clone();
        self.spawn_job(ctx, move |tx| {
            let send_progress = {
                let tx = tx.clone();
                move |p: Progress| {
                    let _ = tx.send(JobMsg::Progress(p));
                    ctx2.request_repaint();
                }
            };
            let result = archive::create(&output, &inputs, method, true, send_progress);
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

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            self.staged.extend(dropped);
            self.status = format!("{} file(s) staged", self.staged.len());
        }

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_enabled_ui(!self.busy(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Open Archive\u{2026}").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter(
                                "Archives",
                                &["zip", "tar", "gz", "tgz", "bz2", "xz", "zst", "7z", "rar"],
                            )
                            .pick_file()
                        {
                            self.open_archive(path);
                        }
                    }
                    if ui.button("New Archive\u{2026}").clicked() {
                        if let Some(files) = rfd::FileDialog::new().pick_files() {
                            self.staged.extend(files);
                            self.status = format!("{} file(s) staged", self.staged.len());
                        }
                    }
                    if let Some(p) = &self.archive_path {
                        ui.label(format!("Open: {}", p.display()));
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("actions").show(ctx, |ui| {
            ui.add_enabled_ui(!self.busy(), |ui| {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_label("Method")
                        .selected_text(format!("{:?}", self.method))
                        .show_ui(ui, |ui| {
                            use rust_zip_archive::cli::Compression::*;
                            for m in [Store, Deflate, Bzip2, Zstd] {
                                ui.selectable_value(&mut self.method, m, format!("{m:?}"));
                            }
                        });
                    if ui.button("Create\u{2026}").clicked() {
                        self.start_create(ctx);
                    }
                    if !self.staged.is_empty() {
                        ui.label(format!("{} staged", self.staged.len()));
                        if ui.button("Clear").clicked() {
                            self.staged.clear();
                        }
                    }
                });
            });
            ui.add_enabled_ui(!self.busy() && self.archive_path.is_some(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Extract Selected\u{2026}").clicked() {
                        self.start_extract(ctx, true);
                    }
                    if ui.button("Extract All\u{2026}").clicked() {
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
            if self.entries.is_empty() {
                ui.label("Open an archive to browse its contents.");
                return;
            }

            // Breadcrumb + Up.
            ui.horizontal(|ui| {
                if ui.button("\u{2b06} Up").clicked() && !self.current_dir.is_empty() {
                    self.current_dir = match self.current_dir.rsplit_once('/') {
                        Some((parent, _)) => parent.to_string(),
                        None => String::new(),
                    };
                }
                if ui.link("\u{1f4c1} root").clicked() {
                    self.current_dir.clear();
                }
                let segments: Vec<String> = if self.current_dir.is_empty() {
                    Vec::new()
                } else {
                    self.current_dir.split('/').map(|s| s.to_string()).collect()
                };
                let mut acc = String::new();
                for seg in segments {
                    ui.label("\u{203a}");
                    if acc.is_empty() {
                        acc = seg.clone();
                    } else {
                        acc = format!("{acc}/{seg}");
                    }
                    if ui.link(&seg).clicked() {
                        self.current_dir = acc.clone();
                    }
                }
            });
            ui.separator();

            let nodes = crate::tree::children(&self.entries, &self.current_dir);
            let mut enter_dir: Option<String> = None;

            // Snapshot selection state before building the table to avoid
            // borrow-checker issues with mutating self.selected inside closures.
            let selected_snapshot: HashSet<String> = self.selected.clone();
            let mut toggle_file: Option<String> = None;
            let mut clear_selection = false;

            use egui_extras::{Column, TableBuilder};
            TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .column(Column::remainder().at_least(180.0)) // Name
                .column(Column::auto().at_least(80.0)) // Size
                .column(Column::auto().at_least(80.0)) // Packed
                .column(Column::auto().at_least(60.0)) // Ratio
                .header(20.0, |mut header| {
                    header.col(|ui| {
                        ui.strong("Name");
                    });
                    header.col(|ui| {
                        ui.strong("Size");
                    });
                    header.col(|ui| {
                        ui.strong("Packed");
                    });
                    header.col(|ui| {
                        ui.strong("Ratio");
                    });
                })
                .body(|mut body| {
                    for node in &nodes {
                        body.row(20.0, |mut row| {
                            let is_selected =
                                !node.is_dir && selected_snapshot.contains(&node.full_path);
                            row.set_selected(is_selected);

                            row.col(|ui| {
                                let glyph = if node.is_dir {
                                    "\u{1f4c1}"
                                } else {
                                    "\u{1f4c4}"
                                };
                                let label = format!("{glyph} {}", node.name);
                                let resp = ui.selectable_label(is_selected, label);
                                if resp.clicked() {
                                    if node.is_dir {
                                        clear_selection = true;
                                    } else {
                                        toggle_file = Some(node.full_path.clone());
                                    }
                                }
                                if resp.double_clicked() && node.is_dir {
                                    enter_dir = Some(node.full_path.clone());
                                }
                            });
                            row.col(|ui| {
                                ui.label(if node.is_dir {
                                    "\u{2014}".into()
                                } else {
                                    human_size(node.size)
                                });
                            });
                            row.col(|ui| {
                                ui.label(if node.is_dir {
                                    "\u{2014}".into()
                                } else {
                                    human_size(node.compressed)
                                });
                            });
                            row.col(|ui| {
                                if node.is_dir || node.size == 0 {
                                    ui.label("\u{2014}");
                                } else {
                                    let ratio =
                                        100.0 * (1.0 - node.compressed as f64 / node.size as f64);
                                    ui.label(format!("{ratio:.0}%"));
                                }
                            });
                        });
                    }
                });

            // Apply deferred mutations after the table block to avoid borrow conflicts.
            if clear_selection {
                self.selected.clear();
            }
            if let Some(path) = toggle_file {
                if !self.selected.remove(&path) {
                    self.selected.insert(path);
                }
            }
            if let Some(dir) = enter_dir {
                self.current_dir = dir;
                self.selected.clear();
            }
        });
    }
}

/// Format a byte count compactly (e.g. 1.5 KB).
fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}
