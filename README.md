# peacrust

A responsive, customizable clock, timer and stopwatch for the terminal, with a built-in [cava](https://github.com/karlstav/cava) audio visualizer. A Rust take on [peaclock](https://github.com/octobanana/peaclock).

Created by Hiwarkar.

## Install

Download a binary from the releases page, or build from source with Rust 1.88+:

```sh
git clone <this repo> && cd peacrust
./install.sh            # cargo install, then offers to install cava
```

`./install.sh` runs `cargo install --path . --locked`. If cava is missing, it asks before installing it with brew, apt-get, dnf, pacman, zypper or apk. You can run that step on its own with `peacrust --install-cava`.

Supported on macOS and Linux.

## Use

```sh
peacrust                  # runs ~/.config/peacrust/config if it exists
peacrust presets/neon     # run a preset
peacrust --help           # every key, command and option
```

| Keys | Action |
| --- | --- |
| `w` `e` `r` | clock, timer, stopwatch |
| `R` `E` `T` `W` `Y` | digital, ascii, binary, date, icon view |
| `space` / `backspace` | start-stop / clear the timer or stopwatch |
| `a` `s` `d` `f` `g` | 24h, seconds, date, auto-size, auto-ratio |
| `z` | cava visualizer |
| `u` `i` `o` `p` then `h` `j` `k` `l` | adjust ratio, margin, padding, block |
| `x` `c` `v` `b` `n` `m` `,` `.` then `h` `l` `j` `k` `;` `'` `/` | adjust a color's hue, saturation, luminosity, or clear it |
| `:` | command prompt |
| `q` | quit |

## Make your own view

Every setting is a command. Type it at the `:` prompt to try it live, then save the result with `:mkconfig! ~/.config/peacrust/config`.

```
view ascii
set auto-ratio on
ratio 2 1
fill-active '▓'
style active-fg #ff2bd6
style colon-fg #00e5ff
set cava on
cava-bars 48
timer 25m
timer-exec 'notify-send peacrust "time is up"'
```

`presets/` has five starting points: `default`, `digital`, `binary`, `neon` and `minimal`. Copy one, edit it, and run it with `peacrust path/to/file` or `:source path/to/file`.

## cava

The visualizer panel runs `cava` in raw mode and draws its bars inside peacrust. It keeps the `[input]` section of your `~/.config/cava/config`, so it listens to the same source as cava does. Without one, macOS uses portaudio (the default input device) and Linux uses cava's default.

To visualize system audio on macOS, set a loopback device such as BlackHole as the input. You may also need to give your terminal the audio recording permission. If cava stops, the panel shows the reason cava printed.

## Develop

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

`docs/spec.md`, `docs/architecture.md` and `docs/implementation-plan.md` describe the design. Pushing a `v*` tag builds release binaries for Linux and macOS (x86_64 and aarch64).
