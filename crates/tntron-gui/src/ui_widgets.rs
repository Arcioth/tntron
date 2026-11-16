use iced::widget::{
    button, column, container, image, progress_bar, row, slider, space, stack, text, text_input,
    tooltip, Column, Container, Row,
};
use iced::{Alignment, Background, Border, Color, ContentFit, Element, Length, Shadow, Theme, Vector};
use std::collections::HashMap;
use tntron_core::ExtWidget;

use crate::button_style::{compute_button_style, wrap_chrome, ButtonKind};
use crate::gif_viz::GifVizView;
use crate::icons::{self, resolve_icon};
use crate::theme::ThemeSpec;
use crate::visualizer::{visualizer_canvas, VizFrameData};

#[derive(Clone)]
pub struct ExtVizCtx<'a, Message: Clone> {
    pub frame: VizFrameData,
    pub gif: GifVizView<'a>,
    pub on_next_gif: Message,
}

pub fn openify_panel<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    theme: &ThemeSpec,
) -> Container<'a, Message> {
    let bg = theme.surface;
    let border = theme.border;
    container(content).style(move |_| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: border,
            width: 0.5,
            radius: 12.0.into(),
        },
        ..Default::default()
    })
}

pub fn card_box<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    theme: &ThemeSpec,
    active_border: bool,
) -> Container<'a, Message> {
    let bg = theme.card;
    let bcol = if active_border { theme.accent } else { theme.border };
    container(content).style(move |_| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: bcol,
            width: if active_border { 1.4 } else { 1.0 },
            radius: 10.0.into(),
        },
        ..Default::default()
    })
}

pub fn slot_badge<'a, Message: Clone + 'a>(
    label: impl iced::widget::text::IntoFragment<'a>,
    icon_name: Option<&'a str>,
    accent: bool,
    theme: &ThemeSpec,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let bg = if accent { theme.accent } else { theme.card };
    let fg = if accent { theme.accent_fg } else { theme.text };
    let border = if accent { theme.accent } else { theme.border };
    let hover_bg = theme.card_hover;

    let inner: Element<'a, Message> = if let Some(iname) = icon_name {
        row![
            icons::icon(resolve_icon(iname), 11.0, fg),
            text(label)
                .size(11)
                .color(fg)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
    } else {
        text(label)
            .size(11)
            .color(fg)
            .wrapping(iced::widget::text::Wrapping::None)
            .into()
    };

    if let Some(msg) = on_press {
        let th = theme.clone();
        button(wrap_chrome(inner, [4, 10], ButtonKind::Pill, &th, th.surface))
            .padding(0)
            .style(move |_: &Theme, status| {
                let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
                compute_button_style(ButtonKind::Pill, accent, hover, bg, hover_bg, fg, border, 20.0, &th)
            })
            .on_press(msg)
            .into()
    } else {
        container(inner)
            .padding([4, 10])
            .style(move |_| container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border,
                    width: 1.0,
                    radius: 20.0.into(),
                },
                ..Default::default()
            })
            .into()
    }
}

pub fn pill_button<'a, Message: Clone + 'a>(
    label: impl iced::widget::text::IntoFragment<'a>,
    theme: &ThemeSpec,
    active: bool,
    msg: Message,
) -> Element<'a, Message> {
    pill_icon_button(label, None, theme, active, msg)
}

pub fn pill_icon_button<'a, Message: Clone + 'a>(
    label: impl iced::widget::text::IntoFragment<'a>,
    icon_glyph: Option<&'a str>,
    theme: &ThemeSpec,
    active: bool,
    msg: Message,
) -> Element<'a, Message> {
    let accent = theme.accent;
    let accent_fg = theme.accent_fg;
    let card = theme.card;
    let hover_bg = theme.card_hover;
    let border = theme.border;
    let text_col = theme.text;
    let fg = if active { accent_fg } else { text_col };

    let content: Element<'a, Message> = if let Some(g) = icon_glyph {
        row![
            icons::icon(g, 12.0, fg),
            text(label).size(12).color(fg),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
    } else {
        text(label).size(12).color(fg).into()
    };

    let th = theme.clone();
    button(wrap_chrome(content, [6, 14], ButtonKind::Pill, &th, th.surface))
        .padding(0)
        .style(move |_: &Theme, status| {
            let is_hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
            compute_button_style(
                ButtonKind::Pill,
                active,
                is_hover,
                card,
                hover_bg,
                fg,
                if active { accent } else { border },
                20.0,
                &th,
            )
        })
        .on_press(msg)
        .into()
}

