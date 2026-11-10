use std::collections::BTreeMap;

impl App {
    fn composer_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(RichText::new("Composer").size(24.0).strong().color(TEXT));
                ui.label(
                    RichText::new("Build a request, copy it, or let Map Local answer it.")
                        .size(11.5)
                        .color(MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                pill(ui, "POC", FAINT, INPUT);
                ui.label(
                    RichText::new("real send via curl")
                        .size(10.0)
                        .color(FAINT),
                );
            });
        });
        ui.add_space(10.0);
        let mut preset: Option<bool> = None;
        ui.horizontal(|ui| {
            if chip(ui, "GraphQL + Map Local", self.composer_url.contains("graphql")).clicked() {
                preset = Some(true);
            }
            if chip(ui, "REST live request", self.composer_url.contains("jsonplaceholder")).clicked()
            {
                preset = Some(false);
            }
            ui.add_space(2.0);
            ui.label(
                RichText::new("The GraphQL preset is answered locally; the REST preset uses the network.")
                    .size(10.0)
                    .color(FAINT),
            );
        });
        if let Some(g) = preset {
            self.set_composer_preset(g);
        }
        ui.add_space(8.0);

        let mut do_import_curl = false;
        let mut do_import_and_send = false;
        let mut do_save_mock = false;
        Frame::none()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(9.0)
            .inner_margin(Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    pill(
                        ui,
                        "PASTE cURL / DIRECT URL",
                        CYAN,
                        Color32::from_rgba_unmultiplied(88, 196, 255, 18),
                    );
                    let w = (ui.available_width() - 295.0).max(160.0);
                    input_frame().show(ui, |ui| {
                        ui.set_width(w);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.composer_curl_input)
                                .frame(false)
                                .desired_width(w)
                                .font(TextStyle::Monospace)
                                .text_color(TEXT)
                                .hint_text(
                                    "Paste curl -X POST 'https://api.example.com/v1/orders' -H ... -d '{...}' or direct URL",
                                ),
                        );
                    });
                    if secondary_button(ui, "Import cURL").clicked() {
                        do_import_curl = true;
                    }
                    if primary_button(ui, "Import & Send").clicked() {
                        do_import_and_send = true;
                    }
                    if secondary_button(ui, "Save as Mock").clicked() {
                        do_save_mock = true;
                    }
                });
            });
        if do_import_curl {
            self.import_curl_to_composer(ctx);
        }
        if do_import_and_send {
            self.import_curl_to_composer(ctx);
            self.send_composer(ctx);
        }
        if do_save_mock {
            self.save_composer_as_mock(ctx);
        }
        ui.add_space(10.0);

        let mut do_send = false;
        let mut do_copy_curl = false;
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let total = ui.available_width();
                let left = (total * 0.60)
                    .clamp(470.0, 760.0)
                    .min((total - 300.0).max(360.0));
                let right = (total - left - 16.0).max(270.0);
                let h = ui.available_height().max(420.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(left, h),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(left);
                            self.composer_builder(ui, &mut do_send, &mut do_copy_curl);
                        },
                    );
                    ui.add_space(16.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(right, h),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(right);
                            self.composer_result(ui, ctx);
                        },
                    );
                });
            });
        if do_send {
            self.send_composer(ctx);
        }
        if do_copy_curl {
            let curl = build_curl(
                &self.composer_method,
                &self.composer_url,
                &self.composer_headers,
                &self.composer_body,
            );
            self.copy(ctx, curl, "cURL command copied to clipboard");
        }
    }

    fn composer_builder(&mut self, ui: &mut Ui, do_send: &mut bool, do_copy_curl: &mut bool) {
        let matched = self
            .map_rules
            .iter()
            .find(|r| r.matches(&self.composer_method, &self.composer_url, &self.composer_body))
            .map(|r| r.name.clone());

        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("REQUEST")
                        .size(9.0)
                        .color(FAINT)
                        .strong(),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("draft").size(9.5).color(FAINT));
                });
            });
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("composer_method")
                    .width(82.0)
                    .selected_text(
                        RichText::new(&self.composer_method)
                            .font(mono(10.5))
                            .color(method_color(&self.composer_method)),
                    )
                    .show_ui(ui, |ui| {
                        for method in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
                            ui.selectable_value(
                                &mut self.composer_method,
                                method.to_string(),
                                method,
                            );
                        }
                    });
                let send_w = 96.0;
                let url_w = (ui.available_width() - send_w - 8.0).max(120.0);
                input_frame().show(ui, |ui| {
                    ui.set_width(url_w);
                    ui.add_sized(
                        [url_w, 18.0],
                        egui::TextEdit::singleline(&mut self.composer_url)
                            .frame(false)
                            .desired_width(url_w)
                            .font(TextStyle::Monospace)
                            .text_color(TEXT)
                            .hint_text("https://api.example.com/endpoint"),
                    );
                });
                if primary_button(ui, "Send").clicked() {
                    *do_send = true;
                }
            });
            if let Some(name) = &matched {
                ui.add_space(7.0);
                Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(255, 181, 84, 14))
                    .stroke(Stroke::new(
                        1.0,
                        Color32::from_rgba_unmultiplied(255, 181, 84, 80),
                    ))
                    .rounding(7.0)
                    .inner_margin(Margin::symmetric(8.0, 5.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            dot(ui, AMBER, 6.0);
                            ui.label(
                                RichText::new("Map Local will answer this")
                                    .size(10.0)
                                    .color(AMBER),
                            );
                            ui.label(RichText::new(name).size(10.0).color(MUTED));
                        });
                    });
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if composer_tab(ui, "Headers", 66.0, self.composer_tab == ComposerTab::Headers)
                    .clicked()
                {
                    self.composer_tab = ComposerTab::Headers;
                }
                if composer_tab(ui, "Body", 52.0, self.composer_tab == ComposerTab::Body).clicked()
                {
                    self.composer_tab = ComposerTab::Body;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} headers",
                            parse_headers(&self.composer_headers).len()
                        ))
                        .size(9.5)
                        .color(FAINT),
                    );
                });
            });
            ui.add_space(6.0);
            match self.composer_tab {
                ComposerTab::Headers => {
                    input_frame().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(
                            egui::TextEdit::multiline(&mut self.composer_headers)
                                .frame(false)
                                .desired_rows(6)
                                .desired_width(f32::INFINITY)
                                .font(TextStyle::Monospace)
                                .text_color(TEXT)
                                .hint_text("key: value"),
                        );
                    });
                }
                ComposerTab::Body => {
                    input_frame().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(
                            egui::TextEdit::multiline(&mut self.composer_body)
                                .frame(false)
                                .desired_rows(9)
                                .desired_width(f32::INFINITY)
                                .font(TextStyle::Monospace)
                                .text_color(TEXT)
                                .hint_text("{\n  \"query\": \"...\"\n}"),
                        );
                    });
                }
            }

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("cURL PREVIEW")
                        .size(9.0)
                        .color(FAINT)
                        .strong(),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "Copy cURL").clicked() {
                        *do_copy_curl = true;
                    }
                });
            });
            ui.add_space(4.0);
            let curl = build_curl(
                &self.composer_method,
                &self.composer_url,
                &self.composer_headers,
                &self.composer_body,
            );
            code_block(ui, &curl, 92.0, false);

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if primary_button(ui, "Send request").clicked() {
                    *do_send = true;
                }
                if secondary_button(ui, "Reset").clicked() {
                    self.set_composer_preset(true);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("20 s timeout").size(9.5).color(FAINT));
                });
            });
        });
    }

    fn composer_result(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut copy_body = false;
        let mut inspect = false;
        let mut save_resp_mock = false;
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("RESPONSE")
                        .size(9.0)
                        .color(FAINT)
                        .strong(),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if self.pending_send.is_some() {
                        pill(
                            ui,
                            "SENDING",
                            AMBER,
                            Color32::from_rgba_unmultiplied(255, 181, 84, 24),
                        );
                    } else if self.composer_response.is_some() {
                        pill(
                            ui,
                            "RECEIVED",
                            GREEN,
                            Color32::from_rgba_unmultiplied(61, 214, 149, 18),
                        );
                    } else {
                        pill(ui, "IDLE", FAINT, INPUT);
                    }
                });
            });
            ui.add_space(10.0);

            if self.pending_send.is_some() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.spinner();
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("Sending through curl...")
                            .size(12.0)
                            .color(MUTED),
                    );
                    ui.label(
                        RichText::new("Waiting for a response")
                            .size(10.0)
                            .color(FAINT),
                    );
                    ui.add_space(40.0);
                });
            } else if let Some(resp) = &self.composer_response {
                ui.horizontal(|ui| {
                    wave_badge(ui, resp.status);
                    ui.label(
                        RichText::new(status_text(resp.status))
                            .size(12.0)
                            .color(status_color(resp.status)),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{} ms", resp.latency_ms))
                                .font(mono(10.0))
                                .color(MUTED),
                        );
                    });
                });
                if let Some(rule) = &resp.mocked {
                    ui.add_space(6.0);
                    Frame::none()
                        .fill(Color32::from_rgba_unmultiplied(255, 181, 84, 14))
                        .stroke(Stroke::new(
                            1.0,
                            Color32::from_rgba_unmultiplied(255, 181, 84, 80),
                        ))
                        .rounding(7.0)
                        .inner_margin(Margin::symmetric(8.0, 5.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                dot(ui, AMBER, 6.0);
                                ui.label(
                                    RichText::new("Map Local")
                                        .size(10.0)
                                        .strong()
                                        .color(AMBER),
                                );
                                ui.label(RichText::new(rule).size(10.0).color(MUTED));
                            });
                        });
                }
                if let Some(err) = &resp.error {
                    ui.add_space(8.0);
                    Frame::none()
                        .fill(Color32::from_rgba_unmultiplied(255, 107, 107, 12))
                        .stroke(Stroke::new(
                            1.0,
                            Color32::from_rgba_unmultiplied(255, 107, 107, 80),
                        ))
                        .rounding(7.0)
                        .inner_margin(Margin::same(9.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Live send failed")
                                    .size(10.5)
                                    .strong()
                                    .color(RED),
                            );
                            ui.label(RichText::new(err).size(10.0).color(MUTED));
                            ui.label(
                                RichText::new("Try the GraphQL preset to see Map Local answer offline.")
                                    .size(9.5)
                                    .color(FAINT),
                            );
                        });
                }
                ui.add_space(2.0);
                ui.label(
                    RichText::new(format!("{} body", format_bytes(resp.size)))
                        .size(9.5)
                        .color(FAINT),
                );
                if !resp.headers.is_empty() {
                    ui.add_space(6.0);
                    ScrollArea::vertical()
                        .max_height(100.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (k, v) in &resp.headers {
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    ui.label(
                                        RichText::new(k)
                                            .font(mono(9.5))
                                            .color(Color32::from_rgb(126, 178, 255)),
                                    );
                                    ui.label(RichText::new(v).font(mono(9.5)).color(MUTED));
                                });
                            }
                        });
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    section_label(ui, "Response body");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        for format in [
                            ResponseFormat::Html,
                            ResponseFormat::Xml,
                            ResponseFormat::Raw,
                            ResponseFormat::Json,
                        ] {
                            if chip(ui, format.label(), self.response_format == format)
                                .on_hover_text(format.hint())
                                .clicked()
                            {
                                self.response_format = format;
                            }
                        }
                    });
                });
                ui.add_space(5.0);
                if resp.body.is_empty() {
                    empty_body(ui, "Empty response body");
                } else {
                    draw_formatted_body(ui, &resp.body, self.response_format, 250.0);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "Save Response as Mock").clicked() {
                        save_resp_mock = true;
                    }
                    if secondary_button(ui, "Copy body").clicked() {
                        copy_body = true;
                    }
                    if secondary_button(ui, "Inspect").clicked() {
                        inspect = true;
                    }
                });
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(28.0);
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
                    let c = ui.painter();
                    c.circle_stroke(rect.center(), 22.0, Stroke::new(1.0, BORDER));
                    c.circle_stroke(rect.center(), 13.0, Stroke::new(1.0, BORDER));
                    c.circle_filled(rect.center(), 3.0, ACCENT);
                    ui.add_space(12.0);
                    ui.label(RichText::new("No response yet").size(13.0).color(TEXT));
                    ui.label(
                        RichText::new("Send the draft to see status, headers and body.")
                            .size(10.5)
                            .color(FAINT),
                    );
                    ui.add_space(26.0);
                });
            }
            ui.add_space(4.0);
            hsep(ui);
            ui.add_space(6.0);
            ui.label(
                RichText::new("Responses are captured into the traffic list too.")
                    .size(9.5)
                    .color(FAINT),
            );
        });
        if save_resp_mock {
            self.save_composer_as_mock(ctx);
        }
        if copy_body {
            if let Some(resp) = &self.composer_response {
                self.copy(ctx, resp.body.clone(), "Response body copied");
            }
        }
        if inspect {
            self.view = View::Traffic;
            self.filter = TrafficFilter::All;
            self.inspector_tab = InspectorTab::Response;
        }
    }

    fn map_local_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(RichText::new("Map Local").size(24.0).strong().color(TEXT));
                ui.label(
                    RichText::new("Replace a server response with a local fixture. No backend needed.")
                        .size(11.5)
                        .color(MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                pill(
                    ui,
                    "ENGINE ON",
                    GREEN,
                    Color32::from_rgba_unmultiplied(61, 214, 149, 18),
                );
                ui.label(
                    RichText::new("rules apply to sends and replays")
                        .size(9.5)
                        .color(FAINT),
                );
            });
        });
        ui.add_space(12.0);

        let enabled = self.map_rules.iter().filter(|r| r.enabled).count();
        let hits: u32 = self.map_rules.iter().map(|r| r.hits).sum();
        let w = (ui.available_width() - 36.0) / 4.0;
        ui.horizontal(|ui| {
            kpi_card(
                ui,
                w,
                "RULES",
                &self.map_rules.len().to_string(),
                "local mappings",
                ACCENT,
            );
            kpi_card(ui, w, "ENABLED", &enabled.to_string(), "currently matching", GREEN);
            kpi_card(ui, w, "HITS", &hits.to_string(), "served from local", AMBER);
            kpi_card(ui, w, "ENGINE", "ON", "JSON responses", CYAN);
        });
        ui.add_space(12.0);

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let total = ui.available_width();
                let left = (total * 0.63).clamp(500.0, 780.0);
                let right = (total - left - 16.0).max(300.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(left, 620.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(left);
                            ui.horizontal(|ui| {
                                section_label(ui, "Mappings");
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new("first match wins")
                                            .size(9.5)
                                            .color(FAINT),
                                    );
                                });
                            });
                            ui.add_space(5.0);

                            let rules = self.map_rules.clone();
                            let mut remove_id: Option<u64> = None;
                            let mut test_rule: Option<MapRule> = None;
                            for rule in &rules {
                                Frame::none()
                                    .fill(CARD)
                                    .stroke(Stroke::new(
                                        1.0,
                                        if rule.enabled {
                                            BORDER
                                        } else {
                                            Color32::from_rgb(32, 39, 51)
                                        },
                                    ))
                                    .rounding(10.0)
                                    .inner_margin(Margin::same(12.0))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            if toggle(ui, rule.enabled).clicked() {
                                                if let Some(r) = self
                                                    .map_rules
                                                    .iter_mut()
                                                    .find(|r| r.id == rule.id)
                                                {
                                                    r.enabled = !r.enabled;
                                                }
                                                self.log(
                                                    Level::Info,
                                                    "Map Local",
                                                    format!(
                                                        "Rule \"{}\" {}",
                                                        rule.name,
                                                        if rule.enabled {
                                                            "disabled"
                                                        } else {
                                                            "enabled"
                                                        }
                                                    ),
                                                );
                                            }
                                            ui.vertical(|ui| {
                                                ui.spacing_mut().item_spacing.y = 0.0;
                                                ui.label(
                                                    RichText::new(&rule.name)
                                                        .size(12.5)
                                                        .strong()
                                                        .color(if rule.enabled {
                                                            TEXT
                                                        } else {
                                                            MUTED
                                                        }),
                                                );
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} · {} hits",
                                                        rule.content_type, rule.hits
                                                    ))
                                                    .size(9.5)
                                                    .color(FAINT),
                                                );
                                            });
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
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
                                                },
                                            );
                                        });
                                        ui.add_space(8.0);
                                        input_frame().show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(&rule.pattern)
                                                        .font(mono(10.0))
                                                        .color(Color32::from_rgb(
                                                            150, 180, 225,
                                                        )),
                                                );
                                                ui.with_layout(
                                                    Layout::right_to_left(Align::Center),
                                                    |ui| {
                                                        ui.label(
                                                            RichText::new("LOCAL JSON")
                                                                .size(8.5)
                                                                .color(AMBER),
                                                        );
                                                    },
                                                );
                                            });
                                        });
                                        ui.add_space(7.0);
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} ms delay",
                                                    rule.delay_ms
                                                ))
                                                .size(9.5)
                                                .color(FAINT),
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    if secondary_button(ui, "Remove").clicked() {
                                                        remove_id = Some(rule.id);
                                                    }
                                                    if secondary_button(ui, "Send test").clicked() {
                                                        test_rule = Some(rule.clone());
                                                    }
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(7.0);
                            }
                            if let Some(id) = remove_id {
                                self.map_rules.retain(|r| r.id != id);
                                self.log(Level::Warn, "Map Local", "A mapping was removed");
                            }
                            if let Some(rule) = test_rule {
                                self.test_map_rule(&rule, ctx);
                            }

                            ui.add_space(6.0);
                            section_label(ui, "How mapping works");
                            ui.add_space(5.0);
                            Frame::none()
                                .fill(PANEL)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(10.0)
                                .inner_margin(Margin::same(14.0))
                                .show(ui, |ui| {
                                    step_row(
                                        ui,
                                        1,
                                        "Match",
                                        "A method and a URL substring pick the request.",
                                    );
                                    step_row(
                                        ui,
                                        2,
                                        "Replace",
                                        "The fixture body is returned with the chosen status.",
                                    );
                                    step_row(
                                        ui,
                                        3,
                                        "Inspect",
                                        "The capture is marked MOCK and never reaches a server.",
                                    );
                                });
                        },
                    );
                    ui.add_space(16.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(right, 620.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(right);
                            section_label(ui, "New mapping");
                            ui.add_space(5.0);
                            card().show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                form_label(ui, "Rule name");
                                input_frame().show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.rule_name)
                                            .frame(false)
                                            .desired_width(f32::INFINITY)
                                            .text_color(TEXT),
                                    );
                                });
                                ui.add_space(7.0);
                                form_label(ui, "URL contains");
                                input_frame().show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.rule_pattern)
                                            .frame(false)
                                            .desired_width(f32::INFINITY)
                                            .font(TextStyle::Monospace)
                                            .text_color(TEXT)
                                            .hint_text("/v1/products"),
                                    );
                                });
                                ui.add_space(7.0);
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width((ui.available_width() - 8.0) / 2.0);
                                        form_label(ui, "Method");
                                        egui::ComboBox::from_id_salt("rule_method")
                                            .width(ui.available_width())
                                            .selected_text(rule_method_label(self.rule_method))
                                            .show_ui(ui, |ui| {
                                                for m in [
                                                    RuleMethod::Any,
                                                    RuleMethod::Get,
                                                    RuleMethod::Post,
                                                    RuleMethod::Put,
                                                    RuleMethod::Delete,
                                                ] {
                                                    ui.selectable_value(
                                                        &mut self.rule_method,
                                                        m,
                                                        m.label(),
                                                    );
                                                }
                                            });
                                    });
                                    ui.vertical(|ui| {
                                        ui.set_width((ui.available_width() - 8.0) / 2.0);
                                        form_label(ui, "Status");
                                        input_frame().show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.add(
                                                egui::TextEdit::singleline(&mut self.rule_status)
                                                    .frame(false)
                                                    .desired_width(f32::INFINITY)
                                                    .font(TextStyle::Monospace)
                                                    .text_color(TEXT),
                                            );
                                        });
                                    });
                                });
                                ui.add_space(7.0);
                                form_label(ui, "Delay (ms)");
                                input_frame().show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.rule_delay)
                                            .frame(false)
                                            .desired_width(f32::INFINITY)
                                            .font(TextStyle::Monospace)
                                            .text_color(TEXT),
                                    );
                                });
                                ui.add_space(7.0);
                                form_label(ui, "JSON body");
                                input_frame().show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.add(
                                        egui::TextEdit::multiline(&mut self.rule_body)
                                            .frame(false)
                                            .desired_rows(8)
                                            .desired_width(f32::INFINITY)
                                            .font(TextStyle::Monospace)
                                            .text_color(TEXT),
                                    );
                                });
                                ui.add_space(10.0);
                                if primary_button(ui, "Save mapping").clicked() {
                                    if self.rule_name.trim().is_empty()
                                        || self.rule_pattern.trim().is_empty()
                                    {
                                        let until = ui.input(|i| i.time) + 2.4;
                                        self.toast = Some(Toast {
                                            text: "Give the rule a name and a URL pattern".into(),
                                            until,
                                        });
                                    } else {
                                        let id = self
                                            .map_rules
                                            .iter()
                                            .map(|r| r.id)
                                            .max()
                                            .unwrap_or(0)
                                            + 1;
                                        let status =
                                            self.rule_status.trim().parse::<u16>().unwrap_or(200);
                                        let delay =
                                            self.rule_delay.trim().parse::<u64>().unwrap_or(0);
                                        let name = self.rule_name.clone();
                                        self.map_rules.push(MapRule {
                                            id,
                                            name: name.clone(),
                                            pattern: self.rule_pattern.clone(),
                                            method: self.rule_method,
                                            status,
                                            content_type: "application/json".into(),
                                            delay_ms: delay,
                                            body: self.rule_body.clone(),
                                            graphql_name: String::new(),
                                            enabled: true,
                                            hits: 0,
                                            ..MapRule::default()
                                        });
                                        self.log(
                                            Level::Info,
                                            "Map Local",
                                            format!("New mapping created: {}", name),
                                        );
                                        let until = ui.input(|i| i.time) + 2.4;
                                        self.toast = Some(Toast {
                                            text: format!("Mapping saved: {}", name),
                                            until,
                                        });
                                    }
                                }
                            });
                            ui.add_space(8.0);
                            Frame::none()
                                .fill(INPUT)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(8.0)
                                .inner_margin(Margin::same(10.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new("Tip: use Send test to capture a sample response without a backend.")
                                            .size(9.5)
                                            .color(FAINT),
                                    );
                                });
                        },
                    );
                });
            });
    }

    fn test_map_rule(&mut self, rule: &MapRule, ctx: &egui::Context) {
        let method = match rule.method {
            RuleMethod::Any => "GET",
            RuleMethod::Get => "GET",
            RuleMethod::Post => "POST",
            RuleMethod::Put => "PUT",
            RuleMethod::Delete => "DELETE",
        };
        let url = if rule.pattern.starts_with('/') {
            format!("https://api.shopwave.dev{}", rule.pattern)
        } else {
            format!("https://api.shopwave.dev/{}", rule.pattern)
        };
        let mut c = Capture {
            id: 0,
            method: method.into(),
            url,
            status: rule.status,
            latency_ms: rule.delay_ms,
            size_bytes: rule.body.len() as u64,
            kind: detect_kind(method, &rule.pattern, ""),
            started: now_stamp(),
            remote: "map-local".into(),
            protocol: "Local".into(),
            req_headers: vec![("accept".into(), "application/json".into())],
            req_body: String::new(),
            resp_headers: vec![
                ("content-type".into(), rule.content_type.clone()),
                ("x-proxzy-rule".into(), rule.name.clone()),
            ],
            resp_body: rule.body.clone(),
            graphql: None,
            mock: Some(rule.name.clone()),
            pinned: false,
            aborted: false,
        };
        c.graphql = graphql_info(method, "");
        if let Some(r) = self.map_rules.iter_mut().find(|r| r.id == rule.id) {
            r.hits += 1;
        }
        self.add_capture(c);
        self.view = View::Traffic;
        self.inspector_tab = InspectorTab::Overview;
        self.log(
            Level::Info,
            "Map Local",
            format!("Test request captured through \"{}\"", rule.name),
        );
        let until = ctx.input(|i| i.time) + 2.6;
        self.toast = Some(Toast {
            text: format!("Test captured: {}", rule.name),
            until,
        });
    }

    fn breakpoints_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(
                    RichText::new("Breakpoints")
                        .size(24.0)
                        .strong()
                        .color(TEXT),
                );
                ui.label(
                    RichText::new("Pause a request or response, edit it, then continue.")
                        .size(11.5)
                        .color(MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                pill(
                    ui,
                    "INTERCEPTOR READY",
                    AMBER,
                    Color32::from_rgba_unmultiplied(255, 181, 84, 18),
                );
            });
        });
        ui.add_space(12.0);
        let armed = self.breakpoints.iter().filter(|b| b.enabled).count();
        let w = (ui.available_width() - 36.0) / 4.0;
        ui.horizontal(|ui| {
            kpi_card(
                ui,
                w,
                "RULES",
                &self.breakpoints.len().to_string(),
                "URL conditions",
                ACCENT,
            );
            kpi_card(ui, w, "ARMED", &armed.to_string(), "waiting to pause", AMBER);
            kpi_card(ui, w, "PAUSED", "0", "no request held", GREEN);
            kpi_card(ui, w, "EDITS", "2", "applied this session", PURPLE);
        });
        ui.add_space(12.0);

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let total = ui.available_width();
                let left = (total * 0.61).clamp(490.0, 760.0);
                let right = (total - left - 16.0).max(320.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(left, 560.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(left);
                            section_label(ui, "Rules");
                            ui.add_space(5.0);
                            let rules = self.breakpoints.clone();
                            let mut pause_rule: Option<BreakpointRule> = None;
                            for rule in &rules {
                                Frame::none()
                                    .fill(CARD)
                                    .stroke(Stroke::new(1.0, BORDER))
                                    .rounding(10.0)
                                    .inner_margin(Margin::same(12.0))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            if toggle(ui, rule.enabled).clicked() {
                                                if let Some(r) = self
                                                    .breakpoints
                                                    .iter_mut()
                                                    .find(|r| r.id == rule.id)
                                                {
                                                    r.enabled = !r.enabled;
                                                }
                                                self.log(
                                                    Level::Info,
                                                    "Breakpoints",
                                                    format!(
                                                        "Rule \"{}\" {}",
                                                        rule.name,
                                                        if rule.enabled {
                                                            "disarmed"
                                                        } else {
                                                            "armed"
                                                        }
                                                    ),
                                                );
                                            }
                                            ui.vertical(|ui| {
                                                ui.spacing_mut().item_spacing.y = 0.0;
                                                ui.label(
                                                    RichText::new(&rule.name)
                                                        .size(12.5)
                                                        .strong()
                                                        .color(if rule.enabled {
                                                            TEXT
                                                        } else {
                                                            MUTED
                                                        }),
                                                );
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} · {} pauses",
                                                        rule.pattern, rule.hits
                                                    ))
                                                    .font(mono(9.5))
                                                    .color(FAINT),
                                                );
                                            });
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
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
                                                    if rule.enabled {
                                                        pill(
                                                            ui,
                                                            "ARMED",
                                                            GREEN,
                                                            Color32::from_rgba_unmultiplied(
                                                                61, 214, 149, 16,
                                                            ),
                                                        );
                                                    }
                                                },
                                            );
                                        });
                                        ui.add_space(8.0);
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(if rule.stage == BpStage::Request {
                                                    "Pauses before the request leaves the app"
                                                } else {
                                                    "Pauses before the response reaches the app"
                                                })
                                                .size(9.5)
                                                .color(FAINT),
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    if secondary_button(ui, "Pause now").clicked() {
                                                        pause_rule = Some(rule.clone());
                                                    }
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(7.0);
                            }
                            if let Some(rule) = pause_rule {
                                self.pause_with_rule(&rule, ctx);
                            }

                            ui.add_space(6.0);
                            section_label(ui, "How a pause works");
                            ui.add_space(5.0);
                            Frame::none()
                                .fill(PANEL)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(10.0)
                                .inner_margin(Margin::same(14.0))
                                .show(ui, |ui| {
                                    step_row(
                                        ui,
                                        1,
                                        "Arm",
                                        "A URL pattern decides which request pauses.",
                                    );
                                    step_row(
                                        ui,
                                        2,
                                        "Edit",
                                        "Change the method, headers or JSON body while it waits.",
                                    );
                                    step_row(
                                        ui,
                                        3,
                                        "Continue",
                                        "The edited traffic is recorded in the inspector.",
                                    );
                                });
                        },
                    );
                    ui.add_space(16.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(right, 560.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(right);
                            section_label(ui, "Test bench");
                            ui.add_space(5.0);
                            card().show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                if let Some(c) = self.captures.get(self.selected) {
                                    ui.label(
                                        RichText::new("SELECTED REQUEST")
                                            .size(8.5)
                                            .color(FAINT)
                                            .strong(),
                                    );
                                    ui.add_space(5.0);
                                    ui.horizontal(|ui| {
                                        method_badge(ui, &c.method);
                                        wave_badge(ui, c.status);
                                    });
                                    ui.add_space(3.0);
                                    ui.label(
                                        RichText::new(c.host_path())
                                            .font(mono(10.0))
                                            .color(TEXT),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new("No request selected")
                                            .size(11.0)
                                            .color(MUTED),
                                    );
                                }
                                ui.add_space(10.0);
                                form_label(ui, "Pause at");
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
                                ui.add_space(10.0);
                                if primary_button(ui, "Pause selected request").clicked() {
                                    self.trigger_breakpoint(ctx);
                                }
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new("Edits stay local. Abort records a cancelled request with a red status.")
                                        .size(9.5)
                                        .color(FAINT),
                                );
                            });
                            ui.add_space(10.0);
                            Frame::none()
                                .fill(Color32::from_rgba_unmultiplied(255, 181, 84, 10))
                                .stroke(Stroke::new(
                                    1.0,
                                    Color32::from_rgba_unmultiplied(255, 181, 84, 60),
                                ))
                                .rounding(9.0)
                                .inner_margin(Margin::same(11.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new("Breakpoint pause is a POC simulation")
                                            .size(10.5)
                                            .strong()
                                            .color(AMBER),
                                    );
                                    ui.label(
                                        RichText::new("Live rules pause the real request at the request stage; the editor then releases it or aborts it.")
                                            .size(9.5)
                                            .color(MUTED),
                                    );
                                });
                            ui.add_space(10.0);
                            section_label(ui, "Recent breakpoint events");
                            ui.add_space(5.0);
                            Frame::none()
                                .fill(PANEL)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(9.0)
                                .inner_margin(Margin::same(11.0))
                                .show(ui, |ui| {
                                    let mut shown = 0;
                                    for log in &self.logs {
                                        if log.source == "Breakpoints" && shown < 3 {
                                            shown += 1;
                                            ui.horizontal(|ui| {
                                                pill(
                                                    ui,
                                                    log.level.label(),
                                                    log.level.color(),
                                                    Color32::from_rgba_unmultiplied(
                                                        log.level.color().r(),
                                                        log.level.color().g(),
                                                        log.level.color().b(),
                                                        16,
                                                    ),
                                                );
                                                ui.label(
                                                    RichText::new(&log.message)
                                                        .size(9.5)
                                                        .color(MUTED),
                                                );
                                            });
                                        }
                                    }
                                    if shown == 0 {
                                        ui.label(
                                            RichText::new("No breakpoint events yet")
                                                .size(9.5)
                                                .color(FAINT),
                                        );
                                    }
                                });
                        },
                    );
                });
            });
    }

    fn pause_with_rule(&mut self, rule: &BreakpointRule, ctx: &egui::Context) {
        let Some(c) = self.captures.get(self.selected).cloned() else {
            let until = ctx.input(|i| i.time) + 2.4;
            self.toast = Some(Toast {
                text: "Select a request first".into(),
                until,
            });
            return;
        };
        let mut rule = rule.clone();
        rule.stage = self.bp_stage_override;
        self.open_breakpoint(c, rule, false);
    }

    fn logs_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(
                    RichText::new("Log Dashboard")
                        .size(24.0)
                        .strong()
                        .color(TEXT),
                );
                ui.label(
                    RichText::new("Traffic, rule hits and breakpoint events in one timeline.")
                        .size(11.5)
                        .color(MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if secondary_button(ui, "Clear logs").clicked() {
                    self.logs.clear();
                }
                if secondary_button(ui, "Copy summary").clicked() {
                    let summary = self.summary_text();
                    self.copy(ctx, summary, "Session summary copied");
                }
                pill(
                    ui,
                    "LIVE",
                    GREEN,
                    Color32::from_rgba_unmultiplied(61, 214, 149, 18),
                );
            });
        });
        ui.add_space(12.0);

        let total = self.captures.len();
        let errors = self.captures.iter().filter(|c| c.is_error()).count();
        let avg = if total == 0 {
            0
        } else {
            self.captures.iter().map(|c| c.latency_ms).sum::<u64>() / total as u64
        };
        let mocked = self.captures.iter().filter(|c| c.mock.is_some()).count();
        let w = (ui.available_width() - 36.0) / 4.0;
        ui.horizontal(|ui| {
            let mix = self.session_mix_label();
            kpi_card(
                ui,
                w,
                "REQUESTS",
                &total.to_string(),
                &mix,
                ACCENT,
            );
            kpi_card(
                ui,
                w,
                "ERROR RATE",
                &format!(
                    "{:.0}%",
                    if total == 0 {
                        0.0
                    } else {
                        errors as f64 * 100.0 / total as f64
                    }
                ),
                &format!("{} failed", errors),
                if errors > 0 { RED } else { GREEN },
            );
            kpi_card(
                ui,
                w,
                "AVG LATENCY",
                &format!("{} ms", avg),
                "across all captures",
                CYAN,
            );
            kpi_card(ui, w, "MOCKED", &mocked.to_string(), "served by Map Local", AMBER);
        });
        ui.add_space(12.0);

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let total_w = ui.available_width();
                let chart_w = (total_w * 0.63).max(440.0);
                let side_w = (total_w - chart_w - 16.0).max(260.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(chart_w, 205.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(chart_w);
                            Frame::none()
                                .fill(CARD)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(11.0)
                                .inner_margin(Margin::same(14.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Traffic over time")
                                                .size(12.5)
                                                .strong()
                                                .color(TEXT),
                                        );
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new("last 60 min")
                                                        .size(9.0)
                                                        .color(FAINT),
                                                );
                                            },
                                        );
                                    });
                                    ui.add_space(4.0);
                                    traffic_chart(ui, total);
                                });
                        },
                    );
                    ui.add_space(16.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(side_w, 205.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(side_w);
                            Frame::none()
                                .fill(CARD)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(11.0)
                                .inner_margin(Margin::same(14.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new("Status families")
                                            .size(12.5)
                                            .strong()
                                            .color(TEXT),
                                    );
                                    ui.add_space(8.0);
                                    status_bars(ui, &self.captures);
                                });
                        },
                    );
                });
                ui.add_space(12.0);

                let log_w = (total_w * 0.63).max(440.0);
                let hosts_w = (total_w - log_w - 16.0).max(260.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(log_w, 320.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(log_w);
                            Frame::none()
                                .fill(CARD)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(11.0)
                                .inner_margin(Margin::same(14.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Event log")
                                                .size(12.5)
                                                .strong()
                                                .color(TEXT),
                                        );
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                for (level, label) in [
                                                    (None, "All"),
                                                    (Some(Level::Info), "Info"),
                                                    (Some(Level::Warn), "Warn"),
                                                    (Some(Level::Error), "Error"),
                                                ] {
                                                    if chip(
                                                        ui,
                                                        label,
                                                        self.log_level_filter == level,
                                                    )
                                                    .clicked()
                                                    {
                                                        self.log_level_filter = level;
                                                    }
                                                }
                                            },
                                        );
                                    });
                                    ui.add_space(7.0);
                                    hsep(ui);
                                    ui.add_space(4.0);
                                    let logs = self.logs.clone();
                                    ScrollArea::vertical()
                                        .max_height(245.0)
                                        .auto_shrink([false, false])
                                        .show(ui, |ui| {
                                            for log in &logs {
                                                if let Some(f) = self.log_level_filter {
                                                    if log.level != f {
                                                        continue;
                                                    }
                                                }
                                                ui.horizontal(|ui| {
                                                    ui.add_sized(
                                                        [58.0, 16.0],
                                                        egui::Label::new(
                                                            RichText::new(&log.time)
                                                                .font(mono(9.5))
                                                                .color(FAINT),
                                                        ),
                                                    );
                                                    ui.add_sized(
                                                        [52.0, 16.0],
                                                        egui::Label::new(
                                                            RichText::new(log.level.label())
                                                                .font(mono(8.5))
                                                                .color(log.level.color()),
                                                        ),
                                                    );
                                                    ui.add_sized(
                                                        [76.0, 16.0],
                                                        egui::Label::new(
                                                            RichText::new(&log.source)
                                                                .size(9.5)
                                                                .color(MUTED),
                                                        ),
                                                    );
                                                    ui.add(
                                                        egui::Label::new(
                                                            RichText::new(&log.message)
                                                                .size(10.0)
                                                                .color(TEXT),
                                                        )
                                                        .truncate(),
                                                    );
                                                });
                                                ui.add_space(3.0);
                                            }
                                        });
                                });
                        },
                    );
                    ui.add_space(16.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(hosts_w, 320.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(hosts_w);
                            Frame::none()
                                .fill(CARD)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(11.0)
                                .inner_margin(Margin::same(14.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new("Top hosts")
                                            .size(12.5)
                                            .strong()
                                            .color(TEXT),
                                    );
                                    ui.add_space(8.0);
                                    top_hosts(ui, &self.captures);
                                });
                        },
                    );
                });
            });
    }

    fn guide_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(
                    RichText::new("Guide & Stack")
                        .size(24.0)
                        .strong()
                        .color(TEXT),
                );
                ui.label(
                    RichText::new("What every tool does, how to use it, and what this app is built from.")
                        .size(11.5)
                        .color(MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                pill(
                    ui,
                    "FULL ACCESS",
                    GREEN,
                    Color32::from_rgba_unmultiplied(61, 214, 149, 18),
                );
                ui.label(
                    RichText::new("no account · no trial · nothing locked")
                        .size(9.5)
                        .color(FAINT),
                );
            });
        });
        ui.add_space(12.0);

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(88, 140, 255, 12))
                    .stroke(Stroke::new(
                        1.0,
                        Color32::from_rgba_unmultiplied(88, 140, 255, 70),
                    ))
                    .rounding(11.0)
                    .inner_margin(Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 4.0, ACCENT);
                            ui.label(
                                RichText::new("Start here")
                                    .size(12.5)
                                    .strong()
                                    .color(TEXT),
                            );
                            ui.label(
                                RichText::new("A hands-on POC of a network debugging tool. All tools below are included.")
                                    .size(10.5)
                                    .color(MUTED),
                            );
                        });
                    });
                ui.add_space(12.0);

                section_label(ui, "Connect real traffic from a simulator or emulator");
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Proxzy lists the devices it can see before it changes anything. Live Capture shows every booted simulator and running emulator under Detected devices; press Connect… and confirm the action you want.")
                        .size(10.5)
                        .color(MUTED),
                );
                ui.add_space(9.0);
                let device_gap = 14.0;
                let device_col = ((ui.available_width() - device_gap) / 2.0).max(320.0);
                let port = self.proxy_port;
                let ca_pem = self
                    .ca_info
                    .as_ref()
                    .map(|c| c.pem_path.display().to_string())
                    .unwrap_or_else(|| {
                        "~/Library/Application Support/Proxzy/proxzy-ca.pem".into()
                    });
                let ca_cer = self
                    .ca_info
                    .as_ref()
                    .map(|c| c.cer_path.display().to_string())
                    .unwrap_or_else(|| {
                        "~/Library/Application Support/Proxzy/proxzy-ca.cer".into()
                    });
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(device_col, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(device_col);
                            Frame::none()
                                .fill(CARD)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(10.0)
                                .inner_margin(Margin::same(13.0))
                                .show(ui, |ui| {
                                    ui.set_width((device_col - 26.0).max(280.0));
                                    ui.horizontal(|ui| {
                                        section_label(ui, "iOS Simulator");
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                pill(
                                                    ui,
                                                    "SHARES THE MAC",
                                                    CYAN,
                                                    Color32::from_rgba_unmultiplied(
                                                        88, 200, 235, 20,
                                                    ),
                                                );
                                            },
                                        );
                                    });
                                    ui.add_space(7.0);
                                    step_row(ui, 1, "Boot the simulator and confirm it is READY", "Open it from Xcode or Simulator. The Detected devices card refreshes every few seconds.");
                                    step_row(ui, 2, "Press Connect… and choose Connect and trust", "Nothing is installed until you confirm. Certificate only skips the proxy and restart.");
                                    step_row(ui, 3, "Proxzy trusts the CA in that simulator", "It uses the UDID you clicked, points the Mac web proxy at 127.0.0.1, then restarts the simulator so the trust loads.");
                                    step_row(ui, 4, "Run the app and watch All Traffic", "The simulator shares the Mac network, so its HTTP and HTTPS calls arrive in the list.");
                                    ui.add_space(2.0);
                                    command_box(
                                        ui,
                                        "Manual equivalent",
                                        &format!(
                                            "xcrun simctl keychain <UDID> add-root-cert \"{}\"\nnetworksetup -setwebproxy Wi-Fi 127.0.0.1 {}\nnetworksetup -setsecurewebproxy Wi-Fi 127.0.0.1 {}",
                                            ca_pem, port, port
                                        ),
                                        ctx,
                                        self,
                                    );
                                });
                        },
                    );
                    ui.add_space(device_gap);
                    ui.allocate_ui_with_layout(
                        Vec2::new(device_col, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(device_col);
                            Frame::none()
                                .fill(CARD)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(10.0)
                                .inner_margin(Margin::same(13.0))
                                .show(ui, |ui| {
                                    ui.set_width((device_col - 26.0).max(280.0));
                                    ui.horizontal(|ui| {
                                        section_label(ui, "Android emulator or phone");
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                pill(
                                                    ui,
                                                    "ADB",
                                                    GREEN,
                                                    Color32::from_rgba_unmultiplied(
                                                        61, 214, 149, 20,
                                                    ),
                                                );
                                            },
                                        );
                                    });
                                    ui.add_space(7.0);
                                    step_row(ui, 1, "Start the AVD and wait for READY", "Android Studio › Device Manager. adb must be on PATH; the card explains it when adb is missing.");
                                    step_row(ui, 2, "Press Connect… and choose Connect and trust", "The action applies to the serial you clicked, not every attached device.");
                                    step_row(ui, 3, "Set the proxy and copy the certificate", "An emulator reaches this Mac at 10.0.2.2. A physical phone uses the Mac LAN address. The CA lands in /sdcard/Download/proxzy-ca.cer.");
                                    step_row(ui, 4, "Trust the CA on the device", "Settings › Security › Encryption & credentials › Install a certificate › CA certificate. Then reopen the app you are testing.");
                                    ui.add_space(2.0);
                                    command_box(
                                        ui,
                                        "Manual equivalent",
                                        &format!(
                                            "adb -s emulator-5554 shell settings put global http_proxy 10.0.2.2:{}\nadb -s emulator-5554 push \"{}\" /sdcard/Download/proxzy-ca.cer",
                                            port, ca_cer
                                        ),
                                        ctx,
                                        self,
                                    );
                                });
                        },
                    );
                });
                ui.add_space(8.0);
                Frame::none()
                    .fill(INPUT)
                    .stroke(Stroke::new(1.0, BORDER))
                    .rounding(10.0)
                    .inner_margin(Margin::same(13.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            dot(ui, AMBER, 6.0);
                            ui.label(
                                RichText::new("Troubleshooting")
                                    .size(10.5)
                                    .strong()
                                    .color(TEXT),
                            );
                        });
                        ui.add_space(4.0);
                        for line in [
                            "Nothing in the list: open Simulator from Xcode or start an AVD, and make sure adb is on PATH. Press Scan again.",
                            "HTTPS shows only CONNECT tunnels: the certificate is not trusted yet. Reinstall it and restart the simulator or the app.",
                            "Physical Android phone: join the same Wi-Fi, use the Mac LAN address shown in the card, and allow incoming connections in the macOS firewall.",
                            "To undo: choose Certificate only next time, or clear the Android proxy and the Mac web proxy from the buttons under the device card.",
                        ] {
                            ui.horizontal_top(|ui| {
                                dot(ui, FAINT, 5.0);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(line).size(9.5).color(MUTED),
                                    )
                                    .wrap(),
                                );
                            });
                            ui.add_space(3.0);
                        }
                    });
                ui.add_space(14.0);

                section_label(ui, "The one-minute tour");
                ui.add_space(6.0);
                let total = ui.available_width();
                let w = ((total - 52.0) / 5.0).max(150.0);
                ui.horizontal_top(|ui| {
                    for (n, title, body) in [
                        (
                            "1",
                            "Pick a request",
                            "Open All Traffic, then click any row. The right side shows its URL, status, headers and body.",
                        ),
                        (
                            "2",
                            "Copy or share",
                            "Use Copy URL, Copy cURL or Share. The cURL tab gives a ready command for another machine.",
                        ),
                        (
                            "3",
                            "Repeat it",
                            "Press Repeat to replay the selected request into a fresh capture.",
                        ),
                        (
                            "4",
                            "Mock a reply",
                            "Turn on a Map Local rule and a matching send returns your local test data instead of the server.",
                        ),
                        (
                            "5",
                            "Pause & edit",
                            "Arm a Breakpoint, pick a request and Pause selected. Edit headers or body, then Continue or Abort.",
                        ),
                    ] {
                        ui.allocate_ui_with_layout(
                            Vec2::new(w, 148.0),
                            Layout::top_down(Align::Min),
                            |ui| {
                                ui.set_width(w);
                                tour_card(ui, w, n, title, body);
                            },
                        );
                    }
                });
                ui.add_space(14.0);

                section_label(ui, "Every feature and how to use it");
                ui.add_space(6.0);
                let gap = 14.0;
                let col = ((ui.available_width() - gap) / 2.0).max(320.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(col, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(col);
                            feature_card(
                                ui,
                                "Traffic list",
                                "Every captured call with method, status, host, type and timing.",
                                "Use the sidebar filters (GraphQL, REST, Errors, Mocked) or the search box at the top.",
                                ACCENT,
                            );
                            feature_card(
                                ui,
                                "Request inspector",
                                "Overview, Request, Response, Headers and a cURL tab.",
                                "Click a row to pin it: new live calls keep arriving in the list but no longer replace the call you are reading, and Follow live resumes. Response bodies switch between JSON (default), Raw, XML and HTML; JSON is indented and colour-coded.",
                                CYAN,
                            );
                            feature_card(
                                ui,
                                "Copy & share",
                                "Copy the URL, a full cURL command, or a share link and Markdown.",
                                "Use the four buttons under the URL, or the cURL tab for the complete command.",
                                GREEN,
                            );
                            feature_card(
                                ui,
                                "Repeat",
                                "Replay a captured request without rebuilding it by hand.",
                                "Select the request and press Repeat. If a Map Local rule matches, the replay uses the fixture.",
                                AMBER,
                            );
                        },
                    );
                    ui.add_space(gap);
                    ui.allocate_ui_with_layout(
                        Vec2::new(col, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(col);
                            feature_card(
                                ui,
                                "Composer",
                                "Write a GraphQL or REST request and send it for real.",
                                "Choose a preset or type a method, URL, headers and body. Send uses the system curl, so real network answers appear (offline stays offline).",
                                ACCENT,
                            );
                            feature_card(
                                ui,
                                "Map Local",
                                "Answer a matching request with local JSON, a chosen status and delay.",
                                "Create a rule with a URL pattern, method, status and body, switch it on, then Send test or use the Composer.",
                                AMBER,
                            );
                            feature_card(
                                ui,
                                "Breakpoints",
                                "Pause before a request leaves or before a response returns, then edit it.",
                                "Arm a rule, select a request and press Pause selected. Continue records the edited call; Abort records a red cancelled call.",
                                PURPLE,
                            );
                            feature_card(
                                ui,
                                "Log dashboard",
                                "One timeline for captures, mock hits, breakpoint edits and system events.",
                                "Open Log Dashboard; filter by Info, Warn or Error, and copy the session summary.",
                                CYAN,
                            );
                        },
                    );
                });
                ui.add_space(14.0);

                section_label(ui, "What we build from (the stack)");
                ui.add_space(6.0);
                Frame::none()
                    .fill(CARD)
                    .stroke(Stroke::new(1.0, BORDER))
                    .rounding(11.0)
                    .inner_margin(Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        stack_row(ui, "Language & UI", "Rust + egui / eframe", "A native desktop window drawn every frame with an immediate-mode UI.");
                        stack_row(ui, "Capture feed", "Real local proxy", "HTTP and HTTPS requests from a simulator, emulator, curl or a Mac app arrive through the proxy on 127.0.0.1.");
                        stack_row(ui, "Live sending", "System curl", "The Composer runs curl in a background worker, so requests and responses are real when the network is available.");
                        stack_row(ui, "Map Local", "In-app rule engine", "First matching method + URL substring wins, then the local status, delay and JSON body are returned.");
                        stack_row(ui, "Breakpoints", "Modal editor", "The pause screen edits method, URL, headers and body, then writes the result into the traffic list.");
                        stack_row(ui, "Logs & stats", "In-memory session", "Events, status families, top hosts and latency are computed live from the current captures.");
                        stack_row(ui, "Clipboard & picture", "OS clipboard + PNG", "Copy buttons use the system clipboard; --screenshot saves the window for automatic checking.");
                    });
                ui.add_space(12.0);

                section_label(ui, "Real vs simulated in this POC");
                ui.add_space(6.0);
                Frame::none()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, BORDER))
                    .rounding(11.0)
                    .inner_margin(Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        truth_row(ui, "Real", "The local HTTP/HTTPS proxy, certificate generation, live captures, Composer sends, clipboard, cURL export, Map Local answers, breakpoint pauses and log updates.", GREEN);
                        truth_row(ui, "Still sample", "The opening rows are sample calls so the screen is never empty. Clear them from Live Capture to see only traffic you generated.", AMBER);
                        truth_row(ui, "Limit", "HTTPS decryption needs the local certificate trusted on the device. The proxy handles HTTP/1.1 only; it does not install a system-wide VPN profile.", ACCENT);
                    });
                ui.add_space(12.0);

                section_label(ui, "Good next experiments");
                ui.add_space(6.0);
                Frame::none()
                    .fill(INPUT)
                    .stroke(Stroke::new(1.0, BORDER))
                    .rounding(10.0)
                    .inner_margin(Margin::same(13.0))
                    .show(ui, |ui| {
                        for line in [
                            "Create a Map Local rule for the GraphQL endpoint and compare the mocked capture with a live send.",
                            "Pause a request, change one JSON field, Continue, then open Request to see the edit.",
                            "Send a REST request to a public test API in Composer and inspect the real headers.",
                            "Filter the traffic list to Errors, then follow the timeline in the Log Dashboard.",
                        ] {
                            ui.horizontal(|ui| {
                                dot(ui, FAINT, 5.0);
                                ui.label(RichText::new(line).size(10.5).color(MUTED));
                            });
                            ui.add_space(3.0);
                        }
                    });
                ui.add_space(16.0);
            });
    }

    fn summary_text(&self) -> String {
        let total = self.captures.len();
        let errors = self.captures.iter().filter(|c| c.is_error()).count();
        let mocked = self.captures.iter().filter(|c| c.mock.is_some()).count();
        format!(
            "Proxzy POC session summary\nrequests: {}\nerrors: {}\nmocked: {}\nrules: {} enabled\nbreakpoints: {} armed",
            total,
            errors,
            mocked,
            self.map_rules.iter().filter(|r| r.enabled).count(),
            self.breakpoints.iter().filter(|b| b.enabled).count()
        )
    }

    fn modals(&mut self, ctx: &egui::Context) {
        self.rule_modals(ctx);
        self.device_prompt_modal(ctx);
        if self.breakpoint_session.is_some() {
            let mut outcome: Option<u8> = None;
            egui::Area::new(Id::new("bp_dim"))
                .order(egui::Order::Middle)
                .fixed_pos(egui::Pos2::ZERO)
                .show(ctx, |ui| {
                    let rect = ctx.screen_rect();
                    ui.painter().rect_filled(
                        rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(4, 6, 10, 205),
                    );
                    ui.allocate_rect(rect, Sense::click());
                });

            let size = Vec2::new(720.0, 560.0);
            let rect = Rect::from_center_size(ctx.screen_rect().center(), size);
            egui::Area::new(Id::new("bp_modal"))
                .order(egui::Order::Foreground)
                .fixed_pos(rect.min)
                .show(ctx, |ui| {
                    Frame::none()
                        .fill(PANEL)
                        .stroke(Stroke::new(1.0, Color32::from_rgb(58, 70, 90)))
                        .rounding(14.0)
                        .inner_margin(Margin::same(20.0))
                        .show(ui, |ui| {
                            ui.set_width(680.0);
                            ui.set_height(520.0);
                            if let Some(s) = &mut self.breakpoint_session {
                                ui.horizontal(|ui| {
                                    let (r, _) =
                                        ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                                    ui.painter().circle_filled(r.center(), 4.5, AMBER);
                                    ui.label(
                                        RichText::new("Breakpoint paused")
                                            .size(18.0)
                                            .strong()
                                            .color(TEXT),
                                    );
                                    pill(
                                        ui,
                                        s.stage.label(),
                                        if s.stage == BpStage::Request {
                                            AMBER
                                        } else {
                                            PURPLE
                                        },
                                        Color32::from_rgba_unmultiplied(255, 181, 84, 18),
                                    );
                                    ui.with_layout(
                                        Layout::right_to_left(Align::Center),
                                        |ui| {
                                            ui.label(
                                                RichText::new("ESC close")
                                                    .size(9.0)
                                                    .color(FAINT),
                                            );
                                        },
                                    );
                                });
                                ui.add_space(3.0);
                                ui.label(
                                    RichText::new(format!("Rule: {}", s.rule_name))
                                        .size(10.5)
                                        .color(MUTED),
                                );
                                ui.add_space(12.0);
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(100.0);
                                        form_label(ui, "Method");
                                        input_frame().show(ui, |ui| {
                                            ui.set_width(100.0);
                                            ui.add(
                                                egui::TextEdit::singleline(&mut s.method)
                                                    .frame(false)
                                                    .desired_width(96.0)
                                                    .font(TextStyle::Monospace)
                                                    .text_color(TEXT),
                                            );
                                        });
                                    });
                                    ui.vertical(|ui| {
                                        ui.set_width(560.0);
                                        form_label(ui, "URL");
                                        input_frame().show(ui, |ui| {
                                            ui.set_width(560.0);
                                            ui.add(
                                                egui::TextEdit::singleline(&mut s.url)
                                                    .frame(false)
                                                    .desired_width(556.0)
                                                    .font(TextStyle::Monospace)
                                                    .text_color(TEXT),
                                            );
                                        });
                                    });
                                });
                                ui.add_space(9.0);
                                form_label(ui, "Headers");
                                input_frame().show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.add(
                                        egui::TextEdit::multiline(&mut s.headers)
                                            .frame(false)
                                            .desired_rows(3)
                                            .desired_width(f32::INFINITY)
                                            .font(TextStyle::Monospace)
                                            .text_color(TEXT),
                                    );
                                });
                                ui.add_space(9.0);
                                form_label(
                                    ui,
                                    if s.stage == BpStage::Request {
                                        "Request body"
                                    } else {
                                        "Response body"
                                    },
                                );
                                input_frame().show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.add(
                                        egui::TextEdit::multiline(&mut s.body)
                                            .frame(false)
                                            .desired_rows(7)
                                            .desired_width(f32::INFINITY)
                                            .font(TextStyle::Monospace)
                                            .text_color(TEXT),
                                    );
                                });
                                ui.add_space(10.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Changes are applied only inside this POC session.")
                                            .size(9.5)
                                            .color(FAINT),
                                    );
                                    ui.with_layout(
                                        Layout::right_to_left(Align::Center),
                                        |ui| {
                                            if primary_button(ui, "Continue").clicked() {
                                                outcome = Some(1);
                                            }
                                            ui.add_space(4.0);
                                            if secondary_button(ui, "Abort request").clicked() {
                                                outcome = Some(2);
                                            }
                                        },
                                    );
                                });
                            }
                        });
                });

            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                outcome = Some(0);
            }
            match outcome {
                Some(1) => self.continue_breakpoint(ctx),
                Some(2) => self.abort_breakpoint(ctx),
                Some(0) => {
                    if self
                        .breakpoint_session
                        .as_ref()
                        .map(|s| s.live_bp_id.is_some())
                        .unwrap_or(false)
                    {
                        self.abort_breakpoint(ctx);
                    } else {
                        self.breakpoint_session = None;
                    }
                }
                _ => {}
            }
        }

        if self.share_open {
            let c = self.captures.get(self.selected).cloned();
            let mut close = false;
            let mut copy_link = false;
            let mut copy_md = false;
            let mut copy_curl = false;
            egui::Area::new(Id::new("share_dim"))
                .order(egui::Order::Middle)
                .fixed_pos(egui::Pos2::ZERO)
                .show(ctx, |ui| {
                    let rect = ctx.screen_rect();
                    ui.painter().rect_filled(
                        rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(4, 6, 10, 190),
                    );
                    ui.allocate_rect(rect, Sense::click());
                });
            let size = Vec2::new(560.0, 330.0);
            let rect = Rect::from_center_size(ctx.screen_rect().center(), size);
            egui::Area::new(Id::new("share_modal"))
                .order(egui::Order::Foreground)
                .fixed_pos(rect.min)
                .show(ctx, |ui| {
                    Frame::none()
                        .fill(PANEL)
                        .stroke(Stroke::new(1.0, Color32::from_rgb(58, 70, 90)))
                        .rounding(14.0)
                        .inner_margin(Margin::same(20.0))
                        .show(ui, |ui| {
                            ui.set_width(520.0);
                            ui.set_height(290.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Share request")
                                        .size(18.0)
                                        .strong()
                                        .color(TEXT),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if icon_button(ui, "x", "Close").clicked() {
                                        close = true;
                                    }
                                });
                            });
                            ui.add_space(4.0);
                            if let Some(c) = &c {
                                ui.horizontal(|ui| {
                                    method_badge(ui, &c.method);
                                    ui.label(
                                        RichText::new(c.host_path())
                                            .font(mono(11.0))
                                            .color(TEXT),
                                    );
                                });
                                ui.add_space(12.0);
                                form_label(ui, "Share link");
                                let mut link = c.share_link();
                                input_frame().show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.add(
                                        egui::TextEdit::singleline(&mut link)
                                            .frame(false)
                                            .desired_width(f32::INFINITY)
                                            .font(TextStyle::Monospace)
                                            .text_color(Color32::from_rgb(154, 183, 235)),
                                    );
                                });
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("The link is a local placeholder in this POC; no data leaves the app.")
                                        .size(9.5)
                                        .color(FAINT),
                                );
                                ui.add_space(14.0);
                                ui.horizontal(|ui| {
                                    if primary_button(ui, "Copy link").clicked() {
                                        copy_link = true;
                                    }
                                    if secondary_button(ui, "Copy cURL").clicked() {
                                        copy_curl = true;
                                    }
                                    if secondary_button(ui, "Copy as Markdown").clicked() {
                                        copy_md = true;
                                    }
                                    ui.with_layout(
                                        Layout::right_to_left(Align::Center),
                                        |ui| {
                                            if secondary_button(ui, "Close").clicked() {
                                                close = true;
                                            }
                                        },
                                    );
                                });
                            }
                        });
                });
            if close {
                self.share_open = false;
            }
            if (copy_link || copy_md || copy_curl) && c.is_some() {
                let c = c.unwrap();
                if copy_link {
                    self.copy(ctx, c.share_link(), "Share link copied");
                } else if copy_curl {
                    self.copy(ctx, c.curl(), "cURL command copied");
                } else {
                    let md = format!("`{}` [{}]({})", c.method, c.url, c.share_link());
                    self.copy(ctx, md, "Markdown share copied");
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.share_open = false;
            }
        }
    }

    fn live_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut do_start = false;
        let mut do_stop = false;
        let mut do_test = false;
        let mut do_sim = false;
        let mut do_setup_sim = false;
        let mut do_reboot = false;
        let mut do_mac_proxy = false;
        let mut do_mac_clear = false;
        let mut do_android = false;
        let mut do_android_proxy = false;
        let mut do_android_clear = false;
        let mut do_folder = false;
        let mut do_cert = false;
        let mut go_traffic = false;

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(RichText::new("Live Capture").size(24.0).strong().color(TEXT));
                ui.label(
                    RichText::new("Point a simulator, emulator or app at this Mac and watch the calls arrive.")
                        .size(11.5)
                        .color(MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.proxy.is_some() {
                    pill(ui, "PROXY ON", GREEN, Color32::from_rgba_unmultiplied(61, 214, 149, 18));
                } else {
                    pill(ui, "PROXY OFF", AMBER, Color32::from_rgba_unmultiplied(255, 181, 84, 18));
                }
                ui.label(
                    RichText::new("HTTP + HTTPS")
                        .size(10.0)
                        .color(FAINT),
                );
            });
        });
        ui.add_space(12.0);

        // Status banner -----------------------------------------------------
        Frame::none()
            .fill(CARD)
            .stroke(Stroke::new(
                1.0,
                if self.proxy.is_some() {
                    Color32::from_rgba_unmultiplied(61, 214, 149, 90)
                } else {
                    BORDER
                },
            ))
            .rounding(12.0)
            .inner_margin(Margin::same(14.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    dot(
                        ui,
                        if self.proxy.is_some() { GREEN } else { FAINT },
                        8.0,
                    );
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.label(
                            RichText::new(if self.proxy.is_some() {
                                "Proxy is listening"
                            } else {
                                "Proxy is stopped"
                            })
                            .size(13.0)
                            .strong()
                            .color(TEXT),
                        );
                        ui.label(
                            RichText::new(if let Some(err) = &self.proxy_error {
                                err.clone()
                            } else {
                                format!(
                                    "HTTP and HTTPS on 127.0.0.1:{} · {} requests in this session",
                                    self.proxy_port,
                                    self.captures.len()
                                )
                            })
                            .size(10.5)
                            .color(if self.proxy_error.is_some() { AMBER } else { MUTED }),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self.proxy.is_some() {
                            if secondary_button(ui, "Stop").clicked() {
                                do_stop = true;
                            }
                            if primary_button(ui, "Send test request").clicked() {
                                do_test = true;
                            }
                        } else if primary_button(ui, "Start proxy").clicked() {
                            do_start = true;
                        }
                        ui.add_space(4.0);
                        if secondary_button(ui, "View traffic").clicked() {
                            go_traffic = true;
                        }
                        if secondary_button(ui, "Clear samples").clicked() {
                            self.clear_sample_captures(ctx);
                        }
                        ui.add_space(4.0);
                        if chip(ui, if self.recording { "Recording" } else { "Paused" }, self.recording)
                            .clicked()
                        {
                            self.recording = !self.recording;
                            self.log(
                                if self.recording { Level::Info } else { Level::Warn },
                                "Proxy",
                                if self.recording {
                                    "Live capture resumed"
                                } else {
                                    "Live capture paused"
                                },
                            );
                        }
                    });
                });
            });
        ui.add_space(12.0);

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let total = ui.available_width();
                let left = (total * 0.61).clamp(520.0, 820.0).min((total - 340.0).max(420.0));
                let right = (total - left - 14.0).max(300.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(left, ui.available_height()),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(left);
                            self.live_setup_card(
                                ui,
                                ctx,
                                &mut do_sim,
                                &mut do_setup_sim,
                                &mut do_reboot,
                                &mut do_mac_proxy,
                                &mut do_mac_clear,
                                &mut do_android,
                                &mut do_android_proxy,
                                &mut do_android_clear,
                            );
                        },
                    );
                    ui.add_space(14.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(right, ui.available_height()),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(right);
                            self.live_cert_card(ui, ctx, &mut do_sim, &mut do_android, &mut do_folder, &mut do_cert);
                            ui.add_space(14.0);
                            self.live_ssl_allowlist_card(ui, ctx);
                        },
                    );
                });
            });

        if do_start {
            self.start_proxy(ctx);
        }
        if do_stop {
            self.stop_proxy(ctx);
        }
        if do_test {
            self.test_proxy(ctx);
        }
        if do_setup_sim {
            self.setup_ios_simulator(ctx);
        } else if do_sim {
            self.install_ca_ios_sim(ctx);
        }
        if do_reboot {
            self.restart_ios_simulator(ctx);
        }
        if do_mac_proxy {
            self.set_mac_proxy(ctx, false);
        }
        if do_mac_clear {
            self.set_mac_proxy(ctx, true);
        }
        if do_android {
            self.push_ca_android(ctx);
        }
        if do_android_proxy {
            self.set_android_proxy(ctx, false);
        }
        if do_android_clear {
            self.set_android_proxy(ctx, true);
        }
        if do_folder {
            self.open_ca_folder(ctx);
        }
        if do_cert {
            match crate::proxy::ensure_ca() {
                Ok(ca) => {
                    self.ca_info = Some(ca);
                    self.log(Level::Info, "Certificate", "Local CA is ready");
                    self.toast_now(ctx, "Local certificate is ready");
                }
                Err(e) => {
                    self.log(Level::Error, "Certificate", e);
                    self.toast_now(ctx, "Could not create the certificate");
                }
            }
        }
        if go_traffic {
            self.view = View::Traffic;
            self.filter = TrafficFilter::All;
        }
        if self.mac_proxy_confirm_open {
            self.mac_proxy_confirm_modal(ctx, &mut do_mac_proxy);
        }
    }

    fn live_setup_card(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        do_sim: &mut bool,
        do_setup_sim: &mut bool,
        do_reboot: &mut bool,
        _do_mac_proxy: &mut bool,
        do_mac_clear: &mut bool,
        do_android: &mut bool,
        do_android_proxy: &mut bool,
        do_android_clear: &mut bool,
    ) {
        let port = self.proxy_port;
        let scan = self.device_result().cloned();
        let scanning = self.device_scanning();
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                section_label(ui, "Connect a device");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("one proxy · every app").size(9.5).color(FAINT));
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                section_label(ui, "Detected devices");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "Scan again").clicked() {
                        self.request_device_scan();
                    }
                    ui.label(
                        RichText::new(if scanning {
                            "scanning…"
                        } else {
                            "auto-refresh 4s"
                        })
                        .size(8.5)
                        .color(FAINT),
                    );
                });
            });
            ui.add_space(5.0);
            let devices = scan.as_ref().map(|s| s.devices.as_slice()).unwrap_or(&[]);
            if devices.is_empty() {
                Frame::none()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, BORDER))
                    .rounding(8.0)
                    .inner_margin(Margin::symmetric(9.0, 7.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            dot(ui, AMBER, 6.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(if scanning {
                                        "Looking for a device…"
                                    } else {
                                        "No simulator or emulator detected"
                                    })
                                    .size(10.5)
                                    .strong()
                                    .color(TEXT),
                                );
                                ui.label(
                                    RichText::new("Open one in Xcode or Android Studio; this list refreshes automatically.")
                                        .size(9.0)
                                        .color(FAINT),
                                );
                            });
                        });
                    });
            } else {
                ScrollArea::vertical()
                    .id_source("detected-devices")
                    .max_height(168.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for device in devices {
                            Frame::none()
                                .fill(PANEL)
                                .stroke(Stroke::new(1.0, BORDER))
                                .rounding(8.0)
                                .inner_margin(Margin::symmetric(9.0, 6.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        dot(ui, if device.ready { GREEN } else { AMBER }, 6.0);
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(&device.name)
                                                    .size(10.5)
                                                    .strong()
                                                    .color(TEXT),
                                            );
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} · {}",
                                                    device.kind.label(),
                                                    device.detail
                                                ))
                                                .size(8.5)
                                                .color(FAINT),
                                            );
                                        });
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                let (text, ok) = device.state_pill();
                                                pill(
                                                    ui,
                                                    text,
                                                    if ok { GREEN } else { AMBER },
                                                    if ok {
                                                        Color32::from_rgba_unmultiplied(
                                                            61, 214, 149, 18,
                                                        )
                                                    } else {
                                                        Color32::from_rgba_unmultiplied(
                                                            255, 181, 84, 18,
                                                        )
                                                    },
                                                );
                                                if device.ready
                                                    && secondary_button(ui, "Connect…").clicked()
                                                {
                                                    self.device_prompt = Some(DevicePrompt {
                                                        device: device.clone(),
                                                        mode: ConnectMode::Full,
                                                    });
                                                }
                                            },
                                        );
                                    });
                                });
                        }
                    });
            }
            if let Some(scan) = &scan {
                for note in scan.notes.iter().take(2) {
                    ui.label(RichText::new(note).size(8.5).color(FAINT));
                }
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                for (i, label) in [
                    (0usize, "iOS Simulator"),
                    (1, "Android"),
                    (2, "Real device"),
                    (3, "This Mac"),
                ] {
                    if chip(ui, label, self.live_tab == i).clicked() {
                        self.live_tab = i;
                    }
                }
            });
            ui.add_space(10.0);
            match self.live_tab {
                0 => {
                    let ios_ready = self.ios_device_ready();
                    step_row(ui, 1, "Start the proxy", "It is already listening on 127.0.0.1; keep it running.");
                    if ios_ready {
                        step_row(ui, 2, "Trust the certificate in the booted simulator", "A booted simulator was detected above. The button runs xcrun simctl for it.");
                    } else {
                        step_row(ui, 2, "Open a booted simulator first", "No iOS Simulator is detected yet. Open one in Xcode; the list above refreshes automatically.");
                    }
                    step_row(ui, 3, "Turn on the Mac's web proxy", "The simulator shares the Mac's network. Set Wi-Fi HTTP and HTTPS proxy to 127.0.0.1 with the port above.");
                    step_row(ui, 4, "Run the app in the simulator", "Requests appear here and in All Traffic. HTTPS is decrypted only after the certificate is trusted.");
                    ui.add_space(4.0);
                    let cmd = format!(
                        "xcrun simctl keychain booted add-root-cert \"{}\"\nnetworksetup -setwebproxy Wi-Fi 127.0.0.1 {}\nnetworksetup -setsecurewebproxy Wi-Fi 127.0.0.1 {}",
                        self.ca_info
                            .as_ref()
                            .map(|c| c.pem_path.display().to_string())
                            .unwrap_or_else(|| "~/Library/Application Support/Proxzy/proxzy-ca.pem".into()),
                        port,
                        port
                    );
                    command_box(ui, "Commands", &cmd, ctx, self);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(ios_ready, |ui| {
                            if primary_button(ui, "Set up simulator").clicked() {
                                *do_setup_sim = true;
                            }
                            if secondary_button(ui, "Install certificate only").clicked() {
                                *do_sim = true;
                            }
                        });
                    });
                    ui.horizontal(|ui| {
                        if secondary_button(ui, "Restart simulator").clicked() {
                            *do_reboot = true;
                        }
                        if secondary_button(ui, "Open Simulator").clicked() {
                            let _ = std::process::Command::new("open").arg("-a").arg("Simulator").spawn();
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Set up simulator installs the CA, points the Mac proxy at Proxzy and restarts the booted simulator so it loads the new trust. On a real iPhone, also switch on Proxzy Local CA in Settings › General › About › Certificate Trust Settings.")
                            .size(9.5)
                            .color(FAINT),
                    );
                }
                1 => {
                    let android_ready = self.android_device_ready();
                    step_row(ui, 1, "Start the proxy", "Keep the proxy running while the emulator is open.");
                    if android_ready {
                        step_row(ui, 2, "Point the emulator at the Mac", "An adb device was detected above. The Android emulator reaches this Mac at 10.0.2.2; the button sets the global HTTP proxy.");
                    } else {
                        step_row(ui, 2, "Start an Android emulator first", "No adb device is detected yet. Start one in Android Studio; the list above refreshes automatically.");
                    }
                    step_row(ui, 3, "Install the CA certificate", "Push the .cer file, then open Settings › Security › Encryption & credentials › Install a certificate › CA certificate.");
                    step_row(ui, 4, "Open the app", "Debug builds trust user certificates; some release builds need a network-security-config.");
                    ui.add_space(4.0);
                    let cmd = format!(
                        "adb shell settings put global http_proxy 10.0.2.2:{}\nadb push \"{}\" /sdcard/Download/proxzy-ca.cer\nadb shell settings put global http_proxy :0",
                        port,
                        self.ca_info
                            .as_ref()
                            .map(|c| c.cer_path.display().to_string())
                            .unwrap_or_else(|| "~/Library/Application Support/Proxzy/proxzy-ca.cer".into())
                    );
                    command_box(ui, "Commands", &cmd, ctx, self);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(android_ready, |ui| {
                            if primary_button(ui, "Set emulator proxy").clicked() {
                                *do_android_proxy = true;
                            }
                            if secondary_button(ui, "Push certificate").clicked() {
                                *do_android = true;
                            }
                        });
                        if secondary_button(ui, "Clear proxy").clicked() {
                            *do_android_clear = true;
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Android 7+ ignores user certificates for apps that opt out. Use a debug build or add the CA to the app's network security config.")
                            .size(9.5)
                            .color(FAINT),
                    );
                }
                2 => {
                    step_row(ui, 1, "Use the same Wi-Fi", "The phone and this Mac must be on the same network.");
                    step_row(ui, 2, "Set a manual proxy", "On the phone's Wi-Fi settings, set HTTP proxy to this Mac's IP address and the port above.");
                    step_row(ui, 3, "Install the certificate", "AirDrop or email the .pem file, then install it from Settings › General › VPN & Device Management.");
                    step_row(ui, 4, "Trust it", "Open Settings › General › About › Certificate Trust Settings and enable Proxzy Local CA.");
                    ui.add_space(4.0);
                    let cmd = format!(
                        "curl -x http://<this-mac-ip>:{} https://example.com/ -I\n# certificate: {}",
                        port,
                        self.ca_info
                            .as_ref()
                            .map(|c| c.pem_path.display().to_string())
                            .unwrap_or_else(|| "~/Library/Application Support/Proxzy/proxzy-ca.pem".into())
                    );
                    command_box(ui, "Commands", &cmd, ctx, self);
                }
                _ => {
                    step_row(ui, 1, "Trust the CA on this Mac", "Add the certificate to the login keychain and set it to Always Trust.");
                    step_row(ui, 2, "Set the macOS web proxy", "Use the commands below for the active network service, then open the app you want to inspect.");
                    step_row(ui, 3, "Or use curl", "Pass -x http://127.0.0.1:PORT for a single command without changing system settings.");
                    ui.add_space(4.0);
                    let cmd = format!(
                        "security add-trusted-cert -d -r trustRoot -k ~/Library/Keychains/login.keychain-db \"{}\"\nnetworksetup -setwebproxy Wi-Fi 127.0.0.1 {}\nnetworksetup -setsecurewebproxy Wi-Fi 127.0.0.1 {}\ncurl -x http://127.0.0.1:{} https://example.com/ -I",
                        self.ca_info
                            .as_ref()
                            .map(|c| c.pem_path.display().to_string())
                            .unwrap_or_else(|| "~/Library/Application Support/Proxzy/proxzy-ca.pem".into()),
                        port,
                        port,
                        port
                    );
                    command_box(ui, "Commands", &cmd, ctx, self);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if primary_button(ui, "Set Mac proxy").clicked() {
                            self.mac_proxy_confirm_open = true;
                        }
                        if secondary_button(ui, "Clear Mac proxy").clicked() {
                            *do_mac_clear = true;
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("This changes the web proxy for every enabled network service. Proxzy restores it automatically when you quit; Clear Mac proxy does it now.")
                            .size(9.5)
                            .color(FAINT),
                    );
                }
            }
        });
    }

    fn live_cert_card(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        do_sim: &mut bool,
        do_android: &mut bool,
        do_folder: &mut bool,
        do_cert: &mut bool,
    ) {
        let ca = self.ca_info.clone();
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                section_label(ui, "Local certificate");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ca.is_some() {
                        pill(ui, "CA READY", GREEN, Color32::from_rgba_unmultiplied(61, 214, 149, 18));
                    } else {
                        pill(ui, "NOT READY", AMBER, Color32::from_rgba_unmultiplied(255, 181, 84, 18));
                    }
                });
            });
            ui.add_space(8.0);
            if let Some(ca) = &ca {
                ui.label(
                    RichText::new("The proxy signs HTTPS with this local authority. Install it only on devices you own.")
                        .size(10.5)
                        .color(MUTED),
                );
                ui.add_space(8.0);
                form_label(ui, "SHA-256 fingerprint");
                Frame::none()
                    .fill(INPUT)
                    .stroke(Stroke::new(1.0, BORDER))
                    .rounding(7.0)
                    .inner_margin(Margin::symmetric(8.0, 7.0))
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&ca.fingerprint)
                                    .font(mono(8.8))
                                    .color(Color32::from_rgb(178, 204, 240)),
                            )
                            .wrap(),
                        );
                    });
                ui.add_space(8.0);
                form_label(ui, "Certificate file");
                ui.add(
                    egui::Label::new(
                        RichText::new(ca.pem_path.display().to_string())
                            .font(mono(9.0))
                            .color(FAINT),
                    )
                    .wrap(),
                );
                ui.add_space(10.0);
                if primary_button(ui, "Install in booted simulator").clicked() {
                    *do_sim = true;
                }
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    if secondary_button(ui, "Push to Android").clicked() {
                        *do_android = true;
                    }
                    if secondary_button(ui, "Open folder").clicked() {
                        *do_folder = true;
                    }
                });
                ui.add_space(5.0);
                if secondary_button(ui, "Copy certificate path").clicked() {
                    self.copy(ctx, ca.pem_path.display().to_string(), "Certificate path copied");
                }
                ui.add_space(9.0);
                hsep(ui);
                ui.add_space(7.0);
                ui.label(RichText::new("TRUST CHECK").size(8.5).color(FAINT).strong());
                ui.add_space(2.0);
                ui.label(
                    RichText::new("After installing, open a site or app through the proxy. If HTTPS rows appear in All Traffic, the trust worked.")
                        .size(9.5)
                        .color(MUTED),
                );
            } else {
                ui.label(
                    RichText::new("Start the proxy to create a local authority on this Mac.")
                        .size(10.5)
                        .color(MUTED),
                );
                ui.add_space(10.0);
                if primary_button(ui, "Create certificate").clicked() {
                    *do_cert = true;
                }
            }
            ui.add_space(10.0);
            hsep(ui);
            ui.add_space(7.0);
            ui.label(RichText::new("SAFE BY DEFAULT").size(8.5).color(FAINT).strong());
            ui.add_space(2.0);
            ui.label(
                RichText::new("The private key stays on this Mac. Traffic is captured locally and never uploaded. Turn the proxy off when you are done.")
                    .size(9.5)
                    .color(FAINT),
            );
            ui.add_space(8.0);
            ui.label(RichText::new("LIMITS").size(8.5).color(FAINT).strong());
            ui.add_space(2.0);
            ui.label(
                RichText::new("HTTPS is inspected as HTTP/1.1. Apps that insist on HTTP/2, HTTP/3 or gRPC may not connect through this POC.")
                    .size(9.5)
                    .color(FAINT),
            );
        });
    }

    fn live_ssl_allowlist_card(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                section_label(ui, "SSL Proxying & Safe Mode");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if self.ssl_allowlist.is_empty() {
                        pill(
                            ui,
                            "SAFE MODE: 100% PASSTHROUGH",
                            GREEN,
                            Color32::from_rgba_unmultiplied(61, 214, 149, 18),
                        );
                    } else {
                        pill(
                            ui,
                            &format!("SELECTIVE: {} ACTIVE", self.ssl_allowlist.len()),
                            CYAN,
                            Color32::from_rgba_unmultiplied(77, 208, 225, 18),
                        );
                    }
                });
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new("Only matching domains have TLS intercepted & decrypted. All unlisted domains pass through securely without TLS tampering, preventing browser security warnings.")
                    .size(9.5)
                    .color(MUTED),
            );
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.checkbox(&mut self.ssl_proxying_enabled, "Enable Selective SSL Interception").changed() {
                    self.sync_proxy_config();
                }
            });
            ui.add_space(8.0);

            // Add Domain Form
            ui.horizontal(|ui| {
                ui.label(RichText::new("Domain:").size(10.0).color(FAINT));
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.new_ssl_domain)
                        .hint_text("e.g. *.myapi.com, api.github.com")
                        .desired_width(180.0),
                );
                let can_add = !self.new_ssl_domain.trim().is_empty();
                if ui.add_enabled(can_add, egui::Button::new("Add Domain")).clicked()
                    || (can_add && resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    let domain = self.new_ssl_domain.trim().to_lowercase();
                    if !self.ssl_allowlist.contains(&domain) {
                        self.ssl_allowlist.push(domain);
                        self.sync_proxy_config();
                        self.toast_now(ctx, "Domain added to SSL allowlist");
                    }
                    self.new_ssl_domain.clear();
                }
                if secondary_button(ui, "Intercept All (*)")
                    .on_hover_text("Intercept and decrypt all application HTTPS (*), while safely preserving raw data on protected system services")
                    .clicked()
                {
                    self.enable_ssl_for_domain("*", ctx);
                }
                if secondary_button(ui, "Sample API").clicked() {
                    self.enable_ssl_for_domain("*.shopwave.dev", ctx);
                }
            });
            ui.add_space(8.0);

            // Safety policy notice
            Frame::none()
                .fill(Color32::from_rgba_unmultiplied(61, 214, 149, 10))
                .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(61, 214, 149, 45)))
                .rounding(7.0)
                .inner_margin(Margin::symmetric(10.0, 6.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        dot(ui, GREEN, 6.0);
                        ui.label(
                            RichText::new("🛡️ Safety Policy: Apple APNS, OS Updates & OCSP/CRLs always bind raw TCP data to prevent system certificate errors.")
                                .size(9.5)
                                .color(TEXT),
                        );
                    });
                });
            ui.add_space(8.0);

            // List of domains
            if self.ssl_allowlist.is_empty() {
                Frame::none()
                    .fill(INPUT)
                    .stroke(Stroke::new(1.0, BORDER))
                    .rounding(7.0)
                    .inner_margin(Margin::symmetric(10.0, 8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            dot(ui, GREEN, 6.0);
                            ui.label(
                                RichText::new("Empty allowlist = All HTTPS passes through as raw TCP tunnels in All Traffic. You can click 'Enable TLS Decryption' on any domain in 1 click.")
                                    .size(9.5)
                                    .color(MUTED),
                            );
                        });
                    });
            } else {
                let mut to_remove = None;
                for (idx, domain) in self.ssl_allowlist.iter().enumerate() {
                    ui.horizontal(|ui| {
                        dot(ui, CYAN, 6.0);
                        ui.label(RichText::new(domain).font(mono(10.0)).color(TEXT));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("✕").on_hover_text("Remove domain").clicked() {
                                to_remove = Some(idx);
                            }
                        });
                    });
                }
                if let Some(idx) = to_remove {
                    self.ssl_allowlist.remove(idx);
                    self.sync_proxy_config();
                    self.toast_now(ctx, "Domain removed");
                }
            }
        });
    }

    /// A quiet warning bar shown while a leftover dead web proxy is detected.
    /// It explains the browser outage the person is seeing and offers the
    /// one-click fix; dismissing keeps a slim reminder in the status bar.
    fn stale_proxy_banner(&mut self, ctx: &egui::Context) {
        let states = match &self.stale_proxy {
            Some(states) if !states.is_empty() => states.clone(),
            _ => return,
        };
        if self.mac_proxy_on {
            // This run set the proxy itself; the status bar already covers it.
            return;
        }
        let ports: Vec<String> = states
            .iter()
            .map(|s| format!("{}:{}", s.host, s.port))
            .collect();
        let services: Vec<String> = states.iter().map(|s| s.service.clone()).collect();

        if self.stale_proxy_dismissed {
            // Slim reminder only.
            egui::TopBottomPanel::bottom("stale_proxy_reminder")
                .frame(
                    Frame::none()
                        .fill(Color32::from_rgba_unmultiplied(255, 159, 67, 20))
                        .inner_margin(Margin::symmetric(14.0, 5.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        dot(ui, AMBER, 7.0);
                        ui.label(
                            RichText::new(format!(
                                "Leftover proxy {} still set — browsers cannot reach the internet",
                                ports.join(", ")
                            ))
                            .size(10.0)
                            .color(AMBER),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Clear it now").size(10.0).color(AMBER),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                self.clear_stale_proxy(ctx);
                            }
                        });
                    });
                });
            return;
        }

        // Full modal: this is the "no internet" the person is looking at.
        egui::TopBottomPanel::top("stale_proxy_banner")
            .frame(
                Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(255, 159, 67, 26))
                    .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 159, 67, 70)))
                    .inner_margin(Margin::symmetric(16.0, 10.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    dot(ui, AMBER, 9.0);
                    ui.label(
                        RichText::new("A leftover proxy is blocking the internet")
                            .size(13.0)
                            .strong()
                            .color(TEXT),
                    );
                });
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "A previous Proxzy run left the macOS web proxy pointing at {}, where nothing is listening. Browsers show ERR_PROXY_CONNECTION_FAILED until it is cleared (services: {}).",
                        ports.join(", "),
                        services.join(", ")
                    ))
                    .size(10.5)
                    .color(MUTED),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, "Clear leftover proxy").clicked() {
                        self.clear_stale_proxy(ctx);
                    }
                    if secondary_button(ui, "Not now").clicked() {
                        self.stale_proxy_dismissed = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new("Manual: networksetup -setwebproxystate \"Wi-Fi\" off")
                                .size(9.0)
                                .color(FAINT),
                        );
                    });
                });
            });
    }

    fn mac_proxy_confirm_modal(&mut self, ctx: &egui::Context, do_mac_proxy: &mut bool) {
        egui::Area::new(Id::new("mac_dim"))
            .order(egui::Order::Middle)
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let rect = ctx.screen_rect();
                ui.painter().rect_filled(
                    rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(4, 6, 10, 180),
                );
            });

        let size = Vec2::new(440.0, 220.0);
        let rect = Rect::from_center_size(ctx.screen_rect().center(), size);
        egui::Area::new(Id::new("mac_modal"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                Frame::none()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, AMBER))
                    .rounding(12.0)
                    .inner_margin(Margin::same(18.0))
                    .show(ui, |ui| {
                        ui.set_width(404.0);
                        ui.horizontal(|ui| {
                            dot(ui, AMBER, 8.0);
                            ui.label(RichText::new("Confirm System-wide Mac Proxy").size(14.0).strong().color(TEXT));
                        });
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Setting the Mac proxy changes networksetup on every active network interface. With Selective Passthrough, unlisted sites will not trigger certificate errors, but we still advise testing with curl -x or iOS Simulator for zero-risk isolation.")
                                .size(10.5)
                                .color(MUTED),
                        );
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            if primary_button(ui, "Proceed & Set Proxy").clicked() {
                                self.mac_proxy_confirm_open = false;
                                *do_mac_proxy = true;
                            }
                            if secondary_button(ui, "Cancel").clicked() {
                                self.mac_proxy_confirm_open = false;
                            }
                        });
                    });
            });
    }

    fn device_prompt_modal(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.device_prompt.clone() else {
            return;
        };
        let mut cancel = false;
        let mut confirm: Option<ConnectMode> = None;

        egui::Area::new(Id::new("device_dim"))
            .order(egui::Order::Middle)
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let rect = ctx.screen_rect();
                ui.painter().rect_filled(
                    rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(4, 6, 10, 205),
                );
                ui.allocate_rect(rect, Sense::click());
            });

        let size = Vec2::new(540.0, 390.0);
        let rect = Rect::from_center_size(ctx.screen_rect().center(), size);
        egui::Area::new(Id::new("device_modal"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                Frame::none()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, Color32::from_rgb(58, 70, 90)))
                    .rounding(14.0)
                    .inner_margin(Margin::same(20.0))
                    .show(ui, |ui| {
                        ui.set_width(500.0);
                        ui.horizontal(|ui| {
                            section_label(ui, "Connect device");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                pill(
                                    ui,
                                    prompt.device.kind.badge(),
                                    ACCENT,
                                    Color32::from_rgba_unmultiplied(88, 140, 255, 26),
                                );
                            });
                        });
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(&prompt.device.name)
                                .size(20.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{} · {}",
                                prompt.device.kind.label(),
                                prompt.device.detail
                            ))
                            .size(10.0)
                            .color(MUTED),
                        );
                        ui.add_space(12.0);
                        hsep(ui);
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("Proxzy found this device. It will install the local certificate and point the device at the proxy on this Mac.")
                                .size(11.0)
                                .color(MUTED),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(if prompt.device.kind == devices::DeviceKind::IosSimulator {
                                "The booted simulator restarts once so it loads the new trust."
                            } else {
                                "The certificate is copied to Download. Finish installing it in Settings › Security."
                            })
                            .size(10.0)
                            .color(FAINT),
                        );
                        ui.add_space(14.0);
                        Frame::none()
                            .fill(CARD)
                            .stroke(Stroke::new(1.0, BORDER))
                            .rounding(8.0)
                            .inner_margin(Margin::symmetric(10.0, 8.0))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Current state").size(9.0).color(FAINT),
                                    );
                                    let (text, ok) = prompt.device.state_pill();
                                    pill(
                                        ui,
                                        text,
                                        if ok { GREEN } else { AMBER },
                                        if ok {
                                            Color32::from_rgba_unmultiplied(61, 214, 149, 18)
                                        } else {
                                            Color32::from_rgba_unmultiplied(255, 181, 84, 18)
                                        },
                                    );
                                });
                            });
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            if primary_button(ui, "Connect and trust").clicked() {
                                confirm = Some(ConnectMode::Full);
                            }
                            if secondary_button(ui, "Certificate only").clicked() {
                                confirm = Some(ConnectMode::CertOnly);
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if secondary_button(ui, "Cancel").clicked() {
                                    cancel = true;
                                }
                            });
                        });
                    });
            });

        if let Some(mode) = confirm {
            let mut prompt = prompt;
            prompt.mode = mode;
            self.device_prompt = None;
            self.connect_detected_device(prompt, ctx);
        } else if cancel {
            self.device_prompt = None;
        }
    }

    fn draw_toast(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let Some(t) = &self.toast else {
            return;
        };
        if now >= t.until {
            self.toast = None;
            return;
        }
        let text = t.text.clone();
        ctx.request_repaint_after(Duration::from_millis(120));
        egui::Area::new(Id::new("toast"))
            .order(egui::Order::Tooltip)
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -38.0))
            .show(ctx, |ui| {
                Frame::none()
                    .fill(Color32::from_rgb(31, 39, 52))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(71, 87, 112)))
                    .rounding(8.0)
                    .inner_margin(Margin::symmetric(14.0, 9.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            dot(ui, ACCENT, 7.0);
                            ui.label(RichText::new(text).size(11.5).color(TEXT));
                        });
                    });
            });
    }
}

