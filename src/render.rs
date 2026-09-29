use crate::color::Color;
use crate::config::{Config, Style, Target, Toggle, View};
use crate::font::{Px, bitmap};

pub const BANNER: (&str, &str) = ("peacrust", " · created by Hiwarkar");
const EIGHTHS: [char; 8] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇'];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub reverse: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    pub w: u16,
    pub h: u16,
    pub cells: Vec<Cell>,
    pub cursor: Option<(u16, u16)>,
}

impl Grid {
    fn new(w: u16, h: u16, bg: Option<Color>) -> Grid {
        let blank = Cell {
            ch: ' ',
            fg: None,
            bg,
            reverse: false,
        };
        Grid {
            w,
            h,
            cells: vec![blank; w as usize * h as usize],
            cursor: None,
        }
    }

    fn put(&mut self, x: i32, y: i32, ch: char, fg: Style, bg: Style) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let cell = &mut self.cells[y as usize * self.w as usize + x as usize];
        cell.ch = ch;
        cell.fg = fg.color;
        if bg.color.is_some() {
            cell.bg = bg.color;
        }
        cell.reverse = fg.reverse || bg.reverse;
    }

    fn text_at(&mut self, x: i32, y: i32, s: &str, style: Style) -> i32 {
        let mut x = x;
        for ch in s.chars() {
            self.put(x, y, ch, style, Style::default());
            x += 1;
        }
        x
    }

    pub fn row(&self, y: u16) -> String {
        let start = y as usize * self.w as usize;
        self.cells[start..start + self.w as usize]
            .iter()
            .map(|c| c.ch)
            .collect()
    }

    /// The frame as plain text, trailing spaces trimmed. Used by tests.
    pub fn text(&self) -> String {
        (0..self.h)
            .map(|y| self.row(y).trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub enum Bottom<'a> {
    Idle,
    Status(&'a str, Target),
    Prompt(&'a str, usize),
}

pub enum Panel<'a> {
    Off,
    Bars(&'a [u16]),
    Message(&'a str),
}

pub struct Scene<'a> {
    pub cfg: &'a Config,
    pub time: &'a str,
    pub date: &'a str,
    pub bottom: Bottom<'a>,
    pub panel: Panel<'a>,
}

fn fits(n: u32, cells: u32, pad: u32, avail: u32) -> bool {
    n.saturating_mul(cells)
        .saturating_add(n.saturating_sub(1).saturating_mul(pad))
        <= avail
}

/// Block size for a `cols` x `rows` pixel bitmap inside `avail` cells.
pub fn block_size(cfg: &Config, (cols, rows): (u16, u16), (aw, ah): (u16, u16)) -> (u16, u16) {
    let (c, r, px, py) = (
        cols as u32,
        rows as u32,
        cfg.padding.0 as u32,
        cfg.padding.1 as u32,
    );
    let (aw, ah) = (aw as u32, ah as u32);
    let largest = |n: u32, pad: u32, avail: u32| {
        (avail.saturating_sub(n.saturating_sub(1).saturating_mul(pad)) / n.max(1)).max(1)
    };
    if cols == 0 || rows == 0 {
        return cfg.block;
    }
    if cfg.on(Toggle::AutoRatio) {
        let (rx, ry) = (cfg.ratio.0 as u32, cfg.ratio.1 as u32);
        let mut k = 1;
        while fits(c, (k + 1) * rx, px, aw) && fits(r, (k + 1) * ry, py, ah) {
            k += 1;
        }
        return (
            (k * rx).min(u16::MAX as u32) as u16,
            (k * ry).min(u16::MAX as u32) as u16,
        );
    }
    if cfg.on(Toggle::AutoSize) {
        return (largest(c, px, aw) as u16, largest(r, py, ah) as u16);
    }
    cfg.block
}

pub fn render(s: &Scene, w: u16, h: u16) -> Grid {
    let cfg = s.cfg;
    let mut g = Grid::new(w, h, cfg.style(Target::Background).color);
    if w == 0 || h == 0 {
        return g;
    }
    let bottom_y = h as i32 - 1;
    draw_bottom(&mut g, s, bottom_y);

    let panel_h = match s.panel {
        Panel::Off => 0,
        _ => cfg.cava_height.min((h - 1) / 2),
    };
    let panel_y = bottom_y - panel_h as i32;
    draw_panel(&mut g, cfg, &s.panel, panel_y, panel_h);

    let area_h = panel_y.max(0) as u16;
    let (mx, my) = (cfg.margin.0, cfg.margin.1);
    let aw = w.saturating_sub(mx * 2);
    let ah = area_h.saturating_sub(my * 2);
    let show_date = !s.date.is_empty() && (cfg.view == View::Date || cfg.on(Toggle::Date));
    let date_h = match (show_date, cfg.view) {
        (false, _) => 0,
        (true, View::Date) => 1,
        (true, _) => cfg.date_padding.saturating_add(1),
    };

    let bm = match cfg.view {
        View::Date | View::Icon => Vec::new(),
        view => bitmap(view, s.time),
    };
    let (cols, rows) = (bm.first().map_or(0, Vec::len) as u16, bm.len() as u16);
    let (bx, by) = block_size(cfg, (cols, rows), (aw, ah.saturating_sub(date_h)));
    let (bx, by) = (bx.min(w), by.min(h));
    let (clock_w, clock_h) = match cfg.view {
        View::Date => (0, 0),
        View::Icon => (s.time.chars().count() as u16, 1),
        _ => (
            cols.saturating_mul(bx)
                .saturating_add(cols.saturating_sub(1).saturating_mul(cfg.padding.0)),
            rows.saturating_mul(by)
                .saturating_add(rows.saturating_sub(1).saturating_mul(cfg.padding.1)),
        ),
    };

    let y0 = my as i32 + ah.saturating_sub(clock_h.saturating_add(date_h)) as i32 / 2;
    let x0 = mx as i32 + aw.saturating_sub(clock_w) as i32 / 2;
    match cfg.view {
        View::Date => {}
        View::Icon => {
            g.text_at(x0, y0, s.time, cfg.style(Target::ActiveFg));
        }
        _ => draw_bitmap(&mut g, cfg, &bm, (bx, by), x0, y0),
    }
    let dy = y0 + clock_h as i32 + date_h as i32 - 1;
    if show_date && dy < area_h as i32 {
        let dw = s.date.chars().count() as i32;
        g.text_at((w as i32 - dw) / 2, dy, s.date, cfg.style(Target::Date));
    }
    g
}

fn draw_bitmap(g: &mut Grid, cfg: &Config, bm: &[Vec<Px>], (bx, by): (u16, u16), x0: i32, y0: i32) {
    let (px, py) = (cfg.padding.0 as i32, cfg.padding.1 as i32);
    for (r, row) in bm.iter().enumerate() {
        for (c, p) in row.iter().enumerate() {
            let (fill, fg, bg) = match p {
                Px::Blank => continue,
                Px::On => (&cfg.fill_active, Target::ActiveFg, Target::ActiveBg),
                Px::Off => (&cfg.fill_inactive, Target::InactiveFg, Target::InactiveBg),
                Px::Colon => (&cfg.fill_colon, Target::ColonFg, Target::ColonBg),
            };
            let chars: Vec<char> = fill.chars().collect();
            let cx = x0.saturating_add((c as i32).saturating_mul(bx as i32 + px));
            let cy = y0.saturating_add((r as i32).saturating_mul(by as i32 + py));
            if cx >= g.w as i32 || cy >= g.h as i32 {
                continue;
            }
            for dy in 0..by as i32 {
                for dx in 0..bx as i32 {
                    let ch = if chars.is_empty() {
                        ' '
                    } else {
                        chars[dx as usize % chars.len()]
                    };
                    g.put(cx + dx, cy + dy, ch, cfg.style(fg), cfg.style(bg));
                }
            }
        }
    }
}

fn draw_bottom(g: &mut Grid, s: &Scene, y: i32) {
    let cfg = s.cfg;
    match s.bottom {
        Bottom::Prompt(input, cursor) => {
            let x = g.text_at(0, y, ":", cfg.style(Target::Prompt));
            g.text_at(x, y, input, cfg.style(Target::Text));
            g.cursor = Some(((1 + cursor).min(g.w as usize - 1) as u16, y as u16));
        }
        Bottom::Status(msg, t) => {
            g.text_at(0, y, msg, cfg.style(t));
        }
        Bottom::Idle if cfg.on(Toggle::Banner) => {
            let len = (BANNER.0.chars().count() + BANNER.1.chars().count()) as i32;
            let x = g.text_at(
                (g.w as i32 - len) / 2,
                y,
                BANNER.0,
                cfg.style(Target::ActiveFg),
            );
            g.text_at(x, y, BANNER.1, cfg.style(Target::Banner));
        }
        Bottom::Idle => {}
    }
}

fn draw_panel(g: &mut Grid, cfg: &Config, panel: &Panel, top: i32, ph: u16) {
    let st = cfg.style(Target::Cava);
    match *panel {
        Panel::Off => {}
        Panel::Message(msg) => {
            g.text_at(
                (g.w as i32 - msg.chars().count() as i32) / 2,
                top + ph as i32 / 2,
                msg,
                st,
            );
        }
        Panel::Bars(bars) if !bars.is_empty() && ph > 0 => {
            let fill: Vec<char> = cfg.fill_cava.chars().collect();
            let full = |dx: usize| {
                if fill.is_empty() {
                    '█'
                } else {
                    fill[dx % fill.len()]
                }
            };
            let n = bars.len();
            let w = g.w as usize;
            let bw = (w / n).max(1);
            let used = if w >= n { bw * n } else { w };
            let off = (w - used) / 2;
            for x in 0..used {
                let (i, dx) = if w >= n {
                    (x / bw, x % bw)
                } else {
                    (x * n / w, 0)
                };
                if bw >= 2 && dx == bw - 1 {
                    continue;
                }
                let eighths = bars[i].min(1000) as usize * ph as usize * 8 / 1000;
                for r in 0..ph as usize {
                    let level = eighths.saturating_sub(r * 8);
                    let ch = if level >= 8 { full(dx) } else { EIGHTHS[level] };
                    if ch != ' ' {
                        g.put(
                            (off + x) as i32,
                            top + ph as i32 - 1 - r as i32,
                            ch,
                            st,
                            Style::default(),
                        );
                    }
                }
            }
        }
        Panel::Bars(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::execute;

    fn cfg(lines: &[&str]) -> Config {
        let mut c = Config::default();
        for l in lines {
            execute(l, &mut c).unwrap();
        }
        c
    }

    fn scene<'a>(c: &'a Config, time: &'a str, date: &'a str) -> Scene<'a> {
        Scene {
            cfg: c,
            time,
            date,
            bottom: Bottom::Idle,
            panel: Panel::Off,
        }
    }

    #[test]
    fn ascii_view_frame() {
        let c = cfg(&[
            "view ascii",
            "set auto-ratio off",
            "block 1 1",
            "padding 0 0",
            "margin 0 0",
            "fill-inactive '.'",
            "set banner off",
        ]);
        let g = render(&scene(&c, "1:2", "Mon"), 9, 8);
        assert_eq!(
            g.text(),
            ".█.   ███\n██. █ ..█\n.█.   ███\n.█. █ █..\n███   ███\n\n   Mon\n"
        );
    }

    #[test]
    fn banner_is_centered_at_bottom() {
        let c = Config::default();
        let g = render(&scene(&c, "", ""), 34, 3);
        assert_eq!(g.row(2), "  peacrust · created by Hiwarkar  ");
    }

    #[test]
    fn date_view_shows_only_date() {
        let c = cfg(&["view date", "set banner off"]);
        assert_eq!(
            render(&scene(&c, "12:00", "Tue Sep 29"), 12, 4).text(),
            "\n Tue Sep 29\n\n"
        );
    }

    #[test]
    fn icon_view_is_one_line() {
        let c = cfg(&["view icon", "set date off", "set banner off", "margin 0 0"]);
        assert_eq!(render(&scene(&c, "09:41", ""), 9, 3).text(), "  09:41\n\n");
    }

    #[test]
    fn prompt_and_status_replace_banner() {
        let c = Config::default();
        let mut s = scene(&c, "", "");
        s.bottom = Bottom::Prompt("view bin", 8);
        let g = render(&s, 20, 2);
        assert_eq!((g.row(1).trim_end(), g.cursor), (":view bin", Some((9, 1))));
        s.bottom = Bottom::Status("error: nope", Target::Error);
        assert_eq!(render(&s, 20, 2).row(1).trim_end(), "error: nope");
    }

    #[test]
    fn auto_size_and_auto_ratio() {
        let c = cfg(&["set auto-ratio off", "set auto-size on", "padding 1 0"]);
        assert_eq!(block_size(&c, (10, 5), (60, 20)), (5, 4));
        let c = cfg(&["set auto-ratio on", "ratio 2 1", "padding 0 0"]);
        assert_eq!(block_size(&c, (10, 5), (60, 20)), (6, 3));
        let c = cfg(&["set auto-ratio off", "block 3 2"]);
        assert_eq!(block_size(&c, (10, 5), (60, 20)), (3, 2));
        let c = cfg(&["set auto-ratio on"]);
        assert_eq!(block_size(&c, (0, 0), (60, 20)), (2, 1));
        assert_eq!(block_size(&c, (10, 5), (1, 1)), (2, 1));
    }

    #[test]
    fn cava_bars_fill_from_bottom() {
        let c = cfg(&["set banner off", "cava-height 2", "view date"]);
        let mut s = scene(&c, "", "");
        let bars = [1000, 500, 0, 250];
        s.panel = Panel::Bars(&bars);
        let g = render(&s, 8, 5);
        assert_eq!(g.row(2), "█       ");
        assert_eq!(g.row(3), "█ █   ▄ ");
        s.panel = Panel::Message("cava not found");
        assert!(render(&s, 20, 5).text().contains("cava not found"));
    }

    #[test]
    fn never_panics_at_any_size() {
        for view in ["digital", "ascii", "binary", "date", "icon"] {
            let v = format!("view {view}");
            let normal = cfg(&[&v, "margin 3 3", "date-padding 9"]);
            let huge = cfg(&[
                &v,
                "set auto-size off",
                "block 65535 65535",
                "padding 65535 65535",
                "date-padding 65535",
            ]);
            for c in [&normal, &huge] {
                for (w, h) in [(0, 0), (1, 1), (2, 3), (5, 40), (80, 24), (400, 120)] {
                    let mut s = scene(c, "12:34:56", "Tue Sep 29 2026");
                    s.panel = Panel::Bars(&[1000; 64]);
                    s.bottom = Bottom::Prompt("a long prompt input", 19);
                    assert_eq!(render(&s, w, h).cells.len(), w as usize * h as usize);
                }
            }
        }
    }
}
