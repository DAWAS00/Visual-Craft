//! Widgets the typed diagram panels share (Use Case and Class): picture tiles for tools, list rows,
//! small picture buttons, labelled fields, the resize grip, and the canvas's hint chip and dashed
//! box. Pictures are passed in as a drawing function, so each diagram keeps its own glyphs.

use egui::{Align2, Color32, CornerRadius, Id, Painter, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::theme::{Tokens, regular, semibold};

/// Draws a picture filling a box in an ink colour.
pub trait Picture: Fn(&Painter, Rect, Color32) {}
impl<F: Fn(&Painter, Rect, Color32)> Picture for F {}

/// A section heading.
pub fn section(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.add_space(12.0);
    ui.label(RichText::new(text).font(semibold(11.0)).color(t.text));
    ui.add_space(4.0);
}

/// Secondary text: a hint, an empty state or a field's label.
pub fn note(ui: &mut Ui, t: &Tokens, text: &str, size: f32) {
    ui.label(RichText::new(text).font(regular(size)).color(t.text_secondary));
}

/// A tool: its picture over its name; outlined in the accent colour while it is the tool.
pub fn tile(ui: &mut Ui, t: &Tokens, picture: impl Picture, label: &str, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(68.0, 60.0), Sense::click());
    let p = ui.painter();
    let fill = if on {
        t.accent.gamma_multiply(0.18)
    } else if resp.hovered() {
        t.hover
    } else {
        t.field
    };
    p.rect_filled(rect, CornerRadius::same(8), fill);
    if on {
        p.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.5, t.accent), StrokeKind::Inside);
    }
    let ink = if on { t.accent_text } else { t.icon };
    picture(p, Rect::from_center_size(pos2(rect.center().x, rect.min.y + 23.0), vec2(30.0, 30.0)), ink);
    p.text(pos2(rect.center().x, rect.max.y - 10.0), Align2::CENTER_CENTER, label, regular(10.0), if on { t.accent_text } else { t.text });
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, label));
    resp
}

/// A small picture button (remove, connect from here), named for screen readers.
pub fn icon_button(ui: &mut Ui, t: &Tokens, picture: impl Picture, name: &str, danger: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(5), t.hover);
    }
    let ink = if danger && resp.hovered() { t.danger } else { t.icon };
    picture(ui.painter(), rect.shrink(3.0), ink);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    resp.on_hover_text(name)
}

/// A list row: picture and text across the panel; highlighted when selected.
pub fn row(ui: &mut Ui, t: &Tokens, picture: impl Picture, text: &str, selected: bool, indent: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
    let p = ui.painter_at(rect);
    if selected {
        p.rect_filled(rect, CornerRadius::same(5), t.selected_row);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(5), t.hover);
    }
    let g = Rect::from_center_size(pos2(rect.min.x + indent + 12.0, rect.center().y), vec2(18.0, 18.0));
    picture(&p, g, t.icon);
    p.text(pos2(g.max.x + 8.0, rect.center().y), Align2::LEFT_CENTER, text, regular(11.0), t.text);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, selected, text));
    resp
}

/// A labelled text field: the new text when it is committed changed.
pub fn field(ui: &mut Ui, t: &Tokens, label: &str, id: Id, value: &str) -> Option<String> {
    note(ui, t, label, 10.0);
    let w = ui.available_width();
    let new = crate::widgets::text_field_named(ui, id, value, w, label);
    new.filter(|n| n.trim() != value)
}

/// The resize grip in a panel's bottom-right corner. A drag changes the panel's size; the result
/// is the size after this frame's drag, kept within `min` and `max`.
pub fn grip(ui: &mut Ui, t: &Tokens, id: Id, size: egui::Vec2, min: egui::Vec2, max: egui::Vec2) -> egui::Vec2 {
    const SIDE: f32 = 16.0;
    let corner = ui.max_rect().max;
    let rect = Rect::from_min_max(corner - vec2(SIDE, SIDE), corner);
    let resp = ui.interact(rect, id, Sense::drag());
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
    }
    let painter = ui.painter();
    for k in [4.0, 8.0] {
        painter.line_segment([corner - vec2(k, 2.0), corner - vec2(2.0, k)], Stroke::new(1.0, t.icon));
    }
    (size + resp.drag_delta()).clamp(min, max)
}

/// A floating panel's size: the stored one (or `default`), kept on screen; and where it opens.
pub fn panel_size(ctx: &egui::Context, size_id: Id, default_w: f32) -> (egui::Vec2, egui::Vec2, egui::Vec2, Pos2) {
    let content = ctx.content_rect();
    let min = vec2(180.0, 200.0);
    let max = vec2((content.width() - 32.0).max(min.x), (content.height() - 120.0).max(min.y));
    let stored = ctx.data(|d| d.get_temp::<egui::Vec2>(size_id));
    let size = stored.unwrap_or_else(|| vec2(default_w, (content.height() - 220.0).max(160.0))).clamp(min, max);
    let start = pos2((content.right() - size.x - 16.0).max(content.left()), content.top() + 96.0);
    (size, min, max, start)
}

/// A small label beside the pointer, outlined in `ink`: what a press or a drop there does.
pub fn chip(painter: &Painter, at: Pos2, text: String, ink: Color32) {
    let t = Tokens::get(painter.ctx());
    let galley = painter.layout_no_wrap(text, regular(11.0), t.text);
    let r = Rect::from_min_size(at + vec2(14.0, 14.0), galley.size() + vec2(12.0, 6.0));
    painter.rect_filled(r, CornerRadius::same(6), t.panel);
    painter.rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, ink), StrokeKind::Inside);
    painter.galley(r.min + vec2(6.0, 3.0), galley, t.text);
}

/// A dashed outline: a ghost while dragging, where a placed element would go.
pub fn dashed_box(p: &Painter, r: Rect, ink: Color32) {
    let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    p.extend(Shape::dashed_line(&corners, Stroke::new(1.5, ink), 6.0, 4.0));
}
