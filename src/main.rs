use cathode8::app;

fn main() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 720.0])
            .with_min_inner_size([640.0, 480.0])
            .with_title("Cathode-8"),
        vsync: true,
        ..Default::default()
    };

    eframe::run_native(
        "Cathode-8",
        options,
        Box::new(|cc| Ok(Box::new(app::NesApp::new(cc)))),
    )
    .map_err(|err| anyhow::anyhow!("failed to run app: {err}"))
}
