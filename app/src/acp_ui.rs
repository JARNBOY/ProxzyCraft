// ---------------------------------------------------------------------------
// ACP Bridge screen.
//
// This is the editor-to-agent half of the agent story. Agent Control and
// Screen Play own the MCP/tool half; this page shows the session handshake,
// the permission gate and copy-ready ACP setup. The transcript is a concept
// lab, clearly labelled as simulated, and the real control plane stays on the
// HTTP side.
// ---------------------------------------------------------------------------

impl App {
    fn acp_reset(&mut self, play: bool) {
        self.acp_progress = 0;
        self.acp_selected = 0;
        self.acp_permission = AcpPermission::Pending;
        self.acp_playing = play;
        self.acp_entered = Instant::now();
    }

    fn acp_show_full(&mut self, ctx: &egui::Context) {
        self.acp_permission = AcpPermission::AllowOnce;
        let len = acp_transcript(self.proxy_port, self.acp_permission).len();
        self.acp_progress = len.saturating_sub(1);
        self.acp_selected = self.acp_progress;
        self.acp_playing = false;
        self.toast_now(ctx, "Full ACP session shown — scroll the message list");
    }

    fn acp_decide(&mut self, ctx: &egui::Context, permission: AcpPermission) {
        self.acp_permission = permission;
        let len = acp_transcript(self.proxy_port, permission).len();
        self.acp_progress = (ACP_GATE_INDEX + 1).min(len.saturating_sub(1));
        self.acp_selected = self.acp_progress;
        self.acp_playing = true;
        self.acp_entered = Instant::now();
        let message = if permission == AcpPermission::RejectOnce {
            "Permission rejected — the agent stops and traffic is unchanged"
        } else {
            "Permission granted — the tool result and stop reason follow"
        };
        self.toast_now(ctx, message);
    }

    fn acp_next(&mut self, ctx: &egui::Context) {
        let len = acp_transcript(self.proxy_port, self.acp_permission).len();
        if self.acp_permission == AcpPermission::Pending
            && self.acp_progress >= ACP_GATE_INDEX
        {
            self.toast_now(ctx, "Choose a permission response to continue");
            return;
        }
        if self.acp_progress + 1 >= len {
            self.toast_now(ctx, "End of the ACP session — replay to start again");
            return;
        }
        self.acp_progress += 1;
        self.acp_selected = self.acp_progress;
        self.acp_playing = false;
    }

    /// Advance the concept session while it is playing, and stop on the
    /// permission gate until the person answers it.
    fn poll_acp(&mut self, ctx: &egui::Context) {
        if !self.acp_playing {
            return;
        }
        let len = acp_transcript(self.proxy_port, self.acp_permission).len();
        if self.acp_permission == AcpPermission::Pending && self.acp_progress >= ACP_GATE_INDEX {
            self.acp_playing = false;
            return;
        }
        if self.acp_progress + 1 >= len {
            self.acp_playing = false;
            return;
        }
        if self.acp_entered.elapsed() >= Duration::from_millis(620) {
            self.acp_progress += 1;
            self.acp_selected = self.acp_progress;
            self.acp_entered = Instant::now();
        }
        ctx.request_repaint_after(Duration::from_millis(120));
    }

    fn acp_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        self.poll_acp(ctx);
        let transcript = acp_transcript(self.proxy_port, self.acp_permission);
        let len = transcript.len().max(1);
        self.acp_progress = self.acp_progress.min(len - 1);
        self.acp_selected = self.acp_selected.min(self.acp_progress);

        let pending_gate = self.acp_permission == AcpPermission::Pending
            && self.acp_progress >= ACP_GATE_INDEX;
        let at_end = self.acp_progress + 1 >= len;
        let (status_text, status_color) = if pending_gate {
            ("WAITING FOR PERMISSION", AMBER)
        } else if self.acp_playing {
            ("STREAMING", CYAN)
        } else if self.acp_permission == AcpPermission::RejectOnce && at_end {
            ("REJECTED · STOPPED", RED)
        } else if at_end {
            ("SESSION COMPLETE", GREEN)
        } else {
            ("PAUSED", MUTED)
        };

