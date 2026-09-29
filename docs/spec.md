# peacrust spec

peacrust is a terminal clock, timer and stopwatch written in Rust. It reproduces every documented feature of octobanana's peaclock (see `peaclockRef`) and adds a cava audio visualizer panel, user presets and a `peacrust / created by Hiwarkar` banner.

## Goals

- Feature parity with peaclock: 3 modes, 5 views, keybindings, command prompt, config files, styling, CLI.
- Single static-ish binary, three runtime dependencies (`crossterm`, `jiff`, `libc`).
- Runs on macOS and Linux (x86_64, aarch64). Always restores the terminal on exit or panic.
- Idle CPU near zero. Redraw only when the frame changes.
- Anyone can clone the repo, pick or write a preset and run it.

## Non-goals

Alarms, multiple timers, pomodoro, world clock, Windows support, a GUI. Wide (CJK/emoji) fill characters are not supported; fills are assumed one column per char.

## Modes

| Mode | Key | Behavior |
| --- | --- | --- |
| clock | `w` | Current time in the configured timezone. 12/24h, optional seconds. |
| timer | `e` | Counts down from the initial duration. At zero it stops, shows `00:00:00` and runs `timer-exec` once through `sh -c` without blocking the UI. |
| stopwatch | `r` | Counts up from the initial offset. |

`Space` toggles start/stop of the current mode's counter. `Backspace` clears it (timer back to its initial duration, stopwatch back to its initial offset). Both do nothing in clock mode. Timer and stopwatch keep running while you view another mode. Counters use a monotonic clock, so system clock changes don't affect them.

## Views

| View | Key | Drawing |
| --- | --- | --- |
| digital | `R` | Seven-segment digits on a 3x5 block grid. Unlit segments draw in the inactive style. |
| ascii | `E` | 3x5 bitmap font. Unlit pixels draw in the inactive style. |
| binary | `T` | One column per digit (HHMMSS), rows are bits 8,4,2,1. |
| date | `W` | Only the formatted date string. |
| icon | `Y` | One compact text line, e.g. `12:34:56`. |

With 12h time the hour shows 1 to 12. With seconds off, the seconds digits are hidden in every view. When the date toggle is on, the formatted date is drawn under the clock, `date-padding` rows below it. The date format uses strftime syntax (`%a %b %d %p`), the configured `locale` and the configured `timezone`. An invalid timezone is an error and leaves the old value in place.

## Layout

Top to bottom: clock area (clock + date, centered), cava panel (when on), bottom line.

The bottom line shows the prompt when open, else a status message for `rate-status` ms, else the banner `peacrust · created by Hiwarkar` (when on).

Block geometry, per view that draws blocks: `block x y` sets each pixel's size in cells, `padding x y` sets the gap between pixels, `margin x y` reserves space around the clock. `auto-size` picks the largest block that fits the clock area. `auto-ratio` picks the largest block whose width:height equals `ratio x y`. Auto-ratio wins over auto-size, and both override `block`. Blocks are never smaller than 1x1. When the clock doesn't fit, it is clipped, never panics.

## Keybindings

Normal mode:

| Key | Action |
| --- | --- |
| `q` `Q` `Ctrl+C` | quit |
| `:` | open prompt |
| `Esc` | clear status message |
| `Space` / `Backspace` | start-stop / clear counter |
| `w e r` | clock / timer / stopwatch |
| `W E R T Y` | date / ascii / digital / binary / icon view |
| `a s d f g` | toggle 24h / seconds / date / auto-size / auto-ratio |
| `z` | toggle cava panel |
| `u i o p` | select ratio / margin / padding / block for adjusting |
| `x c v b n m , .` | select active-fg / inactive-fg / colon-fg / active-bg / inactive-bg / colon-bg / background / date color |
| `h l` | selected geometry x -1 / +1, or hue -10 / +10 |
| `j k` | selected geometry y -1 / +1, or saturation -5 / +5 |
| `; '` | luminosity -5 / +5 (colors only) |
| `/` | clear the selected style |

Adjusting a color that has no value starts from white. 4-bit and 8-bit colors are converted to RGB before adjusting.

Prompt mode: `Enter` submit, `Esc`/`Ctrl+C` close, `Ctrl+U` clear, `Up`/`Ctrl+P` and `Down`/`Ctrl+N` previous and next history entry matching the typed text as a fuzzy subsequence, `Left`/`Ctrl+B`, `Right`/`Ctrl+F`, `Home`/`Ctrl+A`, `End`/`Ctrl+E`, `Delete`/`Ctrl+D` delete under cursor (at end, delete previous), `Backspace`/`Ctrl+H` delete previous.

## Commands

