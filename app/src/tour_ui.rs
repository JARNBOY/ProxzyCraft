// ---------------------------------------------------------------------------
// Tutorial page and guided tour overlay.
//
// This file is included from app.rs, so it shares the App fields and uses the
// same palette, chips, cards and buttons as the rest of the app. The point is
// findability: the person said they could see the traffic list but never found
// Agent Control. The page names every main screen, and the overlay switches to
// that screen while it explains it.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TourArt {
    Window,
    Traffic,
    Inspector,
    Composer,
    MapLocal,
    Breakpoint,
    Logs,
    Agent,
    AgentHook,
    Tests,
    Screen,
    Done,
}

#[derive(Clone, Copy)]
struct TourStep {
    section: &'static str,
    title: &'static str,
    body: &'static str,
    bullets: &'static [&'static str],
    where_to: &'static str,
    view: View,
    filter: Option<TrafficFilter>,
    inspector: Option<InspectorTab>,
    select_first: bool,
    agent_hook: bool,
    art: TourArt,
}

const fn tour_step(
    section: &'static str,
    title: &'static str,
    body: &'static str,
    bullets: &'static [&'static str],
    where_to: &'static str,
    view: View,
    art: TourArt,
) -> TourStep {
    TourStep {
        section,
        title,
        body,
        bullets,
        where_to,
        view,
        filter: None,
        inspector: None,
        select_first: false,
        agent_hook: false,
        art,
    }
}

const TOUR_STEPS: &[TourStep] = &[
    TourStep {
        filter: Some(TrafficFilter::All),
        ..tour_step(
            "WELCOME",
            "This is more than a traffic list",
            "Proxzy is a small network lab. The left rail groups everything: Traffic for calls, Automation & agents for Agent Control, and Tools for mocks, breakpoints and logs.",
            &[
                "It opens on All Traffic, so it can look like a viewer only.",
                "Agent Control is under Automation & agents — not inside Live Capture.",
                "The tour opens each page for you; leave at any time.",
            ],
            "Left rail · all three sections",
            View::Traffic,
            TourArt::Window,
        )
    },
    TourStep {
        select_first: true,
        filter: Some(TrafficFilter::All),
        ..tour_step(
            "TRAFFIC",
            "All Traffic is the live feed",
            "Every call through the local proxy lands here with method, status, type, time and size. Sample rows are badged SAMPLE; real calls are badged LIVE.",
            &[
                "Filters: Live, Pinned, GraphQL, REST, Errors, Mocked, Tunnels.",
                "Click a row to pin it; Follow live jumps back to the newest call.",
                "Run capture check proves one real request came through the gate.",
            ],
            "Traffic → All Traffic",
            View::Traffic,
            TourArt::Traffic,
        )
    },
    TourStep {
        select_first: true,
        filter: Some(TrafficFilter::All),
        inspector: Some(InspectorTab::Overview),
        ..tour_step(
            "INSPECTOR",
            "Copy, share and repeat a request",
            "Select a call and the right-hand inspector shows Overview, Request, Response, Headers and cURL. The action row is the Postman-style part.",
            &[
                "Copy URL, Copy cURL, Share link and Markdown are one click.",
                "Repeat replays the call; Mock this turns it into a Map Local rule.",
                "Response bodies switch between JSON (default), Raw, XML and HTML.",
            ],
            "Right panel · action row + tabs",
            View::Traffic,
            TourArt::Inspector,
        )
    },
    TourStep {
        ..tour_step(
            "COMPOSER",
            "Write REST or GraphQL requests",
            "The Composer works like a small Postman: pick a preset or paste a cURL command, edit method, URL, headers and body, then Send.",
            &[
                "The GraphQL preset builds a query with operation name and variables.",
                "Responses keep the same JSON / Raw / XML / HTML switch.",
                "After a send, choose a scenario slot and press Save Response as Mock.",
            ],
            "Tools → Composer",
            View::Composer,
            TourArt::Composer,
        )
    },
    TourStep {
        ..tour_step(
            "MAP LOCAL",
            "Answer any endpoint from a local file",
            "Map Local matches a URL pattern and returns your own status, JSON and delay. Three scenarios live in every rule: Success, Error and Empty.",
            &[
                "Use the Direct URL / cURL Hook bar, then + Hook Mock.",
                "Switch a whole module in one click; Passthrough sends real traffic.",
                "Inject a token or username without editing the base fixture.",
            ],
            "Tools → Map Local",
            View::MapLocal,
            TourArt::MapLocal,
        )
    },
    TourStep {
        ..tour_step(
            "BREAKPOINTS",
            "Pause a request and edit it",
            "A breakpoint catches matching real traffic before it moves on. The request or response editor opens with Continue and Abort.",
            &[
                "Arm a catch rule and turn Catching on.",
                "Edit method, URL, headers or body at the request stage.",
                "Continue releases the call; Abort ends it.",
            ],
            "Tools → Breakpoints",
            View::Breakpoints,
            TourArt::Breakpoint,
        )
    },
    TourStep {
        ..tour_step(
            "LOG DASHBOARD",
            "One timeline for every event",
            "Captures, mock hits, breakpoint stops and system messages are written to one log. KPI cards and top hosts update from the session.",
            &[
                "Filter by All, Info, Warn or Error.",
                "Copy summary gives a shareable session report.",
                "The same events appear while a practice run is playing.",
            ],
            "Monitor → Log Activity",
            View::Logs,
            TourArt::Logs,
        )
    },
    TourStep {
        ..tour_step(
            "AUTOMATION",
            "Agent Control is a real screen",
            "This is the part that was easy to miss. Agent Control lives in the sidebar under Automation & agents. It lets an external agent or IDE drive the mock engine and prove its own calls.",
            &[
                "Pick a rule, switch Success / Error / Empty, inject a value.",
                "The control-plane feed lists every /__proxzy/* call as it arrives.",
                "Engine READY means the loopback control plane is listening.",
            ],
            "Automation & agents → Agent Control",
            View::Agent,
            TourArt::Agent,
        )
    },
    TourStep {
        agent_hook: true,
        ..tour_step(
            "AGENT CONTROL",
            "Hook an IDE in one paste",
            "Scroll to the Hook an IDE card. Choose Cursor, VS Code, Claude Code or JetBrains, then paste the brief into the rules file that client reads.",
            &[
                "Copy brief carries the port, CA path and endpoints as plain text.",
                "Copy check gives the agent one health command to run first.",
                "Its request appears in the feed, so the result is verified.",
            ],
            "Agent Control → Hook an IDE (scroll down)",
            View::Agent,
            TourArt::AgentHook,
        )
    },
    TourStep {
        ..tour_step(
            "API TEST BENCH",
            "Run saved requests like a test suite",
            "The Test Bench sends saved REST or GraphQL requests through the mock engine and pins the expected verdict next to each one.",
            &[
                "Pick a target and press Run selected or Run all.",
                "Verdicts compare hit counts and response status.",
                "Import or export the suite as JSON to share it.",
            ],
            "Automation & agents → API Test Bench",
            View::Tests,
            TourArt::Tests,
        )
    },
    TourStep {
        ..tour_step(
            "SCREEN PLAY",
            "Record a tour of the app or an agent flow",
            "Screen Play mixes engine steps with device steps. Scenario and Inject run for real; Tap, Type and ExpectVisible are simulated because the external harness owns the device.",
            &[
                "Hit checkpoints wait for real traffic instead of sample hits.",
                "Export a play as JSON for a recording or a demo.",
                "MCP and CLI agents can run the same steps.",
            ],
            "Automation & agents → Screen Play & MCP",
            View::Screen,
            TourArt::Screen,
        )
    },
    TourStep {
        ..tour_step(
            "FINISH",
            "You have seen the whole lab",
            "Start with All Traffic for capture, Map Local for mocking, Breakpoints for interception, and Agent Control when an AI agent or IDE needs a hook.",
            &[
                "Replay this tour from the Tutorial page or the top-bar chip.",
                "Guide & Stack explains certificates and device setup.",
                "Nothing is locked behind an account or trial.",
            ],
            "Tools → Guide & Stack",
            View::Guide,
            TourArt::Done,
        )
    },
];