fn command_box(
    ui: &mut Ui,
    label: &str,
    command: &str,
    ctx: &egui::Context,
    app: &mut App,
) {
    ui.horizontal(|ui| {
        form_label(ui, label);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if secondary_button(ui, "Copy").clicked() {
                app.copy(ctx, command.to_string(), "Commands copied");
            }
        });
    });
    ui.add_space(3.0);
    code_block(ui, command, 128.0, false);
}

fn tour_card(ui: &mut Ui, width: f32, n: &str, title: &str, body: &str) {
    Frame::none()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(10.0)
        .inner_margin(Margin::same(11.0))
        .show(ui, |ui| {
            ui.set_width((width - 24.0).max(120.0));
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 11.0, Color32::from_rgb(36, 48, 68));
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                n,
                FontId::monospace(10.0),
                Color32::from_rgb(165, 195, 255),
            );
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(title).size(11.5).strong().color(TEXT)).wrap());
            ui.add_space(1.0);
            ui.add(egui::Label::new(RichText::new(body).size(9.5).color(MUTED)).wrap());
        });
}

fn feature_card(ui: &mut Ui, title: &str, what: &str, how: &str, accent: Color32) {
    Frame::none()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(10.0)
        .inner_margin(Margin::same(12.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                dot(ui, accent, 7.0);
                ui.label(RichText::new(title).size(12.0).strong().color(TEXT));
            });
            ui.add_space(3.0);
            ui.add(egui::Label::new(RichText::new(what).size(10.0).color(MUTED)).wrap());
            ui.add_space(4.0);
            ui.horizontal_top(|ui| {
                ui.label(
                    RichText::new("HOW")
                        .size(8.0)
                        .strong()
                        .color(accent),
                );
                ui.add(
                    egui::Label::new(RichText::new(how).size(9.5).color(FAINT)).wrap(),
                );
            });
        });
    ui.add_space(9.0);
}

