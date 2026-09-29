# peacrust implementation plan

Each step ends with `cargo test` green and a commit. Steps are ordered so every step builds on tested code.

1. **Scaffold.** `Cargo.toml` deps (crossterm, jiff, libc) and release profile (lto, strip, codegen-units 1). `lib.rs` declaring modules.
2. **color.rs.** Parse all color formats, format back, HSL adjust. Tests: every format, hue wrap, clamps.
3. **config.rs + command.rs.** `Config` defaults, tokenizer, every command, duration parse/format, `to_commands`. Tests: each command, errors leave config unchanged, mkconfig round trip.
4. **clock.rs.** `Counter`, digits for clock/timer/stopwatch, 12/24h, strftime with locale, timezone. Tests: noon/midnight, countdown saturation, pause/resume.
5. **font.rs + render.rs.** Glyphs, geometry, five views, date line, bottom line, cava bars. Tests: frame text for each view, auto-size/auto-ratio numbers, binary bits, no panic across sizes.
6. **prompt.rs.** Editor keys and fuzzy history. Tests: each key, history matching, persistence.
7. **cava.rs.** Config text, frame parsing, spawn and install. Tests: parsing, malformed input.
8. **app.rs.** Key dispatch, adjust mode, command side effects, timer exec. Tests: key sequences through `App`, timer fires once, mkconfig writes a file.
9. **cli.rs + term.rs + main.rs.** Args, terminal guard, loop. Tests: arg parsing. Manual: run in tmux, capture pane.
10. **Ship.** `presets/`, `install.sh`, CI and release workflows, README, LICENSE.
11. **Verify.** clippy clean, release build size, tmux run of every view and cava panel.
