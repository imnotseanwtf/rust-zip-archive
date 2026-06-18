use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "rza — Archive Utility",
        options,
        Box::new(|_cc| Ok(Box::new(RzaApp::default()))),
    )
}

#[derive(Default)]
struct RzaApp {}

impl eframe::App for RzaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("rza — Archive Utility");
            ui.label("GUI scaffold — coming together.");
        });
    }
}