fn tour_index_for(view: View) -> usize {
    TOUR_STEPS
        .iter()
        .enumerate()
        .position(|(index, step)| index > 0 && step.view == view)
        .unwrap_or(0)
}

fn tour_index_for_art(art: TourArt) -> usize {
    TOUR_STEPS
        .iter()
        .position(|step| step.art == art)
        .unwrap_or(0)
}

impl App {
    fn begin_tour(&mut self, index: usize) {
        let last = TOUR_STEPS.len().saturating_sub(1);
        self.tour = Some(index.min(last));
        self.apply_tour_step();
    }

    fn apply_tour_step(&mut self) {
        let Some(index) = self.tour else {
            return;
        };
        let Some(step) = TOUR_STEPS.get(index).copied() else {
            self.tour = None;
            return;
        };
        self.view = step.view;
        if let Some(filter) = step.filter {
            self.filter = filter;
        }
        if let Some(tab) = step.inspector {
            self.inspector_tab = tab;
        }
        if step.select_first {
            self.follow_latest = false;
            if !self.captures.is_empty() {
                self.selected = 0;
            }
        }
    }

    fn next_tour(&mut self, ctx: &egui::Context) {
        let Some(index) = self.tour else {
            return;
        };
        if index + 1 >= TOUR_STEPS.len() {
            self.finish_tour(ctx);
        } else {
            self.tour = Some(index + 1);
            self.apply_tour_step();
        }
    }

    fn previous_tour(&mut self) {
        let Some(index) = self.tour else {
            return;
        };
        self.tour = Some(index.saturating_sub(1));
        self.apply_tour_step();
    }

    fn finish_tour(&mut self, ctx: &egui::Context) {
        self.tour = None;
        self.toast_now(
            ctx,
            "Tour complete — Tutorial is always in the top bar",
        );
    }

    fn agent_hook_offset(&self) -> f32 {
        if self
            .tour
            .and_then(|index| TOUR_STEPS.get(index))
            .map(|step| step.agent_hook)
            .unwrap_or(false)
        {
            return 980.0;
        }
        agent_scroll_offset()
    }

    // =====================================================================
    // Tutorial page
    // =====================================================================

    fn tutorial_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut start_tour: Option<usize> = None;
        let mut open_view: Option<View> = None;

