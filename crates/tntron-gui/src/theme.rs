use iced::theme::Palette;
use iced::{Color, Theme};
use tntron_core::ExtRegisteredTheme;

#[derive(Debug, Clone)]
pub struct ThemeSpec {
    pub slug: String,
    pub label: String,
    pub is_light: bool,
    pub base: Color,
    pub sidebar: Color,
    pub surface: Color,
    pub card: Color,
    pub card_hover: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent_fg: Color,
    pub danger: Color,
}

impl ThemeSpec {
    pub fn to_iced_theme(&self) -> Theme {
        Theme::custom(
            self.label.clone(),
            Palette {
                background: self.base,
                text: self.text,
                primary: self.accent,
                success: self.accent,
                warning: rgb(0xf5, 0x9e, 0x0b),
                danger: self.danger,
            },
        )
    }

    pub fn from_extension(ext: &ExtRegisteredTheme) -> Self {
        let base = parse_hex_color(&ext.base).unwrap_or(rgb(0x00, 0x00, 0x00));
        let text = parse_hex_color(&ext.text).unwrap_or(rgb(0xff, 0xff, 0xff));
        let accent = parse_hex_color(&ext.accent).unwrap_or(rgb(0x1d, 0xb9, 0x54));
        let is_light = luma(base) > 0.55;
        Self {
            slug: ext.slug.clone(),
            label: ext.label.clone(),
            is_light,
            base,
            sidebar: parse_hex_color(&ext.sidebar).unwrap_or(rgb(0x12, 0x12, 0x12)),
            surface: parse_hex_color(&ext.main).unwrap_or(rgb(0x12, 0x12, 0x12)),
            card: parse_hex_color(&ext.card).unwrap_or(rgb(0x18, 0x18, 0x18)),
            card_hover: parse_hex_color(&ext.card_hover).unwrap_or(rgb(0x28, 0x28, 0x28)),
            border: if is_light {
                rgba(0x00, 0x00, 0x00, 0.12)
            } else {
                rgba(0xff, 0xff, 0xff, 0.10)
            },
            text,
            muted: parse_hex_color(&ext.text_sub).unwrap_or(rgb(0xb3, 0xb3, 0xb3)),
            accent,
            accent_fg: if luma(accent) > 0.50 {
                Color::BLACK
            } else {
                Color::WHITE
            },
            danger: rgb(0xef, 0x44, 0x44),
        }
    }
}

struct BuiltinDef {
    slug: &'static str,
    label: &'static str,
    is_light: bool,
    base: Color,
    sidebar: Color,
    surface: Color,
    card: Color,
    card_hover: Color,
    text: Color,
    muted: Color,
    accent: Color,
    accent_fg: Color,
}

impl BuiltinDef {
    fn to_spec(&self) -> ThemeSpec {
        ThemeSpec {
            slug: self.slug.to_string(),
            label: self.label.to_string(),
            is_light: self.is_light,
            base: self.base,
            sidebar: self.sidebar,
            surface: self.surface,
            card: self.card,
            card_hover: self.card_hover,
            border: if self.is_light {
                rgba(0x00, 0x00, 0x00, 0.12)
            } else {
                rgba(0xff, 0xff, 0xff, 0.10)
            },
            text: self.text,
            muted: self.muted,
            accent: self.accent,
            accent_fg: self.accent_fg,
            danger: rgb(0xef, 0x44, 0x44),
        }
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color::from_rgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a)
}

fn luma(c: Color) -> f32 {
    0.299 * c.r + 0.587 * c.g + 0.114 * c.b
}

pub fn parse_hex_color(raw: &str) -> Option<Color> {
    let hex = raw.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::from_rgb8(r, g, b))
}

