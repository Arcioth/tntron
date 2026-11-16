use std::cell::RefCell;

use iced::border::Radius;
use iced::gradient::Linear;
use iced::mouse;
use iced::widget::canvas::{self, Canvas, Frame, Path, Stroke};
use iced::widget::{button, container, stack};
use iced::{Background, Border, Color, Element, Gradient, Length, Point, Radians, Rectangle, Shadow, Theme, Vector};
use tntron_core::ButtonPackConfig;

use crate::theme::{parse_hex_color, ThemeSpec};

thread_local! {
    static ACTIVE_BUTTON_PACK: RefCell<ButtonPackConfig> = RefCell::new(ButtonPackConfig::default());
}

pub fn set_active_pack(cfg: &ButtonPackConfig) {
    ACTIVE_BUTTON_PACK.with(|cell| {
        *cell.borrow_mut() = cfg.clone();
    });
}

pub fn active_pack() -> ButtonPackConfig {
    ACTIVE_BUTTON_PACK.with(|cell| cell.borrow().clone())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Pill,
    Primary,
    Glyph,
    Nav,
    Row,
    Tile,
}

pub fn pack_applies(pack: &ButtonPackConfig, kind: ButtonKind) -> bool {
    if !pack.enabled {
        return false;
    }
    match kind {
        ButtonKind::Pill => pack.apply_pills,
        ButtonKind::Primary => pack.apply_primary,
        ButtonKind::Glyph => pack.apply_glyphs,
        ButtonKind::Nav => pack.apply_nav,
        ButtonKind::Row => pack.apply_rows,
        ButtonKind::Tile => pack.apply_tiles,
    }
}