fn stack_row(ui: &mut Ui, role: &str, tech: &str, detail: &str) {
    ui.horizontal_top(|ui| {
        ui.add_sized(
            [118.0, 16.0],
            egui::Label::new(RichText::new(role).size(10.0).strong().color(MUTED)),
        );
        ui.add_sized(
            [172.0, 16.0],
            egui::Label::new(RichText::new(tech).font(mono(9.5)).color(CYAN)),
        );
        ui.add(
            egui::Label::new(RichText::new(detail).size(9.5).color(FAINT)).wrap(),
        );
    });
    ui.add_space(5.0);
}

fn truth_row(ui: &mut Ui, label: &str, text: &str, color: Color32) {
    ui.horizontal_top(|ui| {
        ui.add_sized(
            [82.0, 18.0],
            egui::Label::new(RichText::new(label).size(9.0).strong().color(color)),
        );
        ui.add(
            egui::Label::new(RichText::new(text).size(9.5).color(MUTED)).wrap(),
        );
    });
    ui.add_space(5.0);
}

fn build_curl(method: &str, url: &str, headers: &str, body: &str) -> String {
    let mut out = format!("curl -X {} '{}'", method, url);
    for (k, v) in parse_headers(headers) {
        out.push_str(&format!(" \\\n  -H '{}: {}'", k, v.replace('\'', "'\\''")));
    }
    if !body.is_empty() {
        out.push_str(&format!(
            " \\\n  --data-raw '{}'",
            body.replace('\'', "'\\''")
        ));
    }
    out
}

