use crate::model::*;
use egui::{
    Color32, FontFamily, FontId, Frame, Margin, Response, RichText, Sense, Stroke, TextStyle, Ui,
    Vec2,
};

pub fn configure(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = INPUT;
    visuals.faint_bg_color = HOVER;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(88, 140, 255, 70);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;
    visuals.widgets.noninteractive.bg_fill = CARD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.inactive.weak_bg_fill = CARD;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.bg_fill = HOVER;
    visuals.widgets.hovered.weak_bg_fill = HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(65, 78, 99));
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.active.bg_fill = HOVER;
    visuals.widgets.active.weak_bg_fill = HOVER;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.striped = true;

    style.visuals = visuals;
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(11.0, 6.0);
    style.spacing.interact_size = Vec2::new(28.0, 26.0);
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating = true;
    style.text_styles = [
        (TextStyle::Heading, FontId::proportional(22.0)),
        (TextStyle::Body, FontId::proportional(13.0)),
        (TextStyle::Monospace, FontId::monospace(12.0)),
        (TextStyle::Button, FontId::proportional(12.5)),
        (TextStyle::Small, FontId::proportional(11.0)),
    ]
    .into();

    ctx.set_style(style);
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// A thin, always-visible scrollbar for lists where the cut edge matters:
/// the left rail, the all-features sheet and the mapping editor. The default
/// macOS-style floating bar is invisible until the pointer is already on it,
/// which makes a clipped list look complete. The handle is kept muted so it
/// reads as an edge, not as a selected row.
pub fn subtle_scroll(ui: &mut Ui) {
    ui.spacing_mut().scroll.floating = false;
    ui.spacing_mut().scroll.foreground_color = false;
    ui.spacing_mut().scroll.bar_width = 6.0;
    ui.spacing_mut().scroll.bar_inner_margin = 2.0;
    ui.visuals_mut().widgets.inactive.bg_fill = Color32::from_rgb(64, 71, 84);
    ui.visuals_mut().widgets.hovered.bg_fill = Color32::from_rgb(92, 103, 122);
    ui.visuals_mut().widgets.active.bg_fill = ACCENT;
}

pub fn card() -> Frame {
    Frame::none()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(12.0)
        .inner_margin(Margin::same(16.0))
}

pub fn input_frame() -> Frame {
    Frame::none()
        .fill(INPUT)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(8.0)
        .inner_margin(Margin::symmetric(10.0, 7.0))
}

pub fn section_label(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .size(10.0)
            .color(FAINT)
            .strong(),
    );
}

pub fn dot(ui: &mut Ui, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter().circle_filled(rect.center(), size * 0.5, color);
}

pub fn pill(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) -> Response {
    Frame::none()
        .fill(bg)
        .rounding(5.0)
        .inner_margin(Margin::symmetric(6.0, 2.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(10.0).color(fg).strong());
        })
        .response
}

pub fn chip(ui: &mut Ui, text: &str, selected: bool) -> Response {
    let (fg, bg, stroke) = if selected {
        (
            Color32::from_rgb(210, 226, 255),
            Color32::from_rgba_unmultiplied(88, 140, 255, 38),
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(88, 140, 255, 120)),
        )
    } else {
        (MUTED, CARD, Stroke::new(1.0, BORDER))
    };
    Frame::none()
        .fill(bg)
        .stroke(stroke)
        .rounding(7.0)
        .inner_margin(Margin::symmetric(10.0, 4.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(fg));
        })
        .response
        .interact(Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(12.0).color(Color32::WHITE).strong())
            .fill(ACCENT)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(0.0, 30.0)),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn secondary_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(11.5).color(TEXT))
            .fill(CARD)
            .stroke(Stroke::new(1.0, BORDER))
            .min_size(Vec2::new(0.0, 27.0)),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn icon_button(ui: &mut Ui, text: &str, tip: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(12.0).color(MUTED))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, Color32::TRANSPARENT))
            .min_size(Vec2::new(28.0, 26.0)),
    )
    .on_hover_text(tip)
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn toggle(ui: &mut Ui, on: bool) -> Response {
    let desired = Vec2::new(36.0, 20.0);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    let t = ui.ctx().animate_bool(resp.id, on);
    let bg = if on {
        Color32::from_rgb(47, 170, 120)
    } else {
        Color32::from_rgb(55, 64, 80)
    };
    ui.painter().rect_filled(rect, 10.0, bg);
    let cx = rect.left() + 10.0 + (rect.width() - 20.0) * t;
    ui.painter()
        .circle_filled(egui::pos2(cx, rect.center().y), 7.0, Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn hsep(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, BORDER));
}

