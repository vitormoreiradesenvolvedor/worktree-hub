//! Design tokens and egui style: one stone-green accent over a black canvas,
//! generous rounding, soft shadows, IBM Plex Sans for the interface and
//! IBM Plex Mono for terminals.

use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow, Stroke,
    Vec2,
};
use std::sync::Arc;

/// "Verde pedra": a jade green that holds up on pure black.
pub const BRAND: Color32 = Color32::from_rgb(0x1F, 0xB1, 0x84);
pub const BRAND_DEEP: Color32 = Color32::from_rgb(0x0C, 0x5F, 0x48);
pub const BRAND_SOFT: Color32 = Color32::from_rgb(0x5F, 0xDD, 0xB2);

pub const R_CARD: u8 = 18;
pub const R_INPUT: u8 = 12;
pub const R_PILL: u8 = 100;

pub const S1: f32 = 4.0;
pub const S2: f32 = 8.0;
pub const S3: f32 = 12.0;
pub const S4: f32 = 16.0;
pub const S5: f32 = 24.0;
pub const S6: f32 = 32.0;

pub const SIDEBAR_W: f32 = 248.0;
/// Width of the sidebar when it is folded down to icons.
pub const RAIL_W: f32 = 64.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Light,
    #[default]
    Dark,
}

impl Mode {
    pub fn is_dark(self) -> bool {
        self == Mode::Dark
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Light => crate::i18n::tr("theme.light"),
            Mode::Dark => crate::i18n::tr("theme.dark"),
        }
    }
}

/// Accent colors the user can pick from.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    #[default]
    Green,
    Gray,
    Blue,
    Yellow,
    Purple,
}

impl Accent {
    pub const ALL: [Accent; 5] = [
        Accent::Green,
        Accent::Gray,
        Accent::Blue,
        Accent::Yellow,
        Accent::Purple,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Accent::Green => crate::i18n::tr("accent.green"),
            Accent::Gray => crate::i18n::tr("accent.gray"),
            Accent::Blue => crate::i18n::tr("accent.blue"),
            Accent::Yellow => crate::i18n::tr("accent.yellow"),
            Accent::Purple => crate::i18n::tr("accent.purple"),
        }
    }

    /// brand, deep, soft, tint, on_brand.
    fn colors(self, mode: Mode) -> [Color32; 5] {
        let rgb = Color32::from_rgb;
        match (self, mode) {
            (Accent::Green, Mode::Dark) => [
                BRAND,
                BRAND_DEEP,
                BRAND_SOFT,
                rgb(0x0C, 0x2A, 0x21),
                rgb(0x03, 0x12, 0x0D),
            ],
            (Accent::Green, Mode::Light) => [
                rgb(0x0C, 0x7F, 0x5E),
                rgb(0x07, 0x5A, 0x42),
                rgb(0x43, 0xB9, 0x94),
                rgb(0xE4, 0xF6, 0xEF),
                Color32::WHITE,
            ],
            (Accent::Gray, Mode::Dark) => [
                rgb(0xB6, 0xC2, 0xBE),
                rgb(0x5A, 0x67, 0x64),
                rgb(0xD8, 0xE0, 0xDE),
                rgb(0x1B, 0x22, 0x20),
                rgb(0x0A, 0x0F, 0x0E),
            ],
            (Accent::Gray, Mode::Light) => [
                rgb(0x4B, 0x59, 0x56),
                rgb(0x2F, 0x38, 0x36),
                rgb(0x8A, 0x97, 0x94),
                rgb(0xED, 0xF1, 0xF0),
                Color32::WHITE,
            ],
            (Accent::Blue, Mode::Dark) => [
                rgb(0x3B, 0x82, 0xF6),
                rgb(0x1E, 0x40, 0xAF),
                rgb(0x93, 0xB4, 0xFF),
                rgb(0x12, 0x1C, 0x33),
                Color32::WHITE,
            ],
            (Accent::Blue, Mode::Light) => [
                rgb(0x1D, 0x4E, 0xD8),
                rgb(0x16, 0x32, 0x8F),
                rgb(0x6B, 0x93, 0xF0),
                rgb(0xE8, 0xEF, 0xFD),
                Color32::WHITE,
            ],
            (Accent::Yellow, Mode::Dark) => [
                rgb(0xF2, 0xC1, 0x4E),
                rgb(0x8A, 0x6B, 0x16),
                rgb(0xFF, 0xDE, 0x94),
                rgb(0x2A, 0x22, 0x10),
                rgb(0x1A, 0x14, 0x03),
            ],
            (Accent::Yellow, Mode::Light) => [
                rgb(0x9A, 0x6B, 0x00),
                rgb(0x71, 0x4D, 0x00),
                rgb(0xD9, 0xA9, 0x3C),
                rgb(0xFC, 0xF3, 0xE0),
                Color32::WHITE,
            ],
            (Accent::Purple, Mode::Dark) => [
                rgb(0xA8, 0x55, 0xF7),
                rgb(0x6B, 0x21, 0xA8),
                rgb(0xD8, 0xB4, 0xFE),
                rgb(0x22, 0x12, 0x33),
                Color32::WHITE,
            ],
            (Accent::Purple, Mode::Light) => [
                rgb(0x7C, 0x3A, 0xED),
                rgb(0x5B, 0x21, 0xB6),
                rgb(0xA7, 0x8B, 0xFA),
                rgb(0xF2, 0xEB, 0xFE),
                Color32::WHITE,
            ],
        }
    }
}