One command per line. Used by the prompt, config files and presets. `#` starts a comment line. Arguments may be quoted with `'` or `"`. A successful prompt command shows a success status, a failed one shows an error status and changes nothing.

```
mode clock|timer|stopwatch
view digital|ascii|binary|date|icon
locale 'en_US.UTF-8'        timezone 'Europe/London'    date '%a %b %d'
block X Y   block-x X   block-y Y
padding X Y padding-x X padding-y Y
margin X Y  margin-x X  margin-y Y
ratio X Y   ratio-x X   ratio-y Y
date-padding N
set hour-24|seconds|date|auto-size|auto-ratio|cava|banner on|off
fill 'S'    fill-active 'S'   fill-inactive 'S'   fill-colon 'S'   fill-cava 'S'
rate-input MS   rate-refresh MS   rate-status MS
style TARGET COLOR|reverse|clear
timer DURATION|start|stop|clear       timer-exec 'shell command'
stopwatch DURATION|start|stop|clear
cava-bars N     cava-height N
help [term]     quit    exit
mkconfig FILE   mkconfig! FILE        source FILE
```

- Durations: `10m:0s`, `01h:30m:00s`, `90s`, `5m`, or bare seconds `300`.
- Empty string clears `locale`, `timezone`, `date`, `timer-exec` and fills (fill falls back to the default `█`).
- Colors: `red green blue yellow magenta cyan black white`, `bright-red` etc., `0`..`255`, `#rgb`, `#rrggbb`.
- Style targets: `active-fg inactive-fg colon-fg active-bg inactive-bg colon-bg background date text prompt success error cava banner`.
- `mkconfig FILE` refuses to overwrite. `mkconfig! FILE` overwrites. The output is a command file that reproduces the current config exactly.
- `source FILE` runs a command file. peacrust-only; this is how presets switch at runtime.
- `help [term]` shows the help lines containing the term in the status line.

## Files

Config dir: `$XDG_CONFIG_HOME/peacrust` or `~/.config/peacrust`.

- `config` runs at startup when present. A missing file is not an error. A bad line is skipped and reported after startup.
- `history` stores prompt history, newest last, capped at 1000, consecutive duplicates dropped.

The repo ships `presets/` (default, digital, binary, neon, minimal). `peacrust presets/neon` runs one. Users copy one to `~/.config/peacrust/config` or write their own.

## CLI

```
peacrust [CONFIG]
  -h, --help            help
  -v, --version         version and "created by Hiwarkar"
      --license         license
  -u, --config PATH     config file; NONE skips it
      --config-dir DIR  config dir; NONE disables config and history
      --colour on|off|auto   color in help/version output
      --install-cava    install cava with the system package manager
```

Unknown flags print an error and exit 2.

## cava

When the `cava` toggle is on, peacrust starts `cava -p <tempfile>` with raw ASCII output to stdout, reads bar heights on a background thread and draws them as vertical bars (`▁▂▃▄▅▆▇█`) in a panel `cava-height` rows tall. The child is killed and the temp file deleted when the panel is turned off or peacrust exits. If cava is missing, the panel reads `cava not found · run peacrust --install-cava`. If cava exits, the panel reads `cava exited`.

`--install-cava` and `install.sh` detect `brew`, `apt-get`, `dnf`, `pacman`, `zypper` or `apk`, print the exact command and ask `y/N` before running it. They never run `sudo` silently.

## Distribution

- `cargo install --path .` or `./install.sh` (builds, installs cava if missing).
- GitHub Actions CI runs fmt, clippy and tests on ubuntu and macOS.
- Tagging `v*` builds release binaries for x86_64/aarch64 Linux (gnu) and macOS, uploads `.tar.gz` plus `.sha256` to the GitHub release.

## Expected behavior checklist (each has a test)

1. Keys switch mode and view; the rendered frame changes accordingly.
2. Timer counts down, stops at zero, fires `timer-exec` exactly once.
3. Stopwatch counts up, pause freezes, clear resets to initial offset.
4. 12h shows `12` at noon and midnight, 24h shows `00` at midnight.
5. Seconds off removes the seconds digits.
6. Binary view lights the right bits.
7. auto-size and auto-ratio give the documented block sizes; ratio wins.
8. Every command parses; invalid input returns an error and leaves config unchanged.
9. `mkconfig` output, sourced into defaults, reproduces the config.
10. Color parsing for all formats; HSL adjust wraps hue and clamps saturation and luminosity.
11. Prompt editing keys and fuzzy history navigation.
12. CLI flags parse; unknown flags error.
13. cava raw frames parse; malformed lines are ignored.
14. Rendering never panics for terminal sizes from 1x1 to 400x120.
15. Banner reads `peacrust · created by Hiwarkar`.
