// The sidebar every app hub shares, laid out as the Forms one (the design the
// others follow): the hub's name and its «, a search, the filters with their
// counts, a line, then a heading with a + over the hub's own items. A hub
// makes new things from that +, never from a filled button by its name.

use super::*;

/// What a filter row shows before its name.
#[derive(Clone, Copy)]
pub(crate) enum FilterIcon {
    /// My avatar.
    Me,
    /// A star.
    Star,
    Line(Icon),
}

/// A filter row: the value it sets, its icon, its English name (shown
/// translated) and how many items it holds (none for a page that is not a
/// list, like Teams' Org Chart).
pub(crate) struct SideFilter<'a> {
    pub key: u8,
    pub icon: FilterIcon,
    pub label: &'a str,
    pub count: Option<usize>,
}

impl App {
    /// The hub sidebar's panel, with the id and width of Home's so one
    /// dragged width applies everywhere; closed, it leaves the "»" strip.
    pub(crate) fn hub_side_panel(&mut self, ui: &mut Ui, body: impl FnOnce(&mut Self, &mut Ui)) {
        if self.sidebar_collapsed {
            self.collapsed_sidebar(ui);
            return;
        }
        let t = self.t();
        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(255.0)
            .width_range(200.0..=400.0)
            .frame(side_panel_frame(&t, egui::Margin::symmetric(8, 6)))
            .show_inside(ui, |ui| body(self, ui));
    }

    /// The top row: the hub's name (translated) and the « that closes the
    /// sidebar, as Cmd+\ does.
    pub(crate) fn side_title(&mut self, ui: &mut Ui, title: &str) {
        let t = self.t();
        if side_title_row(ui, &t, title, true) {
            self.sidebar_collapsed = true;
        }
    }

    /// The filter rows; returns the key of the one clicked.
    pub(crate) fn side_filters(
        &self,
        ui: &mut Ui,
        filters: &[SideFilter],
        selected: Option<u8>,
    ) -> Option<u8> {
        let t = self.t();
        let mut out = None;
        for f in filters {
            let sel = selected == Some(f.key);
            let (r, resp) = side_row(ui, &t, f.label, sel, 30.0);
            let ic = r.left_center() + vec2(16.0, 0.0);
            match f.icon {
                FilterIcon::Me => {
                    if let Some(m) = self.data.member(self.data.me) {
                        paint_avatar(ui.painter(), ic, 8.0, m, t.sidebar);
                    }
                }
                FilterIcon::Star => {
                    ui.painter().text(
                        ic,
                        Align2::CENTER_CENTER,
                        "\u{2606}",
                        FontId::proportional(15.0),
                        t.text2,
                    );
                }
                FilterIcon::Line(icon) => {
                    line_icon(ui.painter(), icon, ic, t.text2);
                }
            }
            ui.painter().text(
                r.left_center() + vec2(34.0, 0.0),
                Align2::LEFT_CENTER,
                tr(f.label),
                FontId::proportional(14.0),
                if sel { t.text } else { t.text2 },
            );
            if let Some(n) = f.count {
                ui.painter().text(
                    r.right_center() - vec2(10.0, 0.0),
                    Align2::RIGHT_CENTER,
                    n.to_string(),
                    FontId::proportional(12.0),
                    t.text3,
                );
            }
            if resp.clicked() {
                out = Some(f.key);
            }
        }
        out
    }
}

/// The top row of a sidebar or list panel: its name (translated) and, when
/// it `closes`, a « at the right. Returns whether the « was clicked.
pub(crate) fn side_title_row(ui: &mut Ui, t: &Theme, title: &str, closes: bool) -> bool {
    let w = ui.available_width();
    let (hr, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
    ui.painter().text(
        hr.left_center() + vec2(6.0, 0.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(17.0),
        t.text,
    );
    let mut clicked = false;
    if closes {
        let cr = Rect::from_center_size(pos2(hr.right() - 14.0, hr.center().y), vec2(28.0, 28.0));
        let close = ui.interact(cr, ui.id().with("side_collapse"), Sense::click());
        name_it(&close, "Close sidebar");
        if close.hovered() {
            ui.painter().rect_filled(cr, 6.0, t.row_selected);
        }
        ui.painter().text(
            cr.center(),
            Align2::CENTER_CENTER,
            "\u{00AB}",
            FontId::proportional(16.0),
            t.text2,
        );
        clicked = close.on_hover_text(tr("Close sidebar")).clicked();
    }
    ui.add_space(4.0);
    clicked
}

/// The search under the title, its magnifier inside the field: `hint` is
/// shown (translated), `name` is for screen readers and tests.
pub(crate) fn side_search(ui: &mut Ui, t: &Theme, text: &mut String, hint: &str, name: &str) {
    let r = ui.add(
        field(text)
            .margin(egui::Margin {
                left: 28,
                right: 8,
                top: 5,
                bottom: 5,
            })
            .hint_text(hint)
            .desired_width(f32::INFINITY),
    );
    name_field(&r, name);
    line_icon_scaled(
        ui.painter(),
        Icon::Search,
        // (a text field's response is its text area, inside the margin)
        pos2(r.rect.left() - 13.0, r.rect.center().y),
        t.text3,
        0.85,
    );
    ui.add_space(6.0);
}

/// The line between the filters and the items.
pub(crate) fn side_rule(ui: &mut Ui, t: &Theme) {
    ui.add_space(6.0);
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(
        r.left() + 6.0..=r.right() - 6.0,
        r.top(),
        Stroke::new(1.0_f32, t.border),
    );
    ui.add_space(8.0);
}

/// A heading over items (translated), with a + at its end that makes a new
/// one: `add` is the +'s name for screen readers and tests and its tip
/// (translated); none when this person may not make one. Returns whether
/// the + was clicked.
pub(crate) fn side_heading(
    ui: &mut Ui,
    t: &Theme,
    heading: &str,
    add: Option<(&str, &str)>,
) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        // as tall with or without the +, so the rows under it don't move
        ui.set_min_height(crate::widgets::CONTROL_H);
        ui.add_space(8.0);
        ui.label(RichText::new(heading).size(12.0).strong().color(t.text2));
        if let Some((name, tip)) = add {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                clicked = icon_button(ui, t, Icon::Plus, "", name, false)
                    .on_hover_text(tip)
                    .clicked();
            });
        }
    });
    clicked
}

/// What a list says when it has nothing to show (translated), wrapped when
/// the sidebar is narrow.
pub(crate) fn side_empty(ui: &mut Ui, t: &Theme, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.add(egui::Label::new(RichText::new(text).size(13.0).color(t.text3)).wrap());
    });
}

/// One row of the sidebar, a filter or an item: `h` tall, named `name`,
/// filled while it is the one shown or under the pointer.
pub(crate) fn side_row(
    ui: &mut Ui,
    t: &Theme,
    name: &str,
    sel: bool,
    h: f32,
) -> (Rect, egui::Response) {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    name_it(&resp, name);
    if sel {
        ui.painter().rect_filled(r, 7.0, t.row_selected);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 7.0, t.hover);
    }
    (r, resp)
}

/// An item row's name after its icon, shortened to fit before `right`.
pub(crate) fn side_row_text(ui: &Ui, t: &Theme, r: Rect, text: &str, sel: bool, right: f32) {
    let font = FontId::proportional(14.0);
    let left = r.left() + 34.0;
    let shown = fit_text(ui.painter(), text, &font, (right - left).max(10.0));
    ui.painter().text(
        pos2(left, r.center().y),
        Align2::LEFT_CENTER,
        shown,
        font,
        if sel { t.text } else { t.text2 },
    );
}