pub fn vsep(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, height), Sense::hover());
    ui.painter().vline(rect.center().x, rect.y_range(), Stroke::new(1.0, BORDER));
}

pub fn search_box(ui: &mut Ui, query: &mut String, width: f32) {
    Frame::none()
        .fill(INPUT)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(8.0)
        .inner_margin(Margin::symmetric(10.0, 5.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                let c = MUTED;
                let p = ui.painter();
                p.circle_stroke(rect.center() + Vec2::new(-1.0, -1.0), 4.5, Stroke::new(1.3, c));
                p.line_segment(
                    [
                        rect.center() + Vec2::new(2.2, 2.2),
                        rect.center() + Vec2::new(5.0, 5.0),
                    ],
                    Stroke::new(1.3, c),
                );
                ui.add(
                    egui::TextEdit::singleline(query)
                        .hint_text(RichText::new("Search URL, method, status").color(FAINT))
                        .frame(false)
                        .desired_width(width)
                        .text_color(TEXT)
                        .font(TextStyle::Body),
                );
            });
        });
}

pub fn code_block(ui: &mut Ui, text: &str, max_height: f32, highlight_json: bool) {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    let salt = hasher.finish();
    Frame::none()
        .fill(INPUT)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(9.0)
        .inner_margin(Margin::same(12.0))
        .show(ui, |ui| {
            egui::ScrollArea::both()
                .id_salt(("code-block", salt))
                .max_height(max_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    if highlight_json {
                        json_label(ui, text);
                    } else {
                        ui.add(
                            egui::Label::new(RichText::new(text).font(mono(11.5)).color(TEXT))
                                .selectable(true),
                        );
                    }
                });
        });
}

pub fn json_label(ui: &mut Ui, text: &str) {
    let mut job = egui::text::LayoutJob::default();
    let base = egui::TextFormat {
        font_id: mono(11.5),
        color: Color32::from_rgb(190, 200, 216),
        ..Default::default()
    };
    let mut chars = text.char_indices().peekable();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if ch == '"' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let s = &text[start..i];
            let is_key = text[i..]
                .trim_start()
                .starts_with(':');
            let color = if is_key {
                Color32::from_rgb(126, 178, 255)
            } else {
                Color32::from_rgb(150, 220, 165)
            };
            job.append(s, 0.0, egui::TextFormat { color, ..base.clone() });
        } else if ch.is_ascii_digit()
            || (ch == '-' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit())
        {
            let start = i;
            i += 1;
            while i < bytes.len()
                && ((bytes[i] as char).is_ascii_digit()
                    || bytes[i] == b'.'
                    || bytes[i] == b'e'
                    || bytes[i] == b'E'
                    || bytes[i] == b'-'
                    || bytes[i] == b'+')
            {
                i += 1;
            }
            job.append(
                &text[start..i],
                0.0,
                egui::TextFormat {
                    color: AMBER,
                    ..base.clone()
                },
            );
        } else if text[i..].starts_with("true")
            || text[i..].starts_with("false")
            || text[i..].starts_with("null")
        {
            let word = if text[i..].starts_with("true") {
                "true"
            } else if text[i..].starts_with("false") {
                "false"
            } else {
                "null"
            };
            job.append(
                word,
                0.0,
                egui::TextFormat {
                    color: PURPLE,
                    ..base.clone()
                },
            );
            i += word.len();
        } else {
            let start = i;
            i += 1;
            while i < bytes.len()
                && bytes[i] != b'"'
                && !(bytes[i] as char).is_ascii_digit()
                && !text[i..].starts_with("true")
                && !text[i..].starts_with("false")
                && !text[i..].starts_with("null")
            {
                i += 1;
            }
            job.append(
                &text[start..i],
                0.0,
                egui::TextFormat {
                    color: Color32::from_rgb(128, 140, 158),
                    ..base.clone()
                },
            );
        }
        let _ = chars.next();
    }
    job.wrap.max_width = f32::INFINITY;
    let galley = ui.fonts(|f| f.layout_job(job));
    ui.add(egui::Label::new(galley).selectable(true));
}

