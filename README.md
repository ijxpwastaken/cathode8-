# Cathode-8
<img width="1261" height="688" alt="Cathode-8 emulator window" src="https://github.com/user-attachments/assets/a5a8011a-787f-4d45-b5c8-69ef3a003f1f" />
**Cathode-8** is a from-scratch NES emulator in Rust with a native desktop UI built with `eframe/egui` and an accuracy-focused emulation core.

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