fn format_bytes(n: u64) -> String {
    if n >= 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{} B", n)
    }
}

fn composer_tab(ui: &mut Ui, label: &str, width: f32, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(
            Rect::from_min_max(
                egui::pos2(rect.left() + 6.0, rect.bottom() - 2.0),
                egui::pos2(rect.right() - 6.0, rect.bottom()),
            ),
            1.0,
            ACCENT,
        );
    }
    painter.text(
        rect.center() - Vec2::new(0.0, 1.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        if selected { TEXT } else { MUTED },
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn form_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(9.5).color(MUTED).strong());
}

fn rule_method_label(m: RuleMethod) -> &'static str {
    m.label()
}

fn step_row(ui: &mut Ui, n: u32, title: &str, detail: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
        ui.painter()
            .circle_filled(rect.center(), 9.0, Color32::from_rgb(36, 48, 68));
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            n.to_string(),
            FontId::monospace(9.0),
            Color32::from_rgb(150, 180, 230),
        );
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(RichText::new(title).size(10.5).strong().color(TEXT));
            ui.label(RichText::new(detail).size(9.5).color(FAINT));
        });
    });
    ui.add_space(6.0);
}

fn kpi_card(ui: &mut Ui, width: f32, label: &str, value: &str, sub: &str, color: Color32) {
    Frame::none()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(10.0)
        .inner_margin(Margin::same(13.0))
        .show(ui, |ui| {
            ui.set_width((width - 28.0).max(60.0));
            ui.horizontal(|ui| {
                ui.label(RichText::new(label).size(8.5).color(FAINT).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    dot(ui, color, 6.0);
                });
            });
            ui.add_space(2.0);
            ui.label(RichText::new(value).size(21.0).strong().color(TEXT));
            ui.label(RichText::new(sub).size(9.0).color(FAINT));
        });
}