pub fn primary_pill_icon_button<'a, Message: Clone + 'a>(
    label: impl iced::widget::text::IntoFragment<'a>,
    icon_glyph: Option<&'a str>,
    theme: &ThemeSpec,
    msg: Message,
) -> Element<'a, Message> {
    let accent = theme.accent;
    let accent_fg = theme.accent_fg;

    let content: Element<'a, Message> = if let Some(g) = icon_glyph {
        row![
            icons::icon(g, 12.0, accent_fg),
            text(label).size(13).color(accent_fg),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    } else {
        text(label).size(13).color(accent_fg).into()
    };

    let th = theme.clone();
    button(wrap_chrome(content, [9, 20], ButtonKind::Primary, &th, th.surface))
        .padding(0)
        .style(move |_: &Theme, status| {
            let is_hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
            compute_button_style(
                ButtonKind::Primary,
                true,
                is_hover,
                accent,
                Color { a: 0.90, ..accent },
                accent_fg,
                accent,
                20.0,
                &th,
            )
        })
        .on_press(msg)
        .into()
}

pub fn glyph_button<'a, Message: Clone + 'a>(
    glyph: &'a str,
    size: f32,
    theme: &ThemeSpec,
    active: bool,
    msg: Message,
) -> Element<'a, Message> {
    let accent = theme.accent;
    let muted = theme.muted;
    let text_col = theme.text;
    let hover_bg = theme.card_hover;
    let th = theme.clone();

    button(wrap_chrome(
        text(icons::map_glyph(glyph))
            .font(icons::current_font())
            .size(icons::adjusted_size(size)),
        [6, 9],
        ButtonKind::Glyph,
        &th,
        th.surface,
    ))
    .padding(0)
    .style(move |_: &Theme, status| {
        let is_hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let fg = if active {
            accent
        } else if is_hover {
            text_col
        } else {
            muted
        };
        compute_button_style(
            ButtonKind::Glyph,
            active,
            is_hover,
            Color::TRANSPARENT,
            hover_bg,
            fg,
            Color::TRANSPARENT,
            999.0,
            &th,
        )
    })
    .on_press(msg)
    .into()
}

pub fn play_circle_button<'a, Message: Clone + 'a>(
    glyph: &'a str,
    theme: &ThemeSpec,
    playing: bool,
    msg: Message,
) -> Element<'a, Message> {
    let idle_bg = theme.text;
    let idle_fg = theme.base;
    let accent = theme.accent;
    let accent_fg = theme.accent_fg;
    let th = theme.clone();

    button(wrap_chrome(
        container(
            text(icons::map_glyph(glyph))
                .font(icons::current_font())
                .size(icons::adjusted_size(14.0)),
        )
        .width(Length::Fixed(36.0))
        .height(Length::Fixed(36.0))
        .center_x(Length::Fixed(36.0))
        .center_y(Length::Fixed(36.0)),
        [0, 0],
        ButtonKind::Glyph,
        &th,
        th.sidebar,
    ))
    .padding(0)
    .style(move |_: &Theme, status| {
        let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let active_col = playing || hover;
        compute_button_style(
            ButtonKind::Glyph,
            playing,
            hover,
            if active_col { accent } else { idle_bg },
            accent,
            if active_col { accent_fg } else { idle_fg },
            Color::TRANSPARENT,
            18.0,
            &th,
        )
    })
    .on_press(msg)
    .into()
}