const BUILTIN_THEMES: &[BuiltinDef] = &[
    BuiltinDef {
        slug: "openify",
        label: "Openify",
        is_light: false,
        base: rgb(0x00, 0x00, 0x00),
        sidebar: rgb(0x12, 0x12, 0x12),
        surface: rgb(0x12, 0x12, 0x12),
        card: rgb(0x18, 0x18, 0x18),
        card_hover: rgb(0x28, 0x28, 0x28),
        text: rgb(0xff, 0xff, 0xff),
        muted: rgb(0xb3, 0xb3, 0xb3),
        accent: rgb(0x1d, 0xb9, 0x54),
        accent_fg: rgb(0x00, 0x00, 0x00),
    },
    BuiltinDef {
        slug: "dark",
        label: "Pure Dark",
        is_light: false,
        base: rgb(0x0a, 0x0a, 0x0a),
        sidebar: rgb(0x14, 0x14, 0x14),
        surface: rgb(0x14, 0x14, 0x14),
        card: rgb(0x1a, 0x1a, 0x1a),
        card_hover: rgb(0x26, 0x26, 0x26),
        text: rgb(0xff, 0xff, 0xff),
        muted: rgb(0x88, 0x88, 0x88),
        accent: rgb(0xff, 0xff, 0xff),
        accent_fg: rgb(0x00, 0x00, 0x00),
    },
    BuiltinDef {
        slug: "light",
        label: "Daylight",
        is_light: true,
        base: rgb(0xf4, 0xf4, 0xf6),
        sidebar: rgb(0xff, 0xff, 0xff),
        surface: rgb(0xf9, 0xf9, 0xfb),
        card: rgb(0xea, 0xec, 0xef),
        card_hover: rgb(0xdf, 0xe2, 0xe6),
        text: rgb(0x18, 0x18, 0x1b),
        muted: rgb(0x52, 0x52, 0x5b),
        accent: rgb(0x18, 0x18, 0x1b),
        accent_fg: rgb(0xff, 0xff, 0xff),
    },
    BuiltinDef {
        slug: "blue",
        label: "Deep Blue",
        is_light: false,
        base: rgb(0x05, 0x0a, 0x14),
        sidebar: rgb(0x0b, 0x15, 0x28),
        surface: rgb(0x0f, 0x1d, 0x36),
        card: rgb(0x17, 0x2a, 0x4f),
        card_hover: rgb(0x1f, 0x39, 0x6b),
        text: rgb(0xf0, 0xf7, 0xff),
        muted: rgb(0x94, 0xa3, 0xb8),
        accent: rgb(0x3b, 0x82, 0xf6),
        accent_fg: rgb(0xff, 0xff, 0xff),
    },
    BuiltinDef {
        slug: "purple",
        label: "Amethyst",
        is_light: false,
        base: rgb(0x0d, 0x07, 0x14),
        sidebar: rgb(0x18, 0x0d, 0x26),
        surface: rgb(0x21, 0x12, 0x33),
        card: rgb(0x2d, 0x1a, 0x45),
        card_hover: rgb(0x3b, 0x22, 0x59),
        text: rgb(0xfa, 0xf5, 0xff),
        muted: rgb(0xa7, 0x8b, 0xfa),
        accent: rgb(0xa8, 0x55, 0xf7),
        accent_fg: rgb(0xff, 0xff, 0xff),
    },
    BuiltinDef {
        slug: "red",
        label: "Crimson",
        is_light: false,
        base: rgb(0x14, 0x05, 0x05),
        sidebar: rgb(0x24, 0x0a, 0x0a),
        surface: rgb(0x2e, 0x0e, 0x0e),
        card: rgb(0x42, 0x15, 0x15),
        card_hover: rgb(0x57, 0x1d, 0x1d),
        text: rgb(0xff, 0xf5, 0xf5),
        muted: rgb(0xf8, 0x71, 0x71),
        accent: rgb(0xef, 0x44, 0x44),
        accent_fg: rgb(0xff, 0xff, 0xff),
    },
    BuiltinDef {
        slug: "gold",
        label: "Gilded",
        is_light: false,
        base: rgb(0x12, 0x0e, 0x02),
        sidebar: rgb(0x1f, 0x18, 0x04),
        surface: rgb(0x29, 0x20, 0x06),
        card: rgb(0x3a, 0x2d, 0x09),
        card_hover: rgb(0x4d, 0x3c, 0x0e),
        text: rgb(0xfe, 0xfc, 0xe8),
        muted: rgb(0xfa, 0xcc, 0x15),
        accent: rgb(0xea, 0xb3, 0x08),
        accent_fg: rgb(0x00, 0x00, 0x00),
    },
    BuiltinDef {
        slug: "cyberpunk",
        label: "Cyberpunk",
        is_light: false,
        base: rgb(0x08, 0x04, 0x12),
        sidebar: rgb(0x12, 0x09, 0x26),
        surface: rgb(0x18, 0x0c, 0x33),
        card: rgb(0x24, 0x12, 0x4d),
        card_hover: rgb(0x31, 0x18, 0x69),
        text: rgb(0x00, 0xf0, 0xff),
        muted: rgb(0xc0, 0x26, 0xd3),
        accent: rgb(0xff, 0x00, 0x7f),
        accent_fg: rgb(0xff, 0xff, 0xff),
    },
    BuiltinDef {
        slug: "terminal",
        label: "Terminal",
        is_light: false,
        base: rgb(0x00, 0x05, 0x00),
        sidebar: rgb(0x00, 0x10, 0x00),
        surface: rgb(0x00, 0x14, 0x00),
        card: rgb(0x00, 0x22, 0x00),
        card_hover: rgb(0x00, 0x33, 0x00),
        text: rgb(0x00, 0xff, 0x00),
        muted: rgb(0x00, 0xaa, 0x00),
        accent: rgb(0x00, 0xff, 0x00),
        accent_fg: rgb(0x00, 0x00, 0x00),
    },
    BuiltinDef {
        slug: "pharoh",
        label: "Pharoh",
        is_light: false,
        base: rgb(0x1a, 0x14, 0x09),
        sidebar: rgb(0x26, 0x1d, 0x0d),
        surface: rgb(0x33, 0x27, 0x12),
        card: rgb(0x47, 0x36, 0x19),
        card_hover: rgb(0x5c, 0x46, 0x21),
        text: rgb(0xfa, 0xe8, 0xb4),
        muted: rgb(0xc2, 0xa6, 0x61),
        accent: rgb(0xd4, 0xaf, 0x37),
        accent_fg: rgb(0x00, 0x00, 0x00),
    },
    BuiltinDef {
        slug: "turk",
        label: "Crimson Moon",
        is_light: false,
        base: rgb(0x14, 0x03, 0x04),
        sidebar: rgb(0x24, 0x06, 0x08),
        surface: rgb(0x30, 0x09, 0x0c),
        card: rgb(0x47, 0x10, 0x15),
        card_hover: rgb(0x5e, 0x16, 0x1d),
        text: rgb(0xff, 0xff, 0xff),
        muted: rgb(0xfc, 0xa5, 0xa5),
        accent: rgb(0xe3, 0x0a, 0x17),
        accent_fg: rgb(0xff, 0xff, 0xff),
    },
    BuiltinDef {
        slug: "clouds",
        label: "Clouds",
        is_light: true,
        base: rgb(0xe8, 0xf1, 0xf8),
        sidebar: rgb(0xff, 0xff, 0xff),
        surface: rgb(0xf2, 0xf7, 0xfc),
        card: rgb(0xd9, 0xe8, 0xf5),
        card_hover: rgb(0xc6, 0xdc, 0xf0),
        text: rgb(0x1e, 0x3a, 0x5f),
        muted: rgb(0x5a, 0x7c, 0x9e),
        accent: rgb(0x38, 0xbd, 0xf8),
        accent_fg: rgb(0x00, 0x00, 0x00),
    },
];

pub fn all_themes(ext_themes: &[ExtRegisteredTheme]) -> Vec<ThemeSpec> {
    let mut out: Vec<ThemeSpec> = BUILTIN_THEMES.iter().map(BuiltinDef::to_spec).collect();
    for et in ext_themes {
        out.push(ThemeSpec::from_extension(et));
    }
    out
}

pub fn resolve_theme(slug: &str, ext_themes: &[ExtRegisteredTheme]) -> ThemeSpec {
    if let Some(et) = ext_themes.iter().find(|t| t.slug == slug) {
        return ThemeSpec::from_extension(et);
    }
    BUILTIN_THEMES
        .iter()
        .find(|t| t.slug == slug)
        .unwrap_or(&BUILTIN_THEMES[0])
        .to_spec()
}
