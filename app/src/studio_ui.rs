// ---------------------------------------------------------------------------
// Automation studio screens: Agent Control, API Test Bench, Screen Play & MCP,
// and the Map Local connector pipeline.
//
// This file is included from app.rs, so it shares the App fields and can call
// the existing helpers (cards, chips, form labels, curl sender, log, copy).
// ---------------------------------------------------------------------------

impl App {
    // =====================================================================
    // Shared bits
    // =====================================================================

    fn studio_heading(ui: &mut Ui, title: &str, subtitle: &str) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(RichText::new(title).size(24.0).strong().color(TEXT));
            ui.label(RichText::new(subtitle).size(11.5).color(MUTED));
        });
    }

    fn engine_online(&self) -> bool {
        self.proxy.is_some()
    }

    fn session_snapshot(&self) -> Option<TestSession> {
        self.proxy
            .as_ref()
            .and_then(|handle| handle.config.read().ok())
            .and_then(|cfg| cfg.active_session.clone())
    }

    fn target_uses_lan(&self, platform: TargetPlatform) -> bool {
        matches!(platform, TargetPlatform::IosDevice)
            || (platform == TargetPlatform::AndroidDevice
                && self.android_transport == AndroidTransport::WifiLan)
    }

    fn screen_target_host(&self) -> String {
        let lan_ip = if self.lan_enabled && !self.lan_ip.trim().is_empty() {
            self.lan_ip.as_str()
        } else {
            "127.0.0.1"
        };
        studio::host_for_target(self.screen_target, lan_ip, self.android_transport)
    }

    fn screen_target_token(&self) -> Option<String> {
        let token = self.control_token.trim();
        if (self.lan_enabled || self.target_uses_lan(self.screen_target)) && !token.is_empty() {
            Some(token.to_string())
        } else {
            None
        }
    }

    fn target_bridge_command(&self, platform: TargetPlatform, port: u16) -> Option<String> {
        studio::bridge_command_for(platform, port, self.android_transport)
    }

    fn android_transport_chips(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("ANDROID LINK")
                    .size(8.5)
                    .strong()
                    .color(FAINT),
            );
            for transport in AndroidTransport::ALL {
                let active = self.android_transport == transport;
                if chip(ui, transport.label(), active).clicked() {
                    self.android_transport = transport;
                }
            }
        });
    }

    fn set_rule_scenario(&mut self, ctx: &egui::Context, id: u64, scenario: MockScenario) {
        let name = if let Some(rule) = self.map_rules.iter_mut().find(|r| r.id == id) {
            rule.switch_scenario(scenario);
            rule.enabled = true;
            rule.name.clone()
        } else {
            return;
        };
        self.map_enabled = true;
        self.sync_proxy_config();
        self.log(
            Level::Warn,
            "Agent Control",
            format!("Switched \"{}\" to {}", name, scenario.label()),
        );
        if let Some(index) = self.map_rules.iter().position(|r| r.id == id) {
            let rule = self.map_rules[index].clone();
            self.record_agent_action(
                "SCENARIO",
                &format!("{} → {}", rule.name, scenario.slug()),
            );
        }
        self.toast_now(ctx, &format!("{} is now {}", name, scenario.label()));
    }

    fn set_rule_live(&mut self, ctx: &egui::Context, id: u64) {
        let name = if let Some(rule) = self.map_rules.iter_mut().find(|r| r.id == id) {
            rule.enabled = false;
            rule.name.clone()
        } else {
            return;
        };
        self.sync_proxy_config();
        self.log(
            Level::Info,
            "Agent Control",
            format!("Bypassed \"{}\" — live traffic passes through", name),
        );
        self.record_agent_action("PASSTHROUGH", &name);
        self.toast_now(ctx, &format!("{} now passes through to the live API", name));
    }

    fn inject_into_rule(&mut self, ctx: &egui::Context, id: u64, key: &str, value: &str) {
        let key = key.trim();
        if key.is_empty() {
            self.toast_now(ctx, "Enter a property name first");
            return;
        }
        let name = if let Some(rule) = self.map_rules.iter_mut().find(|r| r.id == id) {
            rule.inject_props.insert(key.to_string(), value.to_string());
            rule.enabled = true;
            rule.name.clone()
        } else {
            return;
        };
        self.map_enabled = true;
        self.sync_proxy_config();
        self.log(
            Level::Warn,
            "Agent Control",
            format!("Injected {}={} into \"{}\"", key, value, name),
        );
        self.record_agent_action("INJECT", &format!("{} ← {}", name, key));
        self.toast_now(ctx, &format!("{} injected into {}", key, name));
    }

    fn record_agent_action(&mut self, method: &str, summary: &str) {
        self.agent_feed.insert(
            0,
            ControlEvent {
                at: now_stamp(),
                method: method.into(),
                path: summary.into(),
                ok: true,
            },
        );
        if self.agent_feed.len() > 24 {
            self.agent_feed.truncate(24);
        }
    }

    fn start_agent_session(&mut self, ctx: &egui::Context) {
        let Some(handle) = &self.proxy else {
            self.toast_now(ctx, "Start the proxy first");
            return;
        };
        let name = self.agent_session_name.trim().to_string();
        let env = self.agent_session_env.trim().to_string();
        let version = self.agent_session_version.trim().to_string();
        let session = TestSession::new(
            if name.is_empty() { "TC_SESSION_01" } else { &name },
            "Automation",
            &version,
            "",
            &env,
            Vec::new(),
            Vec::new(),
        );
        let display = session.name.clone();
        if let Ok(mut cfg) = handle.config.write() {
            cfg.active_session = Some(session);
        }
        self.log(
            Level::Info,
            "Session",
            format!("Started \"{}\" (v{} @ {})", display, version, env),
        );
        self.record_agent_action("SESSION", &format!("start {}", display));
        self.toast_now(ctx, "Test session started");
    }

    fn end_agent_session(&mut self, ctx: &egui::Context) {
        let Some(handle) = &self.proxy else {
            self.toast_now(ctx, "No proxy is running");
            return;
        };
        let summary = handle
            .config
            .write()
            .ok()
            .and_then(|mut cfg| cfg.active_session.take());
        if let Some(session) = summary {
            self.log(
                Level::Info,
                "Session",
                format!(
                    "Ended \"{}\" — {} calls ({} mocked, {} upstream), {} breaking drift(s)",
                    session.name,
                    session.total_calls,
                    session.mock_calls,
                    session.upstream_calls,
                    session.breaking_drifts
                ),
            );
            self.record_agent_action("SESSION", &format!("end {}", session.name));
            self.toast_now(ctx, "Session ended — summary written to the logs");
        } else {
            self.toast_now(ctx, "There is no active session");
        }
    }

    fn restart_engine(&mut self, ctx: &egui::Context) {
        let Ok(port) = self.agent_port_input.trim().parse::<u16>() else {
            self.toast_now(ctx, "Enter a port number between 1 and 65535");
            return;
        };
        if port == 0 {
            self.toast_now(ctx, "Enter a port number between 1 and 65535");
            return;
        }
        let old_port = self.proxy_port;
        if let Some(mut handle) = self.proxy.take() {
            handle.stop();
        }
        self.proxy_rx = None;
        self.ca_info = None;
        let (tx, rx) = channel();
        match proxy::start(port, tx) {
            Ok(handle) => {
                self.proxy_port = handle.port;
                self.ca_info = Some(handle.ca.clone());
                self.proxy = Some(handle);
                self.proxy_rx = Some(rx);
                self.proxy_error = None;
                self.agent_port_input = self.proxy_port.to_string();
                self.sync_proxy_config();
                self.log(
                    Level::Info,
                    "Proxy",
                    format!("Engine restarted on 127.0.0.1:{}", self.proxy_port),
                );
                self.toast_now(ctx, &format!("Engine listening on port {}", self.proxy_port));
            }
            Err(err) => {
                self.proxy_error = Some(err.clone());
                self.log(Level::Error, "Proxy", err);
                // Put the engine back on the old port so the person is never
                // left with the app thinking it is listening when it is not.
                let (tx, rx) = channel();
                if let Ok(handle) = proxy::start(old_port, tx) {
                    self.proxy_port = handle.port;
                    self.ca_info = Some(handle.ca.clone());
                    self.proxy = Some(handle);
                    self.proxy_rx = Some(rx);
                    self.agent_port_input = self.proxy_port.to_string();
                    self.sync_proxy_config();
                }
                self.toast_now(ctx, "That port is busy — kept the previous engine");
            }
        }
    }

    // =====================================================================
    // Agent Control
    // =====================================================================

    fn agent_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let port = self.proxy_port;
        let online = self.engine_online();
        let session = self.session_snapshot();
        ui.horizontal(|ui| {
            Self::studio_heading(
                ui,
                "Agent Control",
                "Let an agent switch mocks, inject values and open a test session — then watch the calls arrive.",
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                pill(
                    ui,
                    if online { "ENGINE READY" } else { "ENGINE OFF" },
                    if online { GREEN } else { AMBER },
                    if online {
                        Color32::from_rgba_unmultiplied(61, 214, 149, 18)
                    } else {
                        Color32::from_rgba_unmultiplied(255, 181, 84, 18)
                    },
                );
                ui.label(
                    RichText::new(format!("127.0.0.1:{}", port))
                        .font(mono(10.0))
                        .color(FAINT),
                );
            });
        });
        ui.add_space(10.0);

        let hook_offset = self.agent_hook_offset();
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .vertical_scroll_offset(hook_offset)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    stat_box(
                        ui,
                        154.0,
                        "CONTROL PLANE",
                        if online { "READY" } else { "OFF" },
                        if online { GREEN } else { AMBER },
                    );
                    stat_box(ui, 138.0, "ENGINE PORT", &port.to_string(), ACCENT);
                    stat_box(
                        ui,
                        176.0,
                        "TEST SESSION",
                        session
                            .as_ref()
                            .map(|s| s.name.as_str())
                            .unwrap_or("not started"),
                        if session.is_some() { CYAN } else { FAINT },
                    );
                    stat_box(
                        ui,
                        138.0,
                        "AGENT CALLS",
                        &self.agent_feed.len().to_string(),
                        if self.agent_feed.is_empty() { FAINT } else { PURPLE },
                    );
                });
                ui.add_space(12.0);

                let gap = 14.0;
                let total = ui.available_width();
                let left = ((total - gap) * 0.60).clamp(540.0, 720.0);
                let right = (total - left - gap).max(300.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(left, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(left);
                            self.agent_left(ui, ctx, session.as_ref());
                        },
                    );
                    ui.add_space(gap);
                    ui.allocate_ui_with_layout(
                        Vec2::new(right, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(right);
                            self.agent_right(ui, ctx, session.as_ref());
                        },
                    );
                });
                ui.add_space(12.0);
                self.agent_feed_card(ui);
                ui.add_space(10.0);
                self.agent_endpoint_card(ui, ctx, port);
                ui.add_space(10.0);
                self.agent_ide_card(ui, ctx, port);
                ui.add_space(16.0);
            });
    }

    fn agent_left(&mut self, ui: &mut Ui, ctx: &egui::Context, session: Option<&TestSession>) {
        let _ = session;
        let mut edit: Option<MapRule> = None;
        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Mock control").size(14.0).strong().color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let on = self.map_enabled;
                    if toggle(ui, on).clicked() {
                        self.map_enabled = !on;
                        self.sync_proxy_config();
                        self.log(
                            if on { Level::Warn } else { Level::Info },
                            "Agent Control",
                            if on {
                                "Map Local engine turned off by the person"
                            } else {
                                "Map Local engine armed for agent traffic"
                            },
                        );
                    }
                    ui.label(
                        RichText::new(if on { "Map Local armed" } else { "Map Local off" })
                            .size(10.0)
                            .color(if on { GREEN } else { FAINT }),
                    );
                });
            });
            ui.add_space(2.0);
            studio_note(
                ui,
                "Pick a rule and switch its state in one click. An agent can do the same through the control plane.",
            );
            ui.add_space(8.0);

            let mut folders: Vec<String> = self
                .map_rules
                .iter()
                .map(|r| r.folder_name().to_string())
                .collect();
            folders.sort();
            folders.dedup();
            let mut folder_pick: Option<Option<String>> = None;
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("MODULE").size(9.0).color(FAINT).strong());
                if chip(ui, &format!("All ({})", self.map_rules.len()), self.agent_folder.is_none())
                    .clicked()
                {
                    folder_pick = Some(None);
                }
                for folder in folders {
                    let active = self.agent_folder.as_deref() == Some(folder.as_str());
                    if chip(ui, &folder, active).clicked() {
                        folder_pick = Some(Some(folder.clone()));
                    }
                }
            });
            if let Some(pick) = folder_pick {
                self.agent_folder = pick;
            }
            ui.add_space(6.0);

            let rules: Vec<MapRule> = self
                .map_rules
                .iter()
                .filter(|r| {
                    self.agent_folder
                        .as_deref()
                        .map(|f| r.folder_name() == f)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            let mut switch: Option<(u64, MockScenario)> = None;
            let mut live: Option<u64> = None;
            let mut select: Option<u64> = None;
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), 236.0),
                Layout::top_down(Align::Min),
                |ui| {
            ScrollArea::vertical()
                .id_salt("agent_rules")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if rules.is_empty() {
                        studio_note(ui, "No Map Local rules in this module yet.");
                    }
                    for rule in &rules {
                        let selected = self.agent_rule == Some(rule.id)
                            || (self.agent_rule.is_none() && rules.first().map(|r| r.id) == Some(rule.id));
                        let row = Frame::none()
                            .fill(if selected { HOVER } else { INPUT })
                            .stroke(Stroke::new(
                                1.0,
                                if selected { Color32::from_rgb(70, 92, 130) } else { BORDER },
                            ))
                            .rounding(8.0)
                            .inner_margin(Margin::symmetric(9.0, 7.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    dot(
                                        ui,
                                        if rule.enabled { GREEN } else { FAINT },
                                        6.0,
                                    );
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(200.0, 32.0),
                                        Layout::top_down(Align::Min),
                                        |ui| {
                                            ui.set_width(200.0);
                                            if ui
                                                .add(
                                                    egui::Label::new(
                                                        RichText::new(&rule.name)
                                                            .size(11.0)
                                                            .strong()
                                                            .color(TEXT),
                                                    )
                                                    .sense(Sense::click()),
                                                )
                                                .clicked()
                                            {
                                                select = Some(rule.id);
                                            }
                                            ui.label(
                                                RichText::new(format!(
                                                    "{}  {}",
                                                    rule.method.label(),
                                                    rule.pattern
                                                ))
                                                .font(mono(9.0))
                                                .color(FAINT),
                                            );
                                        },
                                    );
                                    for scenario in [
                                        MockScenario::Success,
                                        MockScenario::Error,
                                        MockScenario::Empty,
                                    ] {
                                        let active = rule.enabled
                                            && rule.active_scenario == scenario;
                                        if chip(ui, scenario.label(), active).clicked() {
                                            switch = Some((rule.id, scenario));
                                        }
                                    }
                                    let live_active = !rule.enabled;
                                    if chip(ui, "Live", live_active).clicked() {
                                        live = Some(rule.id);
                                    }
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(
                                            RichText::new(format!("{} hits", rule.hits))
                                                .font(mono(9.0))
                                                .color(if rule.hits > 0 { CYAN } else { FAINT }),
                                        );
                                    });
                                });
                            });
                        let _ = row;
                        ui.add_space(4.0);
                    }
                });
                },
            );
            if let Some(pick) = select {
                self.agent_rule = Some(pick);
            }
            if let Some((id, scenario)) = switch {
                self.agent_rule = Some(id);
                self.set_rule_scenario(ctx, id, scenario);
            }
            if let Some(id) = live {
                self.agent_rule = Some(id);
                self.set_rule_live(ctx, id);
            }

            let selected_rule = self
                .agent_rule
                .and_then(|id| self.map_rules.iter().find(|r| r.id == id))
                .cloned()
                .or_else(|| self.map_rules.first().cloned());
            ui.add_space(10.0);
            hsep(ui);
            ui.add_space(9.0);
            if let Some(rule) = selected_rule {
                ui.horizontal(|ui| {
                    pill(
                        ui,
                        "CONNECTOR FLOW",
                        ACCENT,
                        Color32::from_rgba_unmultiplied(88, 140, 255, 18),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} {} → {} → {} → {}",
                            rule.method.label(),
                            rule.pattern,
                            rule.active_scenario.label(),
                            if rule.inject_props.is_empty() {
                                "no injection".to_string()
                            } else {
                                format!("{} injected", rule.inject_props.len())
                            },
                            rule.status
                        ))
                        .font(mono(9.5))
                        .color(MUTED),
                    );
                });
                ui.add_space(7.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Inject into \"{}\"", rule.name))
                            .size(10.0)
                            .strong()
                            .color(TEXT),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if secondary_button(ui, "Edit rule").clicked() {
                            edit = Some(rule.clone());
                        }
                    });
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let key_w = 120.0;
                    input_frame().show(ui, |ui| {
                        ui.set_width(key_w);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.agent_inject_key)
                                .frame(false)
                                .desired_width(key_w)
                                .font(mono(10.0))
                                .hint_text("USERNAME"),
                        );
                    });
                    ui.label(RichText::new("=").color(FAINT));
                    let value_w = (ui.available_width() - 90.0).max(90.0);
                    input_frame().show(ui, |ui| {
                        ui.set_width(value_w);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.agent_inject_value)
                                .frame(false)
                                .desired_width(value_w)
                                .font(mono(10.0))
                                .hint_text("qa.agent"),
                        );
                    });
                    if primary_button(ui, "Inject").clicked() {
                        let key = self.agent_inject_key.clone();
                        let value = self.agent_inject_value.clone();
                        self.inject_into_rule(ctx, rule.id, &key, &value);
                    }
                });
                ui.add_space(5.0);
                if !rule.inject_props.is_empty() {
                    let mut tokens: Vec<(String, String)> =
                        rule.inject_props.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                    tokens.sort();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("BODY TOKENS").size(8.5).color(FAINT).strong());
                        for (key, value) in tokens {
                            pill(
                                ui,
                                &format!("{{{{{}}}}} = {}", key, value),
                                GREEN,
                                Color32::from_rgba_unmultiplied(61, 214, 149, 16),
                            );
                        }
                    });
                    studio_note(
                        ui,
                        "The token is replaced in the response body when this rule is served.",
                    );
                } else {
                    studio_note(
                        ui,
                        "No injection yet. Add KEY / value and it appears as {{KEY}} inside the body.",
                    );
                }
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    if secondary_button(ui, "Open connector pipeline").clicked() {
                        self.map_view = MapViewMode::Pipeline;
                        self.view = View::MapLocal;
                    }
                    if secondary_button(ui, "Send a test request").clicked() {
                        self.view = View::Tests;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !rule.enabled {
                            pill(ui, "PASSING THROUGH", AMBER, Color32::from_rgba_unmultiplied(255, 181, 84, 18));
                        }
                    });
                });
            }
        });
        if let Some(rule) = edit {
            self.open_map_editor(Some(rule));
        }
    }

    // ---------------------------------------------------------------------
    // Agent Control — right column
    // ---------------------------------------------------------------------

    fn agent_right(&mut self, ui: &mut Ui, ctx: &egui::Context, session: Option<&TestSession>) {
        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Engine port").size(13.0).strong().color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    dot(
                        ui,
                        if self.engine_online() { GREEN } else { FAINT },
                        6.0,
                    );
                    ui.label(
                        RichText::new(if self.engine_online() {
                            "listening"
                        } else {
                            "stopped"
                        })
                        .size(9.5)
                        .color(FAINT),
                    );
                });
            });
            ui.add_space(2.0);
            studio_note(
                ui,
                "The control plane lives on this port. Change it here and every snippet updates.",
            );
            ui.add_space(7.0);
            ui.horizontal(|ui| {
                input_frame().show(ui, |ui| {
                    ui.set_width(84.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.agent_port_input)
                            .frame(false)
                            .desired_width(84.0)
                            .font(mono(11.0))
                            .hint_text("9090"),
                    );
                });
                if primary_button(ui, "Restart engine").clicked() {
                    self.restart_engine(ctx);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("http://127.0.0.1:{}", self.proxy_port))
                            .font(mono(9.0))
                            .color(FAINT),
                    );
                });
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new("MOBILE BRIDGE")
                    .size(8.5)
                    .strong()
                    .color(FAINT),
            );
            ui.add_space(3.0);
            ui.horizontal_wrapped(|ui| {
                for platform in TargetPlatform::ALL {
                    let active = self.screen_target == platform;
                    if chip(ui, platform.badge(), active)
                        .on_hover_text(format!("target id: {}", platform.slug()))
                        .clicked()
                    {
                        self.screen_target = platform;
                    }
                }
            });
            if self.screen_target == TargetPlatform::AndroidDevice {
                ui.add_space(4.0);
                self.android_transport_chips(ui);
            }
            ui.add_space(4.0);
            let needs_lan = self.target_uses_lan(self.screen_target);
            let mut turn_on_lan = false;
            if needs_lan && !self.lan_enabled {
                ui.horizontal(|ui| {
                    pill(
                        ui,
                        "LAN OFF",
                        AMBER,
                        Color32::from_rgba_unmultiplied(255, 181, 84, 18),
                    );
                    if secondary_button(ui, "Turn on LAN access").clicked() {
                        turn_on_lan = true;
                    }
                });
                ui.add_space(3.0);
            }
            if turn_on_lan {
                self.set_lan_enabled(ctx, true);
            }
            let bridge = self.target_bridge_command(self.screen_target, self.proxy_port);
            match bridge {
                Some(command) => {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(command.clone())
                                .font(mono(9.5))
                                .color(CYAN),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if secondary_button(ui, "Copy").clicked() {
                                self.copy(ctx, command.clone(), "Bridge command copied");
                            }
                        });
                    });
                    studio_note(ui, self.screen_target.note());
                }
                None => {
                    let host = self.screen_target_host();
                    if needs_lan && self.lan_enabled {
                        studio_note(
                            ui,
                            &format!(
                                "Point the phone HTTP proxy at {}:{} and send X-Proxzy-Control from test harnesses.",
                                host, self.proxy_port
                            ),
                        );
                    } else if needs_lan {
                        studio_note(ui, self.screen_target.note());
                    } else {
                        studio_note(
                            ui,
                            "This target sends traffic straight to the loopback port — nothing to bridge.",
                        );
                    }
                }
            }
        });
        ui.add_space(10.0);

        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Test session").size(13.0).strong().color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if session.is_some() {
                        if secondary_button(ui, "End").clicked() {
                            self.end_agent_session(ctx);
                        }
                    } else if primary_button(ui, "Start").clicked() {
                        self.start_agent_session(ctx);
                    }
                });
            });
            ui.add_space(2.0);
            studio_note(
                ui,
                "A session records which calls were mocked or upstream, plus any drift an agent introduced.",
            );
            ui.add_space(7.0);
            ui.horizontal(|ui| {
                form_label(ui, "NAME");
                input_frame().show(ui, |ui| {
                    ui.set_width(120.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.agent_session_name)
                            .frame(false)
                            .desired_width(120.0)
                            .font(mono(10.0))
                            .hint_text("TC_WALLET_01"),
                    );
                });
                form_label(ui, "ENV");
                input_frame().show(ui, |ui| {
                    ui.set_width(70.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.agent_session_env)
                            .frame(false)
                            .desired_width(70.0)
                            .font(mono(10.0))
                            .hint_text("preprod"),
                    );
                });
                form_label(ui, "VER");
                input_frame().show(ui, |ui| {
                    ui.set_width(56.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.agent_session_version)
                            .frame(false)
                            .desired_width(56.0)
                            .font(mono(10.0))
                            .hint_text("1.1.0"),
                    );
                });
            });
            ui.add_space(8.0);
            match session {
                Some(session) => {
                    ui.horizontal(|ui| {
                        stat_box(ui, 112.0, "CALLS", &session.total_calls.to_string(), ACCENT);
                        stat_box(ui, 112.0, "MOCKED", &session.mock_calls.to_string(), GREEN);
                        stat_box(ui, 112.0, "UPSTREAM", &session.upstream_calls.to_string(), CYAN);
                        stat_box(
                            ui,
                            112.0,
                            "DRIFTS",
                            &session.breaking_drifts.to_string(),
                            if session.breaking_drifts > 0 { AMBER } else { FAINT },
                        );
                    });
                    ui.add_space(4.0);
                    studio_note(
                        ui,
                        &format!(
                            "{} · v{} @ {} · started {}",
                            session.name, session.app_version, session.env, session.started_at
                        ),
                    );
                }
                None => {
                    studio_note(
                        ui,
                        "No session yet. Start one before the UI test launches so drift is attributed.",
                    );
                }
            }
        });
        ui.add_space(10.0);

        let port = self.proxy_port;
        let target = self
            .agent_rule
            .and_then(|id| self.map_rules.iter().find(|r| r.id == id))
            .map(|r| r.name.clone())
            .unwrap_or_else(|| "Wallet balance".to_string());
        studio_card(ui, |ui| {
            let host = self.screen_target_host();
            let token = self.screen_target_token();
            let token_ref = token.as_deref();
            let health_cmd = studio::health_curl_for(&host, port, token_ref);
            let scenario_cmd =
                studio::scenario_curl_for(&host, port, &target, "error", token_ref);
            let inject_cmd =
                studio::inject_curl_for(&host, port, &target, "USERNAME", "qa.agent", token_ref);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Give the agent a command")
                        .size(13.0)
                        .strong()
                        .color(TEXT),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "Health").clicked() {
                        self.copy(ctx, health_cmd.clone(), "Health check copied");
                    }
                });
            });
            ui.add_space(2.0);
            studio_note(
                ui,
                "Three calls cover most runs: read health, switch a scenario, inject a property.",
            );
            ui.add_space(7.0);
            form_label(ui, "HEALTH");
            code_block(ui, &health_cmd, 52.0, false);
            ui.add_space(5.0);
            form_label(ui, "SWITCH SCENARIO");
            code_block(ui, &scenario_cmd, 74.0, false);
            ui.add_space(5.0);
            form_label(ui, "INJECT A PROPERTY");
            code_block(ui, &inject_cmd, 74.0, false);
        });
    }

    fn agent_feed_card(&mut self, ui: &mut Ui) {
        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Control plane feed")
                        .size(13.0)
                        .strong()
                        .color(TEXT),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !self.agent_feed.is_empty() && secondary_button(ui, "Clear").clicked() {
                        self.agent_feed.clear();
                    }
                    ui.label(
                        RichText::new("Newest first · in-app actions and agent calls")
                            .size(9.0)
                            .color(FAINT),
                    );
                });
            });
            ui.add_space(6.0);
            if self.agent_feed.is_empty() {
                Frame::none()
                    .fill(INPUT)
                    .stroke(Stroke::new(1.0, BORDER))
                    .rounding(8.0)
                    .inner_margin(Margin::symmetric(10.0, 14.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("Nothing has driven the engine yet")
                                    .size(11.0)
                                    .color(MUTED),
                            );
                            ui.label(
                                RichText::new(
                                    "Switch a scenario above, or point an external agent at the control plane.",
                                )
                                .size(9.5)
                                .color(FAINT),
                            );
                        });
                    });
            } else {
                for event in self.agent_feed.iter().take(6) {
                    let color = if event.method == "INJECT" {
                        AMBER
                    } else if event.method == "SCENARIO" {
                        ACCENT
                    } else if event.method == "SESSION" {
                        PURPLE
                    } else if event.method == "PASSTHROUGH" {
                        MUTED
                    } else {
                        CYAN
                    };
                    Frame::none()
                        .fill(INPUT)
                        .stroke(Stroke::new(1.0, BORDER))
                        .rounding(7.0)
                        .inner_margin(Margin::symmetric(9.0, 6.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&event.at).font(mono(8.5)).color(FAINT),
                                );
                                pill(
                                    ui,
                                    &format!(" {} ", event.method),
                                    color,
                                    Color32::from_rgba_unmultiplied(
                                        color.r(),
                                        color.g(),
                                        color.b(),
                                        16,
                                    ),
                                );
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&event.path).size(10.0).color(TEXT),
                                    )
                                    .truncate(),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if event.ok {
                                        pill(
                                            ui,
                                            "OK",
                                            GREEN,
                                            Color32::from_rgba_unmultiplied(61, 214, 149, 18),
                                        );
                                    } else {
                                        pill(
                                            ui,
                                            "403",
                                            RED,
                                            Color32::from_rgba_unmultiplied(255, 107, 107, 18),
                                        );
                                    }
                                });
                            });
                        });
                    ui.add_space(3.0);
                }
            }
        });
    }

    fn agent_endpoint_card(&mut self, ui: &mut Ui, ctx: &egui::Context, port: u16) {
        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("What an agent can call")
                        .size(13.0)
                        .strong()
                        .color(TEXT),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "Copy base URL").clicked() {
                        self.copy(
                            ctx,
                            format!("http://127.0.0.1:{}", port),
                            "Control plane URL copied",
                        );
                    }
                    pill(
                        ui,
                        "LOOPBACK ONLY",
                        GREEN,
                        Color32::from_rgba_unmultiplied(61, 214, 149, 16),
                    );
                });
            });
            ui.add_space(8.0);
            let rows = [
                (
                    "GET",
                    "/__proxzy/health",
                    "Port, CA path, rule count and the active session.",
                ),
                (
                    "POST",
                    "/__proxzy/scenario",
                    "{\"pattern\":\"Wallet balance\",\"scenario\":\"error\"} — success, error, empty or passthrough.",
                ),
                (
                    "POST",
                    "/__proxzy/inject",
                    "{\"pattern\":\"Wallet balance\",\"inject\":{\"USERNAME\":\"qa.agent\"}} — adds a body token.",
                ),
                (
                    "POST",
                    "/__proxzy/record",
                    "{\"domains\":[\"api.example.com\"],\"enabled\":true} — auto-catch live 200s for replay.",
                ),
                (
                    "POST",
                    "/__proxzy/reset",
                    "{\"pattern\":\"Wallet balance\"} — clears injection and returns to success.",
                ),
            ];
            for (method, path, note) in rows {
                let color = method_color(method);
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(46.0, 16.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.label(RichText::new(method).font(mono(9.0)).strong().color(color));
                        },
                    );
                    ui.label(RichText::new(path).font(mono(9.5)).color(TEXT));
                });
                ui.label(RichText::new(note).size(9.0).color(FAINT));
                ui.add_space(5.0);
            }
            studio_note(
                ui,
                "External agents reach these over 127.0.0.1 only. Mobile devices use the bridge above.",
            );
        });
    }

    // ---------------------------------------------------------------------
    // Agent Control — IDE hook
    // ---------------------------------------------------------------------

    fn agent_ide_card(&mut self, ui: &mut Ui, ctx: &egui::Context, port: u16) {
        let mut copy: Option<(String, &'static str)> = None;
        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Hook an IDE").size(13.0).strong().color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    pill(
                        ui,
                        "PLAIN TEXT",
                        CYAN,
                        Color32::from_rgba_unmultiplied(88, 196, 221, 16),
                    );
                });
            });
            ui.add_space(2.0);
            studio_note(
                ui,
                "Pick the agent you use. Paste its brief into the rules file it reads, and that agent can drive this engine and verify its own calls.",
            );
            ui.add_space(8.0);

            let clients = [
                ("Cursor", ".cursor/rules/proxzy.mdc", "project rule"),
                ("VS Code", ".github/copilot-instructions.md", "workspace instructions"),
                ("Claude Code", "CLAUDE.md", "project memory"),
                ("JetBrains", ".junie/guidelines.md", "project guidelines"),
            ];
            for (name, place, kind) in clients {
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(96.0, 18.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.label(RichText::new(name).size(10.5).strong().color(TEXT));
                        },
                    );
                    ui.allocate_ui_with_layout(
                        Vec2::new(230.0, 18.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.label(RichText::new(place).font(mono(9.0)).color(CYAN));
                        },
                    );
                    ui.label(RichText::new(kind).size(9.0).color(FAINT));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if secondary_button(ui, "Copy brief").clicked() {
                            copy = Some((
                                ide_brief(port, name, place),
                                "IDE brief copied — paste it into the rules file",
                            ));
                        }
                    });
                });
                ui.add_space(3.0);
            }

            ui.add_space(4.0);
            hsep(ui);
            ui.add_space(7.0);
            let health = format!("curl -s http://127.0.0.1:{}/__proxzy/health", port);
            ui.horizontal(|ui| {
                pill(
                    ui,
                    "VERIFY THE HOOK",
                    GREEN,
                    Color32::from_rgba_unmultiplied(61, 214, 149, 16),
                );
                ui.label(
                    RichText::new("Ask the agent to run this first; its call lands in the feed below.")
                        .size(9.5)
                        .color(MUTED),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "Copy check").clicked() {
                        copy = Some((health.clone(), "Health check copied"));
                    }
                });
            });
            ui.add_space(4.0);
            code_block(ui, &health, 26.0, false);
        });
        if let Some((text, message)) = copy {
            self.copy(ctx, text, message);
        }
    }

    // =====================================================================
    // API Test Bench
    // =====================================================================

    fn select_test(&mut self, index: usize) {
        if index >= self.tests.len() {
            return;
        }
        self.test_selected = index;
        self.test_headers = self.tests[index].headers_text();
        self.test_injects = self.tests[index]
            .injects
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("\n");
    }

    fn tests_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        if self.tests.is_empty() {
            self.tests = studio::sample_tests();
        }
        self.test_selected = self.test_selected.min(self.tests.len() - 1);
        if self.test_headers.is_empty() && !self.tests[self.test_selected].headers.is_empty() {
            self.test_headers = self.tests[self.test_selected].headers_text();
        }
        let passed = self
            .tests
            .iter()
            .filter(|t| t.last.as_ref().map(|r| r.passed).unwrap_or(false))
            .count();
        let failed = self
            .tests
            .iter()
            .filter(|t| t.last.as_ref().map(|r| !r.passed).unwrap_or(false))
            .count();
        let not_run = self.tests.len().saturating_sub(passed + failed);
        let running = self.test_send.is_some();
        let port = self.proxy_port;

        let mut run_all = false;
        let mut run_one = false;
        let mut toggle_io = false;

        ui.horizontal(|ui| {
            Self::studio_heading(
                ui,
                "API Test Bench",
                "Write a request once, pin its expected response, then run the suite through the mock engine.",
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary_button(ui, if running { "Running…" } else { "Run all" }).clicked()
                    && !running
                {
                    run_all = true;
                }
                if secondary_button(ui, "Run selected").clicked() && !running {
                    run_one = true;
                }
                if chip(ui, "JSON", self.test_io).clicked() {
                    toggle_io = true;
                }
                if !self.map_enabled && chip(ui, "Arm engine", false).clicked() {
                    self.map_enabled = true;
                    self.sync_proxy_config();
                    self.log(Level::Info, "Test Bench", "Map Local engine armed");
                }
            });
        });
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("TARGET")
                    .size(8.5)
                    .strong()
                    .color(FAINT),
            );
            for platform in TargetPlatform::ALL {
                let active = self.test_target == platform;
                if chip(ui, platform.badge(), active).clicked() {
                    self.test_target = platform;
                }
            }
            ui.label(
                RichText::new(format!("· port {} · {}", port, self.test_target.label()))
                    .font(mono(9.0))
                    .color(FAINT),
            );
            if self.test_target == TargetPlatform::AndroidDevice {
                for transport in AndroidTransport::ALL {
                    let active = self.android_transport == transport;
                    if chip(ui, transport.label(), active).clicked() {
                        self.android_transport = transport;
                    }
                }
            }
            if let Some(note) = self.target_bridge_command(self.test_target, port) {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "Copy bridge").clicked() {
                        self.copy(ctx, note, "Bridge command copied");
                    }
                });
            }
        });
        ui.add_space(10.0);
        if toggle_io {
            self.test_io = !self.test_io;
        }

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    stat_box(ui, 120.0, "TESTS", &self.tests.len().to_string(), ACCENT);
                    stat_box(ui, 120.0, "PASSED", &passed.to_string(), GREEN);
                    stat_box(ui, 120.0, "FAILED", &failed.to_string(), if failed > 0 { RED } else { FAINT });
                    stat_box(ui, 120.0, "NOT RUN", &not_run.to_string(), if not_run > 0 { MUTED } else { FAINT });
                    stat_box(
                        ui,
                        156.0,
                        "RUNNER",
                        if running { "SENDING" } else { "IDLE" },
                        if running { AMBER } else { FAINT },
                    );
                });
                ui.add_space(12.0);

                let gap = 14.0;
                let total = ui.available_width();
                let left = ((total - gap) * 0.32).clamp(280.0, 360.0);
                let right = (total - left - gap).max(420.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(left, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(left);
                            self.test_list_card(ui, ctx);
                        },
                    );
                    ui.add_space(gap);
                    ui.allocate_ui_with_layout(
                        Vec2::new(right, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(right);
                            self.test_editor(ui, ctx);
                        },
                    );
                });

                if self.test_io {
                    ui.add_space(12.0);
                    self.test_io_card(ui, ctx);
                }
                ui.add_space(16.0);
            });

        if run_one {
            let queue = vec![self.test_selected];
            self.run_test_queue(ctx, queue);
        }
        if run_all {
            let queue: Vec<usize> = (0..self.tests.len()).collect();
            self.run_test_queue(ctx, queue);
        }
    }

    fn test_list_card(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut pick: Option<usize> = None;
        let mut add = false;
        let mut duplicate = false;
        let mut remove = false;
        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Suite").size(13.0).strong().color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "+").clicked() {
                        add = true;
                    }
                    if secondary_button(ui, "Copy").clicked() {
                        duplicate = true;
                    }
                    if secondary_button(ui, "Del").clicked() {
                        remove = true;
                    }
                });
            });
            ui.add_space(2.0);
            studio_note(ui, "Select a test to edit it. A dot shows its last result.");
            ui.add_space(7.0);
            let selected = self.test_selected;
            for (index, test) in self.tests.iter().enumerate() {
                let is_selected = index == selected;
                let (dot_color, status) = match &test.last {
                    Some(run) if run.passed => (GREEN, format!("{} ms", run.latency_ms)),
                    Some(run) => (RED, format!("HTTP {}", run.status)),
                    None => (FAINT, "not run".to_string()),
                };
                let row = Frame::none()
                    .fill(if is_selected { HOVER } else { INPUT })
                    .stroke(Stroke::new(
                        1.0,
                        if is_selected {
                            Color32::from_rgb(70, 92, 130)
                        } else {
                            BORDER
                        },
                    ))
                    .rounding(8.0)
                    .inner_margin(Margin::symmetric(9.0, 7.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            dot(ui, dot_color, 6.0);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&test.name)
                                            .size(10.5)
                                            .strong()
                                            .color(TEXT),
                                    )
                                    .truncate(),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {}",
                                        test.method, test.suite
                                    ))
                                    .font(mono(8.5))
                                    .color(FAINT),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(status).font(mono(8.5)).color(dot_color),
                                );
                            });
                        });
                    });
                if row
                    .response
                    .interact(Sense::click())
                    .clicked()
                {
                    pick = Some(index);
                }
                ui.add_space(4.0);
            }
        });

        if add {
            let id = self.tests.iter().map(|t| t.id).max().unwrap_or(0) + 1;
            let mut test = ApiTest::new(id, "New request", "Custom", "GET", "http://{host}:{port}/v1/ping");
            test.expect_body = "\"ok\"".into();
            self.tests.push(test);
            let last = self.tests.len() - 1;
            self.select_test(last);
        }
        if duplicate {
            let source = self.tests[self.test_selected].clone();
            let id = self.tests.iter().map(|t| t.id).max().unwrap_or(0) + 1;
            let mut copy = source;
            copy.id = id;
            copy.name = format!("{} copy", copy.name);
            copy.last = None;
            self.tests.push(copy);
            let last = self.tests.len() - 1;
            self.select_test(last);
        }
        if remove && self.tests.len() > 1 {
            let name = self.tests.remove(self.test_selected).name;
            self.test_selected = self.test_selected.min(self.tests.len() - 1);
            self.select_test(self.test_selected);
            self.log(Level::Warn, "Test Bench", format!("Removed \"{}\"", name));
        } else if remove {
            self.toast_now(ctx, "Keep at least one test in the suite");
        }
        if let Some(index) = pick {
            self.select_test(index);
        }
    }

    fn test_editor(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let index = self.test_selected;
        let mut draft = self.tests[index].clone();
        let port = self.proxy_port;
        let mut run = false;

        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    input_frame().show(ui, |ui| {
                        ui.set_width(300.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut draft.name)
                                .frame(false)
                                .desired_width(300.0)
                                .font(egui::FontId::proportional(13.0))
                                .hint_text("Test name"),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("SUITE").size(8.0).strong().color(FAINT));
                        ui.add(
                            egui::TextEdit::singleline(&mut draft.suite)
                                .frame(false)
                                .desired_width(130.0)
                                .font(mono(9.5))
                                .hint_text("Wallet"),
                        );
                    });
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if primary_button(ui, "Run test").clicked() {
                        run = true;
                    }
                    let source = if draft.scenario.is_some() {
                        "SCENARIO PINNED"
                    } else {
                        "LIVE ROUTE"
                    };
                    ui.label(
                        RichText::new(source)
                            .size(9.0)
                            .strong()
                            .color(if draft.scenario.is_some() { GREEN } else { MUTED }),
                    );
                });
            });
            ui.add_space(9.0);

            // ---- request -------------------------------------------------
            ui.horizontal(|ui| {
                form_label(ui, "METHOD");
                for method in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
                    let active = draft.method.eq_ignore_ascii_case(method);
                    if chip(ui, method, active).clicked() {
                        draft.method = method.into();
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("→ {}", studio::resolve_url(&draft.url, port)))
                            .font(mono(8.5))
                            .color(FAINT),
                    );
                });
            });
            ui.add_space(6.0);
            form_label(ui, "URL · {host} AND {port} ARE FILLED FROM THE PORT CONFIG");
            input_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(
                    egui::TextEdit::singleline(&mut draft.url)
                        .frame(false)
                        .desired_width(ui.available_width())
                        .font(mono(10.5))
                        .hint_text("http://{host}:{port}/v1/wallet/balance"),
                );
            });
            ui.add_space(8.0);

            let gap = 12.0;
            let total = ui.available_width();
            let half = ((total - gap) * 0.5).max(200.0);
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(half, 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(half);
                        form_label(ui, "HEADERS · ONE PER LINE");
                        input_frame().show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(
                                egui::TextEdit::multiline(&mut self.test_headers)
                                    .frame(false)
                                    .desired_width(ui.available_width())
                                    .desired_rows(3)
                                    .font(mono(9.5))
                                    .hint_text("content-type: application/json"),
                            );
                        });
                        ui.add_space(6.0);
                        form_label(ui, "BODY");
                        input_frame().show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(
                                egui::TextEdit::multiline(&mut draft.body)
                                    .frame(false)
                                    .desired_width(ui.available_width())
                                    .desired_rows(4)
                                    .font(mono(9.5))
                                    .hint_text("{\"query\":\"…\"}"),
                            );
                        });
                    },
                );
                ui.add_space(gap);
                ui.allocate_ui_with_layout(
                    Vec2::new(half, 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(half);
                        form_label(ui, "INJECT · KEY=VALUE PER LINE");
                        input_frame().show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(
                                egui::TextEdit::multiline(&mut self.test_injects)
                                    .frame(false)
                                    .desired_width(ui.available_width())
                                    .desired_rows(3)
                                    .font(mono(9.5))
                                    .hint_text("USERNAME=qa.agent"),
                            );
                        });
                        ui.add_space(6.0);
                        form_label(ui, "MAP LOCAL SCENARIO");
                        ui.horizontal_wrapped(|ui| {
                            if chip(ui, "Leave", draft.scenario.is_none()).clicked() {
                                draft.scenario = None;
                            }
                            for scenario in [
                                MockScenario::Success,
                                MockScenario::Error,
                                MockScenario::Empty,
                            ] {
                                let active = draft.scenario == Some(scenario);
                                if chip(ui, scenario.label(), active).clicked() {
                                    draft.scenario = Some(scenario);
                                }
                            }
                        });
                        ui.add_space(4.0);
                        input_frame().show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(
                                egui::TextEdit::singleline(&mut draft.scenario_target)
                                    .frame(false)
                                    .desired_width(ui.available_width())
                                    .font(mono(9.5))
                                    .hint_text("Rule name or path, e.g. Wallet balance"),
                            );
                        });
                    },
                );
            });

            ui.add_space(10.0);
            hsep(ui);
            ui.add_space(9.0);
            form_label(ui, "EXPECTATIONS");
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Status").size(9.5).color(MUTED));
                small_field(ui, &mut draft.expect_status, 46.0, "200");
                ui.label(RichText::new("Max ms").size(9.5).color(MUTED));
                small_field(ui, &mut draft.expect_max_ms, 52.0, "400");
                ui.label(RichText::new("JSON key").size(9.5).color(MUTED));
                small_field(ui, &mut draft.expect_json_key, 96.0, "balance");
                ui.label(RichText::new("Body contains").size(9.5).color(MUTED));
                small_field(ui, &mut draft.expect_body, 150.0, "\"ok\"");
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Header").size(9.5).color(MUTED));
                small_field(ui, &mut draft.expect_header_name, 110.0, "content-type");
                ui.label(RichText::new("equals").size(9.5).color(MUTED));
                small_field(
                    ui,
                    &mut draft.expect_header_value,
                    140.0,
                    "application/json",
                );
            });
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Answered by").size(9.5).color(MUTED));
                for source in [ExpectSource::Any, ExpectSource::Mock, ExpectSource::Live] {
                    let active = draft.expect_source == source;
                    if chip(ui, source.label(), active).clicked() {
                        draft.expect_source = source;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let count = draft.checks().len();
                    studio_note(
                        ui,
                        &format!(
                            "{} assertion{} · {}",
                            count,
                            if count == 1 { "" } else { "s" },
                            if draft.is_graphql() { "GraphQL body" } else { "HTTP" }
                        ),
                    );
                });
            });

            ui.add_space(10.0);
            hsep(ui);
            ui.add_space(9.0);
            match &draft.last {
                Some(run) => {
                    ui.horizontal(|ui| {
                        pill(
                            ui,
                            if run.passed { "PASSED" } else { "FAILED" },
                            if run.passed { GREEN } else { RED },
                            if run.passed {
                                Color32::from_rgba_unmultiplied(61, 214, 149, 16)
                            } else {
                                Color32::from_rgba_unmultiplied(255, 107, 107, 16)
                            },
                        );
                        ui.label(
                            RichText::new(format!(
                                "HTTP {} · {} ms · {}",
                                run.status, run.latency_ms, run.ran_at
                            ))
                            .font(mono(9.0))
                            .color(MUTED),
                        );
                        if let Some(error) = &run.error {
                            ui.label(RichText::new(error).size(9.5).color(RED));
                        }
                    });
                    ui.add_space(4.0);
                    for check in &run.checks {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(if check.passed { "✓" } else { "✗" })
                                    .size(10.0)
                                    .strong()
                                    .color(if check.passed { GREEN } else { RED }),
                            );
                            ui.label(
                                RichText::new(&check.label).size(9.5).color(TEXT),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(&check.detail).font(mono(8.5)).color(FAINT),
                                );
                            });
                        });
                    }
                    if !run.body_preview.is_empty() {
                        ui.add_space(4.0);
                        code_block(ui, &run.body_preview, 96.0, true);
                    }
                }
                None => {
                    studio_note(
                        ui,
                        "No run yet. The runner switches the pinned scenario, sends the request through the proxy and grades every assertion above.",
                    );
                }
            }
        });

        // Edits are written back every frame so the suite is always current.
        self.tests[index] = draft;
        self.tests[index].set_headers_text(&self.test_headers.clone());
        self.tests[index].injects = self
            .test_injects
            .lines()
            .filter_map(studio::parse_inject)
            .collect();

        if run {
            self.run_test_queue(ctx, vec![index]);
        }
    }

    fn test_io_card(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut build = false;
        let mut copy_export = false;
        let mut save = false;
        let mut copy_import = false;
        let mut import = false;
        let mut load = false;
        let mut mappings = false;
        let path = proxy::app_data_dir().join("proxzy-tests.json");
        let path_text = path.display().to_string();

        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("JSON suite file")
                        .size(13.0)
                        .strong()
                        .color(TEXT),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "WireMock mappings").clicked() {
                        mappings = true;
                    }
                    studio_note(
                        ui,
                        "Build renders the suite; WireMock mappings renders the Map Local rules.",
                    );
                });
            });
            ui.add_space(2.0);
            studio_note(
                ui,
                &format!("Saved at {}", path_text),
            );
            ui.add_space(8.0);
            let gap = 12.0;
            let total = ui.available_width();
            let half = ((total - gap) * 0.5).max(220.0);
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(half, 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(half);
                        ui.horizontal(|ui| {
                            form_label(ui, "EXPORT");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if secondary_button(ui, "Copy").clicked() {
                                    copy_export = true;
                                }
                                if secondary_button(ui, "Save").clicked() {
                                    save = true;
                                }
                                if secondary_button(ui, "Build").clicked() {
                                    build = true;
                                }
                            });
                        });
                        input_frame().show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(
                                egui::TextEdit::multiline(&mut self.test_export)
                                    .frame(false)
                                    .desired_width(ui.available_width())
                                    .desired_rows(7)
                                    .font(mono(8.5))
                                    .hint_text("Press Build to render the suite as JSON"),
                            );
                        });
                    },
                );
                ui.add_space(gap);
                ui.allocate_ui_with_layout(
                    Vec2::new(half, 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(half);
                        ui.horizontal(|ui| {
                            form_label(ui, "IMPORT");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if secondary_button(ui, "Copy").clicked() {
                                    copy_import = true;
                                }
                                if secondary_button(ui, "Read file").clicked() {
                                    load = true;
                                }
                                if primary_button(ui, "Import").clicked() {
                                    import = true;
                                }
                            });
                        });
                        input_frame().show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(
                                egui::TextEdit::multiline(&mut self.test_import)
                                    .frame(false)
                                    .desired_width(ui.available_width())
                                    .desired_rows(7)
                                    .font(mono(8.5))
                                    .hint_text("Paste a suite here, or press Read file"),
                            );
                        });
                    },
                );
            });
        });

        if build {
            self.test_export = studio::tests_to_json(&self.tests);
            self.log(Level::Info, "Test Bench", "Rendered the suite as JSON");
        }
        if mappings {
            self.test_export = studio::wiremock_mappings(&self.map_rules);
            self.log(
                Level::Info,
                "Test Bench",
                format!(
                    "Rendered {} Map Local rule(s) as WireMock mappings",
                    self.map_rules.len()
                ),
            );
        }
        if save {
            if self.test_export.trim().is_empty() {
                self.test_export = studio::tests_to_json(&self.tests);
            }
            match std::fs::write(&path, &self.test_export) {
                Ok(()) => {
                    self.log(
                        Level::Info,
                        "Test Bench",
                        format!("Suite saved to {}", path.display()),
                    );
                    self.toast_now(ctx, "Suite saved to the app data folder");
                }
                Err(err) => {
                    self.log(Level::Error, "Test Bench", format!("Save failed: {}", err));
                    self.toast_now(ctx, "Could not write the suite file");
                }
            }
        }
        if copy_export {
            let text = if self.test_export.trim().is_empty() {
                studio::tests_to_json(&self.tests)
            } else {
                self.test_export.clone()
            };
            self.copy(ctx, text, "Suite JSON copied");
        }
        if load {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    self.test_import = text;
                    self.toast_now(ctx, "Loaded the saved suite file");
                }
                Err(err) => {
                    self.log(Level::Warn, "Test Bench", format!("Read failed: {}", err));
                    self.toast_now(ctx, "No saved suite yet — press Save first");
                }
            }
        }
        if import {
            match studio::tests_from_json(&self.test_import) {
                Ok(tests) => {
                    let count = tests.len();
                    self.tests = tests;
                    self.test_selected = 0;
                    self.select_test(0);
                    self.log(
                        Level::Info,
                        "Test Bench",
                        format!("Imported {} test(s) from JSON", count),
                    );
                    self.toast_now(ctx, &format!("Imported {} test(s)", count));
                }
                Err(err) => {
                    self.log(Level::Error, "Test Bench", format!("Import failed: {}", err));
                    self.toast_now(ctx, "That JSON is not a valid suite");
                }
            }
        }
        if copy_import {
            self.copy(ctx, self.test_import.clone(), "Import JSON copied");
        }
    }

    fn prepare_test_engine(&mut self, test: &ApiTest) {
        let target = if test.scenario_target.trim().is_empty() {
            test.name.clone()
        } else {
            test.scenario_target.clone()
        };
        let mut rule_name = None;
        if let Some(scenario) = test.scenario {
            if let Some(index) = studio::rule_index_for_target(&self.map_rules, &target) {
                let rule = &mut self.map_rules[index];
                rule.switch_scenario(scenario);
                rule.enabled = true;
                rule_name = Some(rule.name.clone());
            }
        }
        if !test.injects.is_empty() {
            if let Some(index) = studio::rule_index_for_target(&self.map_rules, &target) {
                let rule = &mut self.map_rules[index];
                for (key, value) in &test.injects {
                    rule.inject_props.insert(key.clone(), value.clone());
                }
                rule.enabled = true;
                if rule_name.is_none() {
                    rule_name = Some(rule.name.clone());
                }
            }
        }
        if test.expect_source == ExpectSource::Live {
            if let Some(index) = studio::rule_index_for_target(&self.map_rules, &target) {
                self.map_rules[index].enabled = false;
            }
        }
        if rule_name.is_some() {
            self.map_enabled = true;
        }
        if rule_name.is_some() || test.expect_source == ExpectSource::Live {
            self.sync_proxy_config();
        }
    }

    fn run_test_queue(&mut self, ctx: &egui::Context, queue: Vec<usize>) {
        if queue.is_empty() {
            return;
        }
        if self.test_send.is_some() {
            self.toast_now(ctx, "A test is already on the wire");
            return;
        }
        self.test_queue = queue;
        self.log(
            Level::Info,
            "Test Bench",
            format!("Running {} test(s) through the proxy", self.test_queue.len()),
        );
        self.start_next_test(ctx);
    }

    fn start_next_test(&mut self, ctx: &egui::Context) {
        if self.test_send.is_some() {
            return;
        }
        let Some(index) = self.test_queue.first().copied() else {
            let passed = self
                .tests
                .iter()
                .filter(|t| t.last.as_ref().map(|r| r.passed).unwrap_or(false))
                .count();
            let failed = self
                .tests
                .iter()
                .filter(|t| t.last.as_ref().map(|r| !r.passed).unwrap_or(false))
                .count();
            self.log(
                if failed > 0 { Level::Warn } else { Level::Info },
                "Test Bench",
                format!("Suite finished — {} passed, {} failed", passed, failed),
            );
            self.record_agent_action("TESTS", &format!("{} pass / {} fail", passed, failed));
            return;
        };
        self.test_queue.remove(0);
        if index >= self.tests.len() {
            self.start_next_test(ctx);
            return;
        }
        self.select_test(index);
        let test = self.tests[index].clone();
        self.prepare_test_engine(&test);
        let url = studio::resolve_url(&test.url, self.proxy_port);
        let method = test.method.clone();
        let headers = test.headers.clone();
        let body = test.body.clone();
        let name = test.name.clone();
        let port = self.proxy_port;
        let ca = self.ca_info.as_ref().map(|ca| ca.pem_path.clone());
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let outcome = run_curl(&method, &url, &headers, &body, Some((port, ca)));
            let _ = tx.send(outcome);
        });
        self.test_send = Some(TestSend { index, rx });
        self.log(Level::Info, "Test Bench", format!("→ {}", name));
        let _ = ctx;
    }

    fn poll_tests(&mut self, ctx: &egui::Context) {
        let Some(send) = self.test_send.as_ref() else {
            return;
        };
        match send.rx.try_recv() {
            Ok(outcome) => {
                let index = send.index;
                self.test_send = None;
                if index < self.tests.len() {
                    let response = studio::TestResponse {
                        status: outcome.status,
                        latency_ms: outcome.latency_ms,
                        headers: outcome.resp_headers.clone(),
                        body: outcome.body.clone(),
                        error: outcome.error.clone(),
                    };
                    let run = self.tests[index].evaluate(&response);
                    let name = self.tests[index].name.clone();
                    let passed = run.passed;
                    let status = run.status;
                    let latency = run.latency_ms;
                    self.tests[index].last = Some(run);
                    self.log(
                        if passed { Level::Info } else { Level::Error },
                        "Test Bench",
                        format!(
                            "{} \"{}\" — HTTP {} in {} ms",
                            if passed { "PASS" } else { "FAIL" },
                            name,
                            status,
                            latency
                        ),
                    );
                    self.record_agent_action(
                        if passed { "TEST PASS" } else { "TEST FAIL" },
                        &name,
                    );
                }
                self.start_next_test(ctx);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.test_send = None;
                self.toast_now(ctx, "The test process ended unexpectedly");
                self.start_next_test(ctx);
            }
        }
    }

    // =====================================================================
    // Screen Play & MCP
    // =====================================================================

    fn screen_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        if self.screen_plays.is_empty() {
            self.screen_plays = studio::sample_screen_plays();
        }
        self.screen_selected = self.screen_selected.min(self.screen_plays.len() - 1);
        let running = self
            .screen_run
            .as_ref()
            .map(|r| !r.finished)
            .unwrap_or(false);
        let waiting = self.screen_run.as_ref().map(|r| r.waiting).unwrap_or(false);
        let port = self.proxy_port;
        let steps: usize = self.screen_plays.iter().map(|p| p.steps.len()).sum();
        let engine_steps = self
            .screen_plays
            .iter()
            .flat_map(|p| p.steps.iter())
            .filter(|s| s.kind.is_engine())
            .count();
        let passed = self
            .screen_plays
            .iter()
            .filter(|p| !p.last.is_empty() && p.last.iter().all(|o| o.status != StepStatus::Failed))
            .count();

        ui.horizontal(|ui| {
            Self::studio_heading(
                ui,
                "Screen Play & MCP",
                "Drive a UI test from a play list: switch mocks, inject values and wait for real hits on the engine.",
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if waiting {
                    pill(
                        ui,
                        "WAITING FOR HIT",
                        AMBER,
                        Color32::from_rgba_unmultiplied(255, 181, 84, 18),
                    );
                } else {
                    pill(
                        ui,
                        if running { "PLAYING" } else { "READY" },
                        if running { CYAN } else { GREEN },
                        if running {
                            Color32::from_rgba_unmultiplied(119, 211, 255, 16)
                        } else {
                            Color32::from_rgba_unmultiplied(61, 214, 149, 16)
                        },
                    );
                }
                ui.label(
                    RichText::new(format!("127.0.0.1:{}", port))
                        .font(mono(9.0))
                        .color(FAINT),
                );
            });
        });
        ui.add_space(10.0);

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    stat_box(ui, 120.0, "PLAYS", &self.screen_plays.len().to_string(), ACCENT);
                    stat_box(ui, 120.0, "STEPS", &steps.to_string(), CYAN);
                    stat_box(ui, 144.0, "ENGINE STEPS", &engine_steps.to_string(), GREEN);
                    stat_box(
                        ui,
                        144.0,
                        "LAST CLEAN",
                        &format!("{} / {}", passed, self.screen_plays.len()),
                        if passed == self.screen_plays.len() { GREEN } else { MUTED },
                    );
                    stat_box(
                        ui,
                        168.0,
                        "HARNESS TARGET",
                        self.screen_target.badge(),
                        PURPLE,
                    );
                });
                ui.add_space(12.0);

                let gap = 14.0;
                // Keep the third column inside the window: the play editor and
                // the snippet box both grow with their content.
                let total = (ui.available_width() - 20.0).max(700.0);
                let left = ((total - gap * 2.0) * 0.24).clamp(220.0, 280.0);
                let middle = ((total - gap * 2.0) * 0.42).clamp(360.0, 470.0);
                let right = (total - left - middle - gap * 2.0).max(280.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(left, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(left);
                            self.screen_play_list(ui, ctx);
                        },
                    );
                    ui.add_space(gap);
                    ui.allocate_ui_with_layout(
                        Vec2::new(middle, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(middle);
                            self.screen_play_detail(ui, ctx);
                        },
                    );
                    ui.add_space(gap);
                    ui.allocate_ui_with_layout(
                        Vec2::new(right, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(right);
                            self.screen_harness_card(ui, ctx);
                            ui.add_space(10.0);
                            self.screen_io_card(ui, ctx);
                        },
                    );
                });
                ui.add_space(16.0);
            });
    }

    fn screen_play_list(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut pick: Option<usize> = None;
        let current = self.screen_run.as_ref().map(|r| r.play);
        studio_card(ui, |ui| {
            ui.label(RichText::new("Plays").size(13.0).strong().color(TEXT));
            ui.add_space(2.0);
            studio_note(ui, "A play is a checklist an agent or UI harness can follow.");
            ui.add_space(7.0);
            for (index, play) in self.screen_plays.iter().enumerate() {
                let selected = index == self.screen_selected;
                let playing = current == Some(index);
                let status = if play.last.is_empty() {
                    None
                } else if play.last.iter().any(|o| o.status == StepStatus::Failed) {
                    Some((RED, "failed"))
                } else {
                    Some((GREEN, "passed"))
                };
                let row = Frame::none()
                    .fill(if selected { HOVER } else { INPUT })
                    .stroke(Stroke::new(
                        1.0,
                        if playing {
                            CYAN
                        } else if selected {
                            Color32::from_rgb(70, 92, 130)
                        } else {
                            BORDER
                        },
                    ))
                    .rounding(8.0)
                    .inner_margin(Margin::symmetric(9.0, 8.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            dot(
                                ui,
                                if playing {
                                    CYAN
                                } else {
                                    status.map(|(c, _)| c).unwrap_or(FAINT)
                                },
                                6.0,
                            );
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&play.name)
                                            .size(10.5)
                                            .strong()
                                            .color(TEXT),
                                    )
                                    .wrap(),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{} steps{}",
                                        play.steps.len(),
                                        status
                                            .map(|(_, label)| format!(" · {}", label))
                                            .unwrap_or_default()
                                    ))
                                    .font(mono(8.5))
                                    .color(FAINT),
                                );
                            });
                        });
                    });
                if row.response.interact(Sense::click()).clicked() {
                    pick = Some(index);
                }
                ui.add_space(5.0);
            }
            ui.add_space(4.0);
            hsep(ui);
            ui.add_space(6.0);
            studio_note(
                ui,
                "Engine steps run for real. Tap and Type steps are marked simulated because only the harness can touch the device.",
            );
        });
        if let Some(index) = pick {
            let busy = self
                .screen_run
                .as_ref()
                .map(|r| !r.finished)
                .unwrap_or(false);
            if busy {
                self.toast_now(ctx, "Stop the current play before switching");
            } else {
                self.screen_selected = index;
                self.screen_run = None;
            }
        }
    }

    fn screen_play_detail(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let index = self.screen_selected;
        let mut draft = self.screen_plays[index].clone();
        let play_id = draft.id;
        let run = self.screen_run.as_ref().filter(|r| r.play == index);
        let running = run.map(|r| !r.finished).unwrap_or(false);
        let waiting = run.map(|r| r.waiting).unwrap_or(false);
        let current_step = run.filter(|r| !r.finished).map(|r| r.step);
        let run_outcomes = run.map(|r| r.outcomes.clone()).unwrap_or_default();
        let run_finished = run.map(|r| r.finished).unwrap_or(false);
        let shown_outcomes = if run_outcomes.is_empty() {
            draft.last.clone()
        } else {
            run_outcomes
        };

        let mut start = false;
        let mut stop = false;
        let mut nudge = false;
        let mut skip = false;
        let mut add_step = false;
        let mut remove_step: Option<usize> = None;

        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    input_frame().show(ui, |ui| {
                        ui.set_width(250.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut draft.name)
                                .frame(false)
                                .desired_width(250.0)
                                .font(egui::FontId::proportional(14.0))
                                .hint_text("Play name"),
                        );
                    });
                    studio_note(ui, &draft.goal);
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if running {
                        if secondary_button(ui, "Stop").clicked() {
                            stop = true;
                        }
                    } else if primary_button(ui, "Run play").clicked() {
                        start = true;
                    }
                });
            });
            ui.add_space(7.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("TARGET").size(8.5).strong().color(FAINT));
                for platform in TargetPlatform::ALL {
                    let active = self.screen_target == platform;
                    if chip(ui, platform.badge(), active)
                        .on_hover_text(format!("target id: {}", platform.slug()))
                        .clicked()
                    {
                        self.screen_target = platform;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let label = if waiting {
                        "paused on a hit checkpoint"
                    } else if run_finished {
                        "last run finished"
                    } else if running {
                        "running"
                    } else {
                        "idle"
                    };
                    ui.label(
                        RichText::new(label)
                            .size(9.0)
                            .color(if waiting { AMBER } else { FAINT }),
                    );
                });
            });
            if waiting {
                ui.add_space(5.0);
                Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(255, 181, 84, 14))
                    .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 181, 84, 60)))
                    .rounding(7.0)
                    .inner_margin(Margin::symmetric(9.0, 7.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("The play is waiting for real traffic on this rule.")
                                    .size(9.5)
                                    .color(TEXT),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if secondary_button(ui, "Skip wait").clicked() {
                                    skip = true;
                                }
                                if secondary_button(ui, "Simulate hit").clicked() {
                                    nudge = true;
                                }
                            });
                        });
                    });
            }
            ui.add_space(8.0);
            hsep(ui);
            ui.add_space(7.0);

            ui.label(RichText::new("STEPS").size(8.5).strong().color(FAINT));
            ui.add_space(3.0);
            for (step_index, step) in draft.steps.iter_mut().enumerate() {
                let outcome = shown_outcomes.get(step_index);
                let status_color = match outcome.map(|o| o.status) {
                    Some(StepStatus::Passed) => GREEN,
                    Some(StepStatus::Simulated) => CYAN,
                    Some(StepStatus::Failed) => RED,
                    None => {
                        if current_step == Some(step_index) {
                            AMBER
                        } else {
                            FAINT
                        }
                    }
                };
                Frame::none()
                    .fill(INPUT)
                    .stroke(Stroke::new(1.0, BORDER))
                    .rounding(7.0)
                    .inner_margin(Margin::symmetric(8.0, 6.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{:02}", step_index + 1))
                                    .font(mono(8.5))
                                    .color(FAINT),
                            );
                            dot(ui, status_color, 5.0);
                            egui::ComboBox::from_id_salt(("step_kind", play_id, step_index))
                                .selected_text(step.kind.label())
                                .width(84.0)
                                .show_ui(ui, |ui| {
                                    for kind in [
                                        StepKind::Launch,
                                        StepKind::Tap,
                                        StepKind::Type,
                                        StepKind::ExpectVisible,
                                        StepKind::ExpectHit,
                                        StepKind::Scenario,
                                        StepKind::Inject,
                                        StepKind::Wait,
                                    ] {
                                        ui.selectable_value(
                                            &mut step.kind,
                                            kind,
                                            kind.label(),
                                        );
                                    }
                                });
                            input_frame().show(ui, |ui| {
                                ui.set_width(148.0);
                                ui.add(
                                    egui::TextEdit::singleline(&mut step.action)
                                        .frame(false)
                                        .desired_width(148.0)
                                        .font(mono(8.5))
                                        .hint_text("rule or selector"),
                                );
                            });
                            input_frame().show(ui, |ui| {
                                ui.set_width(76.0);
                                ui.add(
                                    egui::TextEdit::singleline(&mut step.value)
                                        .frame(false)
                                        .desired_width(76.0)
                                        .font(mono(8.5))
                                        .hint_text("value"),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if secondary_button(ui, "×").clicked() {
                                    remove_step = Some(step_index);
                                }
                            });
                        });
                        if let Some(outcome) = outcome {
                            ui.label(
                                RichText::new(format!(
                                    "{} · {}",
                                    outcome.status.label(),
                                    outcome.detail
                                ))
                                .size(8.5)
                                .color(status_color),
                            )
                            .on_hover_text(&outcome.step);
                        }
                    });
                ui.add_space(3.0);
            }
            ui.horizontal(|ui| {
                if secondary_button(ui, "+ Step").clicked() {
                    add_step = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    studio_note(
                        ui,
                        "Engine steps match the rule by name first, then by path pattern.",
                    );
                });
            });
        });

        if let Some(remove) = remove_step {
            if draft.steps.len() <= 1 {
                self.toast_now(ctx, "A play needs at least one step");
            } else {
                draft.steps.remove(remove);
            }
        }
        if add_step {
            draft
                .steps
                .push(ScreenStep::new(StepKind::Tap, "#element", ""));
        }
        self.screen_plays[index] = draft;

        if start {
            self.start_screen_play(ctx, index);
        }
        if stop {
            self.screen_run = None;
            self.log(Level::Warn, "Screen Play", "Play stopped by the person");
        }
        if nudge {
            self.simulate_hit(ctx);
        }
        if skip {
            self.skip_screen_wait(ctx);
        }
    }

    fn screen_harness_card(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let port = self.proxy_port;
        let play = self.screen_plays[self.screen_selected].clone();
        let tabs = ["Env", "Playwright", "MCP", "XCTest", "Espresso"];
        let mut copy_snippet = false;
        let mut open_agent = false;
        let mut turn_on_lan = false;
        let host = self.screen_target_host();
        let token = self.screen_target_token();
        let token_ref = token.as_deref();
        let bridge = self.target_bridge_command(self.screen_target, port);
        let needs_lan = self.target_uses_lan(self.screen_target);
        let snippet = match self.screen_tab {
            0 => studio::env_block_for(&host, port, &play.name, token_ref),
            2 => studio::mcp_snippet_for(&host, port, &play.name, token_ref),
            3 => studio::xctest_snippet_for(&host, port, &play.name, token_ref),
            4 => studio::espresso_snippet_for(&host, port, &play.name, token_ref),
            _ => studio::playwright_snippet_for(&host, port, &play.name),
        };

        studio_card(ui, |ui| {
            ui.label(
                RichText::new("Harness & MCP")
                    .size(13.0)
                    .strong()
                    .color(TEXT),
            );
            ui.add_space(2.0);
            studio_note(
                ui,
                "Point Playwright or a mobilewright-style MCP server at the same loopback port.",
            );
            ui.add_space(7.0);
            ui.horizontal(|ui| {
                stat_box(ui, 96.0, "PROXY", &port.to_string(), ACCENT);
                stat_box(ui, 120.0, "TARGET", self.screen_target.badge(), PURPLE);
                stat_box(
                    ui,
                    116.0,
                    "BRIDGE",
                    if bridge.is_some() {
                        "NEEDED"
                    } else if needs_lan {
                        if self.lan_enabled { "LAN READY" } else { "LAN OFF" }
                    } else {
                        "DIRECT"
                    },
                    if bridge.is_some() {
                        AMBER
                    } else if needs_lan && !self.lan_enabled {
                        AMBER
                    } else {
                        GREEN
                    },
                );
            });
            if self.screen_target == TargetPlatform::AndroidDevice {
                ui.add_space(5.0);
                self.android_transport_chips(ui);
            }
            if needs_lan && !self.lan_enabled {
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    studio_note(ui, "LAN access is off; physical Wi-Fi targets need LAN enabled.");
                    if secondary_button(ui, "Turn on LAN access").clicked() {
                        turn_on_lan = true;
                    }
                });
            }
            if let Some(command) = bridge.clone() {
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(command.clone())
                            .font(mono(8.5))
                            .color(CYAN),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if secondary_button(ui, "Copy").clicked() {
                            self.copy(ctx, command.clone(), "Bridge command copied");
                        }
                    });
                });
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for (index, label) in tabs.iter().enumerate() {
                    if chip(ui, label, self.screen_tab == index).clicked() {
                        self.screen_tab = index;
                    }
                }
            });
            ui.add_space(5.0);
            code_block(ui, &snippet, 168.0, false);
            ui.horizontal(|ui| {
                if secondary_button(ui, "Copy snippet").clicked() {
                    copy_snippet = true;
                }
                if secondary_button(ui, "Agent Control").clicked() {
                    open_agent = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    studio_note(ui, "ENV keys stay the same for every harness.");
                });
            });

            ui.add_space(9.0);
            hsep(ui);
            ui.add_space(7.0);
            ui.label(RichText::new("LIVE ENGINE COMMANDS").size(8.5).strong().color(FAINT));
            ui.add_space(3.0);
            if let Some(step) = play
                .steps
                .iter()
                .find(|s| matches!(s.kind, StepKind::Scenario | StepKind::Inject))
            {
                let command = if step.kind == StepKind::Scenario {
                    studio::scenario_curl_for(&host, port, &step.action, &step.value, token_ref)
                } else {
                    let (key, value) = studio::parse_inject(&step.value)
                        .unwrap_or_else(|| ("KEY".into(), step.value.clone()));
                    studio::inject_curl_for(&host, port, &step.action, &key, &value, token_ref)
                };
                code_block(ui, &command, 72.0, false);
                studio_note(ui, "This is the same command the play runs locally.");
            } else {
                studio_note(ui, "This play has no engine steps to call.");
            }
        });

        if turn_on_lan {
            self.set_lan_enabled(ctx, true);
        }
        if copy_snippet {
            self.copy(ctx, snippet, "Harness snippet copied");
        }
        if open_agent {
            self.view = View::Agent;
        }
    }

    fn start_screen_play(&mut self, ctx: &egui::Context, index: usize) {
        if index >= self.screen_plays.len() {
            return;
        }
        let busy = self
            .screen_run
            .as_ref()
            .map(|r| !r.finished)
            .unwrap_or(false);
        if busy {
            self.toast_now(ctx, "A play is already running");
            return;
        }
        let name = self.screen_plays[index].name.clone();
        self.screen_selected = index;
        self.screen_run = Some(ScreenRun {
            play: index,
            step: 0,
            step_entered: Instant::now(),
            waiting: false,
            hit_base: 0,
            hit_armed: false,
            outcomes: Vec::new(),
            finished: false,
        });
        self.log(Level::Info, "Screen Play", format!("Starting \"{}\"", name));
        self.record_agent_action("PLAY", &format!("start {}", name));
        self.toast_now(ctx, "Play started — engine steps run, device steps are simulated");
    }

    fn advance_screen_run(&mut self, _ctx: &egui::Context) {
        let Some(run) = self.screen_run.as_ref() else {
            return;
        };
        if run.finished {
            return;
        }
        let play_index = run.play;
        let step_index = run.step;
        let waiting = run.waiting;
        let hit_base = run.hit_base;
        let hit_armed = run.hit_armed;
        let elapsed = run.step_entered.elapsed();
        if play_index >= self.screen_plays.len() {
            self.screen_run = None;
            return;
        }
        let steps = self.screen_plays[play_index].steps.clone();
        if step_index >= steps.len() {
            self.finish_screen_run(play_index);
            return;
        }
        let step = steps[step_index].clone();

        match step.kind {
            StepKind::ExpectHit => {
                let need = step.value.trim().parse::<u32>().unwrap_or(1);
                let hits = studio::rule_index_for_target(&self.map_rules, &step.action)
                    .map(|i| self.map_rules[i].hits)
                    .unwrap_or(0);
                if !hit_armed {
                    if let Some(run) = self.screen_run.as_mut() {
                        run.hit_base = hits;
                        run.hit_armed = true;
                        run.waiting = true;
                    }
                    if !waiting {
                        self.log(
                            Level::Warn,
                            "Screen Play",
                            format!(
                                "Waiting for {} new hit(s) on \"{}\"",
                                need, step.action
                            ),
                        );
                    }
                    return;
                }
                let fresh = hits.saturating_sub(hit_base);
                if fresh < need {
                    if let Some(run) = self.screen_run.as_mut() {
                        run.waiting = true;
                    }
                    return;
                }
                let outcome = StepOutcome {
                    step: step.describe(),
                    status: StepStatus::Passed,
                    detail: format!("saw {} new hit(s) on \"{}\"", fresh, step.action),
                };
                self.log(
                    Level::Info,
                    "Screen Play",
                    format!("✓ {}", outcome.detail),
                );
                self.advance_screen_step(play_index, outcome);
            }
            StepKind::Wait => {
                let ms = step.value.trim().parse::<u64>().unwrap_or(0);
                if elapsed.as_millis() < ms as u128 {
                    return;
                }
                let outcome = StepOutcome {
                    step: step.describe(),
                    status: StepStatus::Passed,
                    detail: format!("waited {} ms", ms),
                };
                self.advance_screen_step(play_index, outcome);
            }
            _ => {
                if elapsed.as_millis() < 240 {
                    return;
                }
                let outcome = self.execute_screen_step(&step, play_index);
                self.advance_screen_step(play_index, outcome);
            }
        }
    }

    fn advance_screen_step(&mut self, play_index: usize, outcome: StepOutcome) {
        let total = self
            .screen_plays
            .get(play_index)
            .map(|p| p.steps.len())
            .unwrap_or(0);
        let finished = if let Some(run) = self.screen_run.as_mut() {
            run.outcomes.push(outcome);
            run.step += 1;
            run.step_entered = Instant::now();
            run.waiting = false;
            run.hit_armed = false;
            run.step >= total
        } else {
            false
        };
        if finished {
            self.finish_screen_run(play_index);
        }
    }

    fn execute_screen_step(&mut self, step: &ScreenStep, _play_index: usize) -> StepOutcome {
        match step.kind {
            StepKind::Scenario => {
                let scenario = MockScenario::from_slug(&step.value);
                let Some(scenario) = scenario else {
                    return StepOutcome {
                        step: step.describe(),
                        status: StepStatus::Failed,
                        detail: format!("unknown scenario \"{}\"", step.value),
                    };
                };
                match studio::rule_index_for_target(&self.map_rules, &step.action) {
                    Some(index) => {
                        let rule = &mut self.map_rules[index];
                        rule.switch_scenario(scenario);
                        rule.enabled = true;
                        let name = rule.name.clone();
                        self.map_enabled = true;
                        self.sync_proxy_config();
                        StepOutcome {
                            step: step.describe(),
                            status: StepStatus::Passed,
                            detail: format!("{} · {}", name, scenario.label()),
                        }
                    }
                    None => StepOutcome {
                        step: step.describe(),
                        status: StepStatus::Failed,
                        detail: format!("no Map Local rule matches \"{}\"", step.action),
                    },
                }
            }
            StepKind::Inject => {
                let Some((key, value)) = studio::parse_inject(&step.value) else {
                    return StepOutcome {
                        step: step.describe(),
                        status: StepStatus::Failed,
                        detail: "expected key=value".into(),
                    };
                };
                match studio::rule_index_for_target(&self.map_rules, &step.action) {
                    Some(index) => {
                        let rule = &mut self.map_rules[index];
                        rule.inject_props.insert(key.clone(), value.clone());
                        rule.enabled = true;
                        let name = rule.name.clone();
                        self.map_enabled = true;
                        self.sync_proxy_config();
                        StepOutcome {
                            step: step.describe(),
                            status: StepStatus::Passed,
                            detail: format!("{} · {}={}", name, key, value),
                        }
                    }
                    None => StepOutcome {
                        step: step.describe(),
                        status: StepStatus::Failed,
                        detail: format!("no Map Local rule matches \"{}\"", step.action),
                    },
                }
            }
            StepKind::Launch => StepOutcome {
                step: step.describe(),
                status: StepStatus::Simulated,
                detail: "launch config handed to the harness".into(),
            },
            StepKind::Tap => StepOutcome {
                step: step.describe(),
                status: StepStatus::Simulated,
                detail: "the harness taps this on the device".into(),
            },
            StepKind::Type => StepOutcome {
                step: step.describe(),
                status: StepStatus::Simulated,
                detail: "the harness types this on the device".into(),
            },
            StepKind::ExpectVisible => StepOutcome {
                step: step.describe(),
                status: StepStatus::Simulated,
                detail: "the harness asserts this on screen".into(),
            },
            StepKind::ExpectHit | StepKind::Wait => StepOutcome {
                step: step.describe(),
                status: StepStatus::Simulated,
                detail: "handled by the runner".into(),
            },
        }
    }

    fn finish_screen_run(&mut self, play_index: usize) {
        let outcomes = match self.screen_run.as_mut() {
            Some(run) if !run.finished => {
                run.finished = true;
                run.waiting = false;
                run.outcomes.clone()
            }
            _ => return,
        };
        let failed = outcomes
            .iter()
            .filter(|o| o.status == StepStatus::Failed)
            .count();
        let simulated = outcomes
            .iter()
            .filter(|o| o.status == StepStatus::Simulated)
            .count();
        let name = self.screen_plays[play_index].name.clone();
        let total = self.screen_plays[play_index].steps.len();
        self.screen_plays[play_index].last = outcomes;
        self.log(
            if failed > 0 { Level::Warn } else { Level::Info },
            "Screen Play",
            format!(
                "\"{}\" finished — {} steps, {} simulated, {} failed",
                name, total, simulated, failed
            ),
        );
        self.record_agent_action("PLAY", &format!("{} → {} failed", name, failed));
    }

    fn simulate_hit(&mut self, ctx: &egui::Context) {
        let Some(target) = self
            .screen_run
            .as_ref()
            .filter(|r| !r.finished)
            .and_then(|r| {
                let play = self.screen_plays.get(r.play)?;
                play.steps.get(r.step).map(|s| s.action.clone())
            })
        else {
            return;
        };
        if let Some(index) = studio::rule_index_for_target(&self.map_rules, &target) {
            let rule = &mut self.map_rules[index];
            rule.hits = rule.hits.saturating_add(1);
            let name = rule.name.clone();
            let hits = rule.hits;
            self.sync_proxy_config();
            self.log(
                Level::Warn,
                "Screen Play",
                format!("Simulated a harness hit on \"{}\" (now {})", name, hits),
            );
            self.toast_now(ctx, "Hit simulated for the current checkpoint");
        } else {
            self.toast_now(ctx, "No rule matches the waiting step");
        }
    }

    fn skip_screen_wait(&mut self, ctx: &egui::Context) {
        let Some(run) = self.screen_run.as_ref().filter(|r| !r.finished) else {
            return;
        };
        let play_index = run.play;
        let step_index = run.step;
        let Some(step) = self
            .screen_plays
            .get(play_index)
            .and_then(|p| p.steps.get(step_index))
            .cloned()
        else {
            return;
        };
        let outcome = StepOutcome {
            step: step.describe(),
            status: StepStatus::Simulated,
            detail: "wait skipped — no harness traffic arrived".into(),
        };
        self.log(
            Level::Warn,
            "Screen Play",
            format!("Skipped the hit checkpoint on \"{}\"", step.action),
        );
        self.advance_screen_step(play_index, outcome);
        let _ = ctx;
    }

    // =====================================================================
    // Map Local connector pipeline
    // =====================================================================

    fn map_pipeline(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let port = self.proxy_port;
        let selected = self
            .agent_rule
            .and_then(|id| self.map_rules.iter().find(|r| r.id == id))
            .or_else(|| self.map_rules.first())
            .cloned();
        let mut pick: Option<u64> = None;
        let mut switch: Option<(u64, MockScenario)> = None;
        let mut live: Option<u64> = None;
        let mut edit: Option<MapRule> = None;
        let mut open_tests = false;
        let mut inject_now: Option<(u64, String, String)> = None;
        let mut toggle_engine = false;

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(
                    RichText::new("Connector pipeline")
                        .size(21.0)
                        .strong()
                        .color(TEXT),
                );
                ui.label(
                    RichText::new(
                        "One request, six stages: device · loopback engine · match · scenario · injection · response.",
                    )
                    .size(11.0)
                    .color(MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if secondary_button(ui, "Edit rule").clicked() {
                    if let Some(rule) = selected.clone() {
                        edit = Some(rule);
                    }
                }
                let on = self.map_enabled;
                if toggle(ui, on).clicked() {
                    toggle_engine = true;
                }
                ui.label(
                    RichText::new(if on { "ENGINE ON" } else { "ENGINE OFF" })
                        .size(10.0)
                        .strong()
                        .color(if on { GREEN } else { AMBER }),
                );
            });
        });
        ui.add_space(10.0);

        // ---- rule picker ---------------------------------------------------
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("RULE").size(8.5).strong().color(FAINT));
            for rule in &self.map_rules {
                let active = selected.as_ref().map(|r| r.id) == Some(rule.id);
                if chip(ui, &rule.name, active).clicked() {
                    pick = Some(rule.id);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if secondary_button(ui, "Send a test request").clicked() {
                    open_tests = true;
                }
            });
        });
        ui.add_space(10.0);

        // ---- stage strip ---------------------------------------------------
        let stages: [(&str, &str, String, Color32); 6] = [
            ("1", "DEVICE", self.screen_target.badge().to_string(), CYAN),
            ("2", "ENGINE", format!(":{}", port), ACCENT),
            (
                "3",
                "MATCH",
                selected
                    .as_ref()
                    .map(|r| r.method.label().to_string())
                    .unwrap_or_else(|| "ANY".into()),
                PURPLE,
            ),
            (
                "4",
                "SCENARIO",
                selected
                    .as_ref()
                    .map(|r| {
                        if !self.map_enabled {
                            "Off".into()
                        } else if r.enabled {
                            r.active_scenario.label().to_string()
                        } else {
                            "Live".into()
                        }
                    })
                    .unwrap_or_else(|| "—".into()),
                if self.map_enabled { GREEN } else { AMBER },
            ),
            (
                "5",
                "INJECT",
                selected
                    .as_ref()
                    .map(|r| r.inject_props.len().to_string())
                    .unwrap_or_else(|| "0".into()),
                AMBER,
            ),
            (
                "6",
                "RESPONSE",
                selected
                    .as_ref()
                    .map(|r| {
                        if !self.map_enabled {
                            "held".into()
                        } else if r.enabled {
                            r.status.to_string()
                        } else {
                            "upstream".into()
                        }
                    })
                    .unwrap_or_else(|| "—".into()),
                if self.map_enabled { MUTED } else { AMBER },
            ),
        ];
        ui.horizontal(|ui| {
            for (index, (number, label, value, color)) in stages.iter().enumerate() {
                if index > 0 {
                    ui.label(RichText::new("→").font(mono(11.0)).color(FAINT));
                }
                Frame::none()
                    .fill(INPUT)
                    .stroke(Stroke::new(1.0, BORDER))
                    .rounding(8.0)
                    .inner_margin(Margin::symmetric(9.0, 7.0))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            ui.label(
                                RichText::new(format!("{} {}", number, label))
                                    .size(7.5)
                                    .strong()
                                    .color(FAINT),
                            );
                            ui.label(
                                RichText::new(value.as_str()).size(10.5).strong().color(*color),
                            );
                        });
                    });
            }
        });
        ui.add_space(12.0);

        let gap = 14.0;
        let total = ui.available_width();
        let left = ((total - gap) * 0.55).clamp(460.0, 640.0);
        let right = (total - left - gap).max(330.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(left, 0.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(left);
                    match &selected {
                        Some(rule) => {
                            self.pipeline_stages(ui, ctx, rule, &mut switch, &mut live, &mut inject_now);
                        }
                        None => {
                            studio_card(ui, |ui| studio_note(ui, "Create a Map Local rule first."));
                        }
                    }
                },
            );
            ui.add_space(gap);
            ui.allocate_ui_with_layout(
                Vec2::new(right, 0.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(right);
                    if let Some(rule) = &selected {
                        self.pipeline_preview(ui, ctx, rule);
                    }
                },
            );
        });
        ui.add_space(12.0);

        studio_card(ui, |ui| {
            ui.label(
                RichText::new("Connector contract")
                    .size(12.5)
                    .strong()
                    .color(TEXT),
            );
            ui.add_space(2.0);
            studio_note(
                ui,
                "Anything that can send JSON to loopback can drive these rules — an agent, a script, or a harness.",
            );
            ui.add_space(6.0);
            let rows = [
                ("GET", "/__proxzy/health"),
                ("POST", "/__proxzy/scenario"),
                ("POST", "/__proxzy/inject"),
                ("POST", "/__proxzy/record"),
                ("POST", "/__proxzy/reset"),
            ];
            ui.horizontal_wrapped(|ui| {
                for (method, path) in rows {
                    let color = method_color(method);
                    pill(
                        ui,
                        &format!("{} {}", method, path),
                        color,
                        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 14),
                    );
                }
            });
        });
        ui.add_space(16.0);

        if toggle_engine {
            self.map_enabled = !self.map_enabled;
            self.sync_proxy_config();
            self.log(
                if self.map_enabled { Level::Info } else { Level::Warn },
                "Map Local",
                if self.map_enabled {
                    "Connector pipeline armed"
                } else {
                    "Connector pipeline bypassed"
                },
            );
        }
        if let Some(id) = pick {
            self.agent_rule = Some(id);
        }
        if let Some((id, scenario)) = switch {
            self.agent_rule = Some(id);
            self.set_rule_scenario(ctx, id, scenario);
        }
        if let Some(id) = live {
            self.agent_rule = Some(id);
            self.set_rule_live(ctx, id);
        }
        if let Some((id, key, value)) = inject_now {
            self.agent_rule = Some(id);
            self.inject_into_rule(ctx, id, &key, &value);
        }
        if let Some(rule) = edit {
            self.open_map_editor(Some(rule));
        }
        if open_tests {
            self.view = View::Tests;
        }
    }

    fn pipeline_stages(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        rule: &MapRule,
        switch: &mut Option<(u64, MockScenario)>,
        live: &mut Option<u64>,
        inject_now: &mut Option<(u64, String, String)>,
    ) {
        // Stage 1 — device / harness
        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                stage_badge(ui, "1", "HARNESS", CYAN);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    for platform in TargetPlatform::ALL {
                        let active = self.screen_target == platform;
                        if chip(ui, platform.badge(), active).clicked() {
                            self.screen_target = platform;
                        }
                    }
                });
            });
            ui.add_space(4.0);
            if self.screen_target == TargetPlatform::AndroidDevice {
                self.android_transport_chips(ui);
                ui.add_space(3.0);
            }
            studio_note(
                ui,
                "The app or browser under test must send traffic to the engine. Mobile targets use the bridge or LAN host.",
            );
            if let Some(command) = self.target_bridge_command(self.screen_target, self.proxy_port) {
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(command.clone())
                            .font(mono(8.5))
                            .color(CYAN),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if secondary_button(ui, "Copy").clicked() {
                            self.copy(ctx, command.clone(), "Bridge command copied");
                        }
                    });
                });
            }
        });
        pipeline_arrow(ui);

        // Stage 2 — match
        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                stage_badge(ui, "2", "MATCH", ACCENT);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    pill(
                        ui,
                        if rule.enabled { "ARMED" } else { "BYPASSED" },
                        if rule.enabled { GREEN } else { MUTED },
                        if rule.enabled {
                            Color32::from_rgba_unmultiplied(61, 214, 149, 16)
                        } else {
                            Color32::from_rgba_unmultiplied(148, 163, 184, 14)
                        },
                    );
                });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(rule.method.label())
                        .font(mono(10.5))
                        .strong()
                        .color(method_color(rule.method.label())),
                );
                ui.label(
                    RichText::new(&rule.pattern)
                        .font(mono(10.5))
                        .color(TEXT),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} hits", rule.hits))
                            .font(mono(9.0))
                            .color(if rule.hits > 0 { CYAN } else { FAINT }),
                    );
                });
            });
            studio_note(
                ui,
                "First matching rule wins. * is a wildcard; a GraphQL name can pin one operation.",
            );
        });
        pipeline_arrow(ui);

        // Stage 3 — scenario
        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                stage_badge(ui, "3", "SCENARIO", GREEN);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "Live").clicked() {
                        *live = Some(rule.id);
                    }
                });
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for scenario in [
                    MockScenario::Success,
                    MockScenario::Error,
                    MockScenario::Empty,
                ] {
                    let active = rule.enabled && rule.active_scenario == scenario;
                    if chip(ui, scenario.label(), active).clicked() {
                        *switch = Some((rule.id, scenario));
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "HTTP {} · {} · {} ms delay",
                            rule.status, rule.content_type, rule.delay_ms
                        ))
                        .font(mono(8.5))
                        .color(FAINT),
                    );
                });
            });
            ui.add_space(3.0);
            studio_note(
                ui,
                "Success, Error and Empty are three stored bodies. Live passes the request upstream untouched.",
            );
        });
        pipeline_arrow(ui);

        // Stage 4 — injection
        studio_card(ui, |ui| {
            stage_badge(ui, "4", "INJECTION", AMBER);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                input_frame().show(ui, |ui| {
                    ui.set_width(110.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.agent_inject_key)
                            .frame(false)
                            .desired_width(110.0)
                            .font(mono(9.5))
                            .hint_text("USERNAME"),
                    );
                });
                ui.label(RichText::new("=").color(FAINT));
                input_frame().show(ui, |ui| {
                    ui.set_width(140.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.agent_inject_value)
                            .frame(false)
                            .desired_width(140.0)
                            .font(mono(9.5))
                            .hint_text("qa.agent"),
                    );
                });
                if primary_button(ui, "Add token").clicked() {
                    *inject_now = Some((
                        rule.id,
                        self.agent_inject_key.clone(),
                        self.agent_inject_value.clone(),
                    ));
                }
            });
            ui.add_space(5.0);
            if rule.inject_props.is_empty() {
                studio_note(
                    ui,
                    "No tokens. Add one and {{KEY}} inside the stored body is replaced when served.",
                );
            } else {
                let mut tokens: Vec<(String, String)> =
                    rule.inject_props.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                tokens.sort();
                ui.horizontal_wrapped(|ui| {
                    for (key, value) in tokens {
                        pill(
                            ui,
                            &format!("{{{{{}}}}} = {}", key, value),
                            GREEN,
                            Color32::from_rgba_unmultiplied(61, 214, 149, 16),
                        );
                    }
                });
            }
        });
        pipeline_arrow(ui);

        // Stage 5 — answer
        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                stage_badge(ui, "5", "ANSWER", MUTED);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "Edit body").clicked() {
                        self.open_map_editor(Some(rule.clone()));
                    }
                });
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "{} {} returns HTTP {} with {}{}",
                    rule.method.label(),
                    rule.pattern,
                    rule.status,
                    rule.content_type,
                    if rule.delay_ms > 0 {
                        format!(" after {} ms", rule.delay_ms)
                    } else {
                        String::new()
                    }
                ))
                .size(10.0)
                .color(TEXT),
            );
            studio_note(ui, "Served by the engine — no backend, no DNS, no certificate for this host.");
        });
    }

    fn pipeline_preview(&mut self, ui: &mut Ui, ctx: &egui::Context, rule: &MapRule) {
        let mut tokens: Vec<(String, String)> = rule
            .inject_props
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        tokens.sort();
        let serving = self.map_enabled && rule.enabled;
        let body = if serving {
            apply_tokens(&rule.body, &tokens)
        } else if rule.enabled {
            "The engine is off. Arm it above to serve this stored body.".to_string()
        } else {
            "Passthrough — the live API answers this request.".to_string()
        };
        let header = format!(
            "content-type: {}\nx-proxzy-rule: {}\nx-proxzy-scenario: {}",
            if serving {
                rule.content_type.as_str()
            } else {
                "upstream"
            },
            if serving { rule.name.as_str() } else { "—" },
            if serving {
                rule.active_scenario.slug()
            } else {
                "passthrough"
            }
        );
        let path = if rule.pattern.starts_with('/') {
            rule.pattern.clone()
        } else {
            format!("/{}", rule.pattern)
        };
        let curl = format!(
            "curl -sv -x http://127.0.0.1:{} https://api.example.com{} \\\n  -H 'accept: application/json'",
            self.proxy_port, path
        );
        let mut copy_body = false;
        let mut copy_curl = false;
        let status_pill = if serving {
            format!("HTTP {}", rule.status)
        } else if rule.enabled {
            "ENGINE OFF".to_string()
        } else {
            "LIVE".to_string()
        };

        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Response preview")
                        .size(13.0)
                        .strong()
                        .color(TEXT),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    pill(
                        ui,
                        &status_pill,
                        if serving {
                            GREEN
                        } else if rule.enabled {
                            AMBER
                        } else {
                            CYAN
                        },
                        if serving {
                            Color32::from_rgba_unmultiplied(61, 214, 149, 16)
                        } else if rule.enabled {
                            Color32::from_rgba_unmultiplied(255, 181, 84, 16)
                        } else {
                            Color32::from_rgba_unmultiplied(119, 211, 255, 16)
                        },
                    );
                });
            });
            ui.add_space(2.0);
            studio_note(
                ui,
                if serving {
                    "Exactly what the engine writes back, with tokens resolved."
                } else if rule.enabled {
                    "The rule is armed but the engine is off — nothing is rewritten yet."
                } else {
                    "This rule is bypassed; the request leaves through the proxy."
                },
            );
            ui.add_space(6.0);
            form_label(ui, "HEADERS");
            input_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                code_block(ui, &header, 62.0, false);
            });
            ui.add_space(5.0);
            form_label(ui, "BODY");
            code_block(ui, &body, 210.0, true);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if secondary_button(ui, "Copy body").clicked() {
                    copy_body = true;
                }
                if secondary_button(ui, "Copy cURL").clicked() {
                    copy_curl = true;
                }
            });
            ui.add_space(6.0);
            hsep(ui);
            ui.add_space(6.0);
            ui.label(RichText::new("REPLAY THROUGH THE ENGINE").size(8.5).strong().color(FAINT));
            ui.add_space(3.0);
            code_block(ui, &curl, 86.0, false);
            studio_note(
                ui,
                "Replace api.example.com with the host your app calls; keep the loopback proxy.",
            );
        });

        if copy_body {
            self.copy(
                ctx,
                apply_tokens(&rule.body, &tokens),
                "Stored response body copied",
            );
        }
        if copy_curl {
            self.copy(ctx, curl, "Scoped cURL copied");
        }
    }

    fn screen_io_card(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut build = false;
        let mut copy_export = false;
        let mut save = false;
        let mut read = false;
        let mut import = false;
        let path = proxy::app_data_dir().join("proxzy-plays.json");
        let path_text = path.display().to_string();

        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Play file")
                        .size(12.5)
                        .strong()
                        .color(TEXT),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "Save").clicked() {
                        save = true;
                    }
                    if secondary_button(ui, "Copy").clicked() {
                        copy_export = true;
                    }
                    if secondary_button(ui, "Build").clicked() {
                        build = true;
                    }
                });
            });
            ui.add_space(2.0);
            studio_note(ui, &format!("Saved at {}", path_text));
            ui.add_space(4.0);
            input_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(
                    egui::TextEdit::multiline(&mut self.screen_export)
                        .frame(false)
                        .desired_width(ui.available_width())
                        .desired_rows(4)
                        .font(mono(8.0))
                        .hint_text("Press Build to render every play as JSON"),
                );
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                form_label(ui, "IMPORT");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "Read").clicked() {
                        read = true;
                    }
                    if primary_button(ui, "Import").clicked() {
                        import = true;
                    }
                });
            });
            input_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(
                    egui::TextEdit::multiline(&mut self.screen_import)
                        .frame(false)
                        .desired_width(ui.available_width())
                        .desired_rows(4)
                        .font(mono(8.0))
                        .hint_text("Paste plays here, or press Read"),
                );
            });
        });

        if build {
            self.screen_export = studio::plays_to_json(&self.screen_plays);
            self.log(Level::Info, "Screen Play", "Rendered the play list as JSON");
        }
        if copy_export {
            let text = if self.screen_export.trim().is_empty() {
                studio::plays_to_json(&self.screen_plays)
            } else {
                self.screen_export.clone()
            };
            self.copy(ctx, text, "Plays JSON copied");
        }
        if save {
            if self.screen_export.trim().is_empty() {
                self.screen_export = studio::plays_to_json(&self.screen_plays);
            }
            match std::fs::write(&path, &self.screen_export) {
                Ok(()) => {
                    self.log(
                        Level::Info,
                        "Screen Play",
                        format!("Play file saved to {}", path.display()),
                    );
                    self.toast_now(ctx, "Play file saved");
                }
                Err(err) => {
                    self.log(Level::Error, "Screen Play", format!("Save failed: {}", err));
                    self.toast_now(ctx, "Could not write the play file");
                }
            }
        }
        if read {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    self.screen_import = text;
                    self.toast_now(ctx, "Loaded the saved play file");
                }
                Err(err) => {
                    self.log(Level::Warn, "Screen Play", format!("Read failed: {}", err));
                    self.toast_now(ctx, "No saved play file yet — press Save first");
                }
            }
        }
        if import {
            match studio::plays_from_json(&self.screen_import) {
                Ok(plays) => {
                    let count = plays.len();
                    self.screen_plays = plays;
                    self.screen_selected = 0;
                    self.screen_run = None;
                    self.log(
                        Level::Info,
                        "Screen Play",
                        format!("Imported {} play(s) from JSON", count),
                    );
                    self.toast_now(ctx, &format!("Imported {} play(s)", count));
                }
                Err(err) => {
                    self.log(Level::Error, "Screen Play", format!("Import failed: {}", err));
                    self.toast_now(ctx, "That JSON is not a valid play list");
                }
            }
        }
    }

