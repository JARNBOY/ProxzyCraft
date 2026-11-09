mod app;
mod devices;
mod model;
mod proxy;
mod theme;

fn main() -> eframe::Result<()> {
    let mut screenshot_path: Option<String> = None;
    let mut screenshot_frames: u32 = 118;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--screenshot" {
            screenshot_path = args.next();
        } else if let Some(rest) = arg.strip_prefix("--screenshot=") {
            screenshot_path = Some(rest.to_string());
        } else if arg == "--screenshot-frames" {
            if let Some(value) = args.next() {
                screenshot_frames = value.parse().unwrap_or(118);
            }
        }
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1180.0, 720.0])
            .with_title("Proxzy — Network Inspector"),
        ..Default::default()
    };

    eframe::run_native(
        "Proxzy",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::App::new(
                cc,
                screenshot_path.clone(),
                screenshot_frames,
            )))
        }),
    )
}