/// Computes per-corner border radii; cropped corners flatten to `0.0` so diagonal bevels show cleanly.
pub fn resolve_radii(pack: &ButtonPackConfig, kind: ButtonKind, default_radius: f32) -> Radius {
    if !pack_applies(pack, kind) {
        return default_radius.into();
    }
    let cap = match kind {
        ButtonKind::Glyph => 999.0,
        ButtonKind::Row => 12.0,
        ButtonKind::Nav => 14.0,
        _ => 32.0,
    };
    let r = |val: f32, cropped: bool| -> f32 {
        if cropped {
            0.0
        } else {
            val.min(cap)
        }
    };
    Radius {
        top_left: r(pack.round_tl, pack.crop_tl),
        top_right: r(pack.round_tr, pack.crop_tr),
        bottom_right: r(pack.round_br, pack.crop_br),
        bottom_left: r(pack.round_bl, pack.crop_bl),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn compute_button_style(
    kind: ButtonKind,
    active: bool,
    hovered: bool,
    base_bg: Color,
    hover_bg: Color,
    fg: Color,
    default_border: Color,
    default_radius: f32,
    theme: &ThemeSpec,
) -> button::Style {
    let pack = active_pack();
    if !pack_applies(&pack, kind) {
        let bg = if active {
            theme.accent
        } else if hovered {
            hover_bg
        } else {
            base_bg
        };
        return button::Style {
            background: Some(Background::Color(bg)),
            text_color: fg,
            border: Border {
                color: default_border,
                width: 1.0,
                radius: default_radius.into(),
            },
            ..Default::default()
        };
    }

    let hover_active = hovered && pack.reactive_hover;
    let total_hue = pack.hue_shift_deg
        + if hover_active {
            pack.hover_hue_shift_deg
        } else {
            0.0
        };

    let custom_tint = pack.custom_tint_hex.as_deref().and_then(parse_hex_color);
    let accent_hue = shift_hue(custom_tint.unwrap_or(theme.accent), total_hue);

    let mut fill = if active || kind == ButtonKind::Primary {
        shift_hue(theme.accent, total_hue)
    } else if hovered {
        mix_color(hover_bg, accent_hue, pack.hue_strength.max(0.22))
    } else {
        mix_color(base_bg, accent_hue, pack.hue_strength)
    };

    if hover_active {
        fill = lift_brightness(fill, pack.hover_brightness);
    }

    // Preserve transparent idle look for non-cropped glyphs/rows unless explicitly tinted strongly
    let idle_transparent = base_bg.a <= 0.01 && !active && !hovered && pack.hue_strength < 0.15;
    if idle_transparent {
        fill.a = 0.0;
    } else {
        fill.a = (fill.a * pack.fill_opacity).clamp(0.05, 1.0);
    }

    let outline_base = pack
        .outline_hex
        .as_deref()
        .and_then(parse_hex_color)
        .map(|c| shift_hue(c, if hover_active { pack.hover_hue_shift_deg } else { 0.0 }))
        .unwrap_or(accent_hue);

    let outline_alpha = if !pack.outline_enabled {
        0.0
    } else {
        let boost = if hover_active {
            pack.hover_outline_boost
        } else {
            0.0
        };
        (pack.outline_alpha + boost).clamp(0.0, 1.0)
    };
    let border_col = Color {
        a: outline_alpha,
        ..outline_base
    };
    let border_w = if pack.outline_enabled && !idle_transparent {
        pack.outline_width
            + if hover_active {
                pack.hover_outline_boost * 0.8
            } else {
                0.0
            }
    } else {
        0.0
    };

    let background = if pack.gradient_enabled && !idle_transparent {
        let g_top = pack
            .gradient_top_hex
            .as_deref()
            .and_then(parse_hex_color)
            .map(|c| shift_hue(c, total_hue))
            .unwrap_or_else(|| lift_brightness(fill, 0.25 * pack.gradient_strength + 0.05));
        let g_bot = pack
            .gradient_bottom_hex
            .as_deref()
            .and_then(parse_hex_color)
            .map(|c| shift_hue(c, total_hue))
            .unwrap_or_else(|| darken_color(fill, 0.28 * pack.gradient_strength));

        let top_col = Color {
            a: (fill.a * 0.98).clamp(0.06, 1.0),
            ..mix_color(fill, g_top, pack.gradient_strength.max(0.35))
        };
        let bot_col = Color {
            a: fill.a.clamp(0.06, 1.0),
            ..mix_color(fill, g_bot, pack.gradient_strength.max(0.35))
        };

        let linear = Linear::new(Radians(pack.gradient_angle_deg.to_radians()))
            .add_stop(0.0, top_col)
            .add_stop(1.0, bot_col);
        Background::Gradient(Gradient::Linear(linear))
    } else {
        Background::Color(fill)
    };

    let glow_power = pack.outer_glow
        + if hover_active {
            pack.hover_glow_boost
        } else {
            0.0
        };
    let shadow = if glow_power > 0.02 && !idle_transparent {
        Shadow {
            color: Color {
                a: (0.28 * glow_power).clamp(0.0, 0.78),
                ..accent_hue
            },
            offset: Vector::new(0.0, if pack.bevel_emboss > 0.3 { 1.5 } else { 0.0 }),
            blur_radius: (6.0 + glow_power * 10.0).clamp(4.0, 24.0),
        }
    } else if pack.bevel_emboss > 0.25 && !idle_transparent {
        Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.38 * pack.bevel_emboss),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 4.0,
        }
    } else {
        Shadow::default()
    };

    let txt_col = if active || kind == ButtonKind::Primary {
        theme.accent_fg
    } else if hover_active {
        lift_brightness(fg, 0.22)
    } else {
        fg
    };

    button::Style {
        background: Some(background),
        text_color: txt_col,
        border: Border {
            color: border_col,
            width: border_w,
            radius: resolve_radii(&pack, kind, default_radius),
        },
        shadow,
        snap: false,
    }
}