/// Interface font.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UiFont {
    #[default]
    Plex,
    Inter,
    SourceSans,
    FiraSans,
}

impl UiFont {
    pub const ALL: [UiFont; 4] = [
        UiFont::Plex,
        UiFont::Inter,
        UiFont::SourceSans,
        UiFont::FiraSans,
    ];

    pub fn label(self) -> &'static str {
        match self {
            UiFont::Plex => "IBM Plex Sans",
            UiFont::Inter => "Inter",
            UiFont::SourceSans => "Source Sans 3",
            UiFont::FiraSans => "Fira Sans",
        }
    }
}

/// Terminal font.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MonoFont {
    #[default]
    Plex,
    JetBrains,
    FiraCode,
    SourceCode,
    Cascadia,
}

impl MonoFont {
    pub const ALL: [MonoFont; 5] = [
        MonoFont::Plex,
        MonoFont::JetBrains,
        MonoFont::FiraCode,
        MonoFont::SourceCode,
        MonoFont::Cascadia,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MonoFont::Plex => "IBM Plex Mono",
            MonoFont::JetBrains => "JetBrains Mono",
            MonoFont::FiraCode => "Fira Code",
            MonoFont::SourceCode => "Source Code Pro",
            MonoFont::Cascadia => "Cascadia Mono",
        }
    }
}

/// Everything that decides how the app looks.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Look {
    pub mode: Mode,
    pub accent: Accent,
    /// 0..1 — the window itself, cards included.
    pub app_opacity: f32,
    /// 0..1 — the terminal background only.
    pub term_opacity: f32,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            mode: Mode::default(),
            accent: Accent::default(),
            app_opacity: 1.0,
            term_opacity: 1.0,
        }
    }
}

/// Same color, different alpha. Color32 is premultiplied, so go through
/// `from_rgba_unmultiplied`.
pub fn with_alpha(color: Color32, alpha: f32) -> Color32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a)
}

/// Every color the UI is allowed to use.
#[derive(Clone, Copy)]
pub struct Tokens {
    pub brand: Color32,
    pub brand_deep: Color32,
    pub brand_soft: Color32,
    pub brand_tint: Color32,
    pub on_brand: Color32,

    pub bg: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    pub surface_hover: Color32,
    pub border: Color32,
    pub border_strong: Color32,

    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,

    pub success: Color32,
    pub success_tint: Color32,
    pub warning: Color32,
    pub warning_tint: Color32,
    pub danger: Color32,
    pub danger_tint: Color32,
    pub info: Color32,
    pub info_tint: Color32,

    pub term_bg: Color32,
    pub term_fg: Color32,
    pub term_cursor: Color32,
    pub term_selection: Color32,
    pub ansi: [Color32; 16],

