use std::{collections::HashSet, path::PathBuf};

use anyhow::{Context, Result};
use cathode8::nes::{
    BUTTON_A, BUTTON_B, BUTTON_DOWN, BUTTON_LEFT, BUTTON_RIGHT, BUTTON_SELECT, BUTTON_START,
    BUTTON_UP, Nes,
};

#[derive(Debug, Clone)]
struct Config {
    rom: PathBuf,
    frames: u32,
    hold_input_frames: u32,
    input: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rom: PathBuf::from("AccuracyCoin.nes"),
            frames: 3600,
            hold_input_frames: 0,
            input: 0,
        }
    }
}

fn parse_input_bits(value: &str) -> Result<u8> {
    let mut state = 0u8;
    for token in value.split(',').map(|t| t.trim().to_ascii_lowercase()) {
        match token.as_str() {
            "" | "none" => {}
            "up" | "w" => state |= BUTTON_UP,
            "down" | "s" => state |= BUTTON_DOWN,
            "left" | "a" => state |= BUTTON_LEFT,
            "right" | "d" => state |= BUTTON_RIGHT,
            "start" | "enter" => state |= BUTTON_START,
            "select" | "shift" => state |= BUTTON_SELECT,
            "buttona" | "space" | "z" => state |= BUTTON_A,
            "b" | "x" => state |= BUTTON_B,
            other => anyhow::bail!("unknown input token: {other}"),
        }
    }
    Ok(state)
}

fn parse_args() -> Result<Config> {
    let mut cfg = Config::default();
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--rom" => {
                let value = args
                    .next()
                    .context("--rom requires a path, e.g. --rom AccuracyCoin.nes")?;
                cfg.rom = PathBuf::from(value);
            }
            "--frames" => {
                let value = args
                    .next()
                    .context("--frames requires an integer, e.g. --frames 3600")?;
                cfg.frames = value
                    .parse::<u32>()
                    .with_context(|| format!("invalid --frames value: {value}"))?;
            }
            "--hold-input-frames" => {
                let value = args.next().context(
                    "--hold-input-frames requires an integer, e.g. --hold-input-frames 60",
                )?;
                cfg.hold_input_frames = value
                    .parse::<u32>()
                    .with_context(|| format!("invalid --hold-input-frames value: {value}"))?;
            }
            "--input" => {
                let value = args.next().context(
                    "--input requires a list, e.g. --input start or --input down,buttona",
                )?;
                cfg.input = parse_input_bits(&value)?;
            }
            "--help" | "-h" => {
                println!(
                    "accuracycoin_probe\n\n\
Usage:\n\
  cargo run --bin accuracycoin_probe -- [options]\n\n\
Options:\n\
  --rom <path>                  ROM path (default AccuracyCoin.nes)\n\
  --frames <n>                  Number of frames to emulate (default 3600)\n\
  --hold-input-frames <n>       Hold --input state for first n frames (default 0)\n\
  --input <list>                Comma list: up,down,left,right,start,select,buttona,b\n\
  -h, --help                    Show help\n"
                );
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
    }

    Ok(cfg)
}

fn tile_to_char(tile: u8) -> char {
    match tile {
        0x20..=0x7E => tile as char,
        0x00 => ' ',
        _ => '.',
    }
}

fn dump_nametable_text(nes: &Nes) {
    println!("--- Nametable 0 (32x30 tiles) ---");
    for row in 0..30usize {
        let mut line = String::with_capacity(32);
        for col in 0..32usize {
            let idx = row * 32 + col;
            line.push(tile_to_char(nes.debug_peek_vram(idx)));
        }
        println!("{line}");
    }
}