/// Wraps inner button content with cropped-corner chamfers, inline glow, and 3D bevel emboss overlay.
pub fn wrap_chrome<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    pad: [u16; 2],
    kind: ButtonKind,
    theme: &ThemeSpec,
    surface_behind: Color,
) -> Element<'a, Message> {
    let pack = active_pack();
    let needs_overlay = pack_applies(&pack, kind)
        && (pack.inline_glow > 0.02
            || pack.bevel_emboss > 0.02
            || pack.crop_tl
            || pack.crop_tr
            || pack.crop_br
            || pack.crop_bl);
    let inner = container(content).padding(pad);
    if !needs_overlay {
        return inner.into();
    }

    let accent = pack
        .outline_hex
        .as_deref()
        .and_then(parse_hex_color)
        .or_else(|| pack.custom_tint_hex.as_deref().and_then(parse_hex_color))
        .unwrap_or(theme.accent);

    let overlay: Element<'a, Message> = Canvas::new(ButtonChromeProgram {
        pack,
        accent,
        surface_behind,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

    stack([inner.into(), overlay]).into()
}

#[derive(Debug, Clone)]
struct ButtonChromeProgram {
    pack: ButtonPackConfig,
    accent: Color,
    surface_behind: Color,
}

impl<Message> canvas::Program<Message> for ButtonChromeProgram {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let size = bounds.size();
        let w = size.width;
        let h = size.height;
        if w < 6.0 || h < 6.0 {
            return Vec::new();
        }
        let mut frame = Frame::new(renderer, size);
        let hovered = cursor.is_over(bounds);
        let react = hovered && self.pack.reactive_hover;
        let hue_deg = self.pack.hue_shift_deg
            + if react {
                self.pack.hover_hue_shift_deg
            } else {
                0.0
            };
        let glow_col = shift_hue(self.accent, hue_deg);
        let cut = self.pack.crop_size.clamp(3.0, (w.min(h) * 0.42).max(3.0));

        // 1. Selective Cropped Corners (mask corner triangles + draw crisp chamfer diagonal)
        let corner_mask = Color {
            a: self.surface_behind.a.max(0.85),
            ..self.surface_behind
        };
        let chamfer_col = Color {
            a: (self.pack.outline_alpha
                + if react { self.pack.hover_outline_boost } else { 0.15 })
            .clamp(0.25, 1.0),
            ..glow_col
        };
        let chamfer_w = self.pack.outline_width.max(1.3);

        if self.pack.crop_tl {
            let tri = Path::new(|b| {
                b.move_to(Point::new(0.0, 0.0));
                b.line_to(Point::new(cut, 0.0));
                b.line_to(Point::new(0.0, cut));
                b.close();
            });
            frame.fill(&tri, corner_mask);
            frame.stroke(
                &Path::line(Point::new(0.0, cut), Point::new(cut, 0.0)),
                Stroke::default().with_color(chamfer_col).with_width(chamfer_w),
            );
        }
        if self.pack.crop_tr {
            let tri = Path::new(|b| {
                b.move_to(Point::new(w, 0.0));
                b.line_to(Point::new(w - cut, 0.0));
                b.line_to(Point::new(w, cut));
                b.close();
            });
            frame.fill(&tri, corner_mask);
            frame.stroke(
                &Path::line(Point::new(w - cut, 0.0), Point::new(w, cut)),
                Stroke::default().with_color(chamfer_col).with_width(chamfer_w),
            );
        }
        if self.pack.crop_br {
            let tri = Path::new(|b| {
                b.move_to(Point::new(w, h));
                b.line_to(Point::new(w - cut, h));
                b.line_to(Point::new(w, h - cut));
                b.close();
            });
            frame.fill(&tri, corner_mask);
            frame.stroke(
                &Path::line(Point::new(w - cut, h), Point::new(w, h - cut)),
                Stroke::default().with_color(chamfer_col).with_width(chamfer_w),
            );
        }
        if self.pack.crop_bl {
            let tri = Path::new(|b| {
                b.move_to(Point::new(0.0, h));
                b.line_to(Point::new(cut, h));
                b.line_to(Point::new(0.0, h - cut));
                b.close();
            });
            frame.fill(&tri, corner_mask);
            frame.stroke(
                &Path::line(Point::new(0.0, h - cut), Point::new(cut, h)),
                Stroke::default().with_color(chamfer_col).with_width(chamfer_w),
            );
        }

