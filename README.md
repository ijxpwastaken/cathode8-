# Cathode-8
<img width="1261" height="688" alt="Cathode-8 emulator window" src="https://github.com/user-attachments/assets/a5a8011a-787f-4d45-b5c8-69ef3a003f1f" />

Cathode-8 is a from-scratch NES emulator in Rust with a native desktop UI (`eframe/egui`) and an accuracy-focused emulation core.

## What This Project Focuses On
- Accuracy-first behavior for CPU/PPU/APU timing paths.
- Mapper support across documented NES 2.0 mapper IDs `0..=559`.
- Practical tooling for stress and regression checks.
- Clean-room implementation with no proprietary Nintendo code or bundled ROMs.

## Quick Start

### Prerequisites
- Rust toolchain (stable)
- A desktop environment with audio output support

### Build
```bash
cargo build
```

### Run the GUI
```bash
cargo run --release
```

Then load a ROM by:
- Clicking `Open ROM`
- Pressing `Ctrl+O`
- Dragging a `.nes` file into the window

## Controls

| Action | Input |
|---|---|
| D-Pad | `WASD` or Arrow keys |
| A | `Space` or `Z` |
| B | `X` |
| Start | `Enter` |
| Select | `Shift` |
| Pause/Resume | `P` |
| Reset | `R` |
| Open ROM | `Ctrl+O` |
| Zapper Aim | Mouse over game image |
| Zapper Trigger | Hold left mouse button |

## Mapper Support

### Explicit implementations
- `0` (NROM)
- `1` (MMC1)
- `2` (UxROM)
- `3` (CNROM)
- `4` (MMC3)
- `5` (MMC5)
- `7` (AxROM)
- `9` (MMC2)
- `10` (MMC4)
- `19` (Namco 163)
- `24` (Konami VRC6a)
- `25` (Konami VRC4b/d)
- `26` (Konami VRC6b)
- `66` (GxROM)
- `69` (FME-7 / Sunsoft 5B)
- `71` (Camerica)
- `85` (Konami VRC7)

### Generic fallback
- Mapper IDs up to `559` fall back to a generic mapper path.
- Mapper IDs above `559` are rejected by design.

For broader compatibility notes, see [`COMPATIBILITY.md`](COMPATIBILITY.md).

## Development Commands
```bash
# Format
cargo fmt

# Lints
cargo clippy

# Unit tests
cargo test
```

## Utility Binaries

### Stress runner
```bash
cargo run --release --bin stress_runner -- --rom /path/to/rom.nes --iterations 500 --frames 1800
```

### AccuracyCoin probe
```bash
cargo run --release --bin accuracycoin_probe -- --rom /path/to/AccuracyCoin.nes --frames 4800
```

### ROM suite runner
```bash
cargo run --release --bin rom_test_runner -- --suite external/nes-test-roms/test_roms.xml --rom-root external/nes-test-roms
```

### CLI debugger
```bash
cargo run --release --bin cathode8_debug -- /path/to/rom.nes
```

## Project Layout
- `src/nes/`: emulation core (CPU, PPU, APU, mappers, cartridge parsing)
- `src/app.rs`: desktop UI and input handling
- `src/bin/`: utility binaries (`stress_runner`, `accuracycoin_probe`, `rom_test_runner`, `cathode8_debug`)

## Legal
- This repository does not include commercial ROMs or copyrighted Nintendo assets.
- Use only ROM dumps from cartridges you legally own.
- "NES" and "Nintendo Entertainment System" are trademarks of Nintendo Co., Ltd.
- This project is unaffiliated with and not endorsed by Nintendo.

## References
- NESDev Wiki: https://www.nesdev.org/wiki/
- NESDev Forums: https://forums.nesdev.org/

## License
MIT. See [`LICENSE`](LICENSE).
