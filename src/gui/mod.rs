pub mod app;

pub fn launch() -> ! {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("OARS — Oxide Assembler and Runtime Simulator"),
        ..Default::default()
    };

    eframe::run_native(
        "OARS",
        options,
        Box::new(|cc| Ok(Box::new(app::OarsApp::new(cc)))),
    )
    .expect("GUI failed to start");

    std::process::exit(0);
}
