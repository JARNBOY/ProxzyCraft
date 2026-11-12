mod app;
mod devices;
mod model;
mod proxy;
mod studio;
mod theme;

fn main() -> eframe::Result<()> {
    let mut screenshot_path: Option<String> = None;
    let mut screenshot_frames: u32 = 118;
    let mut headless: bool = false;
    let mut port: u16 = 9090;
    let mut lan = false;
    let mut bind_ip: Option<std::net::IpAddr> = None;
    let mut control_token: Option<String> = None;
    let mut auto_record: bool = false;
    let mut auto_record_domains: Vec<String> = Vec::new();
    let mut full_catch = false;
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
        } else if arg == "--headless" || arg == "-d" || arg == "--daemon" {
            headless = true;
        } else if arg == "--lan" {
            lan = true;
        } else if arg == "--bind" {
            if let Some(value) = args.next() {
                match value.parse() {
                    Ok(ip) => bind_ip = Some(ip),
                    Err(_) => {
                        eprintln!("[ProxzyCraft] --bind needs an IP address, got {}", value);
                        std::process::exit(2);
                    }
                }
            }
        } else if let Some(rest) = arg.strip_prefix("--bind=") {
            match rest.parse() {
                Ok(ip) => bind_ip = Some(ip),
                Err(_) => {
                    eprintln!("[ProxzyCraft] --bind needs an IP address, got {}", rest);
                    std::process::exit(2);
                }
            }
        } else if arg == "--control-token" {
            control_token = args.next();
        } else if let Some(rest) = arg.strip_prefix("--control-token=") {
            control_token = Some(rest.to_string());
        } else if arg == "--port" {
            if let Some(p) = args.next() {
                port = p.parse().unwrap_or(9090);
            }
        } else if let Some(rest) = arg.strip_prefix("--port=") {
            port = rest.parse().unwrap_or(9090);
        } else if arg == "--auto-record" {
            auto_record = true;
        } else if arg == "--full-catch" {
            full_catch = true;
        } else if let Some(rest) = arg.strip_prefix("--auto-record=") {
            auto_record = true;
            for d in rest.split(',') {
                let trimmed = d.trim();
                if !trimmed.is_empty() {
                    auto_record_domains.push(trimmed.to_string());
                }
            }
        }
    }

    let options = proxy::ProxyStartOptions {
        port,
        lan_enabled: lan,
        bind_ip,
        control_token,
    };

    if headless {
        println!("[ProxzyCraft] Running in headless daemon mode on port {}...", port);
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut handle = match proxy::start_with_options(options, tx) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("[ProxzyCraft] Error starting proxy: {}", e);
                std::process::exit(1);
            }
        };

        if auto_record {
            println!("[ProxzyCraft] Auto-record mode ENABLED for live 200 OK responses -> saving to mocks");
            if let Ok(mut cfg) = handle.config.write() {
                cfg.auto_record = true;
                cfg.auto_record_domains = auto_record_domains.clone();
                for d in &auto_record_domains {
                    if !cfg.ssl_allowlist.iter().any(|existing| existing.eq_ignore_ascii_case(d)) {
                        cfg.ssl_allowlist.push(d.clone());
                    }
                }
            }
        }

        if full_catch {
            if let Ok(mut cfg) = handle.config.write() {
                cfg.intercept_all = true;
                cfg.ssl_proxying_enabled = true;
            }
            println!("[ProxzyCraft] Full HTTPS catch is ON (protected system services stay tunnelled).");
        }

        println!(
            "[ProxzyCraft] MITM Proxy & WireMock Control Plane active on http://{}",
            handle.binding.display_address()
        );
        if handle.binding.policy.is_lan() {
            println!(
                "[ProxzyCraft] LAN access is ON. Certificate portal: http://{}/  (ca.cer, ca.pem)",
                handle.binding.display_address()
            );
            println!(
                "[ProxzyCraft] Control token file: {}",
                proxy::app_data_dir().join("control.token").display()
            );
            println!("[ProxzyCraft] Send X-Proxzy-Control on /__proxzy/* requests from other devices.");
        }
        println!("[ProxzyCraft] CA SHA-256: {}", handle.ca.fingerprint);
        println!("[ProxzyCraft] Ready for automated MobileWright E2E suite.");

        // Keep process running until termination
        let (_term_tx, term_rx) = std::sync::mpsc::channel::<()>();
        let _ = term_rx.recv();
        handle.stop();
        return Ok(());
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
                port,
            )))
        }),
    )
}