        // 2. 3D Bevel Emboss (top/left bright highlight, bottom/right deep shadow)
        if self.pack.bevel_emboss > 0.02 {
            let b = self.pack.bevel_emboss.clamp(0.0, 1.0);
            let tl_x = if self.pack.crop_tl { cut } else { 2.0 };
            let tr_x = if self.pack.crop_tr { w - cut } else { w - 2.0 };
            let bl_y = if self.pack.crop_bl { h - cut } else { h - 2.0 };
            let br_x = if self.pack.crop_br { w - cut } else { w - 2.0 };
            let hi = Color::from_rgba(1.0, 1.0, 1.0, 0.42 * b);
            let sh = Color::from_rgba(0.0, 0.0, 0.0, 0.48 * b);
            frame.stroke(
                &Path::line(Point::new(tl_x, 1.5), Point::new(tr_x, 1.5)),
                Stroke::default().with_color(hi).with_width(1.6),
            );
            frame.stroke(
                &Path::line(
                    Point::new(1.5, if self.pack.crop_tl { cut } else { 2.0 }),
                    Point::new(1.5, bl_y),
                ),
                Stroke::default().with_color(hi).with_width(1.4),
            );
            frame.stroke(
                &Path::line(
                    Point::new(if self.pack.crop_bl { cut } else { 2.0 }, h - 1.5),
                    Point::new(br_x, h - 1.5),
                ),
                Stroke::default().with_color(sh).with_width(1.8),
            );
            frame.stroke(
                &Path::line(
                    Point::new(w - 1.5, if self.pack.crop_tr { cut } else { 2.0 }),
                    Point::new(w - 1.5, if self.pack.crop_br { h - cut } else { h - 2.0 }),
                ),
                Stroke::default().with_color(sh).with_width(1.6),
            );
        }

        // 3. Inline Inner Glow (inset luminous border halo inside the button face)
        let ig = self.pack.inline_glow
            + if react {
                self.pack.hover_glow_boost * 0.45
            } else {
                0.0
            };
        if ig > 0.02 {
            let inset = 2.5_f32;
            let iw = (w - inset * 2.0).max(2.0);
            let ih = (h - inset * 2.0).max(2.0);
            let poly = Path::new(|p| {
                let c_tl = if self.pack.crop_tl { (cut - 1.0).max(2.0) } else { 0.0 };
                let c_tr = if self.pack.crop_tr { (cut - 1.0).max(2.0) } else { 0.0 };
                let c_br = if self.pack.crop_br { (cut - 1.0).max(2.0) } else { 0.0 };
                let c_bl = if self.pack.crop_bl { (cut - 1.0).max(2.0) } else { 0.0 };
                p.move_to(Point::new(inset + c_tl, inset));
                p.line_to(Point::new(inset + iw - c_tr, inset));
                if c_tr > 0.0 {
                    p.line_to(Point::new(inset + iw, inset + c_tr));
                }
                p.line_to(Point::new(inset + iw, inset + ih - c_br));
                if c_br > 0.0 {
                    p.line_to(Point::new(inset + iw - c_br, inset + ih));
                }
                p.line_to(Point::new(inset + c_bl, inset + ih));
                if c_bl > 0.0 {
                    p.line_to(Point::new(inset, inset + ih - c_bl));
                }
                p.line_to(Point::new(inset, inset + c_tl));
                p.close();
            });
            frame.stroke(
                &poly,
                Stroke::default()
                    .with_color(Color {
                        a: (0.28 * ig).clamp(0.0, 0.85),
                        ..glow_col
                    })
                    .with_width(3.0),
            );
            frame.stroke(
                &poly,
                Stroke::default()
                    .with_color(Color {
                        a: (0.48 * ig).clamp(0.0, 0.95),
                        ..lift_brightness(glow_col, 0.35)
                    })
                    .with_width(1.1),
            );
        }