        let mut replay = false;
        let mut next = false;
        let mut show_full = false;

        ui.horizontal(|ui| {
            Self::studio_heading(
                ui,
                "ACP Bridge",
                "The universal adapter between an editor and an AI agent: JSON-RPC, sessions, tool approvals — and where Proxzy fits.",
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary_button(ui, "Replay session").clicked() {
                    replay = true;
                }
                if secondary_button(ui, "Next message").clicked() {
                    next = true;
                }
                if secondary_button(ui, "Show full").clicked() {
                    show_full = true;
                }
                pill(
                    ui,
                    status_text,
                    status_color,
                    Color32::from_rgba_unmultiplied(
                        status_color.r(),
                        status_color.g(),
                        status_color.b(),
                        16,
                    ),
                );
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            pill(
                ui,
                "CONCEPT LAB",
                CYAN,
                Color32::from_rgba_unmultiplied(119, 211, 255, 16),
            );
            ui.label(
                RichText::new("ACP v1 · JSON-RPC 2.0 · stdio")
                    .font(mono(9.0))
                    .color(FAINT),
            );
            ui.label(
                RichText::new("The transcript is simulated; the Proxzy control plane is real.")
                    .size(9.5)
                    .color(MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("control plane 127.0.0.1:{}", self.proxy_port))
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
                    stat_box(ui, 134.0, "MESSAGES", &format!("{} / {}", self.acp_progress + 1, len), CYAN);
                    stat_box(ui, 134.0, "PROTOCOL", "JSON-RPC 2.0", ACCENT);
                    stat_box(ui, 124.0, "TRANSPORT", "stdio", PURPLE);
                    stat_box(ui, 142.0, "PERMISSION", self.acp_permission.label(), status_color);
                    stat_box(
                        ui,
                        134.0,
                        "MCP SERVERS",
                        "0 in demo",
                        if self.acp_progress >= 3 { MUTED } else { FAINT },
                    );
                });
                ui.add_space(12.0);

                let gap = 14.0;
                let total = (ui.available_width() - 20.0).max(820.0);
                let left = (total * 0.24).clamp(230.0, 292.0);
                let middle = (total * 0.42).clamp(370.0, 530.0);
                let right = (total - left - middle - gap * 2.0).max(258.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(left);
                        self.acp_left_column(ui);
                    });
                    ui.add_space(gap);
                    ui.vertical(|ui| {
                        ui.set_width(middle);
                        self.acp_timeline(ui, ctx, &transcript);
                    });
                    ui.add_space(gap);
                    ui.vertical(|ui| {
                        ui.set_width(right);
                        self.acp_setup(ui, ctx);
                        ui.add_space(10.0);
                        self.acp_checks_card(ui, ctx);
                        ui.add_space(10.0);
                        self.acp_mixups(ui);
                    });
                });
                ui.add_space(16.0);
            });

        if replay {
            self.acp_reset(true);
        } else if show_full {
            self.acp_show_full(ctx);
        } else if next {
            self.acp_next(ctx);
        }
    }

    fn acp_left_column(&mut self, ui: &mut Ui) {
        studio_card(ui, |ui| {
            ui.label(RichText::new("The bridge in one picture").size(13.0).strong().color(TEXT));
            ui.add_space(2.0);
            studio_note(
                ui,
                "ACP is the top link. MCP is the middle link. Proxzy is the HTTP gate at the bottom.",
            );
            ui.add_space(7.0);
            acp_flow_diagram(ui, Vec2::new(ui.available_width(), 168.0));
            ui.add_space(7.0);
            ui.label(
                RichText::new("Like LSP for AI agents: one protocol instead of one integration per editor.")
                    .size(9.5)
                    .color(MUTED),
            );
        });
        ui.add_space(10.0);

        studio_card(ui, |ui| {
            ui.label(RichText::new("What the adapter buys you").size(13.0).strong().color(TEXT));
            ui.add_space(6.0);
            acp_bullet(
                ui,
                "Swap the agent, keep the editor",
                "The editor speaks ACP once; the agent behind it can change.",
                CYAN,
            );
            acp_bullet(
                ui,
                "Custom agents fit",
                "A local or in-house agent becomes usable from any ACP client.",
                PURPLE,
            );
            acp_bullet(
                ui,
                "You keep the controls",
                "Permissions, file access and terminal access stay on the client side.",
                GREEN,
            );
        });
        ui.add_space(10.0);

        studio_card(ui, |ui| {
            ui.label(RichText::new("Three layers, not one").size(13.0).strong().color(TEXT));
            ui.add_space(6.0);
            acp_layer(ui, "ACP", "editor <-> agent", "session, prompts, approvals", CYAN);
            ui.add_space(5.0);
            acp_layer(ui, "MCP", "agent <-> tools", "resources and tool calls", GREEN);
            ui.add_space(5.0);
            acp_layer(
                ui,
                "HTTP",
                "agent / Proxzy <-> app",
                "the request the gate can mock",
                AMBER,
            );
        });
    }

    fn acp_timeline(&mut self, ui: &mut Ui, ctx: &egui::Context, transcript: &[AcpStep]) {
        let len = transcript.len();
        let pending_gate = self.acp_permission == AcpPermission::Pending
            && self.acp_progress >= ACP_GATE_INDEX;
        let at_end = self.acp_progress + 1 >= len;
        let mut pick: Option<usize> = None;
        let mut decide: Option<AcpPermission> = None;
        let mut copy: Option<String> = None;

        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(RichText::new("Session lab").size(13.0).strong().color(TEXT));
                    ui.label(
                        RichText::new("Click a message to read the exact JSON-RPC.")
                            .size(9.5)
                            .color(MUTED),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "Auto-play").clicked() {
                        self.acp_reset(true);
                    }
                    ui.label(
                        RichText::new(if self.acp_playing { "playing" } else { "paused" })
                            .size(9.0)
                            .color(if self.acp_playing { CYAN } else { FAINT }),
                    );
                });
            });
            ui.add_space(7.0);

            // A small progress rail that also marks the permission gate.
            let (bar, _) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), 7.0),
                Sense::hover(),
            );
            ui.painter().rect_filled(bar, 3.5, INPUT);
            let pct = if len <= 1 {
                1.0
            } else {
                (self.acp_progress + 1) as f32 / len as f32
            };
            let mut filled = bar;
            filled.set_width(bar.width() * pct.clamp(0.0, 1.0));
            ui.painter().rect_filled(filled, 3.5, ACCENT);
            if len > ACP_GATE_INDEX {
                let gx = bar.left() + bar.width() * (ACP_GATE_INDEX as f32 / len as f32);
                ui.painter().line_segment(
                    [
                        egui::pos2(gx, bar.top() - 2.0),
                        egui::pos2(gx, bar.bottom() + 2.0),
                    ],
                    Stroke::new(1.5, AMBER),
                );
            }
            ui.add_space(6.0);

            ScrollArea::vertical()
                .max_height(360.0)
                .auto_shrink([false, false])
                .scroll_bar_visibility(
                    egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                )
                .stick_to_bottom(self.acp_playing || pending_gate || at_end)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for index in 0..=self.acp_progress.min(len - 1) {
                        let step = &transcript[index];
                        let selected = index == self.acp_selected;
                        let is_gate = step.kind == AcpKind::Permission;
                        let accent = match step.kind {
                            AcpKind::Permission => AMBER,
                            AcpKind::Notification => CYAN,
                            AcpKind::Response => GREEN,
                            AcpKind::Request => ACCENT,
                        };
                        let fill = if selected {
                            Color32::from_rgb(37, 45, 62)
                        } else if is_gate {
                            Color32::from_rgba_unmultiplied(255, 181, 84, 12)
                        } else {
                            INPUT
                        };
                        let row = Frame::none()
                            .fill(fill)
                            .stroke(Stroke::new(
                                1.0,
                                if selected {
                                    accent
                                } else if is_gate {
                                    Color32::from_rgba_unmultiplied(255, 181, 84, 80)
                                } else {
                                    BORDER
                                },
                            ))
                            .rounding(7.0)
                            .inner_margin(Margin::symmetric(8.0, 5.0))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(34.0, 20.0),
                                        Layout::left_to_right(Align::Center),
                                        |ui| {
                                            ui.label(
                                                RichText::new(step.actor.direction())
                                                    .font(mono(8.0))
                                                    .color(if step.actor == AcpActor::Client {
                                                        ACCENT
                                                    } else {
                                                        GREEN
                                                    }),
                                            );
                                        },
                                    );
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y = 0.0;
                                        ui.label(
                                            RichText::new(step.method)
                                                .font(mono(9.0))
                                                .color(accent),
                                        );
                                        ui.label(
                                            RichText::new(step.title)
                                                .size(9.5)
                                                .color(TEXT),
                                        );
                                    });
                                    ui.with_layout(
                                        Layout::right_to_left(Align::Center),
                                        |ui| {
                                            ui.label(
                                                RichText::new(step.kind.label())
                                                    .size(7.5)
                                                    .strong()
                                                    .color(FAINT),
                                            );
                                            if is_gate {
                                                pill(
                                                    ui,
                                                    "GATE",
                                                    AMBER,
                                                    Color32::from_rgba_unmultiplied(
                                                        255, 181, 84, 20,
                                                    ),
                                                );
                                            }
                                        },
                                    );
                                });
                            });
                        if row.response.interact(Sense::click()).clicked() {
                            pick = Some(index);
                        }
                        ui.add_space(4.0);
                    }
                });

            if pending_gate {
                ui.add_space(2.0);
                Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(255, 181, 84, 14))
                    .stroke(Stroke::new(
                        1.0,
                        Color32::from_rgba_unmultiplied(255, 181, 84, 90),
                    ))
                    .rounding(9.0)
                    .inner_margin(Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            pill(
                                ui,
                                "PERMISSION GATE",
                                AMBER,
                                Color32::from_rgba_unmultiplied(255, 181, 84, 22),
                            );
                            ui.label(
                                RichText::new("The agent is paused before it changes MAP-01.")
                                    .size(9.8)
                                    .color(TEXT),
                            );
                        });
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if secondary_button(ui, "Allow once").clicked() {
                                decide = Some(AcpPermission::AllowOnce);
                            }
                            if secondary_button(ui, "Always allow").clicked() {
                                decide = Some(AcpPermission::AllowAlways);
                            }
                            if secondary_button(ui, "Reject").clicked() {
                                decide = Some(AcpPermission::RejectOnce);
                            }
                            ui.label(
                                RichText::new("This is the same choice an ACP editor shows.")
                                    .size(9.0)
                                    .color(FAINT),
                            );
                        });
                    });
            } else if self.acp_permission == AcpPermission::RejectOnce
                && self.acp_progress + 1 >= len
            {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    pill(
                        ui,
                        "STOPPED",
                        RED,
                        Color32::from_rgba_unmultiplied(255, 107, 107, 16),
                    );
                    ui.label(
                        RichText::new("No Map Local change and no request were sent.")
                            .size(9.8)
                            .color(MUTED),
                    );
                });
            } else if self.acp_permission.is_allowed() && self.acp_progress + 1 >= len {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    pill(
                        ui,
                        "END TURN",
                        GREEN,
                        Color32::from_rgba_unmultiplied(61, 214, 149, 16),
                    );
                    ui.label(
                        RichText::new("The agent reports a stop reason and the editor is free again.")
                            .size(9.8)
                            .color(MUTED),
                    );
                });
            }

            let selected = transcript[self.acp_selected.min(len - 1)].clone();
            ui.add_space(9.0);
            hsep(ui);
            ui.add_space(7.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(selected.method)
                        .font(mono(9.5))
                        .color(TEXT),
                );
                pill(
                    ui,
                    selected.kind.label(),
                    if selected.kind == AcpKind::Permission {
                        AMBER
                    } else {
                        CYAN
                    },
                    Color32::from_rgba_unmultiplied(119, 211, 255, 12),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "Copy message").clicked() {
                        copy = Some(selected.json.clone());
                    }
                });
            });
            ui.add_space(3.0);
            code_block(ui, &selected.json, 214.0, true);
            ui.add_space(3.0);
            ui.label(RichText::new(selected.note).size(9.2).color(FAINT));
        });

        if let Some(index) = pick {
            self.acp_selected = index;
            self.acp_playing = false;
        }
        if let Some(permission) = decide {
            self.acp_decide(ctx, permission);
        }
        if let Some(json) = copy {
            self.copy(ctx, json, "JSON-RPC message copied");
        }
    }

    fn acp_setup(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let port = self.proxy_port;
        let tabs = ["Editor config", "MCP entry", "Agent prompt"];
        let snippet = match self.acp_tab {
            1 => acp_mcp_entry(port),
            2 => acp_agent_prompt(port),
            _ => acp_editor_config(port),
        };
        let mut copy = false;
        let mut open_agent = false;
        let mut open_screen = false;
        let mut open_docs = false;

        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(RichText::new("Hook it up").size(13.0).strong().color(TEXT));
                    ui.label(
                        RichText::new("Three steps; no ACP server runs inside Proxzy.")
                            .size(9.5)
                            .color(MUTED),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    pill(
                        ui,
                        "TEMPLATE",
                        CYAN,
                        Color32::from_rgba_unmultiplied(119, 211, 255, 14),
                    );
                });
            });
            ui.add_space(7.0);
            acp_numbered(
                ui,
                "1",
                "Launch an ACP agent from the editor",
                "The editor is the client; the agent runs as a subprocess and speaks JSON-RPC on stdio.",
            );
            acp_numbered(
                ui,
                "2",
                "initialize, then session/new",
                "They agree on version 1, a working folder and any MCP servers the agent may use.",
            );
            acp_numbered(
                ui,
                "3",
                "Prompt, approve, verify",
                "The agent asks before a side effect, then calls the real Proxzy control plane.",
            );
            ui.add_space(7.0);
            ui.horizontal_wrapped(|ui| {
                for (index, label) in tabs.iter().enumerate() {
                    if chip(ui, label, self.acp_tab == index).clicked() {
                        self.acp_tab = index;
                    }
                }
            });
            ui.add_space(5.0);
            code_block(ui, &snippet, 172.0, index_json(&snippet));
            ui.horizontal(|ui| {
                if secondary_button(ui, "Copy").clicked() {
                    copy = true;
                }
                if secondary_button(ui, "Agent Control").clicked() {
                    open_agent = true;
                }
                if secondary_button(ui, "MCP screen").clicked() {
                    open_screen = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, "ACP docs").clicked() {
                        open_docs = true;
                    }
                });
            });
            ui.add_space(3.0);
            studio_note(
                ui,
                "Replace the absolute paths. Proxzy exposes plain HTTP control endpoints today; the MCP entry is the slot where a wrapper goes.",
            );
        });

        if copy {
            self.copy(ctx, snippet, "ACP setup copied");
        }
        if open_agent {
            self.view = View::Agent;
        }
        if open_screen {
            self.view = View::Screen;
        }
        if open_docs {
            ctx.open_url(egui::OpenUrl::new_tab(
                "https://agentclientprotocol.com/protocol/overview",
            ));
        }
    }

    fn acp_checks_card(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let checks = acp_checks(self.acp_permission);
        let mut copy = false;
        studio_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Checks on this session").size(13.0).strong().color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let passed = checks.iter().filter(|c| c.ok).count();
                    pill(
                        ui,
                        &format!("{} / {}", passed, checks.len()),
                        if passed == checks.len() { GREEN } else { AMBER },
                        Color32::from_rgba_unmultiplied(
                            if passed == checks.len() { 61 } else { 255 },
                            if passed == checks.len() { 214 } else { 181 },
                            if passed == checks.len() { 149 } else { 84 },
                            16,
                        ),
                    );
                });
            });
            ui.add_space(3.0);
            studio_note(
                ui,
                "Concept checks, not a conformance run. They make the handshake's promises visible.",
            );
            ui.add_space(6.0);
            for check in &checks {
                ui.horizontal_top(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                    let color = if check.ok { GREEN } else { AMBER };
                    ui.painter().circle_filled(
                        r.center(),
                        6.0,
                        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 26),
                    );
                    ui.painter().circle_filled(r.center(), 2.0, color);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(
                            RichText::new(check.title)
                                .size(10.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.add(
                            egui::Label::new(
                                RichText::new(check.detail).size(8.8).color(FAINT),
                            )
                            .wrap(),
                        );
                    });
                });
                ui.add_space(3.0);
            }
            ui.add_space(4.0);
            if secondary_button(ui, "Copy session summary").clicked() {
                copy = true;
            }
        });
        if copy {
            let summary = acp_summary(self.proxy_port, self.acp_permission);
            self.copy(ctx, summary, "ACP session summary copied");
        }
    }

    fn acp_mixups(&mut self, ui: &mut Ui) {
        studio_card(ui, |ui| {
            ui.label(RichText::new("Common mix-ups").size(13.0).strong().color(TEXT));
            ui.add_space(5.0);
            acp_mixup(
                ui,
                "ACP is not an HTTP proxy",
                "It never sees your request bodies. Proxzy is the gate that does.",
                RED,
            );
            acp_mixup(
                ui,
                "MCP is not ACP",
                "MCP gives the agent tools. ACP carries the editor <-> agent session.",
                GREEN,
            );
            acp_mixup(
                ui,
                "The editor launches the agent",
                "Proxzy is a tool behind the agent, not the process that hosts it.",
                CYAN,
            );
            ui.add_space(2.0);
            ui.label(
                RichText::new("Reference: agentclientprotocol.com/protocol/overview")
                    .size(8.5)
                    .color(FAINT),
            );
        });
    }
}