        ui.horizontal(|ui| {
            Self::studio_heading(
                ui,
                "Tutorial",
                "A guided tour of the whole app — including the Agent Control screen that was easy to miss.",
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary_button(ui, "Start the tour").clicked() {
                    start_tour = Some(0);
                }
                if secondary_button(ui, "Open Agent Control").clicked() {
                    open_view = Some(View::Agent);
                }
            });
        });
        ui.add_space(12.0);

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Hero: explanation + a miniature sidebar that shows exactly
                // where Agent Control lives.
                Frame::none()
                    .fill(CARD)
                    .stroke(Stroke::new(
                        1.0,
                        Color32::from_rgba_unmultiplied(88, 140, 255, 70),
                    ))
                    .rounding(14.0)
                    .inner_margin(Margin::same(18.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let total = ui.available_width();
                        let text_w = (total * 0.60).max(420.0);
                        let art_w = (total - text_w - 20.0).max(260.0);
                        ui.horizontal_top(|ui| {
                            ui.allocate_ui_with_layout(
                                Vec2::new(text_w, 190.0),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    ui.set_width(text_w);
                                    pill(
                                        ui,
                                        "3-MINUTE GUIDED TOUR",
                                        ACCENT,
                                        Color32::from_rgba_unmultiplied(88, 140, 255, 22),
                                    );
                                    ui.add_space(7.0);
                                    ui.label(
                                        RichText::new("Learn every main feature by seeing it")
                                            .size(22.0)
                                            .strong()
                                            .color(TEXT),
                                    );
                                    ui.add_space(3.0);
                                    ui.label(
                                        RichText::new(
                                            "Twelve short stops. The app switches to each page, dims the background, and points at the part that matters. Traffic, Inspector, Composer, Map Local, Breakpoints, Logs, Agent Control, the IDE hook, Test Bench, Screen Play and the Guide.",
                                        )
                                        .size(11.5)
                                        .color(MUTED),
                                    );
                                    ui.add_space(10.0);
                                    ui.horizontal(|ui| {
                                        if primary_button(ui, "Start the 12-stop tour").clicked() {
                                            start_tour = Some(0);
                                        }
                                        if secondary_button(ui, "Can't find Agent? Show me").clicked()
                                        {
                                            start_tour = Some(tour_index_for(View::Agent));
                                        }
                                    });
                                },
                            );
                            ui.add_space(20.0);
                            ui.allocate_ui_with_layout(
                                Vec2::new(art_w, 190.0),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    ui.set_width(art_w);
                                    tour_sidebar_sketch(ui, Vec2::new(art_w, 178.0));
                                },
                            );
                        });
                    });
                ui.add_space(6.0);

                // The direct answer to the person's question.
                Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(255, 181, 84, 10))
                    .stroke(Stroke::new(
                        1.0,
                        Color32::from_rgba_unmultiplied(255, 181, 84, 65),
                    ))
                    .rounding(12.0)
                    .inner_margin(Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let (icon_rect, _) =
                                ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                            ui.painter().circle_filled(
                                icon_rect.center(),
                                17.0,
                                Color32::from_rgba_unmultiplied(255, 181, 84, 28),
                            );
                            paint_nav_icon(
                                ui.painter(),
                                Rect::from_center_size(icon_rect.center(), Vec2::splat(20.0)),
                                NavIcon::Agent,
                                AMBER,
                            );
                            ui.add_space(4.0);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                ui.label(
                                    RichText::new("Can't find Agent Control?")
                                        .size(13.5)
                                        .strong()
                                        .color(TEXT),
                                );
                                ui.label(
                                    RichText::new(
                                        "It is not inside Live Capture. Look at the left rail: Automation & agents → Agent Control. It is now above Tools so it stays visible.",
                                    )
                                    .size(10.5)
                                    .color(MUTED),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if secondary_button(ui, "Open Agent Control").clicked() {
                                    open_view = Some(View::Agent);
                                }
                                if primary_button(ui, "Show me the way").clicked() {
                                    start_tour = Some(tour_index_for(View::Agent));
                                }
                            });
                        });
                    });
                ui.add_space(6.0);

                // Route map: all main features, grouped the same way as the rail.
                ui.label(
                    RichText::new("THE WHOLE LAB, IN THREE GROUPS")
                        .size(9.5)
                        .strong()
                        .color(FAINT),
                );
                ui.add_space(6.0);
                let gap = 12.0;
                let card_w = ((ui.available_width() - gap * 2.0) / 3.0).max(280.0);
                ui.horizontal_top(|ui| {
                    tour_group_card(
                        ui,
                        card_w,
                        "TRAFFIC & INSPECT",
                        "See what went in and out.",
                        ACCENT,
                        &[
                            ("All Traffic", "Live rows, filters and search"),
                            ("Inspector", "Copy URL / cURL, Share, Repeat"),
                            ("Composer", "Write REST or GraphQL requests"),
                        ],
                        &mut start_tour,
                        &mut open_view,
                        View::Traffic,
                    );
                    ui.add_space(gap);
                    tour_group_card(
                        ui,
                        card_w,
                        "MOCK & DEBUG",
                        "Change the answer without a backend.",
                        AMBER,
                        &[
                            ("Map Local", "Success / Error / Empty scenarios"),
                            ("Breakpoints", "Pause, edit, Continue or Abort"),
                            ("Log Activity", "One timeline and session summary"),
                        ],
                        &mut start_tour,
                        &mut open_view,
                        View::MapLocal,
                    );
                    ui.add_space(gap);
                    tour_group_card(
                        ui,
                        card_w,
                        "AUTOMATION & AGENTS",
                        "Let a test or an IDE drive the engine.",
                        GREEN,
                        &[
                            ("Agent Control", "Control plane, scenarios, injection"),
                            ("API Test Bench", "Run saved requests with verdicts"),
                            ("Screen Play & MCP", "Scripted engine + device steps"),
                        ],
                        &mut start_tour,
                        &mut open_view,
                        View::Agent,
                    );
                });
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(
                            "Prefer reading? Guide & Stack covers the local CA, simulators and the stack.",
                        )
                        .size(10.5)
                        .color(MUTED),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if secondary_button(ui, "Open Guide & Stack").clicked() {
                            open_view = Some(View::Guide);
                        }
                        if secondary_button(ui, "Replay practice run").clicked() {
                            self.start_practice_session(ctx);
                        }
                    });
                });
                ui.add_space(2.0);
            });

        if let Some(index) = start_tour {
            self.begin_tour(index);
        } else if let Some(view) = open_view {
            self.tour = None;
            self.view = view;
        }
    }

    // =====================================================================
    // Guided tour overlay
    // =====================================================================

    fn tutorial_overlay(&mut self, ctx: &egui::Context) {
        let Some(index) = self.tour else {
            return;
        };
        let Some(step) = TOUR_STEPS.get(index).copied() else {
            self.tour = None;
            return;
        };

        let mut go_next = false;
        let mut go_back = false;
        let mut exit = false;
        let mut jump_agent = false;

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            exit = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            go_next = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            go_back = true;
        }

        let screen = ctx.screen_rect();
        egui::Area::new(Id::new("tour_dim"))
            .order(egui::Order::Middle)
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(4, 6, 10, 196),
                );
                // A soft accent frame so the dimmed page still reads as part
                // of the tour rather than a broken screen.
                ui.painter().rect_stroke(
                    rect.shrink(9.0),
                    14.0,
                    Stroke::new(
                        1.0,
                        Color32::from_rgba_unmultiplied(88, 140, 255, 55),
                    ),
                );
                let _ = response;
            });

        let card_w = 900.0_f32.min(screen.width() - 48.0);
        let card_h = 356.0_f32.min(screen.height() - 60.0);
        let pos = egui::pos2(
            screen.center().x - card_w * 0.5,
            screen.bottom() - card_h - 30.0,
        );

        egui::Area::new(Id::new("tour_card"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                Frame::none()
                    .fill(PANEL)
                    .stroke(Stroke::new(
                        1.0,
                        Color32::from_rgba_unmultiplied(88, 140, 255, 130),
                    ))
                    .rounding(16.0)
                    .inner_margin(Margin::same(18.0))
                    .show(ui, |ui| {
                        ui.set_width(card_w - 36.0);
                        ui.set_height(card_h - 36.0);
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(258.0);
                                tour_art(ui, step.art, Vec2::new(258.0, 196.0));
                                ui.add_space(8.0);
                                let (label, color) = match step.art {
                                    TourArt::Agent | TourArt::AgentHook => {
                                        ("AGENT CONTROL", GREEN)
                                    }
                                    TourArt::Done => ("FINISH", GREEN),
                                    TourArt::Breakpoint => ("INTERCEPT", AMBER),
                                    TourArt::MapLocal | TourArt::Composer => ("MOCK", AMBER),
                                    _ => ("CAPTURE", ACCENT),
                                };
                                Frame::none()
                                    .fill(Color32::from_rgba_unmultiplied(
                                        color.r(),
                                        color.g(),
                                        color.b(),
                                        12,
                                    ))
                                    .stroke(Stroke::new(
                                        1.0,
                                        Color32::from_rgba_unmultiplied(
                                            color.r(),
                                            color.g(),
                                            color.b(),
                                            45,
                                        ),
                                    ))
                                    .rounding(8.0)
                                    .inner_margin(Margin::symmetric(9.0, 5.0))
                                    .show(ui, |ui| {
                                        ui.set_width(240.0);
                                        ui.label(
                                            RichText::new(label)
                                                .size(8.5)
                                                .strong()
                                                .color(color),
                                        );
                                        ui.label(
                                            RichText::new(step.where_to)
                                                .size(9.5)
                                                .color(MUTED),
                                        );
                                    });
                            });
                            ui.add_space(18.0);
                            ui.vertical(|ui| {
                                ui.set_width((card_w - 330.0).max(380.0));
                                ui.horizontal(|ui| {
                                    pill(
                                        ui,
                                        step.section,
                                        ACCENT,
                                        Color32::from_rgba_unmultiplied(88, 140, 255, 22),
                                    );
                                    ui.label(
                                        RichText::new(format!(
                                            "Step {} of {}",
                                            index + 1,
                                            TOUR_STEPS.len()
                                        ))
                                        .size(10.0)
                                        .color(FAINT),
                                    );
                                    ui.with_layout(
                                        Layout::right_to_left(Align::Center),
                                        |ui| {
                                            if secondary_button(ui, "Exit tour").clicked() {
                                                exit = true;
                                            }
                                            ui.label(
                                                RichText::new("Esc to exit · arrows to move")
                                                    .size(9.0)
                                                    .color(FAINT),
                                            );
                                        },
                                    );
                                });
                                ui.add_space(5.0);
                                ui.label(
                                    RichText::new(step.title)
                                        .size(21.0)
                                        .strong()
                                        .color(TEXT),
                                );
                                ui.add_space(3.0);
                                ui.label(RichText::new(step.body).size(11.0).color(MUTED));
                                ui.add_space(7.0);
                                for bullet in step.bullets {
                                    ui.horizontal(|ui| {
                                        let (dot_rect, _) = ui
                                            .allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                                        ui.painter().circle_filled(
                                            dot_rect.center(),
                                            2.4,
                                            Color32::from_rgb(120, 160, 235),
                                        );
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(*bullet).size(10.3).color(TEXT),
                                            )
                                            .wrap(),
                                        );
                                    });
                                }
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    // 12 progress dots.
                                    for i in 0..TOUR_STEPS.len() {
                                        let color = if i == index {
                                            ACCENT
                                        } else if i < index {
                                            Color32::from_rgba_unmultiplied(88, 140, 255, 120)
                                        } else {
                                            Color32::from_rgb(58, 60, 68)
                                        };
                                        let (r, _) = ui.allocate_exact_size(
                                            Vec2::splat(7.0),
                                            Sense::hover(),
                                        );
                                        ui.painter().circle_filled(r.center(), 3.0, color);
                                    }
                                    ui.with_layout(
                                        Layout::right_to_left(Align::Center),
                                        |ui| {
                                            let last = index + 1 >= TOUR_STEPS.len();
                                            if primary_button(
                                                ui,
                                                if last { "Finish" } else { "Next stop" },
                                            )
                                            .clicked()
                                            {
                                                go_next = true;
                                            }
                                            if index > 0
                                                && secondary_button(ui, "Back").clicked()
                                            {
                                                go_back = true;
                                            }
                                            if index < tour_index_for(View::Agent)
                                                && secondary_button(ui, "Skip to Agent").clicked()
                                            {
                                                jump_agent = true;
                                            }
                                        },
                                    );
                                });
                            });
                        });
                    });
            });

        if exit {
            self.finish_tour(ctx);
        } else if jump_agent {
            self.begin_tour(tour_index_for(View::Agent));
        } else if go_next {
            self.next_tour(ctx);
        } else if go_back {
            self.previous_tour();
        }
    }
}