fn traffic_chart(ui: &mut Ui, total: usize) {
    let height = 138.0;
    let width = ui.available_width();
    let (resp, painter) = ui.allocate_painter(Vec2::new(width, height), Sense::hover());
    let rect = resp.rect;
    let mut values = vec![
        4.0, 6.0, 5.0, 8.0, 11.0, 7.0, 12.0, 9.0, 6.0, 8.0, 13.0, 16.0, 11.0, 8.0, 10.0, 14.0,
        17.0, 12.0, 9.0, 11.0, 15.0, 11.0, 7.0, 5.0,
    ];
    if total > 0 {
        if let Some(last) = values.last_mut() {
            *last = (total as f32).max(3.0);
        }
    }
    let max = values.iter().cloned().fold(18.0_f32, f32::max);
    let grid = Color32::from_rgb(34, 42, 56);
    for i in 0..=3 {
        let y = rect.bottom() - 16.0 - (rect.height() - 24.0) * (i as f32 / 3.0);
        painter.hline(rect.x_range(), y, Stroke::new(1.0, grid));
        painter.text(
            egui::pos2(rect.left(), y - 1.0),
            Align2::LEFT_BOTTOM,
            format!("{}", (max * i as f32 / 3.0).round() as u32),
            FontId::monospace(8.0),
            FAINT,
        );
    }
    let n = values.len();
    let slot = rect.width() / n as f32;
    for (i, v) in values.iter().enumerate() {
        let h = (v / max) * (rect.height() - 28.0);
        let x0 = rect.left() + slot * i as f32 + 2.0;
        let x1 = x0 + slot - 4.0;
        let bar = Rect::from_min_max(
            egui::pos2(x0, rect.bottom() - 16.0 - h),
            egui::pos2(x1.max(x0 + 1.0), rect.bottom() - 16.0),
        );
        let active = i + 4 >= n;
        painter.rect_filled(
            bar,
            2.0,
            if active {
                Color32::from_rgba_unmultiplied(88, 140, 255, 190)
            } else {
                Color32::from_rgba_unmultiplied(88, 140, 255, 70)
            },
        );
        if active {
            painter.rect_filled(
                Rect::from_min_max(bar.min, egui::pos2(bar.right(), bar.top() + 3.0)),
                1.0,
                Color32::from_rgb(130, 170, 255),
            );
        }
    }
    for (i, label) in ["11:40", "11:50", "12:00", "12:05"].iter().enumerate() {
        let x = rect.left() + rect.width() * (i as f32 / 3.0);
        painter.text(
            egui::pos2(x, rect.bottom()),
            Align2::LEFT_BOTTOM,
            *label,
            FontId::monospace(8.0),
            FAINT,
        );
    }
}

