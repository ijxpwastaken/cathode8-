# Cathode-8
<img width="1261" height="688" alt="Cathode-8 emulator window" src="https://github.com/user-attachments/assets/a5a8011a-787f-4d45-b5c8-69ef3a003f1f" />
Cathode-8 is a from-scratch NES emulator in Rust with a native desktop UI built with `eframe/egui` and an accuracy-focused emulation core.

It is built for people who care about clean implementation, timing accuracy, mapper coverage, and useful testing tools — not just getting a ROM to boot.

## Highlights

- Accuracy-focused CPU, PPU, and APU behavior
- Native desktop UI with drag-and-drop ROM loading
- Explicit support for major NES mappers
- Generic fallback path for documented NES 2.0 mapper IDs up to 559
- Built-in tooling for stress, regression, and ROM test workflows
- Clean-room implementation with no proprietary Nintendo code or bundled ROMs

## Quick Start

### Requirements

- Rust stable toolchain
- Desktop environment with audio output

### Build

```bash
cargo build
Run
cargo run --release

Then load a ROM by:

clicking Open ROM

pressing Ctrl+O

dragging a .nes file into the window

Controls
Action	Input
D-Pad	WASD or Arrow keys
A	Space or Z
B	X
Start	Enter
Select	Shift
Pause / Resume	P
Reset	R
Open ROM	Ctrl+O
Zapper Aim	Mouse over game image
Zapper Trigger	Hold left mouse button
Mapper Support
Explicitly implemented

0 — NROM

1 — MMC1

2 — UxROM

3 — CNROM

4 — MMC3

5 — MMC5

7 — AxROM

9 — MMC2

10 — MMC4

19 — Namco 163

24 — Konami VRC6a

25 — Konami VRC4b/d

26 — Konami VRC6b

66 — GxROM

69 — FME-7 / Sunsoft 5B

71 — Camerica

85 — Konami VRC7

Generic fallback

Documented NES 2.0 mapper IDs up to 559 fall back to a generic mapper path

Mapper IDs above 559 are rejected by design

For compatibility notes and current limitations, see COMPATIBILITY.md.

Project Goals

Cathode-8 focuses on:

accuracy-first behavior across CPU, PPU, and APU timing paths

broad mapper handling without bundling copyrighted game data

practical tools for stress testing and regression checks

a clean-room codebase written fully in Rust

Development
Common commands
cargo fmt
cargo clippy
cargo test
Utility Binaries
Stress runner
cargo run --release --bin stress_runner -- --rom /path/to/rom.nes --iterations 500 --frames 1800
AccuracyCoin probe
cargo run --release --bin accuracycoin_probe -- --rom /path/to/AccuracyCoin.nes --frames 4800
ROM suite runner
cargo run --release --bin rom_test_runner -- --suite external/nes-test-roms/test_roms.xml --rom-root external/nes-test-roms
CLI debugger
cargo run --release --bin cathode8_debug -- /path/to/rom.nes
Project Layout

src/nes/ — emulation core: CPU, PPU, APU, mappers, cartridge parsing

src/app.rs — desktop UI and input handling

src/bin/ — utility binaries:

stress_runner

accuracycoin_probe

rom_test_runner

cathode8_debug

Legal

This repository does not include commercial ROMs or copyrighted Nintendo assets

Use only ROM dumps from cartridges you legally own

NES and Nintendo Entertainment System are trademarks of Nintendo Co., Ltd.

This project is not affiliated with or endorsed by Nintendo

References

NESDev Wiki

.