        vec![frame.into_geometry()]
    }
}

fn mix_color(a: Color, b: Color, t: f32) -> Color {
    let u = t.clamp(0.0, 1.0);
    Color {
        r: a.r * (1.0 - u) + b.r * u,
        g: a.g * (1.0 - u) + b.g * u,
        b: a.b * (1.0 - u) + b.b * u,
        a: a.a,
    }
}

fn lift_brightness(c: Color, amt: f32) -> Color {
    let u = amt.clamp(0.0, 1.0);
    Color {
        r: c.r + (1.0 - c.r) * u,
        g: c.g + (1.0 - c.g) * u,
        b: c.b + (1.0 - c.b) * u,
        a: c.a,
    }
}

fn darken_color(c: Color, amt: f32) -> Color {
    let f = (1.0 - amt.clamp(0.0, 0.9)).max(0.05);
    Color {
        r: c.r * f,
        g: c.g * f,
        b: c.b * f,
        a: c.a,
    }
}

pub fn shift_hue(col: Color, deg: f32) -> Color {
    if deg.abs() < 0.5 {
        return col;
    }
    let (h, s, l) = rgb_to_hsl(col);
    let out = hsl_to_rgb((h + deg).rem_euclid(360.0), s.max(0.25), l);
    Color { a: col.a, ..out }
}

fn rgb_to_hsl(c: Color) -> (f32, f32, f32) {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let l = (max + min) * 0.5;
    let d = max - min;
    if d < 1e-5 {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let mut h = if (max - c.r).abs() < 1e-5 {
        (c.g - c.b) / d + if c.g < c.b { 6.0 } else { 0.0 }
    } else if (max - c.g).abs() < 1e-5 {
        (c.b - c.r) / d + 2.0
    } else {
        (c.r - c.g) / d + 4.0
    };
    h *= 60.0;
    (h, s, l)
}

fn hsl_to_rgb(h_deg: f32, s: f32, l: f32) -> Color {
    let h = h_deg.rem_euclid(360.0) / 360.0;
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let conv = |mut t: f32| -> f32 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 0.5 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    };
    Color::from_rgb(conv(h + 1.0 / 3.0), conv(h), conv(h - 1.0 / 3.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_pack_selective_crop_and_reactive_hover() {
        let pack = ButtonPackConfig {
            enabled: true,
            preset: "cyberpunk".into(),
            crop_tl: true,
            crop_tr: false,
            crop_br: true,
            crop_bl: false,
            round_tl: 14.0,
            round_tr: 14.0,
            round_br: 14.0,
            round_bl: 14.0,
            gradient_enabled: true,
            hue_strength: 0.7,
            hover_hue_shift_deg: 120.0,
            hover_glow_boost: 1.5,
            ..Default::default()
        };
        set_active_pack(&pack);

        let radii = resolve_radii(&pack, ButtonKind::Pill, 20.0);
        assert_eq!(radii.top_left, 0.0);
        assert_eq!(radii.bottom_right, 0.0);
        assert_eq!(radii.top_right, 14.0);
        assert_eq!(radii.bottom_left, 14.0);

        let theme = crate::theme::resolve_theme("openify", &[]);
        let idle = compute_button_style(
            ButtonKind::Pill,
            false,
            false,
            theme.card,
            theme.card_hover,
            theme.text,
            theme.border,
            20.0,
            &theme,
        );
        let hovered = compute_button_style(
            ButtonKind::Pill,
            false,
            true,
            theme.card,
            theme.card_hover,
            theme.text,
            theme.border,
            20.0,
            &theme,
        );
        assert!(matches!(hovered.background, Some(Background::Gradient(_))));
        assert!(hovered.shadow.blur_radius > idle.shadow.blur_radius);

        set_active_pack(&ButtonPackConfig::default());
    }
}