fn status_bars(ui: &mut Ui, captures: &[Capture]) {
    let mut c2 = 0usize;
    let mut c3 = 0usize;
    let mut c4 = 0usize;
    let mut c5 = 0usize;
    let mut c0 = 0usize;
    for c in captures {
        match c.status {
            0 => c0 += 1,
            200..=299 => c2 += 1,
            300..=399 => c3 += 1,
            400..=499 => c4 += 1,
            _ => c5 += 1,
        }
    }
    let total = captures.len().max(1);
    let rows = [
        ("2xx", c2, GREEN),
        ("3xx", c3, CYAN),
        ("4xx", c4, AMBER),
        ("5xx", c5, RED),
        ("ERR", c0, Color32::from_rgb(200, 80, 90)),
    ];
    for (label, count, color) in rows {
        ui.horizontal(|ui| {
            ui.add_sized(
                [26.0, 14.0],
                egui::Label::new(RichText::new(label).font(mono(9.0)).color(MUTED)),
            );
            let w = (ui.available_width() - 54.0).max(60.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 8.0), Sense::hover());
            ui.painter().rect_filled(rect, 4.0, INPUT);
            let frac = count as f32 / total as f32;
            if count > 0 {
                let fill = Rect::from_min_size(
                    rect.min,
                    Vec2::new((rect.width() * frac).max(3.0), rect.height()),
                );
                ui.painter().rect_filled(fill, 4.0, color);
            }
            ui.add_sized(
                [40.0, 14.0],
                egui::Label::new(
                    RichText::new(format!("{}", count))
                        .font(mono(9.0))
                        .color(if count > 0 { TEXT } else { FAINT }),
                ),
            );
        });
        ui.add_space(2.0);
    }
}

