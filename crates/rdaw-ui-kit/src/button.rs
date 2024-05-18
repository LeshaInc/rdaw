use std::fmt::Display;

use floem::style::{self, CursorStyle, Style, Transition};
use floem::views::{self, Decorators};
use floem::IntoView;

use super::{ColorKind, Level, Theme};

floem::style_class!(pub ButtonClass);

pub fn button<S: Display + 'static>(
    color: ColorKind,
    level: Level,
    label: impl Fn() -> S + 'static,
) -> impl IntoView {
    views::container(views::label(label))
        .keyboard_navigatable()
        .class(ButtonClass)
        .style(move |s| add_style(color, level, s))
}

fn add_style(color: ColorKind, level: Level, s: Style) -> Style {
    let theme = Theme::get();
    let colors = theme.colors[color][level];
    s.cursor(CursorStyle::Pointer)
        .border(1)
        .border_radius(4)
        .padding_horiz(10)
        .padding_vert(4)
        .margin(3)
        .font_family(theme.fonts.normal.m.family.clone())
        .font_size(theme.fonts.normal.m.size)
        .transition(style::TextColor, Transition::linear(0.1))
        .transition(style::Background, Transition::linear(0.1))
        .transition(style::BorderColor, Transition::linear(0.1))
        .transition(style::OutlineColor, Transition::linear(0.1))
        .focus(|s| s.outline(2))
        .background(colors.bg)
        .color(colors.fg)
        .border_color(colors.border)
        .outline_color(colors.border)
        .hover(|s| {
            s.color(colors.fg_hover)
                .background(colors.bg_hover)
                .border_color(colors.border_hover)
                .outline_color(colors.border_hover)
        })
        .active(|s| {
            s.color(colors.fg_active)
                .background(colors.bg_active)
                .border_color(colors.border_active)
                .outline_color(colors.border_active)
        })
}
