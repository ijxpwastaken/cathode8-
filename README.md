# Cathode-8 (v5 accuracy core)



A from-scratch NES emulator written in Rust with a native desktop UI.

> [!IMPORTANT]
> **Legal Disclaimer & Anti-Piracy Policy**
> This project is for educational and research purposes only.
> 1. No Piracy
> This emulator does NOT facilitate, encourage, or condone the use of illegal software.
> DO NOT ask for ROMs or where to find them.
> DO NOT use this software to play games you do not legally own.
> We strongly advocate for the preservation of physical media. To use this emulator, you should buy a legitimate NES console and use a hardware dumper (such as INL-retro or CopyNES) to create a private backup of your own cartridges.
> 2. Intellectual Property
> No Proprietary Code: This software contains zero Nintendo code, BIOS files, or copyrighted assets. It is a 100% original implementation based on public hardware documentation found at the NESDev Wiki.
> Trademarks: "NES" and "Nintendo Entertainment System" are trademarks of Nintendo Co., Ltd. This project is in no way affiliated with, authorized, or endorsed by Nintendo.
> 3. Fair Use
> This project falls under fair use for the purposes of interoperability and hardware research, as established in Sony Computer Entertainment, Inc. v. Connectix Corp.

## Features

- NES ROM loading (`.nes`) via:
  - Open file dialog
  - Drag and drop into the app window
- CPU: 6502-compatible core with major official opcode coverage
- PPU: timing-driven rendering pipeline (scroll registers, shifters, odd-frame skip, sprite evaluation)
- Mapper support:
  - Mapper 0 (NROM)
  - Mapper 1 (MMC1)
  - Mapper 2 (UxROM)
  - Mapper 3 (CNROM)
  - Mapper 4 (MMC3)
  - Mapper 9 (MMC2, Punch-Out class boards)
  - Mapper 66 (GxROM)
  - Mapper 71 (Camerica/Codemasters, Bee 52 class boards)
- MMC3 IRQ clocking from PPU A12 transitions (accuracy-focused behavior)
- Port 2 Zapper support (mouse aim + trigger) for light-gun titles
- Controller input mapping from keyboard
- Real-time APU audio output with low-latency desktop playback
- Dark-mode native UI

## Run

```bash
cargo run
```

## Controls

- D-Pad: Arrow keys or `WASD`
- A: `Z`
- B: `X`
- Start: `Enter`
- Select: `Shift`
- Pause/Run: `Space`
- Reset: `R`
- Open ROM: `Ctrl+O`
- Zapper aim: mouse cursor over game image
- Zapper trigger: hold left mouse button

## Architecture

- `src/nes/cartridge.rs`:
  - iNES/NES2 header parsing
- `src/nes/mapper.rs`:
  - Mapper trait + mapper implementations
- `src/nes/cpu.rs`:
  - 6502 CPU execution and instruction decode
- `src/nes/ppu.rs`:
  - PPU registers, timing loop, rendering
- `src/app.rs`:
  - Native GUI and drag-and-drop ROM loading

## Notes

This build prioritizes accuracy over speed (v5 profile). True 100% compatibility still requires full APU emulation and additional mapper edge-case coverage.

## Hardware References Used

- NESDev Wiki
- NES CPU/PPU memory map documentation
- iNES mapper and cartridge format references