fn top_hosts(ui: &mut Ui, captures: &[Capture]) {
    let mut map: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    for c in captures {
        let e = map.entry(c.host()).or_insert((0, 0));
        e.0 += 1;
        e.1 += c.latency_ms;
    }
    let mut rows: Vec<(String, usize, u64)> = map
        .into_iter()
        .map(|(host, (count, ms))| (host, count, ms / count.max(1) as u64))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1));
    let max = rows.first().map(|r| r.1).unwrap_or(1);
    for (host, count, avg) in rows.iter().take(6) {
        ui.label(
            RichText::new(host)
                .font(mono(9.5))
                .color(Color32::from_rgb(166, 188, 222)),
        );
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 56.0).max(30.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 6.0), Sense::hover());
            ui.painter().rect_filled(rect, 3.0, INPUT);
            let w = rect.width() * (*count as f32 / max as f32);
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(w.max(2.0), rect.height())),
                3.0,
                Color32::from_rgba_unmultiplied(88, 140, 255, 160),
            );
            ui.label(
                RichText::new(format!("{} · {} ms", count, avg))
                    .font(mono(8.5))
                    .color(FAINT),
            );
        });
        ui.add_space(4.0);
    }
}

impl eframe::App for App {
    /// Closing the window must never leave the Mac pointing at a proxy that
    /// has already stopped. If Proxzy set the web proxy, put it back to Direct.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.restore_mac_proxy_for_exit() {
            eprintln!("Proxzy: restored the macOS web proxy before exiting");
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.screenshot_tick(ctx);
        self.poll_send(ctx);
        self.poll_proxy(ctx);
        self.poll_proxy_test();
        self.poll_sim_reboot(ctx);
        self.poll_device_scan();
        self.poll_stale_proxy_scan();
        self.maybe_scan_devices();
        if self.device_scanning() {
            ctx.request_repaint_after(Duration::from_millis(150));
        }
        if self.selected >= self.captures.len() {
            self.selected = 0;
        }
        self.top_bar(ctx);
        self.stale_proxy_banner(ctx);
        self.status_bar(ctx);