    pub shadow: Color32,
}

pub fn tokens(look: Look) -> Tokens {
    let mode = look.mode;
    // Terminal palette tuned for a black background with a green cast.
    let ansi = [
        Color32::from_rgb(0x1A, 0x22, 0x20), // black
        Color32::from_rgb(0xFF, 0x6E, 0x6E), // red
        Color32::from_rgb(0x3D, 0xDC, 0x97), // green
        Color32::from_rgb(0xF5, 0xC2, 0x6B), // yellow
        Color32::from_rgb(0x6F, 0xA8, 0xFF), // blue
        Color32::from_rgb(0xC7, 0x92, 0xEA), // magenta
        Color32::from_rgb(0x4C, 0xD6, 0xD6), // cyan
        Color32::from_rgb(0xD8, 0xE5, 0xE1), // white
        Color32::from_rgb(0x4C, 0x5A, 0x57),
        Color32::from_rgb(0xFF, 0x92, 0x92),
        Color32::from_rgb(0x74, 0xE8, 0xB6),
        Color32::from_rgb(0xFF, 0xD8, 0x8A),
        Color32::from_rgb(0x9B, 0xC4, 0xFF),
        Color32::from_rgb(0xDE, 0xB3, 0xFF),
        Color32::from_rgb(0x85, 0xE8, 0xE8),
        Color32::from_rgb(0xFF, 0xFF, 0xFF),
    ];

    let [brand, brand_deep, brand_soft, brand_tint, on_brand] = look.accent.colors(mode);

    let base = match mode {
        Mode::Light => Tokens {
            brand,
            brand_deep,
            brand_soft,
            brand_tint,
            on_brand,

            bg: Color32::from_rgb(0xF3, 0xF6, 0xF5),
            surface: Color32::WHITE,
            surface_alt: Color32::from_rgb(0xF8, 0xFB, 0xFA),
            surface_hover: Color32::from_rgb(0xEC, 0xF6, 0xF2),
            border: Color32::from_rgb(0xE6, 0xEC, 0xEA),
            border_strong: Color32::from_rgb(0xD1, 0xDC, 0xD8),

            text: Color32::from_rgb(0x12, 0x1A, 0x18),
            text_dim: Color32::from_rgb(0x5E, 0x6E, 0x6A),
            text_faint: Color32::from_rgb(0x8E, 0x9E, 0x9A),

            success: Color32::from_rgb(0x0B, 0x8A, 0x55),
            success_tint: Color32::from_rgb(0xE2, 0xF6, 0xEC),
            warning: Color32::from_rgb(0xA8, 0x6C, 0x00),
            warning_tint: Color32::from_rgb(0xFD, 0xF3, 0xE0),
            danger: Color32::from_rgb(0xCE, 0x2E, 0x3A),
            danger_tint: Color32::from_rgb(0xFD, 0xEC, 0xEE),
            info: Color32::from_rgb(0x1E, 0x66, 0xD0),
            info_tint: Color32::from_rgb(0xE8, 0xF0, 0xFD),

            term_bg: Color32::BLACK,
            term_fg: Color32::from_rgb(0xD8, 0xE5, 0xE1),
            term_cursor: Color32::from_rgb(0x1F, 0xD1, 0x99),
            term_selection: Color32::from_rgb(0x16, 0x44, 0x3A),
            ansi,

            shadow: Color32::from_black_alpha(26),
        },
        Mode::Dark => Tokens {
            brand,
            brand_deep,
            brand_soft,
            brand_tint,
            on_brand,

            bg: Color32::BLACK,
            surface: Color32::from_rgb(0x0B, 0x0E, 0x0D),
            surface_alt: Color32::from_rgb(0x12, 0x17, 0x16),
            surface_hover: Color32::from_rgb(0x19, 0x21, 0x1E),
            border: Color32::from_rgb(0x1C, 0x23, 0x21),
            border_strong: Color32::from_rgb(0x36, 0x42, 0x3E),

            text: Color32::from_rgb(0xE9, 0xF1, 0xEE),
            text_dim: Color32::from_rgb(0x93, 0xA4, 0x9F),
            text_faint: Color32::from_rgb(0x64, 0x73, 0x6F),

            success: Color32::from_rgb(0x3D, 0xDC, 0x97),
            success_tint: Color32::from_rgb(0x0E, 0x2B, 0x1F),
            warning: Color32::from_rgb(0xF2, 0xB0, 0x4A),
            warning_tint: Color32::from_rgb(0x2A, 0x1F, 0x0E),
            danger: Color32::from_rgb(0xFF, 0x6B, 0x6B),
            danger_tint: Color32::from_rgb(0x2C, 0x14, 0x16),
            info: Color32::from_rgb(0x58, 0xA6, 0xFF),
            info_tint: Color32::from_rgb(0x0E, 0x1C, 0x2E),

            term_bg: Color32::BLACK,
            term_fg: Color32::from_rgb(0xD8, 0xE5, 0xE1),
            term_cursor: Color32::from_rgb(0x1F, 0xD1, 0x99),
            term_selection: Color32::from_rgb(0x16, 0x44, 0x3A),
            ansi,

            shadow: Color32::from_black_alpha(170),
        },
    };

    Tokens {
        // The cursor follows the accent, so the terminal belongs to the theme.
        term_cursor: brand_soft,
        // Transparency: only the surfaces, never the text.
        bg: with_alpha(base.bg, look.app_opacity),
        surface: with_alpha(base.surface, look.app_opacity),
        surface_alt: with_alpha(base.surface_alt, look.app_opacity),
        surface_hover: with_alpha(base.surface_hover, look.app_opacity),
        term_bg: with_alpha(base.term_bg, look.term_opacity),
        ..base
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Medium,
    SemiBold,
    Bold,
}

const F_REGULAR: &str = "plex";
const F_MEDIUM: &str = "plex-medium";
const F_SEMIBOLD: &str = "plex-semibold";
const F_BOLD: &str = "plex-bold";
const F_MONO: &str = "plexmono";
const F_MONO_BOLD: &str = "plexmono-bold";
const F_CJK: &str = "cjk";

pub fn font(size: f32, weight: Weight) -> FontId {
    let name = match weight {
        Weight::Regular => F_REGULAR,
        Weight::Medium => F_MEDIUM,
        Weight::SemiBold => F_SEMIBOLD,
        Weight::Bold => F_BOLD,
    };
    FontId::new(size, FontFamily::Name(name.into()))
}

pub fn mono(size: f32, bold: bool) -> FontId {
    let name = if bold { F_MONO_BOLD } else { F_MONO };
    FontId::new(size, FontFamily::Name(name.into()))
}

pub fn install_fonts(ctx: &egui::Context, ui_font: UiFont, mono_font: MonoFont) {
    let mut fonts = FontDefinitions::default();

    let sans: [&'static [u8]; 4] = match ui_font {
        UiFont::Plex => [
            include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSans-Medium.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf"),
            include_bytes!("../assets/fonts/IBMPlexSans-Bold.ttf"),
        ],
        UiFont::Inter => [
            include_bytes!("../assets/fonts/Inter-Regular.ttf"),
            include_bytes!("../assets/fonts/Inter-Medium.ttf"),
            include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
            include_bytes!("../assets/fonts/Inter-Bold.ttf"),
        ],
        UiFont::SourceSans => [
            include_bytes!("../assets/fonts/SourceSans3-Regular.ttf"),
            include_bytes!("../assets/fonts/SourceSans3-Medium.ttf"),
            include_bytes!("../assets/fonts/SourceSans3-Semibold.ttf"),
            include_bytes!("../assets/fonts/SourceSans3-Bold.ttf"),
        ],
        UiFont::FiraSans => [
            include_bytes!("../assets/fonts/FiraSans-Regular.ttf"),
            include_bytes!("../assets/fonts/FiraSans-Medium.ttf"),
            include_bytes!("../assets/fonts/FiraSans-SemiBold.ttf"),
            include_bytes!("../assets/fonts/FiraSans-Bold.ttf"),
        ],
    };
    let mono: [&'static [u8]; 2] = match mono_font {
        MonoFont::Plex => [
            include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf"),
            include_bytes!("../assets/fonts/IBMPlexMono-Bold.ttf"),
        ],
        MonoFont::JetBrains => [
            include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
            include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf"),
        ],
        MonoFont::FiraCode => [
            include_bytes!("../assets/fonts/FiraCode-Regular.ttf"),
            include_bytes!("../assets/fonts/FiraCode-Bold.ttf"),
        ],
        MonoFont::SourceCode => [
            include_bytes!("../assets/fonts/SourceCodePro-Regular.ttf"),
            include_bytes!("../assets/fonts/SourceCodePro-Bold.ttf"),
        ],
        MonoFont::Cascadia => [
            include_bytes!("../assets/fonts/CascadiaMono-Regular.ttf"),
            include_bytes!("../assets/fonts/CascadiaMono-Bold.ttf"),
        ],
    };

    let mut add = |key: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(key.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    add(F_REGULAR, sans[0]);
    add(F_MEDIUM, sans[1]);
    add(F_SEMIBOLD, sans[2]);
    add(F_BOLD, sans[3]);
    add(F_MONO, mono[0]);
    add(F_MONO_BOLD, mono[1]);

    // Chinese (and any other CJK text) needs a font we cannot embed without
    // adding tens of megabytes, so borrow one from the system when it is there.
    let cjk = system_cjk().map(|(bytes, index)| {
        let data = FontData::from_owned(bytes).tweak(egui::FontTweak::default());
        fonts
            .font_data
            .insert(F_CJK.to_owned(), Arc::new(FontData { index, ..data }));
        F_CJK.to_owned()
    });

    // Named families, so widgets can ask for a weight directly.
    for key in [F_REGULAR, F_MEDIUM, F_SEMIBOLD, F_BOLD, F_MONO, F_MONO_BOLD] {
        fonts
            .families
            .insert(FontFamily::Name(key.into()), vec![key.to_owned()]);
    }

    // Defaults: the chosen sans for text, the chosen mono for code, egui's
    // built-ins as glyph fallback (emoji, box drawing).
    let fallback_prop = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let fallback_mono = fonts
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    let mut prop = vec![F_REGULAR.to_owned()];
    prop.extend(cjk.clone());
    prop.extend(fallback_prop);
    let mut monos = vec![F_MONO.to_owned()];
    monos.extend(cjk.clone());
    monos.extend(fallback_mono);

    fonts
        .families
        .insert(FontFamily::Proportional, prop.clone());
    fonts.families.insert(FontFamily::Monospace, monos.clone());

    // Fallbacks for the named families too, so a missing glyph never renders as
    // a blank box in a card title or a terminal cell.
    for key in [F_MEDIUM, F_SEMIBOLD, F_BOLD] {
        let mut chain = vec![key.to_owned()];
        chain.extend(prop.iter().cloned());
        fonts.families.insert(FontFamily::Name(key.into()), chain);
    }
    for key in [F_MONO, F_MONO_BOLD] {
        let mut chain = vec![key.to_owned()];
        chain.extend(monos.iter().cloned());
        fonts.families.insert(FontFamily::Name(key.into()), chain);
    }

    ctx.set_fonts(fonts);
}

/// Bytes and collection index of a system CJK font, if one is installed.
pub fn system_cjk() -> Option<(Vec<u8>, u32)> {
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/google-noto-sans-cjk-vf-fonts/NotoSansCJK-VF.ttc",
        "/usr/share/fonts/google-noto-sans-cjk-fonts/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/wqy-zenhei/wqy-zenhei.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/simhei.ttf",
    ];
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            return Some((bytes, 0));
        }
    }
    None
}