fn index_json(text: &str) -> bool {
    text.trim_start().starts_with('{')
}

fn acp_bullet(ui: &mut Ui, title: &str, body: &str, color: Color32) {
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 2.5, color);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(RichText::new(title).size(10.5).strong().color(TEXT));
            ui.add(
                egui::Label::new(RichText::new(body).size(9.2).color(MUTED)).wrap(),
            );
        });
    });
    ui.add_space(5.0);
}

fn acp_layer(ui: &mut Ui, name: &str, link: &str, detail: &str, color: Color32) {
    Frame::none()
        .fill(INPUT)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(7.0)
        .inner_margin(Margin::symmetric(8.0, 6.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                pill(
                    ui,
                    name,
                    color,
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 18),
                );
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(RichText::new(link).size(9.5).strong().color(TEXT));
                    ui.label(RichText::new(detail).size(8.5).color(FAINT));
                });
            });
        });
}

fn acp_numbered(ui: &mut Ui, number: &str, title: &str, body: &str) {
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::new(20.0, 20.0), Sense::hover());
        ui.painter().circle_filled(
            r.center(),
            9.0,
            Color32::from_rgba_unmultiplied(88, 140, 255, 30),
        );
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            number,
            FontId::proportional(9.5),
            ACCENT,
        );
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(RichText::new(title).size(10.2).strong().color(TEXT));
            ui.add(
                egui::Label::new(RichText::new(body).size(8.9).color(MUTED)).wrap(),
            );
        });
    });
    ui.add_space(4.0);
}

