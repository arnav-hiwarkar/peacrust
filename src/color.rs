#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Ansi(u8),
    Idx(u8),
    Rgb(u8, u8, u8),
}

const NAMES: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

const ANSI_RGB: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (205, 0, 0),
    (0, 205, 0),
    (205, 205, 0),
    (0, 0, 238),
    (205, 0, 205),
    (0, 205, 205),
    (229, 229, 229),
    (127, 127, 127),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (92, 92, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];

impl Color {
    pub fn parse(s: &str) -> Option<Color> {
        let s = s.trim().to_ascii_lowercase();
        if let Some(hex) = s.strip_prefix('#') {
            return parse_hex(hex);
        }
        if let Ok(n) = s.parse::<u8>() {
            return Some(Color::Idx(n));
        }
        let (bright, name) = match s.strip_prefix("bright-") {
            Some(rest) => (8, rest),
            None => (0, s.as_str()),
        };
        NAMES
            .iter()
            .position(|n| *n == name)
            .map(|i| Color::Ansi(i as u8 + bright))
    }

    pub fn to_rgb(self) -> (u8, u8, u8) {
        match self {
            Color::Rgb(r, g, b) => (r, g, b),
            Color::Ansi(n) => ANSI_RGB[(n & 15) as usize],
            Color::Idx(n) if n < 16 => ANSI_RGB[n as usize],
            Color::Idx(n) if n < 232 => {
                let n = n - 16;
                let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                (level(n / 36), level(n / 6 % 6), level(n % 6))
            }
            Color::Idx(n) => {
                let v = 8 + (n - 232) * 10;
                (v, v, v)
            }
        }
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Color::Ansi(n) if n >= 8 => write!(f, "bright-{}", NAMES[(n - 8) as usize & 7]),
            Color::Ansi(n) => write!(f, "{}", NAMES[n as usize & 7]),
            Color::Idx(n) => write!(f, "{n}"),
            Color::Rgb(r, g, b) => write!(f, "#{r:02x}{g:02x}{b:02x}"),
        }
    }
}

fn parse_hex(hex: &str) -> Option<Color> {
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let digit = |i: usize, len: usize| u8::from_str_radix(&hex[i..i + len], 16).ok();
    match hex.len() {
        3 => Some(Color::Rgb(
            digit(0, 1)? * 17,
            digit(1, 1)? * 17,
            digit(2, 1)? * 17,
        )),
        6 => Some(Color::Rgb(digit(0, 2)?, digit(2, 2)?, digit(4, 2)?)),
        _ => None,
    }
}

/// Hue in degrees 0..360, saturation and luminosity in percent 0..100.
pub fn to_hsl((r, g, b): (u8, u8, u8)) -> (f64, f64, f64) {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return (0.0, 0.0, l * 100.0);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, s * 100.0, l * 100.0)
}

pub fn from_hsl(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let (s, l) = (s / 100.0, l / 100.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as u32 % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to(r), to(g), to(b))
}

/// Shift a color in HSL space. A missing color starts from white.
pub fn adjust(color: Option<Color>, dh: f64, ds: f64, dl: f64) -> Color {
    let (h, s, l) = to_hsl(color.map_or((255, 255, 255), Color::to_rgb));
    let (r, g, b) = from_hsl(
        (h + dh).rem_euclid(360.0),
        (s + ds).clamp(0.0, 100.0),
        (l + dl).clamp(0.0, 100.0),
    );
    Color::Rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_format() {
        assert_eq!(Color::parse("red"), Some(Color::Ansi(1)));
        assert_eq!(Color::parse("bright-white"), Some(Color::Ansi(15)));
        assert_eq!(Color::parse("BLUE"), Some(Color::Ansi(4)));
        assert_eq!(Color::parse("208"), Some(Color::Idx(208)));
        assert_eq!(Color::parse("#fff"), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(Color::parse("#7c3aed"), Some(Color::Rgb(0x7c, 0x3a, 0xed)));
    }

    #[test]
    fn rejects_garbage() {
        for bad in ["", "256", "#12", "#gggggg", "purple", "bright-", "#1234567"] {
            assert_eq!(Color::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn formats_back_to_parseable_text() {
        assert_eq!(Color::Ansi(9).to_string(), "bright-red");
        assert_eq!(Color::Idx(42).to_string(), "42");
        assert_eq!(Color::Rgb(1, 2, 255).to_string(), "#0102ff");
        assert_eq!(
            Color::parse("bright-cyan").unwrap().to_string(),
            "bright-cyan"
        );
    }

    #[test]
    fn indexed_colors_map_to_rgb() {
        assert_eq!(Color::Idx(16).to_rgb(), (0, 0, 0));
        assert_eq!(Color::Idx(231).to_rgb(), (255, 255, 255));
        assert_eq!(Color::Idx(196).to_rgb(), (255, 0, 0));
        assert_eq!(Color::Idx(232).to_rgb(), (8, 8, 8));
    }

    #[test]
    fn hsl_round_trips() {
        for rgb in [(255, 0, 0), (0, 128, 255), (124, 58, 237), (10, 10, 10)] {
            let (h, s, l) = to_hsl(rgb);
            assert_eq!(from_hsl(h, s, l), rgb);
        }
    }

    #[test]
    fn adjust_wraps_hue_and_clamps() {
        assert_eq!(
            adjust(Some(Color::Rgb(255, 0, 0)), 120.0, 0.0, 0.0),
            Color::Rgb(0, 255, 0)
        );
        assert_eq!(
            adjust(Some(Color::Rgb(255, 0, 0)), -120.0, 0.0, 0.0),
            Color::Rgb(0, 0, 255)
        );
        assert_eq!(
            adjust(Some(Color::Rgb(255, 0, 0)), 0.0, 0.0, 90.0),
            Color::Rgb(255, 255, 255)
        );
        assert_eq!(adjust(None, 0.0, 0.0, -5.0), Color::Rgb(242, 242, 242));
        assert_eq!(
            adjust(Some(Color::Ansi(1)), 0.0, -100.0, 0.0),
            Color::Rgb(103, 103, 103)
        );
    }
}