// ---------------------------------------------------------------------------
// Tutorial page helpers
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn tour_group_card(
    ui: &mut Ui,
    width: f32,
    title: &str,
    subtitle: &str,
    color: Color32,
    rows: &[(&str, &str)],
    start_tour: &mut Option<usize>,
    open_view: &mut Option<View>,
    first_view: View,
) {
    Frame::none()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(12.0)
        .inner_margin(Margin::same(14.0))
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                Vec2::new((width - 28.0).max(240.0), 0.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width((width - 28.0).max(240.0));
                    ui.horizontal(|ui| {
                        let (r, _) =
                            ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                        ui.painter().circle_filled(r.center(), 3.5, color);
                        ui.label(RichText::new(title).size(9.0).strong().color(color));
                    });
                    ui.add_space(2.0);
                    ui.label(RichText::new(subtitle).size(11.0).color(TEXT));
                    ui.add_space(8.0);
                    for (name, note) in rows {
                        Frame::none()
                            .fill(PANEL)
                            .rounding(7.0)
                            .inner_margin(Margin::symmetric(8.0, 6.0))
                            .show(ui, |ui| {
                                ui.set_width((width - 48.0).max(180.0));
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(*name)
                                            .size(10.5)
                                            .strong()
                                            .color(TEXT),
                                    );
                                    ui.label(RichText::new(*note).size(9.0).color(FAINT));
                                });
                            });
                        ui.add_space(4.0);
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if secondary_button(ui, "Tour this group").clicked() {
                            *start_tour = Some(tour_index_for(first_view));
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if secondary_button(ui, "Open").clicked() {
                                *open_view = Some(first_view);
                            }
                        });
                    });
                },
            );
        });
}

/// A tiny sidebar drawing for the Tutorial hero. It deliberately shows the
/// three rail sections and highlights Automation & agents → Agent Control.
fn tour_sidebar_sketch(ui: &mut Ui, size: Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 10.0, RAIL);
    p.rect_stroke(rect, 10.0, Stroke::new(1.0, BORDER));

    p.text(
        egui::pos2(rect.left() + 14.0, rect.top() + 16.0),
        Align2::LEFT_CENTER,
        "Proxzy",
        FontId::proportional(13.0),
        TEXT,
    );
    p.text(
        egui::pos2(rect.left() + 14.0, rect.top() + 30.0),
        Align2::LEFT_CENTER,
        "NETWORK INSPECTOR",
        FontId::monospace(6.5),
        FAINT,
    );

    let row = |y: f32, label: &str, active: bool| {
        if active {
            p.rect_filled(
                Rect::from_min_max(
                    egui::pos2(rect.left() + 9.0, y - 8.0),
                    egui::pos2(rect.right() - 9.0, y + 8.0),
                ),
                5.0,
                Color32::from_rgba_unmultiplied(255, 181, 84, 34),
            );
            p.rect_filled(
                Rect::from_min_max(
                    egui::pos2(rect.left() + 9.0, y - 8.0),
                    egui::pos2(rect.left() + 11.0, y + 8.0),
                ),
                1.0,
                AMBER,
            );
        }
        p.circle_stroke(
            egui::pos2(rect.left() + 21.0, y),
            3.2,
            Stroke::new(1.0, if active { AMBER } else { MUTED }),
        );
        p.text(
            egui::pos2(rect.left() + 32.0, y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(8.2),
            if active { TEXT } else { MUTED },
        );
    };

    p.text(
        egui::pos2(rect.left() + 14.0, rect.top() + 48.0),
        Align2::LEFT_CENTER,
        "START HERE",
        FontId::proportional(6.2),
        FAINT,
    );
    row(rect.top() + 63.0, "Tutorial", false);

    p.text(
        egui::pos2(rect.left() + 14.0, rect.top() + 81.0),
        Align2::LEFT_CENTER,
        "TRAFFIC",
        FontId::proportional(6.2),
        FAINT,
    );
    row(rect.top() + 96.0, "All Traffic", false);
    row(rect.top() + 113.0, "GraphQL", false);

    p.text(
        egui::pos2(rect.left() + 14.0, rect.top() + 131.0),
        Align2::LEFT_CENTER,
        "AUTOMATION & AGENTS",
        FontId::proportional(6.2),
        AMBER,
    );
    p.text(
        egui::pos2(rect.right() - 14.0, rect.top() + 131.0),
        Align2::RIGHT_CENTER,
        "HERE",
        FontId::proportional(6.2),
        AMBER,
    );
    row(rect.top() + 146.0, "Agent Control", true);

    p.text(
        egui::pos2(rect.left() + 14.0, rect.bottom() - 12.0),
        Align2::LEFT_CENTER,
        "Monitor · Log Activity · Tools · Mocks",
        FontId::proportional(6.8),
        FAINT,
    );
}

// ---------------------------------------------------------------------------
// Miniature "mock screens" used by the tour card. They are intentionally
// schematic: enough to recognise the page behind the overlay, no real data.
// ---------------------------------------------------------------------------