// __STUDIO_APPEND__
}

// Free helpers used by the screens in this file.

fn small_field(ui: &mut Ui, value: &mut String, width: f32, hint: &str) {
    input_frame().show(ui, |ui| {
        ui.set_width(width);
        ui.add(
            egui::TextEdit::singleline(value)
                .frame(false)
                .desired_width(width)
                .font(mono(9.5))
                .hint_text(hint),
        );
    });
}

fn studio_card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    Frame::none()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(11.0)
        .inner_margin(Margin::same(14.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

fn studio_note(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(9.5).color(FAINT));
}

fn stage_badge(ui: &mut Ui, number: &str, label: &str, color: Color32) {
    pill(
        ui,
        &format!("{} · {}", number, label),
        color,
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 16),
    );
}

fn apply_tokens(body: &str, tokens: &[(String, String)]) -> String {
    let mut out = body.to_string();
    for (key, value) in tokens {
        out = out.replace(&format!("{{{{{}}}}}", key), value);
    }
    out
}

/// A tiny down-arrow between two pipeline stages.
fn pipeline_arrow(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.add_space(15.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 12.0), Sense::hover());
        ui.painter().vline(
            rect.center().x,
            rect.y_range(),
            Stroke::new(1.0, BORDER),
        );
    });
}

/// The copy-ready brief an IDE agent reads before it drives the proxy engine.
fn ide_brief(port: u16, ide: &str, place: &str) -> String {
    format!(
        "# Proxzy engine brief — {ide}\n\n\
         Paste into: {place}\n\n\
         Proxy: http://127.0.0.1:{port}\n\
         Control plane: http://127.0.0.1:{port}/__proxzy\n\
         CA for simulators: ~/Library/Application Support/Proxzy/proxzy-ca.pem\n\n\
         Use this local engine for every mocked, replayed or verified call:\n\
         1. GET /__proxzy/health — confirm the engine is ready, then note the port and rule count.\n\
         2. POST /__proxzy/scenario {{\"pattern\":\"Wallet balance\",\"scenario\":\"error\"}} — switch success, error, empty or passthrough.\n\
         3. POST /__proxzy/inject {{\"pattern\":\"Wallet balance\",\"inject\":{{\"USERNAME\":\"qa.agent\"}}}} — add a body token.\n\
         4. Send the app request through the proxy, then read /__proxzy/health and /__proxzy/session/summary to verify what was mocked and what went upstream.\n\
         5. POST /__proxzy/reset when the scenario should return to normal.\n\n\
         Constraints: loopback only (never expose the port), never change host proxy settings, and report a check as passed only after the engine response confirms it.\n"
    )
}

/// Scroll the Agent Control page to the IDE hook for demo screenshots.
fn agent_scroll_offset() -> f32 {
    match std::env::var("PROXZY_DEMO").unwrap_or_default().as_str() {
        "agent-hook" => 980.0,
        "agent-brief" => 1120.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod ide_hook_tests {
    use super::*;

    #[test]
    fn brief_names_port_endpoints_and_guardrails() {
        let brief = ide_brief(9091, "Cursor", ".cursor/rules/proxzy.mdc");
        assert!(brief.contains("http://127.0.0.1:9091/__proxzy"));
        assert!(brief.contains(
            "{\"pattern\":\"Wallet balance\",\"scenario\":\"error\"}"
        ));
        assert!(brief.contains(".cursor/rules/proxzy.mdc"));
        assert!(brief.contains("loopback only"));
    }
}
