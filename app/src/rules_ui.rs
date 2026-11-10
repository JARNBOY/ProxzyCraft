// ---------------------------------------------------------------------------
// Rules workspace: Map Local and Breakpoints, rebuilt around the two things
// people actually do: find a rule, and test a URL before turning it on.
// This file is included from app.rs, so it shares the App fields.
// ---------------------------------------------------------------------------

impl App {
    fn map_match_count(&self, rule: &MapRule) -> usize {
        self.captures
            .iter()
            .filter(|c| rule.matches(&c.method, &c.url, &c.req_body))
            .count()
    }

    fn bp_match_count(&self, rule: &BreakpointRule) -> usize {
        self.captures
            .iter()
            .filter(|c| rule.matches(&c.method, &c.url, &c.req_body))
            .count()
    }

    fn map_local_page_v2(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        // ---- heading + master switch -------------------------------------
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(RichText::new("Map Local").size(24.0).strong().color(TEXT));
                ui.label(
                    RichText::new("Answer a request with your own JSON — no backend needed.")
                        .size(12.0)
                        .color(MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let on = self.map_enabled;
                if toggle(ui, on).clicked() {
                    self.map_enabled = !on;
                    self.log(
                        if on { Level::Warn } else { Level::Info },
                        "Map Local",
                        if on {
                            "Map Local engine turned off"
                        } else {
                            "Map Local engine turned on"
                        },
                    );
                }
                ui.label(
                    RichText::new(if on { "Engine on" } else { "Engine off" })
                        .size(11.5)
                        .strong()
                        .color(if on { GREEN } else { FAINT }),
                );
            });
        });
        ui.add_space(10.0);

        // ---- one compact status line, not four big cards ------------------
        let enabled = self.map_rules.iter().filter(|r| r.enabled).count();
        let hits: u32 = self.map_rules.iter().map(|r| r.hits).sum();
        let selected_url = self.captures.get(self.selected).map(|c| c.url.clone());
        Frame::none()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(8.0)
            .inner_margin(Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    dot(ui, if self.map_enabled { GREEN } else { FAINT }, 7.0);
                    ui.label(
                        RichText::new(format!(
                            "{} mappings · {} on · {} served",
                            self.map_rules.len(),
                            enabled,
                            hits
                        ))
                        .size(11.5)
                        .color(TEXT),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new("first matching rule wins · * is a wildcard")
                                .size(10.0)
                                .color(FAINT),
                        );
                    });
                });
            });
        ui.add_space(10.0);

        // ---- toolbar -------------------------------------------------------
        let mut create_new = false;
        let mut do_quick_mock = false;
        ui.horizontal(|ui| {
            if primary_button(ui, "+  New mapping").clicked() {
                create_new = true;
            }
            if secondary_button(ui, "Use selected request").clicked() {
                if let Some(c) = self.captures.get(self.selected).cloned() {
                    self.open_map_editor_for_capture(&c);
                } else {
                    self.toast_now(ctx, "Select a request in the traffic list first");
                }
            }
            ui.add_space(6.0);
            search_box(ui, &mut self.map_filter, 230.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("click a rule name to edit · switch Success / Error / Empty in 1 click")
                        .size(10.0)
                        .color(FAINT),
                );
            });
        });
        ui.add_space(8.0);

        // ---- Direct URL / Domain / cURL Hook Bar (Device Test Ready) --------
        Frame::none()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(9.0)
            .inner_margin(Margin::symmetric(12.0, 9.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    pill(
                        ui,
                        "DIRECT URL / cURL HOOK",
                        GREEN,
                        Color32::from_rgba_unmultiplied(61, 214, 149, 18),
                    );
                    egui::ComboBox::from_id_salt("quick_mock_method")
                        .width(68.0)
                        .selected_text(
                            RichText::new(self.quick_mock_method.label())
                                .font(mono(10.0))
                                .color(method_color(self.quick_mock_method.label())),
                        )
                        .show_ui(ui, |ui| {
                            for m in [
                                RuleMethod::Any,
                                RuleMethod::Get,
                                RuleMethod::Post,
                                RuleMethod::Put,
                                RuleMethod::Delete,
                            ] {
                                ui.selectable_value(&mut self.quick_mock_method, m, m.label());
                            }
                        });
                    let url_w = (ui.available_width() - 375.0).max(160.0);
                    input_frame().show(ui, |ui| {
                        ui.set_width(url_w);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.quick_mock_url)
                                .frame(false)
                                .desired_width(url_w)
                                .font(TextStyle::Monospace)
                                .text_color(TEXT)
                                .hint_text(
                                    "https://api.mycorp.com/v1/orders*  or  */v1/checkout*  or paste curl",
                                ),
                        );
                    });
                    for sc in [MockScenario::Success, MockScenario::Error, MockScenario::Empty] {
                        let active = self.quick_mock_scenario == sc;
                        if chip(ui, sc.label(), active).clicked() && !active {
                            self.quick_mock_scenario = sc;
                            self.quick_mock_body = sc.default_body();
                        }
                    }
                    if primary_button(ui, "+ Hook Mock").clicked() {
                        do_quick_mock = true;
                    }
                });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Return JSON:")
                            .size(9.5)
                            .color(FAINT)
                            .strong(),
                    );
                    let body_w = (ui.available_width() - 190.0).max(150.0);
                    input_frame().show(ui, |ui| {
                        ui.set_width(body_w);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.quick_mock_body)
                                .frame(false)
                                .desired_width(body_w)
                                .font(TextStyle::Monospace)
                                .text_color(TEXT)
                                .hint_text("{\"ok\":true,\"data\":[]}"),
                        );
                    });
                    ui.label(
                        RichText::new(format!("Auto-hooks 127.0.0.1:{} + SSL Allowlist", self.proxy_port))
                            .font(mono(9.0))
                            .color(FAINT),
                    );
                });
            });
        if do_quick_mock {
            self.add_quick_direct_mock(ctx);
        }
        ui.add_space(8.0);

        // ---- Module / Folder Filter Chips ----------------------------------
        let mut all_folders: Vec<String> = self
            .map_rules
            .iter()
            .map(|r| r.folder_name().to_string())
            .collect();
        all_folders.sort();
        all_folders.dedup();

        if !all_folders.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("MODULE:").size(9.5).color(FAINT).strong());
                let all_active = self.map_selected_folder.is_none();
                if chip(ui, &format!("All ({})", self.map_rules.len()), all_active).clicked() {
                    self.map_selected_folder = None;
                }
                for f in &all_folders {
                    let count = self.map_rules.iter().filter(|r| r.folder_name() == f).count();
                    let is_active = self.map_selected_folder.as_deref() == Some(f.as_str());
                    let label = format!("📁 {} ({})", f, count);
                    if chip(ui, &label, is_active).clicked() {
                        self.map_selected_folder = if is_active { None } else { Some(f.clone()) };
                    }
                }
            });
            ui.add_space(8.0);
        }

        // ---- two panes: rules table + live tester --------------------------
        let total = ui.available_width();
        let right_w = (total * 0.34).clamp(290.0, 360.0);
        let left_w = (total - right_w - 16.0).max(430.0);
        let mut edit_existing: Option<MapRule> = None;
        let mut toggle_id: Option<u64> = None;
        let mut switch_scenario_action: Option<(u64, MockScenario)> = None;
        let mut copy_text_action: Option<(String, &'static str)> = None;
        let mut toggle_collapse_folder: Option<String> = None;
        let mut batch_folder_toggle: Option<(String, bool)> = None;
        let mut batch_folder_scenario: Option<(String, MockScenario)> = None;

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(left_w, 620.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(left_w);
                            let q = self.map_filter.trim().to_lowercase();
                            let rules: Vec<MapRule> = self
                                .map_rules
                                .iter()
                                .filter(|r| {
                                    if let Some(ref sel_f) = self.map_selected_folder {
                                        if r.folder_name() != sel_f {
                                            return false;
                                        }
                                    }
                                    q.is_empty()
                                        || r.name.to_lowercase().contains(&q)
                                        || r.pattern.to_lowercase().contains(&q)
                                        || r.graphql_name.to_lowercase().contains(&q)
                                        || r.folder_name().to_lowercase().contains(&q)
                                })
                                .cloned()
                                .collect();

                            if rules.is_empty() {
                                Frame::none()
                                    .fill(PANEL)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .rounding(10.0)
                                    .inner_margin(Margin::same(18.0))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new("No mappings here yet")
                                                .size(14.0)
                                                .strong()
                                                .color(TEXT),
                                        );
                                        ui.label(
                                            RichText::new("Create one, or type a different search.")
                                                .size(11.0)
                                                .color(MUTED),
                                        );
                                    });
                            }

                            let mut displayed_folders: Vec<String> = rules
                                .iter()
                                .map(|r| r.folder_name().to_string())
                                .collect();
                            displayed_folders.sort();
                            displayed_folders.dedup();

                            for folder in &displayed_folders {
                                let folder_rules: Vec<&MapRule> = rules
                                    .iter()
                                    .filter(|r| r.folder_name() == folder)
                                    .collect();
                                let is_collapsed = self.map_folder_collapsed.contains(folder);

                                Frame::none()
                                    .fill(Color32::from_rgb(26, 26, 30))
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .rounding(8.0)
                                    .inner_margin(Margin::symmetric(10.0, 7.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let arrow = if is_collapsed { "▶" } else { "▼" };
                                            if ui
                                                .button(
                                                    RichText::new(format!("{} 📁", arrow))
                                                        .size(11.0)
                                                        .color(AMBER),
                                                )
                                                .clicked()
                                            {
                                                toggle_collapse_folder = Some(folder.clone());
                                            }
                                            let title_lbl = ui.add(
                                                egui::Label::new(
                                                    RichText::new(folder)
                                                        .size(13.0)
                                                        .strong()
                                                        .color(TEXT),
                                                )
                                                .sense(Sense::click()),
                                            );
                                            if title_lbl.clicked() {
                                                toggle_collapse_folder = Some(folder.clone());
                                            }
                                            pill(
                                                ui,
                                                &format!("{} rules", folder_rules.len()),
                                                Color32::from_rgb(140, 175, 230),
                                                Color32::from_rgba_unmultiplied(80, 120, 200, 22),
                                            );

                                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                let all_enabled = folder_rules.iter().all(|r| r.enabled);
                                                if toggle(ui, all_enabled).clicked() {
                                                    batch_folder_toggle = Some((folder.clone(), !all_enabled));
                                                }
                                                ui.label(
                                                    RichText::new(if all_enabled { "All on" } else { "All off" })
                                                        .size(9.5)
                                                        .color(if all_enabled { GREEN } else { FAINT }),
                                                );

                                                ui.add_space(8.0);
                                                for sc in [
                                                    MockScenario::Empty,
                                                    MockScenario::Error,
                                                    MockScenario::Success,
                                                ] {
                                                    if chip(ui, sc.label(), false).clicked() {
                                                        batch_folder_scenario = Some((folder.clone(), sc));
                                                    }
                                                }
                                                ui.label(
                                                    RichText::new("Batch:")
                                                        .size(9.0)
                                                        .color(FAINT)
                                                        .strong(),
                                                );
                                            });
                                        });
                                    });
                                ui.add_space(4.0);

                                if !is_collapsed {
                                    for rule in &folder_rules {
                                        let live = self.map_match_count(rule);
                                Frame::none()
                                    .fill(if rule.enabled { CARD } else { PANEL })
                                    .stroke(Stroke::new(
                                        1.0,
                                        if rule.enabled { BORDER } else { Color32::from_rgb(42, 42, 46) },
                                    ))
                                    .rounding(9.0)
                                    .inner_margin(Margin::symmetric(11.0, 9.0))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            if toggle(ui, rule.enabled).clicked() {
                                                toggle_id = Some(rule.id);
                                            }
                                            ui.allocate_ui_with_layout(
                                                Vec2::new(
                                                    (ui.available_width() - 205.0).max(150.0),
                                                    40.0,
                                                ),
                                                Layout::top_down(Align::Min),
                                                |ui| {
                                                    ui.horizontal(|ui| {
                                                        let name = ui.add(
                                                            egui::Label::new(
                                                                RichText::new(&rule.name)
                                                                    .size(12.5)
                                                                    .strong()
                                                                    .color(if rule.enabled {
                                                                        TEXT
                                                                    } else {
                                                                        MUTED
                                                                    }),
                                                            )
                                                            .sense(Sense::click()),
                                                        );
                                                        if name.clicked() {
                                                            edit_existing = Some((*rule).clone());
                                                        }
                                                        if !rule.graphql_name.is_empty() {
                                                            pill(
                                                                ui,
                                                                &format!("GQL {}", rule.graphql_name),
                                                                PURPLE,
                                                                Color32::from_rgba_unmultiplied(
                                                                    184, 140, 255, 22,
                                                                ),
                                                            );
                                                        }
                                                        if !rule.inject_props.is_empty() {
                                                            pill(
                                                                ui,
                                                                &format!("INJECTED ({})", rule.inject_props.len()),
                                                                AMBER,
                                                                Color32::from_rgba_unmultiplied(
                                                                    255, 181, 84, 25,
                                                                ),
                                                            );
                                                        }
                                                    });
                                                    ui.label(
                                                        RichText::new(&rule.pattern)
                                                            .font(mono(10.5))
                                                            .color(Color32::from_rgb(150, 183, 235)),
                                                    );
                                                },
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    if secondary_button(ui, "Edit").clicked() {
                                                        edit_existing = Some((*rule).clone());
                                                    }
                                                    ui.label(
                                                        RichText::new(format!("{} hits", rule.hits))
                                                            .size(9.5)
                                                            .color(FAINT),
                                                    );
                                                    pill(
                                                        ui,
                                                        &rule.status.to_string(),
                                                        status_color(rule.status),
                                                        Color32::from_rgba_unmultiplied(
                                                            status_color(rule.status).r(),
                                                            status_color(rule.status).g(),
                                                            status_color(rule.status).b(),
                                                            18,
                                                        ),
                                                    );
                                                    pill(
                                                        ui,
                                                        rule.method.label(),
                                                        method_color(rule.method.label()),
                                                        INPUT,
                                                    );
                                                    let (dot_color, tip) = if live > 0 {
                                                        (GREEN, "matches captured traffic")
                                                    } else {
                                                        (FAINT, "no captured traffic matches yet")
                                                    };
                                                    let _ = tip;
                                                    dot(ui, dot_color, 6.0);
                                                },
                                            );
                                        });
                                        ui.add_space(5.0);
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new("SCENARIO:")
                                                    .size(8.5)
                                                    .color(FAINT)
                                                    .strong(),
                                            );
                                            for sc in [
                                                MockScenario::Success,
                                                MockScenario::Error,
                                                MockScenario::Empty,
                                            ] {
                                                let active = rule.active_scenario == sc;
                                                let st = match sc {
                                                    MockScenario::Success => rule.success_status,
                                                    MockScenario::Error => rule.error_status,
                                                    MockScenario::Empty => rule.empty_status,
                                                };
                                                let label = format!("{} ({})", sc.label(), st);
                                                if chip(ui, &label, active).clicked() && !active {
                                                    switch_scenario_action = Some((rule.id, sc));
                                                }
                                            }
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    let fixture = rule.fixture_path();
                                                    let resp = ui.add(
                                                        egui::Label::new(
                                                            RichText::new(&fixture)
                                                                .font(mono(9.0))
                                                                .color(FAINT),
                                                        )
                                                        .sense(Sense::click()),
                                                    );
                                                    if resp
                                                        .on_hover_text("Click to copy canonical fixture path convention")
                                                        .clicked()
                                                    {
                                                        copy_text_action = Some((
                                                            fixture,
                                                            "Copied canonical mock fixture path",
                                                        ));
                                                    }
                                                },
                                            );
                                        });
                                    });
                                        ui.add_space(6.0);
                                    }
                                }
                                ui.add_space(6.0);
                            }

                            ui.add_space(4.0);
                            Frame::none()
                                .fill(INPUT)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(8.0)
                                .inner_margin(Margin::same(10.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new("A rule only answers requests while its switch is on and the engine is on.")
                                            .size(10.0)
                                            .color(FAINT),
                                    );
                                });
                        },
                    );

                    ui.add_space(16.0);

                    ui.allocate_ui_with_layout(
                        Vec2::new(right_w, 620.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(right_w);
                            section_label(ui, "Test a URL");
                            ui.add_space(5.0);
                            card().show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                input_frame().show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.map_test_url)
                                            .frame(false)
                                            .desired_width(f32::INFINITY)
                                            .font(TextStyle::Monospace)
                                            .text_color(TEXT)
                                            .hint_text("https://api.shopwave.dev/v1/products?limit=20"),
                                    );
                                });
                                ui.add_space(5.0);
                                ui.horizontal(|ui| {
                                    if secondary_button(ui, "Use selected").clicked() {
                                        if let Some(url) = &selected_url {
                                            self.map_test_url = url.clone();
                                        }
                                    }
                                    if secondary_button(ui, "Clear").clicked() {
                                        self.map_test_url.clear();
                                    }
                                });
                                ui.add_space(8.0);
                                let url = if self.map_test_url.trim().is_empty() {
                                    selected_url.clone().unwrap_or_default()
                                } else {
                                    self.map_test_url.trim().to_string()
                                };
                                if url.is_empty() {
                                    ui.label(
                                        RichText::new("Type a URL, or pick a captured request first.")
                                            .size(10.5)
                                            .color(MUTED),
                                    );
                                } else {
                                    let matched: Vec<&MapRule> = self
                                        .map_rules
                                        .iter()
                                        .filter(|r| pattern_matches_url(&r.pattern, &url))
                                        .collect();
                                    if matched.is_empty() {
                                        ui.horizontal(|ui| {
                                            dot(ui, FAINT, 7.0);
                                            ui.label(
                                                RichText::new("No rule matches this URL")
                                                    .size(11.0)
                                                    .color(MUTED),
                                            );
                                        });
                                    } else {
                                        ui.horizontal(|ui| {
                                            dot(ui, GREEN, 7.0);
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} rule{} match this URL",
                                                    matched.len(),
                                                    if matched.len() == 1 { "" } else { "s" }
                                                ))
                                                .size(11.0)
                                                .strong()
                                                .color(GREEN),
                                            );
                                        });
                                        ui.add_space(4.0);
                                        for r in matched.iter().take(4) {
                                            ui.horizontal(|ui| {
                                                pill(
                                                    ui,
                                                    &r.status.to_string(),
                                                    status_color(r.status),
                                                    INPUT,
                                                );
                                                ui.label(
                                                    RichText::new(&r.name)
                                                        .size(10.5)
                                                        .color(if r.enabled { TEXT } else { FAINT }),
                                                );
                                                if !r.enabled {
                                                    pill(ui, "OFF", FAINT, INPUT);
                                                }
                                            });
                                        }
                                        ui.add_space(3.0);
                                        ui.label(
                                            RichText::new("Method and GraphQL name are also checked when a request is sent.")
                                                .size(9.0)
                                                .color(FAINT),
                                        );
                                    }
                                }
                            });

                            ui.add_space(10.0);
                            section_label(ui, "Wildcard cheat sheet");
                            ui.add_space(5.0);
                            Frame::none()
                                .fill(PANEL)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(9.0)
                                .inner_margin(Margin::same(11.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new("* matches anything, including ? and & in a query.")
                                            .size(10.5)
                                            .color(TEXT),
                                    );
                                    ui.label(
                                        RichText::new("A pattern without * still works as “contains”.")
                                            .size(10.0)
                                            .color(FAINT),
                                    );
                                    ui.add_space(6.0);
                                    pattern_example(
                                        ui,
                                        "*/v1/products?*",
                                        "any query string",
                                        Some("https://api.shopwave.dev/v1/products?limit=20"),
                                        &mut self.map_test_url,
                                    );
                                    pattern_example(
                                        ui,
                                        "*/v1/products?limit=20*",
                                        "only this query",
                                        Some("https://api.shopwave.dev/v1/products?limit=20&page=2"),
                                        &mut self.map_test_url,
                                    );
                                    pattern_example(
                                        ui,
                                        "*/graphql*",
                                        "GraphQL endpoint",
                                        Some("https://api.shopwave.dev/graphql"),
                                        &mut self.map_test_url,
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new("? stays literal here, so query rules are easy to write.")
                                            .size(9.0)
                                            .color(FAINT),
                                    );
                                });

                            ui.add_space(10.0);
                            section_label(ui, "UI Test Hook Connector (WireMock / MCP)");
                            ui.add_space(5.0);
                            Frame::none()
                                .fill(PANEL)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(9.0)
                                .inner_margin(Margin::same(11.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "Control hook active on http://127.0.0.1:{}/__proxzy/*",
                                            self.proxy_port
                                        ))
                                        .font(mono(9.5))
                                        .color(GREEN),
                                    );
                                    ui.label(
                                        RichText::new(
                                            "MobileWright, mobile-mcp, XCTest, or curl can switch scenarios and verify hit counts live:",
                                        )
                                        .size(9.5)
                                        .color(MUTED),
                                    );
                                    ui.add_space(6.0);
                                    let port = self.proxy_port;
                                    ui.horizontal(|ui| {
                                        if secondary_button(ui, "Copy Health Check").clicked() {
                                            copy_text_action = Some((
                                                format!("curl -s http://127.0.0.1:{}/__proxzy/health", port),
                                                "Copied /__proxzy/health curl command",
                                            ));
                                        }
                                        if secondary_button(ui, "Copy Scenario Switch").clicked() {
                                            copy_text_action = Some((
                                                format!(
                                                    "curl -s -X POST http://127.0.0.1:{}/__proxzy/scenario -d '{{\"pattern\":\"/v1/checkout\",\"scenario\":\"error\"}}'",
                                                    port
                                                ),
                                                "Copied /__proxzy/scenario curl command",
                                            ));
                                        }
                                    });
                                    ui.add_space(4.0);
                                    if secondary_button(ui, "Copy Dynamic Mock Inject").clicked() {
                                        copy_text_action = Some((
                                            format!(
                                                "curl -s -X POST http://127.0.0.1:{}/__proxzy/mock -d '{{\"pattern\":\"https://api.shopwave.dev/v1/orders*\",\"scenario\":\"success\",\"status\":200,\"body\":\"{{\\\"ok\\\":true}}\"}}'",
                                                port
                                            ),
                                            "Copied /__proxzy/mock curl command",
                                        ));
                                    }
                                });

                            ui.add_space(10.0);
                            section_label(ui, "How mapping works");
                            ui.add_space(5.0);
                            Frame::none()
                                .fill(PANEL)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(9.0)
                                .inner_margin(Margin::same(11.0))
                                .show(ui, |ui| {
                                    step_row(ui, 1, "Match", "Method, URL pattern and optional GraphQL name.");
                                    step_row(ui, 2, "Replace", "Your status, headers and JSON body are returned.");
                                    step_row(ui, 3, "Inspect", "The call is marked MOCK and never reaches a server.");
                                });
                        },
                    );
                });
            });

        if let Some((id, target_sc)) = switch_scenario_action {
            if let Some(r) = self.map_rules.iter_mut().find(|r| r.id == id) {
                r.switch_scenario(target_sc);
                r.enabled = true;
                let name = r.name.clone();
                let st = r.status;
                self.map_enabled = true;
                self.sync_proxy_config();
                self.log(
                    Level::Info,
                    "Map Local",
                    format!(
                        "Switched \"{}\" to {} ({})",
                        name,
                        target_sc.label(),
                        st
                    ),
                );
                self.toast_now(
                    ctx,
                    &format!("Switched \"{}\" → {} ({})", name, target_sc.label(), st),
                );
            }
        }
        if let Some((text, msg)) = copy_text_action {
            self.copy(ctx, text, msg);
        }
        if let Some(id) = toggle_id {
            if let Some(r) = self.map_rules.iter_mut().find(|r| r.id == id) {
                r.enabled = !r.enabled;
                let state = r.enabled;
                let name = r.name.clone();
                self.sync_proxy_config();
                self.log(
                    Level::Info,
                    "Map Local",
                    format!("Rule \"{}\" {}", name, if state { "enabled" } else { "disabled" }),
                );
            }
        }
        if let Some(f) = toggle_collapse_folder {
            if self.map_folder_collapsed.contains(&f) {
                self.map_folder_collapsed.remove(&f);
            } else {
                self.map_folder_collapsed.insert(f);
            }
        }
        if let Some((f, on)) = batch_folder_toggle {
            for r in self.map_rules.iter_mut().filter(|r| r.folder_name() == f) {
                r.enabled = on;
            }
            self.sync_proxy_config();
        }
        if let Some((f, sc)) = batch_folder_scenario {
            for r in self.map_rules.iter_mut().filter(|r| r.folder_name() == f) {
                r.switch_scenario(sc);
                r.enabled = true;
            }
            self.map_enabled = true;
            self.sync_proxy_config();
            self.toast_now(ctx, &format!("Batch switched '{}' → {}", f, sc.label()));
        }
        if create_new {
            self.open_map_editor(None);
        } else if let Some(rule) = edit_existing {
            self.open_map_editor(Some(rule));
        }
    }

    fn breakpoints_page_v2(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(RichText::new("Breakpoints").size(24.0).strong().color(TEXT));
                ui.label(
                    RichText::new("Catch matching traffic, edit it, then continue or abort.")
                        .size(12.0)
                        .color(MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let on = self.catch_enabled;
                if toggle(ui, on).clicked() {
                    self.catch_enabled = !on;
                    self.log(
                        if on { Level::Warn } else { Level::Info },
                        "Breakpoints",
                        if on {
                            "Catching paused"
                        } else {
                            "Catching resumed"
                        },
                    );
                }
                ui.label(
                    RichText::new(if on { "Catching on" } else { "Catching off" })
                        .size(11.5)
                        .strong()
                        .color(if on { AMBER } else { FAINT }),
                );
            });
        });
        ui.add_space(10.0);

        let armed = self.breakpoints.iter().filter(|b| b.enabled).count();
        let ready: Vec<(usize, BreakpointRule)> = self
            .captures
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                self.breakpoints
                    .iter()
                    .find(|r| r.enabled && r.matches(&c.method, &c.url, &c.req_body))
                    .cloned()
                    .map(|r| (i, r))
            })
            .collect();
        let selected = self.captures.get(self.selected).cloned();
        let selected_rule = selected.as_ref().and_then(|c| {
            self.breakpoints
                .iter()
                .find(|r| r.enabled && r.matches(&c.method, &c.url, &c.req_body))
                .cloned()
        });
        Frame::none()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(8.0)
            .inner_margin(Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    dot(ui, if armed > 0 && self.catch_enabled { AMBER } else { FAINT }, 7.0);
                    ui.label(
                        RichText::new(format!(
                            "{} rules · {} armed · {} captured requests would pause",
                            self.breakpoints.len(),
                            armed,
                            ready.len()
                        ))
                        .size(11.5)
                        .color(TEXT),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new("only matching traffic is caught")
                                .size(10.0)
                                .color(FAINT),
                        );
                    });
                });
            });
        ui.add_space(10.0);

        let mut create_new = false;
        ui.horizontal(|ui| {
            if primary_button(ui, "+  New catch rule").clicked() {
                create_new = true;
            }
            if secondary_button(ui, "Use selected request").clicked() {
                if let Some(c) = &selected {
                    self.open_bp_editor_for_capture(c);
                } else {
                    self.toast_now(ctx, "Select a request in the traffic list first");
                }
            }
            ui.add_space(6.0);
            search_box(ui, &mut self.bp_filter, 230.0);
        });
        ui.add_space(10.0);

        let total = ui.available_width();
        let right_w = (total * 0.34).clamp(290.0, 360.0);
        let left_w = (total - right_w - 16.0).max(430.0);
        let mut edit_existing: Option<BreakpointRule> = None;
        let mut toggle_id: Option<u64> = None;
        let mut catch_rule: Option<BreakpointRule> = None;
        let mut catch_target: Option<(usize, BreakpointRule)> = None;
        let mut catch_selected = false;

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(left_w, 620.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(left_w);
                            let q = self.bp_filter.trim().to_lowercase();
                            let rules: Vec<BreakpointRule> = self
                                .breakpoints
                                .iter()
                                .filter(|r| {
                                    q.is_empty()
                                        || r.name.to_lowercase().contains(&q)
                                        || r.pattern.to_lowercase().contains(&q)
                                        || r.graphql_name.to_lowercase().contains(&q)
                                })
                                .cloned()
                                .collect();

                            if rules.is_empty() {
                                Frame::none()
                                    .fill(PANEL)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .rounding(10.0)
                                    .inner_margin(Margin::same(18.0))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new("No catch rules here yet")
                                                .size(14.0)
                                                .strong()
                                                .color(TEXT),
                                        );
                                        ui.label(
                                            RichText::new("Create one, or type a different search.")
                                                .size(11.0)
                                                .color(MUTED),
                                        );
                                    });
                            }

                            for rule in &rules {
                                let live = self.bp_match_count(rule);
                                Frame::none()
                                    .fill(if rule.enabled { CARD } else { PANEL })
                                    .stroke(Stroke::new(
                                        1.0,
                                        if rule.enabled { BORDER } else { Color32::from_rgb(42, 42, 46) },
                                    ))
                                    .rounding(9.0)
                                    .inner_margin(Margin::symmetric(11.0, 9.0))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            if toggle(ui, rule.enabled).clicked() {
                                                toggle_id = Some(rule.id);
                                            }
                                            ui.allocate_ui_with_layout(
                                                Vec2::new(
                                                    (ui.available_width() - 225.0).max(140.0),
                                                    42.0,
                                                ),
                                                Layout::top_down(Align::Min),
                                                |ui| {
                                                    ui.horizontal(|ui| {
                                                        let name = ui.add(
                                                            egui::Label::new(
                                                                RichText::new(&rule.name)
                                                                    .size(12.5)
                                                                    .strong()
                                                                    .color(if rule.enabled {
                                                                        TEXT
                                                                    } else {
                                                                        MUTED
                                                                    }),
                                                            )
                                                            .sense(Sense::click()),
                                                        );
                                                        if name.clicked() {
                                                            edit_existing = Some(rule.clone());
                                                        }
                                                        if !rule.graphql_name.is_empty() {
                                                            pill(
                                                                ui,
                                                                &format!("GQL {}", rule.graphql_name),
                                                                PURPLE,
                                                                Color32::from_rgba_unmultiplied(
                                                                    184, 140, 255, 22,
                                                                ),
                                                            );
                                                        }
                                                    });
                                                    ui.label(
                                                        RichText::new(&rule.pattern)
                                                            .font(mono(10.5))
                                                            .color(Color32::from_rgb(150, 183, 235)),
                                                    );
                                                },
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    if secondary_button(ui, "Catch").clicked() {
                                                        catch_rule = Some(rule.clone());
                                                    }
                                                    if secondary_button(ui, "Edit").clicked() {
                                                        edit_existing = Some(rule.clone());
                                                    }
                                                    ui.label(
                                                        RichText::new(format!("{} hit", live))
                                                            .size(9.5)
                                                            .color(if live > 0 { AMBER } else { FAINT }),
                                                    );
                                                    pill(
                                                        ui,
                                                        rule.stage.label(),
                                                        if rule.stage == BpStage::Request {
                                                            AMBER
                                                        } else {
                                                            PURPLE
                                                        },
                                                        INPUT,
                                                    );
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(6.0);
                            }

                            ui.add_space(4.0);
                            Frame::none()
                                .fill(INPUT)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(8.0)
                                .inner_margin(Margin::same(10.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new("Catch pauses the request at the chosen stage. Continue applies your edits; Abort stops it.")
                                            .size(10.0)
                                            .color(FAINT),
                                    );
                                });
                        },
                    );

                    ui.add_space(16.0);

                    ui.allocate_ui_with_layout(
                        Vec2::new(right_w, 620.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(right_w);
                            section_label(ui, "Live catcher");
                            ui.add_space(5.0);
                            card().show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                if let Some(c) = &selected {
                                    ui.horizontal(|ui| {
                                        method_badge(ui, &c.method);
                                        wave_badge(ui, c.status);
                                        ui.label(
                                            RichText::new(c.host_path())
                                                .font(mono(10.0))
                                                .color(TEXT),
                                        );
                                    });
                                    ui.add_space(5.0);
                                    if let Some(rule) = &selected_rule {
                                        ui.horizontal(|ui| {
                                            dot(ui, AMBER, 7.0);
                                            ui.label(
                                                RichText::new(format!(
                                                    "Will pause at {} with “{}”",
                                                    rule.stage.label(),
                                                    rule.name
                                                ))
                                                .size(10.5)
                                                .color(AMBER),
                                            );
                                        });
                                    } else {
                                        ui.horizontal(|ui| {
                                            dot(ui, FAINT, 7.0);
                                            ui.label(
                                                RichText::new("No armed rule catches this request yet")
                                                    .size(10.5)
                                                    .color(MUTED),
                                            );
                                        });
                                    }
                                } else {
                                    ui.label(
                                        RichText::new("No request selected")
                                            .size(11.0)
                                            .color(MUTED),
                                    );
                                }
                                ui.add_space(8.0);
                                if primary_button(ui, "Catch selected request").clicked() {
                                    catch_selected = true;
                                }
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("If no rule matches, a new one is created from this request.")
                                        .size(9.5)
                                        .color(FAINT),
                                );
                            });

                            ui.add_space(10.0);
                            section_label(ui, "Ready to catch");
                            ui.add_space(5.0);
                            Frame::none()
                                .fill(PANEL)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(9.0)
                                .inner_margin(Margin::same(10.0))
                                .show(ui, |ui| {
                                    if ready.is_empty() {
                                        ui.label(
                                            RichText::new("Nothing matches the armed rules yet.")
                                                .size(10.5)
                                                .color(MUTED),
                                        );
                                        ui.label(
                                            RichText::new("Send a request from Composer to see it here.")
                                                .size(9.5)
                                                .color(FAINT),
                                        );
                                    } else {
                                        for (i, rule) in ready.iter().take(4) {
                                            let c = &self.captures[*i];
                                            ui.horizontal(|ui| {
                                                method_badge(ui, &c.method);
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(c.host_path())
                                                            .font(mono(9.5))
                                                            .color(MUTED),
                                                    )
                                                    .truncate(),
                                                );
                                                ui.with_layout(
                                                    Layout::right_to_left(Align::Center),
                                                    |ui| {
                                                        if secondary_button(ui, "Catch").clicked() {
                                                            catch_target =
                                                                Some((*i, rule.clone()));
                                                        }
                                                    },
                                                );
                                            });
                                        }
                                    }
                                });

                            ui.add_space(10.0);
                            section_label(ui, "Test a URL");
                            ui.add_space(5.0);
                            Frame::none()
                                .fill(PANEL)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(9.0)
                                .inner_margin(Margin::same(10.0))
                                .show(ui, |ui| {
                                    input_frame().show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.add(
                                            egui::TextEdit::singleline(&mut self.bp_test_url)
                                                .frame(false)
                                                .desired_width(f32::INFINITY)
                                                .font(TextStyle::Monospace)
                                                .text_color(TEXT)
                                                .hint_text("*/v1/checkout?*"),
                                        );
                                    });
                                    ui.add_space(5.0);
                                    ui.horizontal(|ui| {
                                        if secondary_button(ui, "Use selected").clicked() {
                                            if let Some(c) = &selected {
                                                self.bp_test_url = c.url.clone();
                                            }
                                        }
                                        if secondary_button(ui, "Clear").clicked() {
                                            self.bp_test_url.clear();
                                        }
                                    });
                                    ui.add_space(6.0);
                                    form_label(ui, "Catch at");
                                    ui.horizontal(|ui| {
                                        if chip(
                                            ui,
                                            "Request",
                                            self.bp_stage_override == BpStage::Request,
                                        )
                                        .clicked()
                                        {
                                            self.bp_stage_override = BpStage::Request;
                                        }
                                        if chip(
                                            ui,
                                            "Response",
                                            self.bp_stage_override == BpStage::Response,
                                        )
                                        .clicked()
                                        {
                                            self.bp_stage_override = BpStage::Response;
                                        }
                                    });
                                    ui.add_space(6.0);
                                    let url = if self.bp_test_url.trim().is_empty() {
                                        selected.as_ref().map(|c| c.url.clone()).unwrap_or_default()
                                    } else {
                                        self.bp_test_url.trim().to_string()
                                    };
                                    if url.is_empty() {
                                        ui.label(
                                            RichText::new("Type a URL to see which rule would pause it.")
                                                .size(10.0)
                                                .color(FAINT),
                                        );
                                    } else {
                                        let matched: Vec<&BreakpointRule> = self
                                            .breakpoints
                                            .iter()
                                            .filter(|r| {
                                                r.enabled && pattern_matches_url(&r.pattern, &url)
                                            })
                                            .collect();
                                        if matched.is_empty() {
                                            ui.horizontal(|ui| {
                                                dot(ui, FAINT, 6.0);
                                                ui.label(
                                                    RichText::new("No armed rule matches")
                                                        .size(10.0)
                                                        .color(MUTED),
                                                );
                                            });
                                        } else {
                                            ui.horizontal(|ui| {
                                                dot(ui, AMBER, 6.0);
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} rule{} would pause it",
                                                        matched.len(),
                                                        if matched.len() == 1 { "" } else { "s" }
                                                    ))
                                                    .size(10.0)
                                                    .strong()
                                                    .color(AMBER),
                                                );
                                            });
                                            for r in matched.iter().take(3) {
                                                ui.label(
                                                    RichText::new(format!(
                                                        "· {} ({})",
                                                        r.name,
                                                        r.stage.label()
                                                    ))
                                                    .size(9.5)
                                                    .color(MUTED),
                                                );
                                            }
                                        }
                                    }
                                });

                            ui.add_space(10.0);
                            section_label(ui, "Wildcard cheat sheet");
                            ui.add_space(5.0);
                            Frame::none()
                                .fill(PANEL)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(9.0)
                                .inner_margin(Margin::same(11.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new("* matches anything, including the whole query string.")
                                            .size(10.5)
                                            .color(TEXT),
                                    );
                                    ui.add_space(5.0);
                                    for (p, why) in [
                                        ("*/v1/checkout?*", "any checkout query"),
                                        ("*/graphql*", "all GraphQL calls"),
                                        ("*/v1/orders/*", "every order id"),
                                    ] {
                                        ui.label(
                                            RichText::new(p)
                                                .font(mono(9.5))
                                                .color(Color32::from_rgb(150, 183, 235)),
                                        );
                                        ui.label(RichText::new(why).size(9.0).color(FAINT));
                                    }
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new("? stays literal so query rules work.")
                                            .size(9.0)
                                            .color(FAINT),
                                    );
                                });
                        },
                    );
                });
            });

        if let Some(id) = toggle_id {
            if let Some(r) = self.breakpoints.iter_mut().find(|r| r.id == id) {
                r.enabled = !r.enabled;
                let state = r.enabled;
                let name = r.name.clone();
                self.log(
                    Level::Info,
                    "Breakpoints",
                    format!("Rule \"{}\" {}", name, if state { "armed" } else { "disarmed" }),
                );
            }
        }
        if create_new {
            self.open_bp_editor(None);
        } else if let Some(rule) = edit_existing {
            self.open_bp_editor(Some(rule));
        } else if catch_selected {
            if let Some(c) = &selected {
                self.catch_capture_now(c, ctx);
            }
        } else if let Some(rule) = catch_rule {
            self.catch_with_rule(&rule, ctx);
        } else if let Some((i, rule)) = catch_target {
            self.selected = i;
            self.catch_with_rule(&rule, ctx);
        }
    }

    fn rule_modals(&mut self, ctx: &egui::Context) {
        self.map_editor_modal_v2(ctx);
        self.bp_editor_modal_v2(ctx);
    }

    fn map_editor_modal_v2(&mut self, ctx: &egui::Context) {
        if self.map_editor.is_none() {
            return;
        }
        let captures = self.captures.clone();
        let selected = self.captures.get(self.selected).cloned();
        let mut save = false;
        let mut cancel = false;

        egui::Area::new(Id::new("map_editor_dim"))
            .order(egui::Order::Middle)
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let rect = ctx.screen_rect();
                ui.painter().rect_filled(
                    rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(3, 3, 4, 200),
                );
                ui.allocate_rect(rect, Sense::click());
            });

        let size = Vec2::new(760.0, 716.0);
        let rect = Rect::from_center_size(ctx.screen_rect().center(), size);
        egui::Area::new(Id::new("map_editor_modal"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                Frame::none()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, Color32::from_rgb(75, 75, 82)))
                    .rounding(14.0)
                    .inner_margin(Margin::same(20.0))
                    .show(ui, |ui| {
                        ui.set_width(720.0);
                        ui.set_height(676.0);
                        if let Some(ed) = &mut self.map_editor {
                            let title = if ed.id.is_some() {
                                "Edit mapping"
                            } else {
                                "New mapping"
                            };
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(title).size(19.0).strong().color(TEXT));
                                pill(
                                    ui,
                                    "MAP LOCAL",
                                    AMBER,
                                    Color32::from_rgba_unmultiplied(255, 181, 84, 20),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if icon_button(ui, "x", "Close").clicked() {
                                        cancel = true;
                                    }
                                });
                            });
                            ui.add_space(8.0);

                            ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .max_height(574.0)
                                .show(ui, |ui| {
                                    section_label(ui, "Quick import from cURL or direct URL");
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        let w = (ui.available_width() - 125.0).max(200.0);
                                        input_frame().show(ui, |ui| {
                                            ui.set_width(w);
                                            ui.add(
                                                egui::TextEdit::singleline(&mut ed.curl_paste)
                                                    .frame(false)
                                                    .desired_width(w)
                                                    .font(TextStyle::Monospace)
                                                    .text_color(TEXT)
                                                    .hint_text("Paste curl -X POST 'https://api.example.com/v1/checkout' ... or direct URL"),
                                            );
                                        });
                                        if secondary_button(ui, "Fill from cURL").clicked() {
                                            let _ = ed.apply_curl();
                                        }
                                    });
                                    ui.add_space(8.0);
                                    section_label(ui, "Matching rule");
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.set_width(290.0);
                                            form_label(ui, "Folder / Module");
                                            input_frame().show(ui, |ui| {
                                                ui.set_width(ui.available_width());
                                                ui.add(
                                                    egui::TextEdit::singleline(&mut ed.folder)
                                                        .frame(false)
                                                        .desired_width(f32::INFINITY)
                                                        .hint_text("General")
                                                        .text_color(TEXT),
                                                );
                                            });
                                        });
                                        ui.vertical(|ui| {
                                            ui.set_width(380.0);
                                            form_label(ui, "Quick Module Chips");
                                            ui.horizontal(|ui| {
                                                for mod_name in ["Auth", "Wallet", "Home", "Watch", "Profile", "General"] {
                                                    let active = ed.folder.eq_ignore_ascii_case(mod_name);
                                                    if chip(ui, mod_name, active).clicked() {
                                                        ed.folder = mod_name.to_string();
                                                    }
                                                }
                                            });
                                        });
                                    });
                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.set_width(470.0);
                                            form_label(ui, "Name");
                                            input_frame().show(ui, |ui| {
                                                ui.set_width(ui.available_width());
                                                ui.add(
                                                    egui::TextEdit::singleline(&mut ed.name)
                                                        .frame(false)
                                                        .desired_width(f32::INFINITY)
                                                        .text_color(TEXT),
                                                );
                                            });
                                        });
                                        ui.vertical(|ui| {
                                            ui.set_width(170.0);
                                            form_label(ui, "Method");
                                            egui::ComboBox::from_id_salt("map_editor_method")
                                                .width(170.0)
                                                .selected_text(rule_method_label(ed.method))
                                                .show_ui(ui, |ui| {
                                                    for m in [
                                                        RuleMethod::Any,
                                                        RuleMethod::Get,
                                                        RuleMethod::Post,
                                                        RuleMethod::Put,
                                                        RuleMethod::Delete,
                                                    ] {
                                                        ui.selectable_value(
                                                            &mut ed.method,
                                                            m,
                                                            m.label(),
                                                        );
                                                    }
                                                });
                                        });
                                    });
                                    ui.add_space(7.0);
                                    form_label(ui, "URL pattern  (use * for anything)");
                                    input_frame().show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.add(
                                            egui::TextEdit::singleline(&mut ed.pattern)
                                                .frame(false)
                                                .desired_width(f32::INFINITY)
                                                .font(TextStyle::Monospace)
                                                .text_color(TEXT)
                                                .hint_text("*/v1/products?*"),
                                        );
                                    });
                                    ui.add_space(5.0);
                                    ui.horizontal(|ui| {
                                        if secondary_button(ui, "Any query").clicked() {
                                            if !ed.pattern.contains('?') {
                                                ed.pattern.push_str("?*");
                                            } else if !ed.pattern.ends_with('*') {
                                                ed.pattern.push('*');
                                            }
                                        }
                                        if secondary_button(ui, "Include subpaths").clicked() {
                                            if !ed.pattern.ends_with('*') {
                                                ed.pattern.push('*');
                                            }
                                        }
                                        if secondary_button(ui, "Use selected request").clicked() {
                                            if let Some(c) = &selected {
                                                ed.pattern = capture_pattern(c);
                                            }
                                        }
                                        if secondary_button(ui, "Clear").clicked() {
                                            ed.pattern.clear();
                                        }
                                        if secondary_button(ui, "Wildcard Remap (Prod/Preprod)").clicked() {
                                            ed.pattern = make_environment_wildcard_pattern(&ed.pattern);
                                        }
                                    });
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new("Examples:  */v1/products?*  ·  */graphql*  ·  */v1/orders/*")
                                            .font(mono(9.0))
                                            .color(FAINT),
                                    );

                                    // Live preview against the traffic already captured.
                                    let preview = MapRule {
                                        id: 0,
                                        name: String::new(),
                                        pattern: ed.pattern.clone(),
                                        method: ed.method,
                                        status: 200,
                                        content_type: String::new(),
                                        delay_ms: 0,
                                        body: String::new(),
                                        graphql_name: ed.graphql_name.clone(),
                                        enabled: true,
                                        hits: 0,
                                        ..MapRule::default()
                                    };
                                    let matches: Vec<&Capture> = captures
                                        .iter()
                                        .filter(|c| {
                                            preview.matches(&c.method, &c.url, &c.req_body)
                                        })
                                        .collect();
                                    ui.add_space(6.0);
                                    Frame::none()
                                        .fill(if matches.is_empty() {
                                            INPUT
                                        } else {
                                            Color32::from_rgba_unmultiplied(61, 214, 149, 12)
                                        })
                                        .stroke(Stroke::new(
                                            1.0,
                                            if matches.is_empty() {
                                                BORDER
                                            } else {
                                                Color32::from_rgba_unmultiplied(61, 214, 149, 70)
                                            },
                                        ))
                                        .rounding(7.0)
                                        .inner_margin(Margin::same(8.0))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                dot(
                                                    ui,
                                                    if matches.is_empty() { FAINT } else { GREEN },
                                                    6.0,
                                                );
                                                ui.label(
                                                    RichText::new(if matches.is_empty() {
                                                        "No captured request matches yet".to_string()
                                                    } else {
                                                        format!(
                                                            "Matches {} captured request{}",
                                                            matches.len(),
                                                            if matches.len() == 1 { "" } else { "s" }
                                                        )
                                                    })
                                                    .size(10.0)
                                                    .color(if matches.is_empty() { MUTED } else { GREEN }),
                                                );
                                            });
                                            if let Some(c) = matches.first() {
                                                ui.label(
                                                    RichText::new(c.host_path())
                                                        .font(mono(9.0))
                                                        .color(MUTED),
                                                );
                                            }
                                        });

                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.set_width(120.0);
                                            form_label(ui, "Status");
                                            input_frame().show(ui, |ui| {
                                                ui.set_width(120.0);
                                                ui.add(
                                                    egui::TextEdit::singleline(&mut ed.status)
                                                        .frame(false)
                                                        .desired_width(116.0)
                                                        .font(TextStyle::Monospace)
                                                        .text_color(TEXT),
                                                );
                                            });
                                        });
                                        ui.vertical(|ui| {
                                            ui.set_width(120.0);
                                            form_label(ui, "Delay (ms)");
                                            input_frame().show(ui, |ui| {
                                                ui.set_width(120.0);
                                                ui.add(
                                                    egui::TextEdit::singleline(&mut ed.delay)
                                                        .frame(false)
                                                        .desired_width(116.0)
                                                        .font(TextStyle::Monospace)
                                                        .text_color(TEXT),
                                                );
                                            });
                                        });
                                        ui.vertical(|ui| {
                                            ui.set_width(220.0);
                                            form_label(ui, "Content type");
                                            input_frame().show(ui, |ui| {
                                                ui.set_width(220.0);
                                                ui.add(
                                                    egui::TextEdit::singleline(&mut ed.content_type)
                                                        .frame(false)
                                                        .desired_width(216.0)
                                                        .font(TextStyle::Monospace)
                                                        .text_color(TEXT),
                                                );
                                            });
                                        });
                                    });
                                    ui.add_space(7.0);
                                    form_label(ui, "GraphQL query name (optional)");
                                    input_frame().show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.add(
                                            egui::TextEdit::singleline(&mut ed.graphql_name)
                                                .frame(false)
                                                .desired_width(f32::INFINITY)
                                                .font(TextStyle::Monospace)
                                                .text_color(TEXT)
                                                .hint_text("GetProducts — leave blank for any operation"),
                                        );
                                    });

                                    ui.add_space(10.0);
                                    section_label(ui, "Response & Multi-Scenario Presets (Success / Error / Empty)");
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        let current_sc = ed.active_scenario;
                                        for sc in [
                                            MockScenario::Success,
                                            MockScenario::Error,
                                            MockScenario::Empty,
                                        ] {
                                            let active = current_sc == sc;
                                            if chip(ui, sc.label(), active).clicked() && !active {
                                                ed.switch_scenario(sc);
                                            }
                                        }
                                        if secondary_button(ui, "Format JSON").clicked() {
                                            ed.body = pretty_print(&ed.body);
                                        }
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            ui.label(
                                                RichText::new(mock_fixture_path(
                                                    ed.method,
                                                    &ed.pattern,
                                                    ed.active_scenario,
                                                ))
                                                .font(mono(9.0))
                                                .color(FAINT),
                                            );
                                        });
                                    });
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("DYNAMIC INJECTIONS:").size(9.0).color(FAINT).strong());
                                        for ph in ["{{USERNAME}}", "{{OTP}}", "{{TOKEN}}", "{{TIER}}"] {
                                            if chip(ui, ph, false).clicked() {
                                                ed.body.push_str(ph);
                                            }
                                        }
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            if secondary_button(ui, "Auto-derive Empty").clicked() {
                                                ed.empty_body = auto_derive_empty_scenario(&ed.body);
                                                ed.empty_status = "200".to_string();
                                            }
                                            if secondary_button(ui, "Redact PII (Sanitize)").clicked() {
                                                ed.body = sanitize_mock_payload(&ed.body);
                                                ed.sync_current_to_slot();
                                            }
                                        });
                                    });
                                    ui.add_space(4.0);
                                    input_frame().show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.add(
                                            egui::TextEdit::multiline(&mut ed.body)
                                                .frame(false)
                                                .desired_rows(9)
                                                .desired_width(f32::INFINITY)
                                                .font(TextStyle::Monospace)
                                                .text_color(TEXT),
                                        );
                                    });
                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        if toggle(ui, ed.enabled).clicked() {
                                            ed.enabled = !ed.enabled;
                                        }
                                        ui.label(
                                            RichText::new("Rule is on")
                                                .size(10.5)
                                                .color(if ed.enabled { GREEN } else { MUTED }),
                                        );
                                    });
                                });

                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Saved rules apply to Composer sends, Repeat and Send test.")
                                        .size(9.5)
                                        .color(FAINT),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if primary_button(ui, "Save mapping").clicked() {
                                        save = true;
                                    }
                                    ui.add_space(4.0);
                                    if secondary_button(ui, "Cancel").clicked() {
                                        cancel = true;
                                    }
                                });
                            });
                        }
                    });
            });

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if save {
            self.save_map_editor(ctx);
        } else if cancel {
            self.map_editor = None;
        }
    }

    fn bp_editor_modal_v2(&mut self, ctx: &egui::Context) {
        if self.bp_editor.is_none() {
            return;
        }
        let captures = self.captures.clone();
        let selected = self.captures.get(self.selected).cloned();
        let mut save = false;
        let mut save_catch = false;
        let mut cancel = false;

        egui::Area::new(Id::new("bp_editor_dim"))
            .order(egui::Order::Middle)
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let rect = ctx.screen_rect();
                ui.painter().rect_filled(
                    rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(3, 3, 4, 200),
                );
                ui.allocate_rect(rect, Sense::click());
            });

        let size = Vec2::new(660.0, 560.0);
        let rect = Rect::from_center_size(ctx.screen_rect().center(), size);
        egui::Area::new(Id::new("bp_editor_modal"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                Frame::none()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, Color32::from_rgb(75, 75, 82)))
                    .rounding(14.0)
                    .inner_margin(Margin::same(20.0))
                    .show(ui, |ui| {
                        ui.set_width(620.0);
                        ui.set_height(520.0);
                        if let Some(ed) = &mut self.bp_editor {
                            let title = if ed.id.is_some() {
                                "Edit catch rule"
                            } else {
                                "New catch rule"
                            };
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(title).size(19.0).strong().color(TEXT));
                                pill(
                                    ui,
                                    "BREAKPOINT",
                                    AMBER,
                                    Color32::from_rgba_unmultiplied(255, 181, 84, 20),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if icon_button(ui, "x", "Close").clicked() {
                                        cancel = true;
                                    }
                                });
                            });
                            ui.add_space(10.0);

                            form_label(ui, "Name");
                            input_frame().show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.add(
                                    egui::TextEdit::singleline(&mut ed.name)
                                        .frame(false)
                                        .desired_width(f32::INFINITY)
                                        .text_color(TEXT),
                                );
                            });
                            ui.add_space(7.0);
                            form_label(ui, "URL pattern  (use * for anything)");
                            input_frame().show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.add(
                                    egui::TextEdit::singleline(&mut ed.pattern)
                                        .frame(false)
                                        .desired_width(f32::INFINITY)
                                        .font(TextStyle::Monospace)
                                        .text_color(TEXT)
                                        .hint_text("*/v1/checkout?*"),
                                );
                            });
                            ui.add_space(5.0);
                            ui.horizontal(|ui| {
                                if secondary_button(ui, "Any query").clicked() {
                                    if !ed.pattern.contains('?') {
                                        ed.pattern.push_str("?*");
                                    } else if !ed.pattern.ends_with('*') {
                                        ed.pattern.push('*');
                                    }
                                }
                                if secondary_button(ui, "Use selected request").clicked() {
                                    if let Some(c) = &selected {
                                        ed.pattern = capture_pattern(c);
                                    }
                                }
                                if secondary_button(ui, "Clear").clicked() {
                                    ed.pattern.clear();
                                }
                            });
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Examples:  */v1/checkout?*  ·  */graphql*  ·  */v1/orders/*")
                                    .font(mono(9.0))
                                    .color(FAINT),
                            );

                            let preview = BreakpointRule {
                                id: 0,
                                name: String::new(),
                                pattern: ed.pattern.clone(),
                                graphql_name: ed.graphql_name.clone(),
                                stage: ed.stage,
                                enabled: true,
                                hits: 0,
                            };
                            let matches: Vec<&Capture> = captures
                                .iter()
                                .filter(|c| preview.matches(&c.method, &c.url, &c.req_body))
                                .collect();
                            ui.add_space(6.0);
                            Frame::none()
                                .fill(if matches.is_empty() {
                                    INPUT
                                } else {
                                    Color32::from_rgba_unmultiplied(255, 181, 84, 12)
                                })
                                .stroke(Stroke::new(
                                    1.0,
                                    if matches.is_empty() {
                                        BORDER
                                    } else {
                                        Color32::from_rgba_unmultiplied(255, 181, 84, 70)
                                    },
                                ))
                                .rounding(7.0)
                                .inner_margin(Margin::same(8.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        dot(
                                            ui,
                                            if matches.is_empty() { FAINT } else { AMBER },
                                            6.0,
                                        );
                                        ui.label(
                                            RichText::new(if matches.is_empty() {
                                                "No captured request matches yet".to_string()
                                            } else {
                                                format!(
                                                    "{} captured request{} ready to catch",
                                                    matches.len(),
                                                    if matches.len() == 1 { "" } else { "s" }
                                                )
                                            })
                                            .size(10.0)
                                            .color(if matches.is_empty() { MUTED } else { AMBER }),
                                        );
                                    });
                                    if let Some(c) = matches.first() {
                                        ui.label(
                                            RichText::new(c.host_path())
                                                .font(mono(9.0))
                                                .color(MUTED),
                                        );
                                    }
                                });

                            ui.add_space(9.0);
                            form_label(ui, "Catch at");
                            ui.horizontal(|ui| {
                                if chip(ui, "Before request", ed.stage == BpStage::Request).clicked() {
                                    ed.stage = BpStage::Request;
                                }
                                if chip(ui, "After response", ed.stage == BpStage::Response).clicked() {
                                    ed.stage = BpStage::Response;
                                }
                            });
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new(if ed.stage == BpStage::Request {
                                    "Pauses before the request leaves the app; edit method, headers or body."
                                } else {
                                    "Pauses before the response reaches the app; edit the response body."
                                })
                                .size(9.5)
                                .color(FAINT),
                            );

                            ui.add_space(9.0);
                            form_label(ui, "GraphQL query name (optional)");
                            input_frame().show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.add(
                                    egui::TextEdit::singleline(&mut ed.graphql_name)
                                        .frame(false)
                                        .desired_width(f32::INFINITY)
                                        .font(TextStyle::Monospace)
                                        .text_color(TEXT)
                                        .hint_text("UpdateCart — leave blank for any operation"),
                                );
                            });

                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                if toggle(ui, ed.enabled).clicked() {
                                    ed.enabled = !ed.enabled;
                                }
                                ui.label(
                                    RichText::new(if ed.enabled { "Armed" } else { "Saved but off" })
                                        .size(10.5)
                                        .color(if ed.enabled { AMBER } else { MUTED }),
                                );
                            });

                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Save & catch opens the pause editor on the next match.")
                                        .size(9.5)
                                        .color(FAINT),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if primary_button(ui, "Save & catch next").clicked() {
                                        save_catch = true;
                                    }
                                    ui.add_space(4.0);
                                    if secondary_button(ui, "Save rule").clicked() {
                                        save = true;
                                    }
                                    ui.add_space(4.0);
                                    if secondary_button(ui, "Cancel").clicked() {
                                        cancel = true;
                                    }
                                });
                            });
                        }
                    });
            });

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if save_catch {
            self.save_bp_editor(ctx, true);
        } else if save {
            self.save_bp_editor(ctx, false);
        } else if cancel {
            self.bp_editor = None;
        }
    }
}

fn pattern_example(
    ui: &mut Ui,
    pattern: &str,
    why: &str,
    sample: Option<&str>,
    target: &mut String,
) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new(pattern)
                    .font(mono(9.5))
                    .color(Color32::from_rgb(150, 183, 235)),
            );
            ui.label(RichText::new(why).size(9.0).color(FAINT));
        });
        if let Some(sample) = sample {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if secondary_button(ui, "Try").clicked() {
                    *target = sample.to_string();
                }
            });
        }
    });
}