fn summarize_result_ram(nes: &Nes) {
    const RESULT_START: u16 = 0x0400;
    const RESULT_END: u16 = 0x048D;

    let mut unrun = 0usize;
    let mut pass = 0usize;
    let mut fail = 0usize;
    let mut in_progress = 0usize;
    let mut other = 0usize;
    let mut failing = Vec::<(u16, u8)>::new();

    for addr in RESULT_START..=RESULT_END {
        let value = nes.debug_peek_internal_ram(addr);
        match value & 0x03 {
            0 => unrun += 1,
            1 => pass += 1,
            2 => {
                fail += 1;
                failing.push((addr, value));
            }
            3 => in_progress += 1,
            _ => other += 1,
        }
    }

    println!(
        "Result RAM  : range ${:04X}-${:04X} total={} pass={} fail={} unrun={} in_progress={} other={}",
        RESULT_START,
        RESULT_END,
        (RESULT_END - RESULT_START + 1),
        pass,
        fail,
        unrun,
        in_progress,
        other
    );

    if !failing.is_empty() {
        println!("Fail entries:");
        for (addr, value) in failing.iter().take(32) {
            let err_code = value >> 2;
            println!(
                "  ${:04X} = ${:02X} (error code {:X})",
                addr, value, err_code
            );
        }
        if failing.len() > 32 {
            println!("  ... {} more", failing.len() - 32);
        }
    }
}

