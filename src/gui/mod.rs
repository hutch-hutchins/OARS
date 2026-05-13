pub mod app;
mod highlighter;

pub fn launch() -> ! {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("OARS — Oxide Assembler and Runtime Simulator")
            .with_icon(std::sync::Arc::new(app_icon())),
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

fn app_icon() -> egui::IconData {
    let bytes = include_bytes!("../../assets/icon_app.png");
    let img = image::load_from_memory(bytes)
        .expect("bundled icon_app.png is valid PNG")
        .into_rgba8();
    let (w, h) = img.dimensions();
    egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    }
}