fn tour_art(ui: &mut Ui, art: TourArt, size: Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 8.0, INPUT);
    p.rect_stroke(rect, 8.0, Stroke::new(1.0, BORDER));

    let inner = rect.shrink(1.0);
    let top_h = 17.0;
    let rail_w = 54.0;
    let mut top = inner;
    top.set_height(top_h);
    p.rect_filled(top, 0.0, RAIL);
    for i in 0..3 {
        p.circle_filled(
            egui::pos2(top.left() + 8.0 + i as f32 * 7.0, top.center().y),
            1.7,
            [RED, AMBER, GREEN][i],
        );
    }
    p.text(
        egui::pos2(top.right() - 8.0, top.center().y),
        Align2::RIGHT_CENTER,
        "PROXZY",
        FontId::monospace(5.8),
        FAINT,
    );

    let mut rail = inner;
    rail.set_top(inner.top() + top_h);
    rail.set_width(rail_w);
    p.rect_filled(rail, 0.0, RAIL);

    let content = Rect::from_min_max(
        egui::pos2(inner.left() + rail_w + 1.0, inner.top() + top_h + 1.0),
        inner.max,
    );
    p.rect_filled(content, 0.0, BG);

    // Rail rows with one highlighted item for the current page.
    let highlight = match art {
        TourArt::Traffic | TourArt::Inspector => 0,
        TourArt::Composer => 1,
        TourArt::MapLocal => 2,
        TourArt::Breakpoint => 3,
        TourArt::Logs => 4,
        TourArt::Agent | TourArt::AgentHook => 5,
        TourArt::Tests => 6,
        TourArt::Screen => 7,
        _ => 9,
    };
    for i in 0..9 {
        let y = rail.top() + 12.0 + i as f32 * 9.5;
        if i == highlight {
            p.rect_filled(
                Rect::from_min_max(
                    egui::pos2(rail.left() + 4.0, y - 4.0),
                    egui::pos2(rail.right() - 4.0, y + 4.0),
                ),
                3.0,
                Color32::from_rgba_unmultiplied(88, 140, 255, 45),
            );
        }
        p.circle_filled(
            egui::pos2(rail.left() + 10.0, y),
            1.3,
            if i == highlight { ACCENT } else { FAINT },
        );
        p.line_segment(
            [
                egui::pos2(rail.left() + 16.0, y),
                egui::pos2(rail.right() - 8.0, y),
            ],
            Stroke::new(1.2, if i == highlight { TEXT } else { MUTED }),
        );
    }
    p.line_segment(
        [
            egui::pos2(rail.left() + 6.0, rail.top() + 5.0),
            egui::pos2(rail.left() + 22.0, rail.top() + 5.0),
        ],
        Stroke::new(2.0, FAINT),
    );

    let x = content.left() + 8.0;
    let y = content.top() + 8.0;
    let w = content.width() - 16.0;

    match art {
        TourArt::Window => {
            p.text(
                egui::pos2(x, y + 4.0),
                Align2::LEFT_CENTER,
                "YOUR NETWORK LAB",
                FontId::proportional(6.5),
                FAINT,
            );
            let card_w = (w - 12.0) / 3.0;
            for (i, (label, color)) in [
                ("TRAFFIC", ACCENT),
                ("MOCK", AMBER),
                ("AGENT", GREEN),
            ]
            .iter()
            .enumerate()
            {
                let r = Rect::from_min_size(
                    egui::pos2(x + i as f32 * (card_w + 6.0), y + 16.0),
                    Vec2::new(card_w, 58.0),
                );
                p.rect_filled(r, 5.0, CARD);
                p.rect_stroke(r, 5.0, Stroke::new(1.0, BORDER));
                p.circle_filled(
                    egui::pos2(r.center().x, r.top() + 16.0),
                    5.0,
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 40),
                );
                p.text(
                    egui::pos2(r.center().x, r.top() + 30.0),
                    Align2::CENTER_CENTER,
                    *label,
                    FontId::proportional(6.5),
                    *color,
                );
                p.line_segment(
                    [
                        egui::pos2(r.left() + 9.0, r.bottom() - 12.0),
                        egui::pos2(r.right() - 9.0, r.bottom() - 12.0),
                    ],
                    Stroke::new(1.0, FAINT),
                );
            }
            p.text(
                egui::pos2(x, y + 86.0),
                Align2::LEFT_CENTER,
                "Start with Traffic. The Agent screen is waiting in the rail.",
                FontId::proportional(6.2),
                MUTED,
            );
            p.line_segment(
                [
                    egui::pos2(x, y + 96.0),
                    egui::pos2(x + 90.0, y + 96.0),
                ],
                Stroke::new(2.0, ACCENT),
            );
        }
        TourArt::Traffic => {
            p.text(
                egui::pos2(x, y + 2.0),
                Align2::LEFT_CENTER,
                "All Traffic",
                FontId::proportional(8.5),
                TEXT,
            );
            for i in 0..3 {
                p.rect_filled(
                    Rect::from_min_size(
                        egui::pos2(x + i as f32 * 31.0, y + 14.0),
                        Vec2::new(27.0, 8.0),
                    ),
                    3.0,
                    if i == 0 { ACCENT } else { CARD },
                );
            }
            for i in 0..5 {
                let ry = y + 30.0 + i as f32 * 17.0;
                p.rect_filled(
                    Rect::from_min_size(egui::pos2(x, ry), Vec2::new(w, 14.0)),
                    3.0,
                    if i == 0 {
                        Color32::from_rgba_unmultiplied(88, 140, 255, 35)
                    } else {
                        CARD
                    },
                );
                let color = [AMBER, GREEN, AMBER, CYAN, PURPLE][i];
                p.rect_filled(
                    Rect::from_min_size(egui::pos2(x + 5.0, ry + 4.0), Vec2::new(17.0, 6.0)),
                    2.0,
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 60),
                );
                p.line_segment(
                    [
                        egui::pos2(x + 28.0, ry + 7.0),
                        egui::pos2(x + w * 0.65, ry + 7.0),
                    ],
                    Stroke::new(1.7, TEXT),
                );
                p.circle_filled(
                    egui::pos2(x + w - 8.0, ry + 7.0),
                    2.5,
                    if i == 2 { RED } else { GREEN },
                );
            }
        }
        TourArt::Inspector => {
            let left_w = w * 0.52;
            p.rect_filled(
                Rect::from_min_size(egui::pos2(x, y), Vec2::new(left_w, 112.0)),
                4.0,
                CARD,
            );
            for i in 0..5 {
                let ry = y + 8.0 + i as f32 * 19.0;
                p.line_segment(
                    [
                        egui::pos2(x + 7.0, ry),
                        egui::pos2(x + left_w - 8.0, ry),
                    ],
                    Stroke::new(1.4, if i == 0 { ACCENT } else { MUTED }),
                );
            }
            let rx = x + left_w + 7.0;
            let right_w = w - left_w - 7.0;
            p.text(
                egui::pos2(rx, y + 1.0),
                Align2::LEFT_CENTER,
                "Inspector",
                FontId::proportional(7.2),
                TEXT,
            );
            p.rect_filled(
                Rect::from_min_size(egui::pos2(rx, y + 11.0), Vec2::new(right_w, 17.0)),
                3.0,
                INPUT,
            );
            for i in 0..3 {
                p.rect_filled(
                    Rect::from_min_size(
                        egui::pos2(rx + i as f32 * (right_w / 3.0), y + 33.0),
                        Vec2::new(right_w / 3.0 - 3.0, 12.0),
                    ),
                    2.0,
                    CARD,
                );
            }
            p.line_segment(
                [
                    egui::pos2(rx, y + 52.0),
                    egui::pos2(rx + right_w, y + 52.0),
                ],
                Stroke::new(1.0, BORDER),
            );
            p.rect_filled(
                Rect::from_min_size(egui::pos2(rx, y + 61.0), Vec2::new(right_w, 32.0)),
                3.0,
                INPUT,
            );
            for i in 0..3 {
                p.line_segment(
                    [
                        egui::pos2(rx + 6.0, y + 69.0 + i as f32 * 7.0),
                        egui::pos2(rx + right_w - 6.0, y + 69.0 + i as f32 * 7.0),
                    ],
                    Stroke::new(1.2, MUTED),
                );
            }
            p.rect_filled(
                Rect::from_min_size(egui::pos2(rx, y + 99.0), Vec2::new(right_w, 13.0)),
                3.0,
                Color32::from_rgba_unmultiplied(88, 140, 255, 70),
            );
            p.text(
                egui::pos2(rx + right_w / 2.0, y + 105.0),
                Align2::CENTER_CENTER,
                "REPEAT",
                FontId::proportional(5.8),
                TEXT,
            );
        }
        TourArt::Composer => {
            p.text(
                egui::pos2(x, y + 2.0),
                Align2::LEFT_CENTER,
                "Composer",
                FontId::proportional(8.5),
                TEXT,
            );
            p.rect_filled(
                Rect::from_min_size(egui::pos2(x, y + 14.0), Vec2::new(w, 14.0)),
                3.0,
                INPUT,
            );
            p.rect_filled(
                Rect::from_min_size(egui::pos2(x + 4.0, y + 17.0), Vec2::new(21.0, 8.0)),
                2.0,
                Color32::from_rgba_unmultiplied(255, 181, 84, 80),
            );
            p.line_segment(
                [
                    egui::pos2(x + 30.0, y + 21.0),
                    egui::pos2(x + w * 0.72, y + 21.0),
                ],
                Stroke::new(1.5, TEXT),
            );
            p.rect_filled(
                Rect::from_min_size(egui::pos2(x + w - 30.0, y + 16.0), Vec2::new(25.0, 10.0)),
                3.0,
                ACCENT,
            );
            let half = (w - 6.0) / 2.0;
            for i in 0..2 {
                p.rect_filled(
                    Rect::from_min_size(
                        egui::pos2(x + i as f32 * (half + 6.0), y + 34.0),
                        Vec2::new(half, 78.0),
                    ),
                    4.0,
                    CARD,
                );
            }
            for i in 0..5 {
                p.line_segment(
                    [
                        egui::pos2(x + 7.0, y + 43.0 + i as f32 * 12.0),
                        egui::pos2(x + half - 8.0, y + 43.0 + i as f32 * 12.0),
                    ],
                    Stroke::new(1.3, if i == 0 { PURPLE } else { MUTED }),
                );
                p.line_segment(
                    [
                        egui::pos2(x + half + 13.0, y + 43.0 + i as f32 * 12.0),
                        egui::pos2(x + w - 8.0, y + 43.0 + i as f32 * 12.0),
                    ],
                    Stroke::new(1.3, if i == 0 { GREEN } else { MUTED }),
                );
            }
        }
        TourArt::MapLocal => {
            p.text(
                egui::pos2(x, y + 2.0),
                Align2::LEFT_CENTER,
                "Map Local",
                FontId::proportional(8.5),
                TEXT,
            );
            for (i, (label, color)) in [("200", GREEN), ("500", RED), ("empty", CYAN)]
                .iter()
                .enumerate()
            {
                let r = Rect::from_min_size(
                    egui::pos2(x + i as f32 * 36.0, y + 13.0),
                    Vec2::new(32.0, 10.0),
                );
                p.rect_filled(
                    r,
                    3.0,
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 45),
                );
                p.text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    *label,
                    FontId::monospace(5.5),
                    *color,
                );
            }
            let row_w = w * 0.67;
            for i in 0..4 {
                let ry = y + 30.0 + i as f32 * 17.0;
                p.rect_filled(
                    Rect::from_min_size(egui::pos2(x, ry), Vec2::new(row_w, 14.0)),
                    3.0,
                    CARD,
                );
                p.circle_filled(
                    egui::pos2(x + 7.0, ry + 7.0),
                    2.2,
                    if i == 0 { GREEN } else { FAINT },
                );
                p.line_segment(
                    [
                        egui::pos2(x + 13.0, ry + 7.0),
                        egui::pos2(x + row_w - 8.0, ry + 7.0),
                    ],
                    Stroke::new(1.4, MUTED),
                );
            }
            p.rect_filled(
                Rect::from_min_size(
                    egui::pos2(x + row_w + 7.0, y + 30.0),
                    Vec2::new(w - row_w - 7.0, 58.0),
                ),
                4.0,
                INPUT,
            );
            p.text(
                egui::pos2(x + row_w + 12.0, y + 38.0),
                Align2::LEFT_CENTER,
                "JSON",
                FontId::monospace(6.0),
                GREEN,
            );
            for i in 0..5 {
                p.line_segment(
                    [
                        egui::pos2(x + row_w + 12.0, y + 47.0 + i as f32 * 7.0),
                        egui::pos2(x + w - 12.0, y + 47.0 + i as f32 * 7.0),
                    ],
                    Stroke::new(1.0, MUTED),
                );
            }
        }
        TourArt::Breakpoint => {
            for i in 0..6 {
                p.line_segment(
                    [
                        egui::pos2(x, y + 5.0 + i as f32 * 14.0),
                        egui::pos2(x + w, y + 5.0 + i as f32 * 14.0),
                    ],
                    Stroke::new(1.4, FAINT),
                );
            }
            p.rect_filled(
                content,
                0.0,
                Color32::from_rgba_unmultiplied(4, 6, 10, 165),
            );
            let modal = Rect::from_min_size(
                egui::pos2(content.center().x - 65.0, y + 22.0),
                Vec2::new(130.0, 84.0),
            );
            p.rect_filled(modal, 6.0, PANEL);
            p.rect_stroke(modal, 6.0, Stroke::new(1.0, AMBER));
            p.circle_filled(
                egui::pos2(modal.left() + 13.0, modal.top() + 13.0),
                3.0,
                AMBER,
            );
            p.text(
                egui::pos2(modal.left() + 21.0, modal.top() + 13.0),
                Align2::LEFT_CENTER,
                "Breakpoint paused",
                FontId::proportional(6.8),
                TEXT,
            );
            p.rect_filled(
                Rect::from_min_size(
                    egui::pos2(modal.left() + 9.0, modal.top() + 23.0),
                    Vec2::new(modal.width() - 18.0, 34.0),
                ),
                3.0,
                INPUT,
            );
            for i in 0..3 {
                p.line_segment(
                    [
                        egui::pos2(modal.left() + 14.0, modal.top() + 30.0 + i as f32 * 8.0),
                        egui::pos2(modal.right() - 14.0, modal.top() + 30.0 + i as f32 * 8.0),
                    ],
                    Stroke::new(1.1, MUTED),
                );
            }
            p.rect_filled(
                Rect::from_min_size(
                    egui::pos2(modal.left() + 9.0, modal.top() + 65.0),
                    Vec2::new(45.0, 12.0),
                ),
                3.0,
                ACCENT,
            );
            p.text(
                egui::pos2(modal.left() + 31.5, modal.top() + 71.0),
                Align2::CENTER_CENTER,
                "Continue",
                FontId::proportional(5.2),
                TEXT,
            );
            p.rect_filled(
                Rect::from_min_size(
                    egui::pos2(modal.left() + 59.0, modal.top() + 65.0),
                    Vec2::new(36.0, 12.0),
                ),
                3.0,
                CARD,
            );
            p.text(
                egui::pos2(modal.left() + 77.0, modal.top() + 71.0),
                Align2::CENTER_CENTER,
                "Abort",
                FontId::proportional(5.2),
                MUTED,
            );
        }
        TourArt::Logs => {
            p.text(
                egui::pos2(x, y + 2.0),
                Align2::LEFT_CENTER,
                "Log Activity",
                FontId::proportional(8.0),
                TEXT,
            );
            let kw = (w - 8.0) / 3.0;
            for i in 0..3 {
                let r = Rect::from_min_size(
                    egui::pos2(x + i as f32 * (kw + 4.0), y + 14.0),
                    Vec2::new(kw, 24.0),
                );
                p.rect_filled(r, 4.0, CARD);
                p.line_segment(
                    [
                        egui::pos2(r.left() + 6.0, r.top() + 8.0),
                        egui::pos2(r.right() - 6.0, r.top() + 8.0),
                    ],
                    Stroke::new(1.5, [ACCENT, RED, CYAN][i]),
                );
                p.line_segment(
                    [
                        egui::pos2(r.left() + 6.0, r.top() + 16.0),
                        egui::pos2(r.right() - 12.0, r.top() + 16.0),
                    ],
                    Stroke::new(1.0, FAINT),
                );
            }
            p.rect_filled(
                Rect::from_min_size(egui::pos2(x, y + 44.0), Vec2::new(w, 32.0)),
                4.0,
                CARD,
            );
            let mut heights = [8.0, 14.0, 9.0, 19.0, 13.0, 23.0, 15.0, 10.0, 18.0];
            for (i, h) in heights.iter_mut().enumerate() {
                p.rect_filled(
                    Rect::from_min_size(
                        egui::pos2(x + 7.0 + i as f32 * 10.0, y + 70.0 - *h),
                        Vec2::new(6.0, *h),
                    ),
                    1.0,
                    Color32::from_rgba_unmultiplied(88, 140, 255, 170),
                );
            }
            for i in 0..3 {
                let ry = y + 83.0 + i as f32 * 12.0;
                p.line_segment(
                    [
                        egui::pos2(x, ry),
                        egui::pos2(x + w, ry),
                    ],
                    Stroke::new(1.1, MUTED),
                );
                p.circle_filled(
                    egui::pos2(x + 4.0, ry),
                    1.6,
                    [GREEN, AMBER, ACCENT][i],
                );
            }
        }
        TourArt::Agent => {
            p.text(
                egui::pos2(x, y + 2.0),
                Align2::LEFT_CENTER,
                "Agent Control",
                FontId::proportional(8.0),
                TEXT,
            );
            let half = (w - 6.0) / 2.0;
            for i in 0..2 {
                p.rect_filled(
                    Rect::from_min_size(
                        egui::pos2(x + i as f32 * (half + 6.0), y + 14.0),
                        Vec2::new(half, 78.0),
                    ),
                    4.0,
                    CARD,
                );
            }
            // Robot head on the right card.
            let cx = x + half + 6.0 + half * 0.5;
            let cy = y + 37.0;
            p.rect_stroke(
                Rect::from_center_size(egui::pos2(cx, cy), Vec2::new(24.0, 18.0)),
                4.0,
                Stroke::new(1.2, GREEN),
            );
            p.circle_filled(egui::pos2(cx - 5.0, cy), 1.5, GREEN);
            p.circle_filled(egui::pos2(cx + 5.0, cy), 1.5, GREEN);
            p.line_segment(
                [
                    egui::pos2(cx - 6.0, cy - 9.0),
                    egui::pos2(cx - 3.0, cy - 13.0),
                ],
                Stroke::new(1.0, GREEN),
            );
            p.line_segment(
                [
                    egui::pos2(cx + 6.0, cy - 9.0),
                    egui::pos2(cx + 3.0, cy - 13.0),
                ],
                Stroke::new(1.0, GREEN),
            );
            // Left card rows.
            for i in 0..3 {
                let ry = y + 22.0 + i as f32 * 17.0;
                p.circle_filled(egui::pos2(x + 8.0, ry), 2.0, if i == 0 { GREEN } else { FAINT });
                p.line_segment(
                    [
                        egui::pos2(x + 14.0, ry),
                        egui::pos2(x + half - 8.0, ry),
                    ],
                    Stroke::new(1.3, MUTED),
                );
                p.rect_filled(
                    Rect::from_min_size(
                        egui::pos2(x + half - 23.0, ry - 4.0),
                        Vec2::new(16.0, 8.0),
                    ),
                    2.0,
                    Color32::from_rgba_unmultiplied(61, 214, 149, 50),
                );
            }
            p.rect_filled(
                Rect::from_min_size(
                    egui::pos2(x + half + 12.0, y + 60.0),
                    Vec2::new(half - 24.0, 14.0),
                ),
                3.0,
                INPUT,
            );
            p.text(
                egui::pos2(x + half + 6.0 + half * 0.5, y + 67.0),
                Align2::CENTER_CENTER,
                "READY",
                FontId::monospace(5.2),
                GREEN,
            );
            p.line_segment(
                [
                    egui::pos2(x + 6.0, y + 98.0),
                    egui::pos2(x + w - 6.0, y + 98.0),
                ],
                Stroke::new(1.0, BORDER),
            );
            for i in 0..2 {
                p.circle_filled(
                    egui::pos2(x + 12.0, y + 105.0 + i as f32 * 7.0),
                    1.5,
                    if i == 0 { GREEN } else { ACCENT },
                );
                p.line_segment(
                    [
                        egui::pos2(x + 18.0, y + 105.0 + i as f32 * 7.0),
                        egui::pos2(x + w * 0.6, y + 105.0 + i as f32 * 7.0),
                    ],
                    Stroke::new(1.0, MUTED),
                );
            }
        }
        TourArt::AgentHook => {
            p.text(
                egui::pos2(x, y + 2.0),
                Align2::LEFT_CENTER,
                "Hook an IDE",
                FontId::proportional(8.0),
                TEXT,
            );
            p.rect_filled(
                Rect::from_min_size(egui::pos2(x, y + 14.0), Vec2::new(w, 70.0)),
                4.0,
                CARD,
            );
            for i in 0..4 {
                let ry = y + 23.0 + i as f32 * 14.0;
                p.text(
                    egui::pos2(x + 7.0, ry),
                    Align2::LEFT_CENTER,
                    ["Cursor", "VS Code", "Claude", "JetBrains"][i],
                    FontId::proportional(5.8),
                    TEXT,
                );
                p.line_segment(
                    [
                        egui::pos2(x + 42.0, ry),
                        egui::pos2(x + w * 0.58, ry),
                    ],
                    Stroke::new(1.0, CYAN),
                );
                p.rect_filled(
                    Rect::from_min_size(
                        egui::pos2(x + w - 42.0, ry - 5.0),
                        Vec2::new(35.0, 10.0),
                    ),
                    2.0,
                    Color32::from_rgba_unmultiplied(88, 140, 255, 80),
                );
                p.text(
                    egui::pos2(x + w - 24.5, ry),
                    Align2::CENTER_CENTER,
                    "Copy",
                    FontId::proportional(4.8),
                    TEXT,
                );
            }
            p.rect_filled(
                Rect::from_min_size(egui::pos2(x, y + 90.0), Vec2::new(w, 14.0)),
                3.0,
                INPUT,
            );
            p.text(
                egui::pos2(x + 6.0, y + 97.0),
                Align2::LEFT_CENTER,
                "curl -s 127.0.0.1/__proxzy/health",
                FontId::monospace(4.9),
                GREEN,
            );
        }
        TourArt::Tests => {
            p.text(
                egui::pos2(x, y + 2.0),
                Align2::LEFT_CENTER,
                "API Test Bench",
                FontId::proportional(8.0),
                TEXT,
            );
            for i in 0..4 {
                let ry = y + 18.0 + i as f32 * 20.0;
                p.rect_filled(
                    Rect::from_min_size(egui::pos2(x, ry), Vec2::new(w, 16.0)),
                    3.0,
                    CARD,
                );
                p.circle_filled(
                    egui::pos2(x + 9.0, ry + 8.0),
                    3.0,
                    if i == 3 { AMBER } else { GREEN },
                );
                p.line_segment(
                    [
                        egui::pos2(x + 17.0, ry + 8.0),
                        egui::pos2(x + w * 0.66, ry + 8.0),
                    ],
                    Stroke::new(1.3, MUTED),
                );
                p.text(
                    egui::pos2(x + w - 8.0, ry + 8.0),
                    Align2::RIGHT_CENTER,
                    if i == 3 { "SKIP" } else { "PASS" },
                    FontId::monospace(4.9),
                    if i == 3 { AMBER } else { GREEN },
                );
            }
            p.rect_filled(
                Rect::from_min_size(egui::pos2(x, y + 100.0), Vec2::new(62.0, 12.0)),
                3.0,
                ACCENT,
            );
            p.text(
                egui::pos2(x + 31.0, y + 106.0),
                Align2::CENTER_CENTER,
                "RUN ALL",
                FontId::proportional(4.8),
                TEXT,
            );
        }
        TourArt::Screen => {
            p.text(
                egui::pos2(x, y + 2.0),
                Align2::LEFT_CENTER,
                "Screen Play",
                FontId::proportional(8.0),
                TEXT,
            );
            let left_w = w * 0.42;
            p.rect_filled(
                Rect::from_min_size(egui::pos2(x, y + 14.0), Vec2::new(left_w, 96.0)),
                4.0,
                CARD,
            );
            for i in 0..4 {
                p.line_segment(
                    [
                        egui::pos2(x + 7.0, y + 24.0 + i as f32 * 17.0),
                        egui::pos2(x + left_w - 8.0, y + 24.0 + i as f32 * 17.0),
                    ],
                    Stroke::new(1.3, if i == 0 { PURPLE } else { MUTED }),
                );
            }
            let rx = x + left_w + 7.0;
            for i in 0..4 {
                let ry = y + 18.0 + i as f32 * 22.0;
                p.circle_filled(
                    egui::pos2(rx + 4.0, ry),
                    2.6,
                    if i < 2 { GREEN } else { FAINT },
                );
                if i < 3 {
                    p.line_segment(
                        [
                            egui::pos2(rx + 4.0, ry + 3.0),
                            egui::pos2(rx + 4.0, ry + 18.0),
                        ],
                        Stroke::new(1.0, FAINT),
                    );
                }
                p.line_segment(
                    [
                        egui::pos2(rx + 12.0, ry),
                        egui::pos2(x + w - 7.0, ry),
                    ],
                    Stroke::new(1.3, MUTED),
                );
            }
            p.circle_filled(egui::pos2(x + w - 16.0, y + 96.0), 8.0, ACCENT);
            p.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(x + w - 19.0, y + 92.0),
                    egui::pos2(x + w - 19.0, y + 100.0),
                    egui::pos2(x + w - 11.0, y + 96.0),
                ],
                Color32::WHITE,
                Stroke::NONE,
            ));
        }
        TourArt::Done => {
            p.circle_filled(
                egui::pos2(content.center().x, y + 32.0),
                19.0,
                Color32::from_rgba_unmultiplied(61, 214, 149, 28),
            );

            p.circle_stroke(
                egui::pos2(content.center().x, y + 32.0),
                14.0,
                Stroke::new(1.6, GREEN),
            );
            p.line_segment(
                [
                    egui::pos2(content.center().x - 6.0, y + 32.0),
                    egui::pos2(content.center().x - 1.0, y + 37.0),
                ],
                Stroke::new(1.8, GREEN),
            );
            p.line_segment(
                [
                    egui::pos2(content.center().x - 1.0, y + 37.0),
                    egui::pos2(content.center().x + 7.0, y + 27.0),
                ],
                Stroke::new(1.8, GREEN),
            );
            p.text(
                egui::pos2(content.center().x, y + 58.0),
                Align2::CENTER_CENTER,
                "TOUR COMPLETE",
                FontId::proportional(7.0),
                GREEN,
            );
            for i in 0..3 {
                let r = Rect::from_min_size(
                    egui::pos2(x + i as f32 * ((w - 8.0) / 3.0 + 4.0), y + 70.0),
                    Vec2::new((w - 8.0) / 3.0, 34.0),
                );
                p.rect_filled(r, 4.0, CARD);
                p.line_segment(
                    [
                        egui::pos2(r.left() + 6.0, r.top() + 10.0),
                        egui::pos2(r.right() - 6.0, r.top() + 10.0),
                    ],
                    Stroke::new(1.4, [ACCENT, AMBER, GREEN][i]),
                );
                p.line_segment(
                    [
                        egui::pos2(r.left() + 6.0, r.top() + 21.0),
                        egui::pos2(r.right() - 12.0, r.top() + 21.0),
                    ],
                    Stroke::new(1.0, MUTED),
                );
            }
        }
    }
}

