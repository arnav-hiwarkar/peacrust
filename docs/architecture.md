# peacrust architecture

One process, one main thread, plus one reader thread while cava runs. The crate is a library (`src/lib.rs`) with a thin binary (`src/main.rs`) so integration tests drive the same code the binary runs.

## Data shapes

```rust
enum Mode { Clock, Timer, Stopwatch }
enum View { Digital, Ascii, Binary, Date, Icon }
enum Color { Ansi(u8 /*0..16*/), Idx(u8), Rgb(u8, u8, u8) }
struct Style { color: Option<Color>, reverse: bool }
enum Target { ActiveFg, InactiveFg, ColonFg, ActiveBg, InactiveBg, ColonBg,
              Background, Date, Text, Prompt, Success, Error, Cava, Banner }

struct Config {            // everything mkconfig writes; Default = factory settings
    mode, view, locale, timezone, date_fmt,
    block, padding, margin, ratio: (u16, u16), date_padding: u16,
    hour24, seconds, date, auto_size, auto_ratio, cava, banner: bool,
    fill_active, fill_inactive, fill_colon, fill_cava: String,
    rate_input, rate_refresh, rate_status: u64,
    styles: [Style; 14],   // indexed by Target
    timer: Duration, timer_exec: String, stopwatch: Duration,
    cava_bars, cava_height: u16,
}

struct Counter { base: Duration, started: Option<Instant> }   // stopwatch and timer share it
enum Command { Set(Setting), Timer(CounterOp), Stopwatch(CounterOp), Help(String),
               Quit, MkConfig { path, force }, Source(path) }
struct Grid { w, h, cells: Vec<Cell> }  struct Cell { ch: char, fg, bg: Option<Color>, reverse }
```

Decisions:

- `Config` is the single source of truth. `mkconfig` serializes it to commands, `source` parses commands into it. The round trip is the test.
- A `Counter` is an elapsed-time accumulator. The stopwatch displays `stopwatch + elapsed`. The timer displays `timer - elapsed`, saturating at zero. One type, two readings, no duplicate logic.
- `Target` indexes a fixed array, so styles serialize and adjust through one code path.
- Rendering is a pure function `render(&App, now, w, h) -> Grid`. The terminal writer compares against the last grid and writes only when it differs. Tests assert on `Grid::text()`.
- Time comes in as arguments (`Instant`, `jiff::Zoned`), never read inside logic, so tests are deterministic.

## Modules

| File | Owns |
| --- | --- |
| `color.rs` | `Color` parse/format, RGB/HSL conversion, hue/sat/lum adjust. |
| `config.rs` | `Config`, `Target`, defaults, `to_commands()` for mkconfig. |
| `command.rs` | tokenizer, `parse(line) -> Result<Command>`, `apply` to `Config`, duration parse/format. |
| `clock.rs` | `Counter`, time-of-day digits, strftime via libc with locale, timezone lookup via jiff. |
| `font.rs` | 3x5 bitmaps and seven-segment masks per glyph. |
| `render.rs` | `Grid`, geometry (auto-size, auto-ratio), view drawing, cava panel, bottom line. |
| `prompt.rs` | line editor, history with fuzzy matching and file persistence. |
| `cava.rs` | spawn, raw frame parsing, reader thread, package-manager install. |
| `app.rs` | `App` state, key handling, command execution side effects (exec, files, cava lifecycle). |
| `cli.rs` | argument parsing into `Args`. |
| `term.rs` | raw mode/alt screen guard, panic hook, grid writer, event loop. |

## Event loop

```
loop {
  timeout = if cava running { 16ms } else { rate_input }
  if poll(timeout) { handle key/resize }
  app.tick(now)                    // timer completion, cava liveness
  grid = render(&app, now, size)
  if grid != last { write(grid); last = grid }
  if app.quit { break }
}
```

`rate-refresh` caps how often a clock-only frame is rebuilt; key events always redraw immediately.

## Safety

- `TermGuard` enables raw mode, alternate screen and hidden cursor; its `Drop` restores them. A panic hook restores the terminal before printing the panic.
- `CavaProc` kills and reaps the child and removes the temp config in `Drop`.
- `timer-exec` runs through `sh -c` with stdin/stdout/stderr null, reaped on a detached thread.
- No `unwrap` on external input. Command errors are values shown in the status line.
- The only `unsafe` is the libc `setlocale`/`strftime` call in `clock.rs`.
