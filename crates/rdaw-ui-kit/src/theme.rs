use std::ops::Index;
use std::rc::Rc;

use floem::peniko::Color;
use floem::reactive::{provide_context, use_context, RwSignal};
use palette::{FromColor, IntoColor, Mix, Oklch, Srgb};

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub fonts: Fonts,
    pub colors: Colors,
}

impl Theme {
    pub fn get() -> Rc<Theme> {
        use_context::<RwSignal<Rc<Theme>>>()
            .expect("no theme in scope")
            .get()
    }

    pub fn provide(self) {
        provide_context(RwSignal::new(Rc::new(self)));
    }

    pub fn light() -> Theme {
        Theme {
            fonts: Fonts::default(),
            colors: Colors::light(),
        }
    }

    pub fn dark() -> Theme {
        Theme {
            fonts: Fonts::default(),
            colors: Colors::dark(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fonts {
    pub normal: FontSet,
    pub mono: FontSet,
}

impl Default for Fonts {
    fn default() -> Self {
        Fonts {
            normal: FontSet::new(16.0, "Inter".into()),
            mono: FontSet::new(16.0, "Iosevka".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontSet {
    pub xl4: Font,
    pub xl3: Font,
    pub xl2: Font,
    pub xl: Font,
    pub l: Font,
    pub m: Font,
    pub s: Font,
    pub xs: Font,
}

impl FontSet {
    pub fn new(size: f32, family: String) -> FontSet {
        FontSet {
            xl4: Font::new(size * 2.0, family.clone()),
            xl3: Font::new(size * 1.75, family.clone()),
            xl2: Font::new(size * 1.5, family.clone()),
            xl: Font::new(size * 1.25, family.clone()),
            l: Font::new(size * 1.125, family.clone()),
            m: Font::new(size * 1.0, family.clone()),
            s: Font::new(size * 0.875, family.clone()),
            xs: Font::new(size * 0.75, family),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    pub size: f32,
    pub family: String,
}

impl Font {
    pub fn new(size: f32, family: String) -> Font {
        Font { size, family }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Colors {
    pub surface: ColorLevels,
    pub accent: ColorLevels,
    pub success: ColorLevels,
    pub warning: ColorLevels,
    pub error: ColorLevels,
}

impl Colors {
    pub fn light() -> Colors {
        Colors {
            surface: ColorLevels::new(Color::rgb8(202, 203, 213)),
            accent: ColorLevels::new(Color::rgb8(166, 195, 242)),
            success: ColorLevels::new(Color::rgb8(160, 207, 169)),
            warning: ColorLevels::new(Color::rgb8(254, 214, 134)),
            error: ColorLevels::new(Color::rgb8(237, 150, 160)),
        }
    }

    pub fn dark() -> Colors {
        Colors {
            surface: ColorLevels::new(Color::rgb8(18, 19, 26)),
            accent: ColorLevels::new(Color::rgb8(18, 36, 43)),
            success: ColorLevels::new(Color::rgb8(20, 41, 24)),
            warning: ColorLevels::new(Color::rgb8(51, 36, 28)),
            error: ColorLevels::new(Color::rgb8(34, 11, 21)),
        }
    }
}

impl Index<ColorKind> for Colors {
    type Output = ColorLevels;

    fn index(&self, index: ColorKind) -> &Self::Output {
        match index {
            ColorKind::Surface => &self.surface,
            ColorKind::Accent => &self.accent,
            ColorKind::Success => &self.success,
            ColorKind::Warning => &self.warning,
            ColorKind::Error => &self.error,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ColorKind {
    Surface,
    Accent,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ColorLevels {
    pub lowest: ColorSet,
    pub low: ColorSet,
    pub mid: ColorSet,
    pub high: ColorSet,
    pub highest: ColorSet,
}

impl ColorLevels {
    pub fn new(mid: Color) -> ColorLevels {
        let alpha = mid.a;
        let mid = color_to_oklch(mid);

        let lowest = darken(mid, 0.06);
        let low = darken(mid, 0.03);
        let high = lighten(mid, 0.03);
        let highest = lighten(mid, 0.06);

        ColorLevels {
            lowest: ColorSet::new_oklch(lowest, alpha),
            low: ColorSet::new_oklch(low, alpha),
            mid: ColorSet::new_oklch(mid, alpha),
            high: ColorSet::new_oklch(high, alpha),
            highest: ColorSet::new_oklch(highest, alpha),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Level {
    Lowest,
    Low,
    Mid,
    High,
    Highest,
}

impl Index<Level> for ColorLevels {
    type Output = ColorSet;

    fn index(&self, index: Level) -> &Self::Output {
        match index {
            Level::Lowest => &self.lowest,
            Level::Low => &self.low,
            Level::Mid => &self.mid,
            Level::High => &self.high,
            Level::Highest => &self.highest,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ColorSet {
    pub bg: Color,
    pub fg: Color,
    pub border: Color,

    pub bg_hover: Color,
    pub fg_hover: Color,
    pub border_hover: Color,

    pub bg_active: Color,
    pub fg_active: Color,
    pub border_active: Color,
}

impl ColorSet {
    fn new_oklch(bg: Oklch, alpha: u8) -> ColorSet {
        if bg.l > 0.5 {
            let fg = set_lightness(bg, 0.3);
            let border = fg.mix(bg, 0.6);

            let bg_hover = darken(bg, 0.05);
            let fg_hover = darken(fg, 0.05);
            let border_hover = fg_hover.mix(bg_hover, 0.5);

            let bg_active = darken(bg, 0.1);
            let fg_active = darken(fg, 0.1);
            let border_active = fg_active.mix(bg_active, 0.5);

            ColorSet {
                bg: color_from_oklch(bg, alpha),
                fg: color_from_oklch(fg, alpha),
                border: color_from_oklch(border, alpha),
                bg_hover: color_from_oklch(bg_hover, alpha),
                fg_hover: color_from_oklch(fg_hover, alpha),
                border_hover: color_from_oklch(border_hover, alpha),
                bg_active: color_from_oklch(bg_active, alpha),
                fg_active: color_from_oklch(fg_active, alpha),
                border_active: color_from_oklch(border_active, alpha),
            }
        } else {
            let fg = set_lightness(bg, 0.7);
            let border = fg.mix(bg, 0.7);

            let bg_hover = lighten(bg, 0.05);
            let fg_hover = lighten(fg, 0.05);
            let border_hover = lighten(border, 0.05);

            let bg_active = lighten(bg, 0.1);
            let fg_active = lighten(fg, 0.1);
            let border_active = lighten(border, 0.1);

            ColorSet {
                bg: color_from_oklch(bg, alpha),
                fg: color_from_oklch(fg, alpha),
                border: color_from_oklch(border, alpha),
                bg_hover: color_from_oklch(bg_hover, alpha),
                fg_hover: color_from_oklch(fg_hover, alpha),
                border_hover: color_from_oklch(border_hover, alpha),
                bg_active: color_from_oklch(bg_active, alpha),
                fg_active: color_from_oklch(fg_active, alpha),
                border_active: color_from_oklch(border_active, alpha),
            }
        }
    }
}

fn set_lightness(color: Oklch, set: f32) -> Oklch {
    Oklch { l: set, ..color }
}

fn lighten(color: Oklch, factor: f32) -> Oklch {
    set_lightness(color, (color.l + factor).min(1.0))
}

fn darken(color: Oklch, factor: f32) -> Oklch {
    set_lightness(color, (color.l - factor).max(0.0))
}

fn color_to_oklch(color: Color) -> Oklch {
    Oklch::from_color(Srgb::new(color.r, color.g, color.b).into_format::<f32>())
}

fn color_from_oklch(oklch: Oklch, a: u8) -> Color {
    let color: Srgb<f32> = oklch.into_color();
    let rgb = color.into_format::<u8>();
    Color::rgba8(rgb.red, rgb.green, rgb.blue, a)
}