pub fn apply(ctx: &egui::Context, look: Look) {
    let t = tokens(look);
    let mode = look.mode;
    ctx.set_theme(match mode {
        Mode::Dark => egui::ThemePreference::Dark,
        Mode::Light => egui::ThemePreference::Light,
    });
    ctx.all_styles_mut(|style| style_for(style, t, mode));
}

/// All visual overrides live here so both egui themes get the same treatment.
fn style_for(style: &mut egui::Style, t: Tokens, mode: Mode) {
    style.visuals.dark_mode = mode.is_dark();
    style.visuals.panel_fill = t.bg;
    style.visuals.window_fill = t.surface;
    style.visuals.window_stroke = Stroke::new(1.0, t.border);
    style.visuals.window_corner_radius = CornerRadius::same(R_CARD);
    style.visuals.window_shadow = card_shadow(t);
    style.visuals.popup_shadow = card_shadow(t);
    style.visuals.menu_corner_radius = CornerRadius::same(R_INPUT);
    style.visuals.faint_bg_color = t.surface_alt;
    style.visuals.extreme_bg_color = t.surface_alt;
    style.visuals.text_edit_bg_color = Some(t.surface_alt);
    style.visuals.code_bg_color = t.surface_alt;
    style.visuals.override_text_color = Some(t.text);
    style.visuals.hyperlink_color = t.brand;
    style.visuals.warn_fg_color = t.warning;
    style.visuals.error_fg_color = t.danger;
    style.visuals.selection.bg_fill = t.brand.gamma_multiply(0.35);
    style.visuals.selection.stroke = Stroke::new(1.0, t.brand);

    let w = &mut style.visuals.widgets;
    w.noninteractive.bg_fill = t.surface;
    w.noninteractive.weak_bg_fill = t.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, t.text_dim);
    w.noninteractive.corner_radius = CornerRadius::same(R_INPUT);

    w.inactive.bg_fill = t.surface_alt;
    w.inactive.weak_bg_fill = t.surface_alt;
    w.inactive.bg_stroke = Stroke::new(1.0, t.border);
    w.inactive.fg_stroke = Stroke::new(1.0, t.text);
    w.inactive.corner_radius = CornerRadius::same(R_INPUT);
    w.inactive.expansion = 0.0;

    w.hovered.bg_fill = t.surface_hover;
    w.hovered.weak_bg_fill = t.surface_hover;
    w.hovered.bg_stroke = Stroke::new(1.0, t.brand.gamma_multiply(0.5));
    w.hovered.fg_stroke = Stroke::new(1.0, t.text);
    w.hovered.corner_radius = CornerRadius::same(R_INPUT);
    w.hovered.expansion = 0.0;

    w.active.bg_fill = t.brand_tint;
    w.active.weak_bg_fill = t.brand_tint;
    w.active.bg_stroke = Stroke::new(1.5, t.brand);
    w.active.fg_stroke = Stroke::new(1.0, t.text);
    w.active.corner_radius = CornerRadius::same(R_INPUT);
    w.active.expansion = 0.0;

    w.open.bg_fill = t.surface_alt;
    w.open.weak_bg_fill = t.surface_alt;
    w.open.bg_stroke = Stroke::new(1.0, t.border_strong);
    w.open.fg_stroke = Stroke::new(1.0, t.text);
    w.open.corner_radius = CornerRadius::same(R_INPUT);

    style.spacing.item_spacing = Vec2::new(S2, S2);
    style.spacing.button_padding = Vec2::new(S3, S2);
    style.spacing.interact_size = Vec2::new(40.0, 32.0);
    style.spacing.window_margin = Margin::same(S5 as i8);
    style.spacing.menu_margin = Margin::same(S2 as i8);
    style.spacing.icon_width = 18.0;
    style.spacing.icon_width_inner = 10.0;
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating = true;

    use egui::TextStyle;
    style.text_styles = [
        (TextStyle::Heading, font(22.0, Weight::SemiBold)),
        (TextStyle::Body, font(14.0, Weight::Regular)),
        (TextStyle::Button, font(14.0, Weight::SemiBold)),
        (TextStyle::Small, font(12.0, Weight::Regular)),
        (TextStyle::Monospace, mono(13.0, false)),
    ]
    .into();
}

pub fn card_shadow(t: Tokens) -> Shadow {
    Shadow {
        offset: [0, 6],
        blur: 20,
        spread: 0,
        color: t.shadow,
    }
}

pub fn soft_shadow(t: Tokens) -> Shadow {
    Shadow {
        offset: [0, 2],
        blur: 10,
        spread: 0,
        color: t.shadow,
    }
}