        egui::SidePanel::left("rail")
            .resizable(false)
            .exact_width(232.0)
            .frame(
                Frame::none()
                    .fill(RAIL)
                    .inner_margin(Margin::symmetric(14.0, 12.0)),
            )
            .show(ctx, |ui| {
                self.sidebar(ui);
            });

        match self.view {
            View::Traffic => {
                egui::SidePanel::right("detail")
                    .resizable(false)
                    .exact_width(524.0)
                    .frame(Frame::none().fill(PANEL).inner_margin(Margin::same(16.0)))
                    .show(ctx, |ui| {
                        self.inspector(ui, ctx);
                    });
                egui::CentralPanel::default()
                    .frame(Frame::none().fill(BG).inner_margin(Margin {
                        left: 18.0,
                        right: 10.0,
                        top: 16.0,
                        bottom: 8.0,
                    }))
                    .show(ctx, |ui| {
                        self.traffic(ui, ctx);
                    });
            }
            View::Live => {
                egui::CentralPanel::default()
                    .frame(
                        Frame::none()
                            .fill(BG)
                            .inner_margin(Margin::symmetric(20.0, 16.0)),
                    )
                    .show(ctx, |ui| {
                        self.live_page(ui, ctx);
                    });
            }
            View::Composer => {
                egui::CentralPanel::default()
                    .frame(
                        Frame::none()
                            .fill(BG)
                            .inner_margin(Margin::symmetric(20.0, 16.0)),
                    )
                    .show(ctx, |ui| {
                        self.composer_page(ui, ctx);
                    });
            }
            View::MapLocal => {
                egui::CentralPanel::default()
                    .frame(
                        Frame::none()
                            .fill(BG)
                            .inner_margin(Margin::symmetric(20.0, 16.0)),
                    )
                    .show(ctx, |ui| {
                        self.map_local_page_v2(ui, ctx);
                    });
            }
            View::Breakpoints => {
                egui::CentralPanel::default()
                    .frame(
                        Frame::none()
                            .fill(BG)
                            .inner_margin(Margin::symmetric(20.0, 16.0)),
                    )
                    .show(ctx, |ui| {
                        self.breakpoints_page_v2(ui, ctx);
                    });
            }
            View::Logs => {
                egui::CentralPanel::default()
                    .frame(
                        Frame::none()
                            .fill(BG)
                            .inner_margin(Margin::symmetric(20.0, 16.0)),
                    )
                    .show(ctx, |ui| {
                        self.logs_page(ui, ctx);
                    });
            }
            View::Guide => {
                egui::CentralPanel::default()
                    .frame(
                        Frame::none()
                            .fill(BG)
                            .inner_margin(Margin::symmetric(20.0, 16.0)),
                    )
                    .show(ctx, |ui| {
                        self.guide_page(ui, ctx);
                    });
            }
        }

        self.modals(ctx);
        self.draw_toast(ctx);
    }
}