fn main() -> Result<()> {
    let cfg = parse_args()?;

    let mut nes = Nes::new();
    nes.load_rom_from_path(&cfg.rom)
        .with_context(|| format!("failed to load ROM {}", cfg.rom.display()))?;

    for frame in 0..cfg.frames {
        if frame < cfg.hold_input_frames {
            nes.set_controller_state(cfg.input);
        } else {
            nes.set_controller_state(0);
        }
        nes.run_frame();
        let _ = nes.take_audio_samples();
    }

    let (a, x, y, p, sp, pc) = nes.debug_cpu_regs();
    let (ppu_ctrl, ppu_mask, ppu_status) = nes.debug_ppu_regs();
    let (scanline, cycle) = nes.debug_ppu_scanline_cycle();
    let (unk_opcode, unk_pc) = nes.debug_last_unknown_opcode();
    let debug = nes.debug_counters();
    let ppu_debug = nes.debug_ppu_counters();
    let mut unique_colors = HashSet::new();
    for px in nes.frame_buffer().chunks_exact(4) {
        let packed = u32::from(px[0]) << 24
            | u32::from(px[1]) << 16
            | u32::from(px[2]) << 8
            | u32::from(px[3]);
        unique_colors.insert(packed);
    }

    println!("ROM         : {}", cfg.rom.display());
    println!("Frames      : {}", cfg.frames);
    println!(
        "CPU         : A={:02X} X={:02X} Y={:02X} P={:02X} SP={:02X} PC={:04X}",
        a, x, y, p, sp, pc
    );
    println!(
        "PPU         : CTRL={:02X} MASK={:02X} STATUS={:02X} SL={} CY={}",
        ppu_ctrl, ppu_mask, ppu_status, scanline, cycle
    );
    println!("Frame stats : unique_rgba_colors={}", unique_colors.len());
    println!(
        "Core        : cycles={} halted={} nmi={} irq={} unk_count={} last_unk={:02X}@{:04X}",
        nes.debug_total_cycles(),
        nes.debug_halted(),
        nes.debug_nmi_serviced_count(),
        debug.irq_serviced_count,
        nes.debug_unknown_opcode_count(),
        unk_opcode,
        unk_pc
    );
    println!(
        "Counters    : cpu_reads={} cpu_writes={} oam_dma={} dmc_dma={} dmc_stall={} ppu_ticks={} sprite_ovf={} last_ovf=({}, {}) sprite0_nonzero={} last_nonzero=({}, {}) bg_at_last_nonzero={} opaque={} sprite0_hit={} last_hit=({}, {}) status_reads={} last_status_read=({}, {}) status_ovf_reads={} last_status_ovf_read=({}, {})",
        debug.cpu_reads,
        debug.cpu_writes,
        debug.dma_transfers,
        debug.dmc_dma_transfers,
        debug.dmc_dma_stall_cycles,
        ppu_debug.ticks,
        ppu_debug.sprite_overflow_events,
        ppu_debug.sprite_overflow_last_scanline,
        ppu_debug.sprite_overflow_last_cycle,
        ppu_debug.sprite0_nonzero_events,
        ppu_debug.sprite0_nonzero_last_scanline,
        ppu_debug.sprite0_nonzero_last_cycle,
        ppu_debug.sprite0_nonzero_last_bg_pixel,
        ppu_debug.sprite0_nonzero_last_bg_opaque,
        ppu_debug.sprite0_hit_events,
        ppu_debug.sprite0_hit_last_scanline,
        ppu_debug.sprite0_hit_last_cycle,
        ppu_debug.status_reads,
        ppu_debug.status_read_last_scanline,
        ppu_debug.status_read_last_cycle,
        ppu_debug.status_overflow_reads,
        ppu_debug.status_overflow_last_scanline,
        ppu_debug.status_overflow_last_cycle
    );
    println!(
        "Scroll I/O  : $2005 writes={} last=({}, {}) value=${:02X} phase={} | $2006 writes={} last=({}, {}) value=${:02X} phase={}",
        ppu_debug.scroll_writes_2005,
        ppu_debug.scroll_write_2005_last_scanline,
        ppu_debug.scroll_write_2005_last_cycle,
        ppu_debug.scroll_write_2005_last_value,
        if ppu_debug.scroll_write_2005_last_phase_second {
            "second"
        } else {
            "first"
        },
        ppu_debug.addr_writes_2006,
        ppu_debug.addr_write_2006_last_scanline,
        ppu_debug.addr_write_2006_last_cycle,
        ppu_debug.addr_write_2006_last_value,
        if ppu_debug.addr_write_2006_last_phase_second {
            "second"
        } else {
            "first"
        }
    );
    println!(
        "Scroll prev : $2005 prev=({}, {}) value=${:02X} phase={} | $2006 prev=({}, {}) value=${:02X} phase={}",
        ppu_debug.scroll_write_2005_prev_scanline,
        ppu_debug.scroll_write_2005_prev_cycle,
        ppu_debug.scroll_write_2005_prev_value,
        if ppu_debug.scroll_write_2005_prev_phase_second {
            "second"
        } else {
            "first"
        },
        ppu_debug.addr_write_2006_prev_scanline,
        ppu_debug.addr_write_2006_prev_cycle,
        ppu_debug.addr_write_2006_prev_value,
        if ppu_debug.addr_write_2006_prev_phase_second {
            "second"
        } else {
            "first"
        }
    );
    println!(
        "RAM markers : $000A={:02X} $00F8={:02X} $00F9={:02X} $00FA={:02X} $00FB={:02X} $00FC={:02X}",
        nes.debug_peek_internal_ram(0x000A),
        nes.debug_peek_internal_ram(0x00F8),
        nes.debug_peek_internal_ram(0x00F9),
        nes.debug_peek_internal_ram(0x00FA),
        nes.debug_peek_internal_ram(0x00FB),
        nes.debug_peek_internal_ram(0x00FC),
    );
    println!(
        "Boot marker : $00EC={:02X}",
        nes.debug_peek_internal_ram(0x00EC),
    );
    println!(
        "OAM[0]      : y={:02X} tile={:02X} attr={:02X} x={:02X}",
        nes.debug_peek_oam(0),
        nes.debug_peek_oam(1),
        nes.debug_peek_oam(2),
        nes.debug_peek_oam(3)
    );
    let sprite0_tile = nes.debug_peek_oam(1) as u16;
    let sprite_table = if (ppu_ctrl & 0x08) != 0 {
        0x1000
    } else {
        0x0000
    };
    print!("Sprite0 CHR: ");
    for row in 0..8u16 {
        let lo = nes.debug_peek_chr(sprite_table + sprite0_tile * 16 + row);
        let hi = nes.debug_peek_chr(sprite_table + sprite0_tile * 16 + row + 8);
        print!("[{:02X}/{:02X}] ", lo, hi);
    }
    println!();
    let mut chr_nonzero = 0usize;
    for addr in 0..0x2000u16 {
        if nes.debug_peek_chr(addr) != 0 {
            chr_nonzero += 1;
        }
    }
    println!("CHR nonzero : {} / 8192", chr_nonzero);
    println!("Mapper state: {}", nes.debug_mapper_state());
    summarize_result_ram(&nes);

    dump_nametable_text(&nes);

    Ok(())
}