#[cfg(test)]
mod tour_tests {
    use super::*;

    #[test]
    fn every_stop_has_something_to_say() {
        assert!(TOUR_STEPS.len() >= 10);
        for (index, step) in TOUR_STEPS.iter().enumerate() {
            assert!(!step.section.trim().is_empty(), "stop {} needs a section", index);
            assert!(!step.title.trim().is_empty(), "stop {} needs a title", index);
            assert!(!step.body.trim().is_empty(), "stop {} needs a body", index);
            assert!(
                !step.bullets.is_empty(),
                "stop {} needs at least one bullet",
                index
            );
            assert!(
                !step.where_to.trim().is_empty(),
                "stop {} needs a location line",
                index
            );
        }
    }

    #[test]
    fn the_tour_covers_every_main_view() {
        for view in [
            View::Traffic,
            View::Live,
            View::Composer,
            View::MapLocal,
            View::Breakpoints,
            View::Logs,
            View::Agent,
            View::Tests,
            View::Screen,
            View::Guide,
        ] {
            let index = tour_index_for(view);
            assert!(
                index < TOUR_STEPS.len(),
                "view {:?} has no tour stop",
                view
            );
        }
    }

    #[test]
    fn agent_control_and_its_ide_hook_have_their_own_stops() {
        let agent = tour_index_for(View::Agent);
        assert!(TOUR_STEPS[agent].where_to.contains("Automation"));
        let hook = tour_index_for_art(TourArt::AgentHook);
        assert_eq!(TOUR_STEPS[hook].view, View::Agent);
        assert!(TOUR_STEPS[hook].agent_hook);
        assert_eq!(hook, agent + 1);
    }

    #[test]
    fn jumping_to_a_page_skips_the_welcome_stop() {
        assert_eq!(TOUR_STEPS[tour_index_for(View::Traffic)].art, TourArt::Traffic);
        assert_eq!(TOUR_STEPS[tour_index_for(View::Agent)].art, TourArt::Agent);
        assert_eq!(TOUR_STEPS[tour_index_for(View::Guide)].art, TourArt::Done);
    }
}