pub fn markup_code_block(ui: &mut Ui, text: &str, max_height: f32) {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    let salt = hasher.finish();
    Frame::none()
        .fill(INPUT)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(9.0)
        .inner_margin(Margin::same(12.0))
        .show(ui, |ui| {
            egui::ScrollArea::both()
                .id_salt(("markup-block", salt))
                .max_height(max_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    markup_label(ui, text);
                });
        });
}

pub fn markup_label(ui: &mut Ui, text: &str) {
    let mut job = egui::text::LayoutJob::default();
    let base = egui::TextFormat {
        font_id: mono(11.5),
        color: Color32::from_rgb(190, 200, 216),
        ..Default::default()
    };
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b'>' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            job.append(
                &text[start..i],
                0.0,
                egui::TextFormat {
                    color: Color32::from_rgb(126, 178, 255),
                    ..base.clone()
                },
            );
        } else {
            let start = i;
            while i < bytes.len() && bytes[i] != b'<' {
                i += 1;
            }
            job.append(
                &text[start..i],
                0.0,
                egui::TextFormat {
                    color: Color32::from_rgb(174, 190, 210),
                    ..base.clone()
                },
            );
        }
    }
    job.wrap.max_width = f32::INFINITY;
    let galley = ui.fonts(|f| f.layout_job(job));
    ui.add(egui::Label::new(galley).selectable(true));
}

pub fn kv(ui: &mut Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [150.0, 18.0],
            egui::Label::new(RichText::new(key).size(11.5).color(MUTED)),
        );
        ui.add(
            egui::Label::new(RichText::new(value).size(12.0).color(TEXT)).selectable(true),
        );
    });
}

pub fn draw_mark(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter().rect_filled(rect, 8.0, Color32::from_rgb(34, 48, 76));
    ui.painter().rect_stroke(
        rect,
        8.0,
        Stroke::new(1.0, Color32::from_rgb(58, 83, 130)),
    );
    let p = ui.painter();
    let c = Color32::from_rgb(120, 166, 255);
    let y = rect.center().y;
    let x = rect.left() + size * 0.27;
    for i in 0..2 {
        let off = i as f32 * 7.0;
        p.line_segment(
            [
                egui::pos2(x + off, y - 5.0),
                egui::pos2(x + 5.0 + off, y),
            ],
            Stroke::new(2.0, c),
        );
        p.line_segment(
            [
                egui::pos2(x + 5.0 + off, y),
                egui::pos2(x + off, y + 5.0),
            ],
            Stroke::new(2.0, c),
        );
    }
}

#[derive(Clone, Copy)]
pub enum NavIcon {
    Traffic,
    Composer,
    Map,
    Breakpoint,
    Logs,
    Graph,
    Rest,
    Error,
    Pin,
    Live,
    Agent,
    Tests,
    Play,
    Shield,
    Help,
    Compass,
    Bridge,
}