pub fn sidebar_nav_item<'a, Message: Clone + 'a>(
    glyph: &'a str,
    label: &'a str,
    active: bool,
    has_plus: bool,
    theme: &ThemeSpec,
    msg: Message,
) -> Element<'a, Message> {
    let text_col = theme.text;
    let muted = theme.muted;
    let accent = theme.accent;
    let accent_fg = theme.accent_fg;
    let card_hover = theme.card_hover;

    let indicator: Element<'a, Message> = if active {
        container(text(""))
            .width(Length::Fixed(3.0))
            .height(Length::Fixed(18.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(accent)),
                border: Border {
                    radius: 2.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    } else {
        space::horizontal().width(Length::Fixed(3.0)).into()
    };

    let icon_cell: Element<'a, Message> = if has_plus {
        let base_icon = container(
            text(icons::map_glyph(glyph))
                .font(icons::current_font())
                .size(icons::adjusted_size(15.0)),
        )
        .width(Length::Fixed(24.0))
        .height(Length::Fixed(20.0))
        .center_y(Length::Fixed(20.0));
        let plus_dot = container(
            container(text("+").size(8).color(accent_fg))
                .padding([0, 3])
                .style(move |_| container::Style {
                    background: Some(Background::Color(accent)),
                    border: Border {
                        radius: 999.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        )
        .width(Length::Fixed(24.0))
        .height(Length::Fixed(20.0))
        .align_right(Length::Fixed(24.0))
        .align_top(Length::Fixed(20.0));
        stack([base_icon.into(), plus_dot.into()]).into()
    } else {
        text(icons::map_glyph(glyph))
            .font(icons::current_font())
            .size(icons::adjusted_size(15.0))
            .width(Length::Fixed(22.0))
            .into()
    };

    let mut row_items = Row::new()
        .push(indicator)
        .push(icon_cell)
        .push(text(label).size(14))
        .spacing(8)
        .align_y(Alignment::Center);
    if has_plus {
        row_items = row_items.push(
            container(text("+").size(10).color(accent_fg))
                .padding([1, 5])
                .style(move |_| container::Style {
                    background: Some(Background::Color(accent)),
                    border: Border {
                        radius: 999.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        );
    }

    let th = theme.clone();
    button(wrap_chrome(row_items, [9, 10], ButtonKind::Nav, &th, th.surface))
    .width(Length::Fill)
    .padding(0)
    .style(move |_: &Theme, status| {
        let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
        compute_button_style(
            ButtonKind::Nav,
            false,
            hover,
            Color::TRANSPARENT,
            card_hover,
            if active || hover { text_col } else { muted },
            Color::TRANSPARENT,
            8.0,
            &th,
        )
    })
    .on_press(msg)
    .into()
}


/// Compact square icon-button tile used by Openify's Compact Menu sidebar grid.
fn compact_short_label(label: &str) -> String {
    match label {
        "Settings" => "Settings".to_string(),
        "Daeboard" => "Daeboard".to_string(),
        "Playlists" => "Playlist".to_string(),
        "Equalizer" => "EQ".to_string(),
        "Visualizer" => "Viz".to_string(),
        "Better Visualizer" => "Viz+".to_string(),
        "Speed & Pitch" => "Speed+".to_string(),
        "A-B Loop" => "A-B Loop".to_string(),
        "BPM & Info" => "BPM Info".to_string(),
        "Now Playing" => "Playing".to_string(),
        "Inspector" => "Inspect".to_string(),
        "Spatial Audio" => "Spatial".to_string(),
        "Reactive UI" => "Reactive".to_string(),
        _ => label.chars().take(8).collect(),
    }
}

pub fn compact_nav_button<'a, Message: Clone + 'a>(
    glyph: &'a str,
    label: &'a str,
    active: bool,
    has_plus: bool,
    theme: &ThemeSpec,
    msg: Message,
) -> Element<'a, Message> {
    let accent = theme.accent;
    let accent_fg = theme.accent_fg;
    let muted = theme.muted;
    let text_col = theme.text;
    let card_hover = theme.card_hover;
    let card = theme.card;
    let border = theme.border;
    let tip_bg = theme.surface;

    let short_label = compact_short_label(label);
    let tile_content: Element<'a, Message> = container(
        column![
            text(icons::map_glyph(glyph))
                .font(icons::current_font())
                .size(icons::adjusted_size(15.0)),
            text(short_label).size(8),
        ]
        .spacing(2)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into();

    let btn_body: Element<'a, Message> = if has_plus {
        let plus_badge = container(
            container(
                text("+")
                    .size(9)
                    .color(if active { accent } else { accent_fg }),
            )
            .padding([0, 4])
            .style(move |_| container::Style {
                background: Some(Background::Color(if active { accent_fg } else { accent })),
                border: Border {
                    radius: 999.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_right(Length::Fill)
        .align_top(Length::Fill)
        .padding([2, 3]);
        stack([tile_content, plus_badge.into()]).into()
    } else {
        tile_content
    };

    let th = theme.clone();
    let btn = button(wrap_chrome(btn_body, [2, 2], ButtonKind::Nav, &th, th.surface))
    .width(Length::Fill)
    .height(Length::Fixed(46.0))
    .padding(0)
    .style(move |_: &Theme, status| {
        let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
        compute_button_style(
            ButtonKind::Nav,
            active,
            hover,
            card,
            card_hover,
            if active {
                accent_fg
            } else if hover {
                text_col
            } else {
                muted
            },
            if active { accent } else { border },
            9.0,
            &th,
        )
    })
    .on_press(msg);

    let tip_txt = if has_plus {
        format!("{label}+ (Enhanced)")
    } else {
        label.to_string()
    };

    container(
        tooltip(
            btn,
            container(text(tip_txt).size(12).color(text_col))
                .padding([4, 8])
                .style(move |_| container::Style {
                    background: Some(Background::Color(tip_bg)),
                    border: Border {
                        color: border,
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                }),
            tooltip::Position::Top,
        ),
    )
    .width(Length::Fill)
    .into()
}

pub fn folder_sidebar_item<'a, Message: Clone + 'a>(
    art: Option<&'a image::Handle>,
    fallback_glyph: &'a str,
    name: &'a str,
    subtitle: String,
    active: bool,
    theme: &ThemeSpec,
    msg: Message,
) -> Element<'a, Message> {
    let accent = theme.accent;
    let card = theme.card;
    let card_hover = theme.card_hover;
    let text_col = theme.text;
    let muted = theme.muted;

    let thumb: Element<'a, Message> = if let Some(h) = art {
        image(h.clone())
            .width(Length::Fixed(44.0))
            .height(Length::Fixed(44.0))
            .into()
    } else {
        container(icons::icon(fallback_glyph, 16.0, if active { accent } else { muted }))
        .width(Length::Fixed(44.0))
        .height(Length::Fixed(44.0))
        .center_x(Length::Fixed(44.0))
        .center_y(Length::Fixed(44.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(card_hover)),
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    };

    let left_bar: Element<'a, Message> = container(text(""))
        .width(Length::Fixed(3.0))
        .height(Length::Fixed(38.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(if active {
                accent
            } else {
                Color::TRANSPARENT
            })),
            border: Border {
                radius: 2.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into();

    let th = theme.clone();
    button(wrap_chrome(
        row![
            left_bar,
            thumb,
            column![
                text(name).size(13).color(if active { accent } else { text_col }),
                text(subtitle).size(11).color(muted),
            ]
            .spacing(2)
            .width(Length::Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        [6, 8],
        ButtonKind::Nav,
        &th,
        th.surface,
    ))
    .width(Length::Fill)
    .padding(0)
    .style(move |_: &Theme, status| {
        let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
        compute_button_style(
            ButtonKind::Nav,
            false,
            hover,
            if active { card } else { Color::TRANSPARENT },
            card_hover,
            text_col,
            Color::TRANSPARENT,
            8.0,
            &th,
        )
    })
    .on_press(msg)
    .into()
}


pub fn theme_swatch_circle<'a, Message: Clone + 'a>(
    spec: &ThemeSpec,
    selected: bool,
    on_select: Message,
) -> Element<'a, Message> {
    let base_col = spec.sidebar;
    let accent_col = spec.logo_color;
    let ring_col = if selected { spec.text } else { spec.border };
    let muted = spec.muted;
    let text_col = spec.text;
    let label = spec.label.clone();
    let logo_thumb: Element<'a, Message> = if let Some(h) = crate::logo::cached_tinted_logo_handle(accent_col) {
        iced::widget::image(h)
            .width(Length::Fixed(18.0))
            .height(Length::Fixed(16.0))
            .content_fit(iced::ContentFit::Contain)
            .into()
    } else {
        text("").into()
    };

    button(
        column![
            container(logo_thumb)
                .width(Length::Fixed(38.0))
                .height(Length::Fixed(38.0))
                .center_x(Length::Fixed(38.0))
                .center_y(Length::Fixed(38.0))
                .style(move |_| container::Style {
                    background: Some(Background::Color(base_col)),
                    border: Border {
                        color: ring_col,
                        width: if selected { 2.5 } else { 1.5 },
                        radius: 19.0.into(),
                    },
                    shadow: if selected {
                        Shadow {
                            color: Color { a: 0.60, ..accent_col },
                            offset: Vector::ZERO,
                            blur_radius: 10.0,
                        }
                    } else {
                        Shadow::default()
                    },
                    ..Default::default()
                }),
            text(label)
                .size(11)
                .color(if selected { text_col } else { muted }),
        ]
        .spacing(5)
        .align_x(Alignment::Center),
    )
    .padding([6, 8])
    .style(|_, status| {
        let hover = matches!(status, button::Status::Hovered);
        button::Style {
            background: Some(Background::Color(if hover {
                Color::from_rgba(1.0, 1.0, 1.0, 0.05)
            } else {
                Color::TRANSPARENT
            })),
            border: Border {
                radius: 10.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .on_press(on_select)
    .into()
}

pub fn track_row_button<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    padding: [u16; 2],
    selected: bool,
    playing: bool,
    theme: &ThemeSpec,
    msg: Message,
) -> Element<'a, Message> {
    let accent = theme.accent;
    let card_hover = theme.card_hover;
    let text_col = theme.text;
    let row_bg = if playing {
        Color { a: 0.14, ..accent }
    } else if selected {
        card_hover
    } else {
        Color::TRANSPARENT
    };

    let th = theme.clone();
    button(wrap_chrome(content, padding, ButtonKind::Row, &th, th.surface))
        .width(Length::Fill)
        .padding(0)
        .style(move |_: &Theme, status| {
            let hover = matches!(status, button::Status::Hovered | button::Status::Pressed);
            compute_button_style(
                ButtonKind::Row,
                false,
                hover,
                row_bg,
                card_hover,
                text_col,
                if playing { accent } else { Color::TRANSPARENT },
                8.0,
                &th,
            )
        })
        .on_press(msg)
        .into()
}

pub fn render_ext_widget<'a, Message: Clone + 'a, F, FIn>(
    widget: &'a ExtWidget,
    ext_id: &'a str,
    theme: &ThemeSpec,
    viz: &ExtVizCtx<'a, Message>,
    inputs: &'a HashMap<(String, String), String>,
    make_action: F,
    make_input: FIn,
) -> Element<'a, Message>
where
    F: Fn(String, String, serde_json::Value) -> Message + Clone + 'a,
    FIn: Fn(String, String, String) -> Message + Clone + 'a,
{
    match widget {
        ExtWidget::Heading { text: txt } => text(txt).size(16).color(theme.text).into(),
        ExtWidget::Text { text: txt, muted } => {
            let col = if *muted { theme.muted } else { theme.text };
            text(txt).size(13).color(col).into()
        }
        ExtWidget::Badge { text: txt, icon, accent } => {
            slot_badge(txt.as_str(), icon.as_deref(), *accent, theme, None)
        }
        ExtWidget::Row { children } => {
            let mut r = Row::new().spacing(8).align_y(Alignment::Center);
            for child in children {
                r = r.push(render_ext_widget(
                    child,
                    ext_id,
                    theme,
                    viz,
                    inputs,
                    make_action.clone(),
                    make_input.clone(),
                ));
            }
            r.wrap().into()
        }
        ExtWidget::Button {
            id,
            label,
            icon,
            accent,
            active,
        } => {
            let msg = make_action(ext_id.to_string(), id.clone(), serde_json::Value::Bool(true));
            let glyph = icon.as_deref().map(resolve_icon);
            if *accent {
                primary_pill_icon_button(label.as_str(), glyph, theme, msg)
            } else {
                pill_icon_button(label.as_str(), glyph, theme, *active, msg)
            }
        }
        ExtWidget::Toggle { id, label, value } => {
            let msg = make_action(
                ext_id.to_string(),
                id.clone(),
                serde_json::Value::Bool(!*value),
            );
            row![
                text(label).size(13).color(theme.text).width(Length::Fill),
                pill_button(if *value { "ON" } else { "OFF" }, theme, *value, msg),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
        }
        ExtWidget::Slider {
            id,
            label,
            min,
            max,
            step,
            value,
            unit,
        } => {
            let eid = ext_id.to_string();
            let aid = id.clone();
            let mk = make_action;
            let lo = *min;
            let hi = (*max).max(lo + 0.001);
            let cur = value.clamp(lo, hi);
            let s_step = (*step).max(0.01);
            let suffix = if unit.is_empty() {
                String::new()
            } else {
                format!(" {unit}")
            };
            row![
                text(format!("{label}: {cur:.1}{suffix}"))
                    .size(13)
                    .color(theme.text)
                    .width(Length::Fixed(185.0)),
                slider(lo..=hi, cur, move |v| {
                    mk(eid.clone(), aid.clone(), serde_json::json!(v))
                })
                .step(s_step)
                .width(Length::Fill),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
        }
        ExtWidget::Progress { label, value, max } => {
            let hi = (*max).max(0.001) as f32;
            let val = (*value as f32).clamp(0.0, hi);
            column![
                row![
                    text(label).size(12).color(theme.text),
                    space::horizontal(),
                    text(format!("{value:.0} / {max:.0}")).size(11).color(theme.muted),
                ],
                progress_bar(0.0..=hi, val).length(Length::Fill),
            ]
            .spacing(4)
            .into()
        }
        ExtWidget::Input {
            id,
            label,
            value,
            placeholder,
            submit_action,
            submit_label,
        } => {
            let cur_val = inputs
                .get(&(ext_id.to_string(), id.clone()))
                .map(|s| s.as_str())
                .unwrap_or(value.as_str());
            let eid_in = ext_id.to_string();
            let wid_in = id.clone();
            let mk_in = make_input;

            let bg = theme.card;
            let border = theme.border;
            let txt_col = theme.text;
            let muted_col = theme.muted;
            let accent = theme.accent;

            let mut inp = text_input(placeholder.as_str(), cur_val)
                .on_input(move |s| mk_in(eid_in.clone(), wid_in.clone(), s))
                .padding([8, 12])
                .size(13)
                .style(move |_, status| {
                    let is_focused = matches!(status, text_input::Status::Focused { .. });
                    text_input::Style {
                        background: Background::Color(bg),
                        border: Border {
                            color: if is_focused { accent } else { border },
                            width: 1.0,
                            radius: 8.0.into(),
                        },
                        icon: muted_col,
                        placeholder: muted_col,
                        value: txt_col,
                        selection: accent,
                    }
                })
                .width(Length::Fill);

            let action_target = submit_action.as_deref().unwrap_or(id.as_str());
            let submit_msg = make_action(
                ext_id.to_string(),
                action_target.to_string(),
                serde_json::Value::String(cur_val.to_string()),
            );
            inp = inp.on_submit(submit_msg.clone());

            let mut r = Row::new().spacing(8).align_y(Alignment::Center);
            if !label.is_empty() {
                r = r.push(
                    text(label)
                        .size(13)
                        .color(theme.text)
                        .width(Length::Fixed(130.0)),
                );
            }
            r = r.push(inp);
            if submit_action.is_some() || submit_label.is_some() {
                let btn_lbl = submit_label.as_deref().unwrap_or("Go");
                r = r.push(primary_pill_icon_button(btn_lbl, None, theme, submit_msg));
            }
            r.into()
        }
        ExtWidget::Visualizer {
            mode,
            playing,
            volume,
            speed,
        } => {
            if mode.trim().eq_ignore_ascii_case("gif") {
                return render_gif_visualizer_stage(theme, viz, *playing);
            }
            card_box(
                column![
                    row![
                        icons::icon(icons::BARS, 13.0, theme.accent),
                        text(format!("{} SCENE", mode.to_uppercase()))
                            .size(11)
                            .color(theme.muted),
                        space::horizontal(),
                        text(if *playing { "LIVE" } else { "IDLE" })
                            .size(11)
                            .color(if *playing { theme.accent } else { theme.muted }),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                    visualizer_canvas(mode, &viz.frame, *playing, *volume, *speed, theme),
                ]
                .spacing(8)
                .padding(10),
                theme,
                *playing,
            )
            .width(Length::Fill)
            .into()
        }
        ExtWidget::Grid { columns, children } => {
            let cols = (*columns).clamp(1, 6);
            let mut grid = Column::new().spacing(10).width(Length::Fill);
            for chunk in children.chunks(cols) {
                let mut r = Row::new().spacing(10).width(Length::Fill);
                for child in chunk {
                    r = r.push(
                        container(render_ext_widget(
                            child,
                            ext_id,
                            theme,
                            viz,
                            inputs,
                            make_action.clone(),
                            make_input.clone(),
                        ))
                        .width(Length::Fill),
                    );
                }
                for _ in chunk.len()..cols {
                    r = r.push(space::horizontal().width(Length::Fill));
                }
                grid = grid.push(r);
            }
            grid.into()
        }
        ExtWidget::Card {
            id,
            title,
            subtitle,
            eyebrow,
            icon,
            badge,
            variant,
            accent,
            active,
        } => render_ext_card(
            ext_id,
            id.as_deref(),
            title,
            subtitle.as_deref(),
            eyebrow.as_deref(),
            icon.as_deref(),
            badge.as_deref(),
            variant.as_deref().unwrap_or("compact"),
            *accent,
            *active,
            theme,
            make_action,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_ext_card<'a, Message: Clone + 'a, F>(
    ext_id: &'a str,
    id: Option<&'a str>,
    title: &'a str,
    subtitle: Option<&'a str>,
    eyebrow: Option<&'a str>,
    icon: Option<&'a str>,
    badge: Option<&'a str>,
    variant: &str,
    accent_flag: bool,
    active: bool,
    theme: &ThemeSpec,
    make_action: F,
) -> Element<'a, Message>
where
    F: Fn(String, String, serde_json::Value) -> Message + Clone + 'a,
{
    let glyph = resolve_icon(icon.unwrap_or("music"));
    let icon_color = if accent_flag || active {
        theme.accent
    } else {
        Color { a: 0.72, ..theme.accent }
    };
    let art_bg = Color { a: 0.85, ..theme.base };
    let art_border = Color { a: 0.55, ..theme.border };

    let art_box = |w: f32, h: f32, radius: f32, glyph_sz: f32| -> Container<'a, Message> {
        container(icons::icon(glyph, glyph_sz, icon_color))
            .width(if w <= 0.0 { Length::Fill } else { Length::Fixed(w) })
            .height(Length::Fixed(h))
            .center_x(if w <= 0.0 { Length::Fill } else { Length::Fixed(w) })
            .center_y(Length::Fixed(h))
            .style(move |_| container::Style {
                background: Some(Background::Color(art_bg)),
                border: Border {
                    color: art_border,
                    width: 1.0,
                    radius: radius.into(),
                },
                ..Default::default()
            })
    };

    let (body, radius, pad): (Element<'a, Message>, f32, [u16; 2]) = match variant {
        "hero" => {
            let mut info = Column::new().spacing(4).width(Length::Fill);
            if let Some(eb) = eyebrow {
                info = info.push(text(eb.to_uppercase()).size(10).color(theme.accent));
            }
            info = info.push(text(title).size(18).color(theme.text));
            if let Some(sub) = subtitle {
                info = info.push(text(sub).size(12).color(theme.muted));
            }
            let mut r = row![art_box(82.0, 82.0, 12.0, 32.0), info]
                .spacing(16)
                .align_y(Alignment::Center)
                .width(Length::Fill);
            if let Some(bg_txt) = badge {
                r = r.push(slot_badge(bg_txt, None, accent_flag || active, theme, None));
            }
            (r.into(), 16.0, [16, 18])
        }
        "tile" => {
            let mut col = column![
                art_box(0.0, 74.0, 10.0, 26.0),
                text(title).size(13).color(theme.text),
            ]
            .spacing(7)
            .width(Length::Fill);
            if let Some(sub) = subtitle {
                col = col.push(text(sub).size(11).color(theme.muted));
            }
            (col.into(), 12.0, [10, 10])
        }
        "stat" => {
            let mut col = Column::new().spacing(4).width(Length::Fill);
            col = col.push(
                row![
                    icons::icon(glyph, 14.0, icon_color),
                    space::horizontal(),
                    text(eyebrow.unwrap_or("")).size(10).color(theme.muted),
                ]
                .align_y(Alignment::Center),
            );
            col = col.push(
                text(title)
                    .size(20)
                    .color(if accent_flag { theme.accent } else { theme.text }),
            );
            if let Some(sub) = subtitle {
                col = col.push(text(sub).size(11).color(theme.muted));
            }
            (col.into(), 12.0, [12, 14])
        }
        _ => {
            // "compact": Openify Quick Pick / Top Folder card shape
            let mut info = Column::new().spacing(2).width(Length::Fill);
            info = info.push(text(title).size(13).color(theme.text));
            if let Some(sub) = subtitle {
                info = info.push(text(sub).size(11).color(theme.muted));
            }
            let mut r = row![art_box(44.0, 44.0, 8.0, 18.0), info]
                .spacing(12)
                .align_y(Alignment::Center)
                .width(Length::Fill);
            if let Some(bg_txt) = badge {
                r = r.push(slot_badge(bg_txt, None, accent_flag || active, theme, None));
            }
            (r.into(), 11.0, [10, 12])
        }
    };

    let bg = if active {
        Color { a: 0.16, ..theme.accent }
    } else {
        theme.card
    };
    let hover_bg = if active {
        Color { a: 0.24, ..theme.accent }
    } else {
        theme.card_hover
    };
    let border_col = if active || accent_flag {
        theme.accent
    } else {
        theme.border
    };
    let text_col = theme.text;

    if let Some(aid) = id {
        let msg = make_action(
            ext_id.to_string(),
            aid.to_string(),
            serde_json::Value::Bool(true),
        );
        let th = theme.clone();
        button(wrap_chrome(container(body).width(Length::Fill), pad, ButtonKind::Tile, &th, th.surface))
            .padding(0)
            .width(Length::Fill)
            .style(move |_, status| {
                let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                compute_button_style(
                    ButtonKind::Tile,
                    active,
                    hovered,
                    bg,
                    hover_bg,
                    text_col,
                    border_col,
                    radius,
                    &th,
                )
            })
            .on_press(msg)
            .into()
    } else {
        container(body)
            .padding(pad)
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border_col,
                    width: if accent_flag { 1.0 } else { 0.8 },
                    radius: radius.into(),
                },
                ..Default::default()
            })
            .into()
    }
}

pub fn sort_header_button<'a, Message: Clone + 'a>(
    label: &str,
    active: bool,
    desc: bool,
    theme: &ThemeSpec,
    msg: Message,
) -> Element<'a, Message> {
    let col = if active { theme.accent } else { theme.muted };
    let text_col = theme.text;
    let arrow = if active {
        if desc { " v" } else { " ^" }
    } else {
        ""
    };
    button(
        text(format!("{label}{arrow}"))
            .size(11)
            .color(col),
    )
    .padding([3, 6])
    .style(move |_, status| {
        let hover = matches!(status, button::Status::Hovered);
        button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: if hover { text_col } else { col },
            ..Default::default()
        }
    })
    .on_press(msg)
    .into()
}

pub fn track_monogram_tile<'a, Message: 'a>(
    title: &str,
    size_px: f32,
    playing: bool,
    theme: &ThemeSpec,
) -> Element<'a, Message> {
    let ch = title
        .chars()
        .find(|c| c.is_ascii_alphanumeric())
        .unwrap_or('M')
        .to_ascii_uppercase();
    let bg = if playing {
        Color { a: 0.28, ..theme.accent }
    } else {
        theme.card_hover
    };
    let fg = if playing { theme.accent } else { theme.text };
    let border = if playing { theme.accent } else { theme.border };
    container(text(ch.to_string()).size((size_px * 0.42).max(12.0)).color(fg))
        .width(Length::Fixed(size_px))
        .height(Length::Fixed(size_px))
        .center_x(Length::Fixed(size_px))
        .center_y(Length::Fixed(size_px))
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: border,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Renders one row inside the right-click context menu.
pub fn context_menu_item<'a, Message: Clone + 'a>(
    glyph: &'static str,
    label: &'a str,
    danger: bool,
    theme: &ThemeSpec,
    msg: Message,
) -> Element<'a, Message> {
    let fg = if danger { theme.danger } else { theme.text };
    let icon_col = if danger { theme.danger } else { theme.accent };
    let hover_bg = if danger {
        Color { a: 0.14, ..theme.danger }
    } else {
        theme.card_hover
    };

    let th = theme.clone();
    button(wrap_chrome(
        row![
            container(icons::icon(glyph, 13.0, icon_col)).width(Length::Fixed(18.0)),
            text(label).size(13).color(fg),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        [7, 12],
        ButtonKind::Row,
        &th,
        th.surface,
    ))
    .width(Length::Fill)
    .padding(0)
    .style(move |_, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        compute_button_style(
            ButtonKind::Row,
            false,
            hovered,
            Color::TRANSPARENT,
            hover_bg,
            fg,
            Color::TRANSPARENT,
            7.0,
            &th,
        )
    })
    .on_press(msg)
    .into()
}


/// Floating elevated surface for the right-click context menu.
pub fn context_menu_panel<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    theme: &ThemeSpec,
) -> Element<'a, Message> {
    let bg = theme.surface;
    let border = theme.border;
    container(content)
        .width(Length::Fixed(228.0))
        .padding(6)
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: border,
                width: 1.0,
                radius: 11.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.55),
                offset: Vector::new(0.0, 8.0),
                blur_radius: 24.0,
            },
            ..Default::default()
        })
        .into()
}

fn truncate_filename(name: &str, max_chars: usize) -> String {
    if name.chars().count() <= max_chars {
        return name.to_string();
    }
    let keep = max_chars.saturating_sub(3);
    let prefix: String = name.chars().take(keep).collect();
    format!("{prefix}...")
}

fn render_gif_visualizer_stage<'a, Message: Clone + 'a>(
    theme: &ThemeSpec,
    viz: &ExtVizCtx<'a, Message>,
    playing: bool,
) -> Element<'a, Message> {
    let bass = (viz.frame.bands[0] + viz.frame.bands[1] + viz.frame.bands[2] + viz.frame.bands[3]) * 0.25;
    let flash = viz.gif.flash.clamp(0.0, 1.0);
    let accent = theme.accent;
    let border_col = Color {
        a: (0.28 + flash * 0.72 + bass * 0.25).clamp(0.25, 1.0),
        ..accent
    };
    let glow_col = Color {
        a: (0.08 + flash * 0.45).clamp(0.0, 0.65),
        ..accent
    };
    let stage_bg = Color::from_rgb8(0x09, 0x0b, 0x10);
    let chip_bg = Color::from_rgba(0.05, 0.06, 0.09, 0.78);
    let text_col = theme.text;
    let muted = theme.muted;

    let stage_body: Element<'a, Message> = if let Some(handle) = viz.gif.handle {
        let img = image(handle.clone())
            .content_fit(ContentFit::Contain)
            .width(Length::Fill)
            .height(Length::Fill);

        let name_lbl = truncate_filename(viz.gif.name, 36);
        let bass_pct = ((bass * 100.0).round() as u32).min(100);
        let overlay = container(
            row![
                container(
                    text(name_lbl).size(11).color(text_col),
                )
                .padding([4, 9])
                .style(move |_| container::Style {
                    background: Some(Background::Color(chip_bg)),
                    border: Border {
                        color: Color { a: 0.25, ..accent },
                        width: 1.0,
                        radius: 999.0.into(),
                    },
                    ..Default::default()
                }),
                space::horizontal(),
                container(
                    text(if flash > 0.25 {
                        format!("BASS HIT · {bass_pct}%")
                    } else {
                        format!("LOW FREQ · {bass_pct}%")
                    })
                    .size(11)
                    .color(if flash > 0.25 { accent } else { muted }),
                )
                .padding([4, 9])
                .style(move |_| container::Style {
                    background: Some(Background::Color(chip_bg)),
                    border: Border {
                        color: if flash > 0.25 { accent } else { Color::from_rgba(1.0, 1.0, 1.0, 0.12) },
                        width: 1.0,
                        radius: 999.0.into(),
                    },
                    ..Default::default()
                }),
            ]
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_bottom(Length::Fill)
        .padding(10);

        button(stack([img.into(), overlay.into()]))
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_, _| button::Style {
                background: None,
                ..Default::default()
            })
            .on_press(viz.on_next_gif.clone())
            .into()
    } else {
        let msg = if viz.gif.total_gifs > 0 {
            format!("Loading random GIFs from {} ({} found)...", viz.gif.dir, viz.gif.total_gifs)
        } else {
            format!("No .gif files found in {}", viz.gif.dir)
        };
        container(text(msg).size(12).color(muted))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    };

    let stage = container(stage_body)
        .width(Length::Fill)
        .height(Length::Fixed(296.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(stage_bg)),
            border: Border {
                color: border_col,
                width: 1.2 + flash * 1.4,
                radius: 12.0.into(),
            },
            shadow: Shadow {
                color: glow_col,
                offset: Vector::ZERO,
                blur_radius: 14.0 * flash,
            },
            ..Default::default()
        });

    let status_lbl = if flash > 0.25 {
        "BASS HIT"
    } else if playing {
        "LIVE"
    } else {
        "IDLE"
    };

    card_box(
        column![
            row![
                icons::icon(icons::WAVE, 13.0, accent),
                text(format!(
                    "GIF SHOWER · {} GIFs · {} bass switches",
                    viz.gif.total_gifs, viz.gif.switches
                ))
                .size(11)
                .color(muted),
                space::horizontal(),
                pill_icon_button(
                    "Next GIF",
                    Some(icons::SHUFFLE),
                    theme,
                    false,
                    viz.on_next_gif.clone(),
                ),
                text(status_lbl)
                    .size(11)
                    .color(if playing || flash > 0.25 { accent } else { muted }),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            stage,
        ]
        .spacing(8)
        .padding(10),
        theme,
        playing || flash > 0.25,
    )
    .width(Length::Fill)
    .into()
}
