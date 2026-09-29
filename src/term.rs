use crate::app::App;
use crate::clock;
use crate::color::Color;
use crate::render::{Cell, Grid};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event};
use crossterm::style::{
    Attribute, Color as TColor, Print, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{
    self, BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::{execute, queue};
use std::io::{self, Write};
use std::time::{Duration, Instant};

/// Raw mode, alternate screen and hidden cursor for as long as it lives.
pub struct TermGuard;

impl TermGuard {
    pub fn enter() -> io::Result<TermGuard> {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            prev(info);
        }));
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(TermGuard)
    }
}

fn restore() {
    let _ = terminal::disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        SetAttribute(Attribute::Reset),
        Show,
        LeaveAlternateScreen
    );
}

impl Drop for TermGuard {
    fn drop(&mut self) {
        restore();
    }
}

fn tcolor(c: Option<Color>) -> TColor {
    const NAMED: [TColor; 16] = [
        TColor::Black,
        TColor::DarkRed,
        TColor::DarkGreen,
        TColor::DarkYellow,
        TColor::DarkBlue,
        TColor::DarkMagenta,
        TColor::DarkCyan,
        TColor::Grey,
        TColor::DarkGrey,
        TColor::Red,
        TColor::Green,
        TColor::Yellow,
        TColor::Blue,
        TColor::Magenta,
        TColor::Cyan,
        TColor::White,
    ];
    match c {
        None => TColor::Reset,
        Some(Color::Ansi(n)) => NAMED[(n & 15) as usize],
        Some(Color::Idx(n)) => TColor::AnsiValue(n),
        Some(Color::Rgb(r, g, b)) => TColor::Rgb { r, g, b },
    }
}

/// Write only the cells that differ from `prev`, in one synchronized update.
fn draw(out: &mut impl Write, grid: &Grid, prev: Option<&Grid>) -> io::Result<()> {
    let prev = prev.filter(|p| p.w == grid.w && p.h == grid.h);
    let mut buf = Vec::with_capacity(grid.cells.len() * 4);
    queue!(buf, BeginSynchronizedUpdate, Hide)?;
    if prev.is_none() {
        queue!(buf, SetAttribute(Attribute::Reset), Clear(ClearType::All))?;
    }
    let mut pen: Option<Cell> = None;
    for y in 0..grid.h {
        let mut at_cursor = false;
        for x in 0..grid.w {
            let i = y as usize * grid.w as usize + x as usize;
            let cell = grid.cells[i];
            if prev.is_some_and(|p| p.cells[i] == cell) {
                at_cursor = false;
                continue;
            }
            if !at_cursor {
                queue!(buf, MoveTo(x, y))?;
                at_cursor = true;
            }
            if pen.is_none_or(|p| (p.fg, p.bg, p.reverse) != (cell.fg, cell.bg, cell.reverse)) {
                let rev = if cell.reverse {
                    Attribute::Reverse
                } else {
                    Attribute::NoReverse
                };
                queue!(
                    buf,
                    SetAttribute(rev),
                    SetForegroundColor(tcolor(cell.fg)),
                    SetBackgroundColor(tcolor(cell.bg))
                )?;
                pen = Some(cell);
            }
            queue!(buf, Print(cell.ch))?;
        }
    }
    if let Some((x, y)) = grid.cursor {
        queue!(buf, MoveTo(x, y), Show)?;
    }
    queue!(buf, EndSynchronizedUpdate)?;
    out.write_all(&buf)?;
    out.flush()
}

pub fn run(app: &mut App) -> io::Result<()> {
    let mut out = io::stdout();
    let mut last: Option<Grid> = None;
    let mut built = Instant::now() - Duration::from_secs(3600);
    loop {
        let timeout = Duration::from_millis(if app.animating() {
            app.cfg.rate_input.min(16)
        } else {
            app.cfg.rate_input
        });
        let mut dirty = false;
        if event::poll(timeout)? {
            while {
                match event::read()? {
                    Event::Key(k) => app.key(k, Instant::now()),
                    Event::Resize(..) => last = None,
                    _ => {}
                }
                dirty = true;
                event::poll(Duration::ZERO)?
            } {}
        }
        if app.quit {
            return Ok(());
        }
        let now = Instant::now();
        app.tick(now);
        if dirty
            || app.animating()
            || now.duration_since(built) >= Duration::from_millis(app.cfg.rate_refresh)
        {
            built = now;
            let (w, h) = terminal::size()?;
            let grid = app.render(now, &clock::now_in(&app.cfg.timezone), w, h);
            if last.as_ref() != Some(&grid) {
                draw(&mut out, &grid, last.as_ref())?;
                last = Some(grid);
            }
        }
    }
}