fn acp_mixup(ui: &mut Ui, title: &str, body: &str, color: Color32) {
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 2.4, color);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(RichText::new(title).size(9.8).strong().color(TEXT));
            ui.add(
                egui::Label::new(RichText::new(body).size(8.6).color(MUTED)).wrap(),
            );
        });
    });
    ui.add_space(4.0);
}

/// A schematic of the five layers. The drawing is intentionally flat and
/// text-first so it stays readable at the narrow column width.
fn acp_flow_diagram(ui: &mut Ui, size: Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 9.0, INPUT);
    p.rect_stroke(rect, 9.0, Stroke::new(1.0, BORDER));

    let x = rect.left() + 10.0;
    let w = rect.width() - 20.0;
    let mut y = rect.top() + 8.0;

    let draw_box = |label: &str,
                    sub: &str,
                    color: Color32,
                    h: f32,
                    box_y: f32,
                    fill: bool| {
        let r = Rect::from_min_size(egui::pos2(x, box_y), Vec2::new(w, h));
        if fill {
            p.rect_filled(
                r,
                4.5,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 22),
            );
        } else {
            p.rect_filled(r, 4.5, CARD);
        }
        p.rect_stroke(r, 4.5, Stroke::new(1.0, color));
        p.text(
            egui::pos2(r.left() + 7.0, r.center().y - 1.0),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(if fill { 7.8 } else { 8.8 }),
            color,
        );
        p.text(
            egui::pos2(r.right() - 7.0, r.center().y - 1.0),
            Align2::RIGHT_CENTER,
            sub,
            FontId::proportional(6.9),
            FAINT,
        );
        r
    };

    let a = draw_box("Editor / IDE", "the client", ACCENT, 24.0, y, false);
    y = a.bottom() + 2.0;
    p.line_segment(
        [
            egui::pos2(a.center().x, a.bottom() + 0.5),
            egui::pos2(a.center().x, a.bottom() + 5.5),
        ],
        Stroke::new(1.2, MUTED),
    );
    y += 6.0;
    let acp = draw_box("ACP  ·  JSON-RPC", "session + permission", CYAN, 19.0, y, true);
    y = acp.bottom() + 2.0;
    p.line_segment(
        [
            egui::pos2(acp.center().x, acp.bottom() + 0.5),
            egui::pos2(acp.center().x, acp.bottom() + 5.5),
        ],
        Stroke::new(1.2, MUTED),
    );
    y += 6.0;
    let agent = draw_box("AI coding agent", "does the work", GREEN, 24.0, y, false);
    y = agent.bottom() + 2.0;
    p.line_segment(
        [
            egui::pos2(agent.center().x, agent.bottom() + 0.5),
            egui::pos2(agent.center().x, agent.bottom() + 5.5),
        ],
        Stroke::new(1.2, MUTED),
    );
    y += 6.0;
    let mcp = draw_box("MCP  ·  tools", "files + control calls", AMBER, 19.0, y, true);
    y = mcp.bottom() + 2.0;
    p.line_segment(
        [
            egui::pos2(mcp.center().x, mcp.bottom() + 0.5),
            egui::pos2(mcp.center().x, mcp.bottom() + 5.5),
        ],
        Stroke::new(1.2, MUTED),
    );
    y += 6.0;
    let gate = draw_box(
        "Proxzy gate",
        "HTTP in · mock out",
        PURPLE,
        24.0,
        y,
        false,
    );
    let _ = gate;

    p.text(
        egui::pos2(rect.right() - 8.0, rect.top() + 7.0),
        Align2::RIGHT_CENTER,
        "concept",
        FontId::proportional(6.2),
        FAINT,
    );
}