pub fn paint_nav_icon(painter: &egui::Painter, rect: egui::Rect, kind: NavIcon, color: Color32) {
    let p = painter;
    let r = rect.shrink(2.0);
    let stroke = Stroke::new(1.35, color);
    match kind {
        NavIcon::Traffic => {
            for i in 0..3 {
                let y = r.top() + 3.0 + i as f32 * 5.0;
                p.circle_filled(egui::pos2(r.left() + 1.5, y), 1.3, color);
                p.line_segment(
                    [egui::pos2(r.left() + 5.0, y), egui::pos2(r.right(), y)],
                    stroke,
                );
            }
        }
        NavIcon::Composer => {
            p.rect_stroke(
                egui::Rect::from_min_max(
                    egui::pos2(r.left(), r.top() + 4.0),
                    egui::pos2(r.right() - 4.0, r.bottom()),
                ),
                3.0,
                stroke,
            );
            p.line_segment(
                [
                    egui::pos2(r.left() + 2.0, r.top() + 2.0),
                    egui::pos2(r.right() - 1.0, r.top() + 2.0),
                ],
                stroke,
            );
        }
        NavIcon::Map => {
            p.rect_stroke(
                egui::Rect::from_min_max(
                    egui::pos2(r.left(), r.top()),
                    egui::pos2(r.right() - 5.0, r.bottom() - 5.0),
                ),
                2.5,
                stroke,
            );
            p.rect_stroke(
                egui::Rect::from_min_max(
                    egui::pos2(r.left() + 5.0, r.top() + 5.0),
                    egui::pos2(r.right(), r.bottom()),
                ),
                2.5,
                stroke,
            );
        }
        NavIcon::Breakpoint => {
            p.circle_stroke(r.center(), 5.5, stroke);
            p.circle_filled(r.center(), 2.0, color);
        }
        NavIcon::Logs => {
            for i in 0..3 {
                let y = r.top() + 4.0 + i as f32 * 5.0;
                let right = if i == 1 { r.right() - 4.0 } else { r.right() };
                p.line_segment(
                    [egui::pos2(r.left(), y), egui::pos2(right, y)],
                    stroke,
                );
            }
        }
        NavIcon::Graph => {
            let a = egui::pos2(r.left() + 2.0, r.bottom() - 3.0);
            let b = egui::pos2(r.left() + 7.0, r.top() + 3.0);
            let c = egui::pos2(r.right() - 2.0, r.bottom() - 4.0);
            p.line_segment([a, b], stroke);
            p.line_segment([b, c], stroke);
            p.circle_filled(a, 1.8, color);
            p.circle_filled(b, 1.8, color);
            p.circle_filled(c, 1.8, color);
        }
        NavIcon::Rest => {
            p.rect_stroke(
                egui::Rect::from_min_max(
                    egui::pos2(r.left() + 1.0, r.top() + 3.0),
                    egui::pos2(r.right() - 1.0, r.bottom() - 3.0),
                ),
                3.0,
                stroke,
            );
        }
        NavIcon::Error => {
            p.circle_stroke(r.center(), 6.0, stroke);
            p.line_segment(
                [
                    r.center() + Vec2::new(-2.5, -2.5),
                    r.center() + Vec2::new(2.5, 2.5),
                ],
                stroke,
            );
            p.line_segment(
                [
                    r.center() + Vec2::new(2.5, -2.5),
                    r.center() + Vec2::new(-2.5, 2.5),
                ],
                stroke,
            );
        }
        NavIcon::Pin => {
            p.line_segment(
                [
                    r.center() + Vec2::new(-3.0, -5.0),
                    r.center() + Vec2::new(3.0, -5.0),
                ],
                stroke,
            );
            p.line_segment(
                [
                    r.center() + Vec2::new(-2.0, -5.0),
                    r.center() + Vec2::new(-1.0, 0.0),
                ],
                stroke,
            );
            p.line_segment(
                [
                    r.center() + Vec2::new(2.0, -5.0),
                    r.center() + Vec2::new(1.0, 0.0),
                ],
                stroke,
            );
            p.line_segment(
                [
                    r.center() + Vec2::new(0.0, 0.0),
                    r.center() + Vec2::new(0.0, 6.0),
                ],
                stroke,
            );
        }
        NavIcon::Live => {
            p.circle_stroke(r.center(), 6.4, stroke);
            p.circle_filled(r.center(), 2.2, color);
            p.line_segment(
                [
                    r.center() + Vec2::new(5.6, -5.6),
                    r.center() + Vec2::new(8.6, -8.6),
                ],
                stroke,
            );
        }
        NavIcon::Agent => {
            // A small robot head with two antennas.
            let head = egui::Rect::from_min_max(
                egui::pos2(r.left() + 2.0, r.top() + 5.0),
                egui::pos2(r.right() - 2.0, r.bottom() - 1.0),
            );
            p.rect_stroke(head, 3.0, stroke);
            p.circle_filled(egui::pos2(head.left() + 3.5, head.center().y), 1.2, color);
            p.circle_filled(egui::pos2(head.right() - 3.5, head.center().y), 1.2, color);
            p.line_segment(
                [
                    egui::pos2(head.left() + 3.0, r.top() + 5.0),
                    egui::pos2(head.left() + 4.5, r.top() + 1.5),
                ],
                stroke,
            );
            p.line_segment(
                [
                    egui::pos2(head.right() - 3.0, r.top() + 5.0),
                    egui::pos2(head.right() - 4.5, r.top() + 1.5),
                ],
                stroke,
            );
        }
        NavIcon::Tests => {
            // A clipboard with a check mark.
            p.rect_stroke(
                egui::Rect::from_min_max(
                    egui::pos2(r.left() + 1.5, r.top() + 2.0),
                    egui::pos2(r.right() - 1.5, r.bottom()),
                ),
                2.5,
                stroke,
            );
            p.line_segment(
                [
                    egui::pos2(r.left() + 5.5, r.center().y + 1.0),
                    egui::pos2(r.center().x, r.center().y + 4.0),
                ],
                stroke,
            );
            p.line_segment(
                [
                    egui::pos2(r.center().x, r.center().y + 4.0),
                    egui::pos2(r.right() - 4.0, r.center().y - 3.0),
                ],
                stroke,
            );
        }
        NavIcon::Play => {
            p.rect_stroke(
                egui::Rect::from_min_max(
                    egui::pos2(r.left(), r.top() + 1.0),
                    egui::pos2(r.right(), r.bottom() - 1.0),
                ),
                3.0,
                stroke,
            );
            let a = egui::pos2(r.left() + 6.0, r.top() + 5.0);
            let b = egui::pos2(r.left() + 6.0, r.bottom() - 5.0);
            let c = egui::pos2(r.right() - 5.0, r.center().y);
            p.line_segment([a, b], stroke);
            p.line_segment([a, c], stroke);
            p.line_segment([b, c], stroke);
        }
        NavIcon::Shield => {
            let top = egui::pos2(r.center().x, r.top() + 0.5);
            let left = egui::pos2(r.left() + 0.5, r.top() + 3.0);
            let right = egui::pos2(r.right() - 0.5, r.top() + 3.0);
            let bottom_left = egui::pos2(r.left() + 2.5, r.bottom() - 1.0);
            let bottom_right = egui::pos2(r.right() - 2.5, r.bottom() - 1.0);
            p.add(egui::Shape::closed_line(
                vec![top, right, bottom_right, bottom_left, left],
                stroke,
            ));
            p.circle_filled(r.center() + Vec2::new(0.0, -0.5), 1.6, color);
            p.line_segment(
                [
                    r.center() + Vec2::new(0.0, 1.0),
                    r.center() + Vec2::new(0.0, 3.5),
                ],
                stroke,
            );
        }
        NavIcon::Help => {
            p.circle_stroke(r.center(), 6.5, stroke);
            p.text(
                r.center() + Vec2::new(0.0, 0.5),
                egui::Align2::CENTER_CENTER,
                "?",
                FontId::proportional(9.0),
                color,
            );
        }
        NavIcon::Compass => {
            p.circle_stroke(r.center(), 6.5, stroke);
            let north = r.center() + Vec2::new(1.6, -4.5);
            let south = r.center() + Vec2::new(-1.6, 4.5);
            let east = r.center() + Vec2::new(4.5, 1.6);
            let west = r.center() + Vec2::new(-4.5, -1.6);
            p.add(egui::Shape::closed_line(
                vec![north, east, south, west],
                Stroke::new(1.1, color),
            ));
        }
        NavIcon::Bridge => {
            // Two endpoints with a double line between them: the adapter.
            let left = r.center() + Vec2::new(-5.4, 0.0);
            let right = r.center() + Vec2::new(5.4, 0.0);
            p.circle_stroke(left, 2.7, stroke);
            p.circle_stroke(right, 2.7, stroke);
            p.line_segment(
                [
                    left + Vec2::new(2.7, -1.3),
                    right + Vec2::new(-2.7, -1.3),
                ],
                stroke,
            );
            p.line_segment(
                [
                    left + Vec2::new(2.7, 1.3),
                    right + Vec2::new(-2.7, 1.3),
                ],
                stroke,
            );
        }
    }
}

pub fn wave_badge(ui: &mut Ui, status: u16) {
    if status == 0 {
        pill(
            ui,
            "ERR",
            RED,
            Color32::from_rgba_unmultiplied(255, 107, 107, 26),
        );
    } else {
        let c = status_color(status);
        pill(
            ui,
            &status.to_string(),
            c,
            Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 24),
        );
    }
}
