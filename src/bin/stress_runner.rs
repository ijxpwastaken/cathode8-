use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use cathode8::nes::{
    BUTTON_A, BUTTON_B, BUTTON_DOWN, BUTTON_LEFT, BUTTON_RIGHT, BUTTON_SELECT, BUTTON_START,
    BUTTON_UP, Nes,
};

#[derive(Debug, Clone)]
struct Config {
    rom: PathBuf,
    iterations: u32,
    frames_per_iteration: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rom: PathBuf::from("external/AccuracyCoinRef/AccuracyCoin.nes"),
            iterations: 500,
            frames_per_iteration: 1800,
        }
    }
}

fn parse_args() -> Result<Config> {
    let mut cfg = Config::default();
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--rom" => {
                let value = args.next().context(
                    "--rom requires a path, e.g. --rom external/AccuracyCoinRef/AccuracyCoin.nes",
                )?;
                cfg.rom = PathBuf::from(value);
            }
            "--iterations" => {
                let value = args
                    .next()
                    .context("--iterations requires an integer, e.g. --iterations 500")?;
                cfg.iterations = value
                    .parse::<u32>()
                    .with_context(|| format!("invalid --iterations value: {value}"))?;
            }
            "--frames" => {
                let value = args
                    .next()
                    .context("--frames requires an integer, e.g. --frames 1800")?;
                cfg.frames_per_iteration = value
                    .parse::<u32>()
                    .with_context(|| format!("invalid --frames value: {value}"))?;
            }
            "--help" | "-h" => {
                println!(
                    "stress_runner\n\n\
Usage:\n\
  cargo run --release --bin stress_runner -- [options]\n\n\
Options:\n\
  --rom <path>          ROM path (default external/AccuracyCoinRef/AccuracyCoin.nes)\n\
  --iterations <n>      Number of independent runs (default 500)\n\
  --frames <n>          Frames per run (default 1800)\n\
  -h, --help            Show this help\n"
                );
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
    }

    Ok(cfg)
}

fn next_state(seed: &mut u32) -> u8 {
    // Xorshift32 for deterministic pseudo-random controller patterns.
    let mut x = *seed;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *seed = x;

    let mut state = 0u8;
    if (x & 0x0001) != 0 {
        state |= BUTTON_UP;
    }
    if (x & 0x0002) != 0 {
        state |= BUTTON_DOWN;
    }
    if (x & 0x0004) != 0 {
        state |= BUTTON_LEFT;
    }
    if (x & 0x0008) != 0 {
        state |= BUTTON_RIGHT;
    }
    if (x & 0x0010) != 0 {
        state |= BUTTON_A;
    }
    if (x & 0x0020) != 0 {
        state |= BUTTON_B;
    }
    if (x & 0x0040) != 0 {
        state |= BUTTON_START;
    }
    if (x & 0x0080) != 0 {
        state |= BUTTON_SELECT;
    }

    // Avoid impossible opposite directions.
    if (state & BUTTON_UP) != 0 && (state & BUTTON_DOWN) != 0 {
        state &= !BUTTON_DOWN;
    }
    if (state & BUTTON_LEFT) != 0 && (state & BUTTON_RIGHT) != 0 {
        state &= !BUTTON_RIGHT;
    }

    state
}

fn run_once(cfg: &Config, iteration: u32, seed: &mut u32) -> Result<(u64, u64, u64)> {
    let mut nes = Nes::new();
    nes.load_rom_from_path(&cfg.rom)
        .with_context(|| format!("failed to load ROM {}", cfg.rom.display()))?;

    for frame in 0..cfg.frames_per_iteration {
        // Change input every 15 frames to stress menu/input handling with bursty state transitions.
        let state = if (frame % 15) == 0 {
            next_state(seed)
        } else {
            0
        };
        nes.set_controller_state(state);
        nes.run_frame();
        let _ = nes.take_audio_samples();
    }

    let unknown = nes.debug_unknown_opcode_count();
    let halted = u64::from(nes.debug_halted());
    let cycles = nes.debug_total_cycles();
    let marker = nes.debug_peek_internal_ram(0x00EC);
    let f8 = nes.debug_peek_internal_ram(0x00F8);
    println!(
        "iter={:03} cycles={} halted={} unknown={} ram[$00EC]=${:02X} ram[$00F8]=${:02X}",
        iteration + 1,
        cycles,
        halted,
        unknown,
        marker,
        f8
    );

    Ok((cycles, unknown, halted))
}

fn main() -> Result<()> {
    let cfg = parse_args()?;
    let start = Instant::now();
    let mut seed = 0xC47D0E8Au32;

    let mut total_cycles = 0u64;
    let mut total_unknown = 0u64;
    let mut halted_runs = 0u64;
    let mut failures = 0u64;

    for i in 0..cfg.iterations {
        match run_once(&cfg, i, &mut seed) {
            Ok((cycles, unknown, halted)) => {
                total_cycles = total_cycles.wrapping_add(cycles);
                total_unknown = total_unknown.wrapping_add(unknown);
                halted_runs = halted_runs.wrapping_add(halted);
            }
            Err(err) => {
                failures = failures.wrapping_add(1);
                eprintln!("iter={:03} ERROR: {err}", i + 1);
            }
        }
    }

    println!();
    println!("Stress Summary");
    println!("- iterations: {}", cfg.iterations);
    println!("- frames/iter: {}", cfg.frames_per_iteration);
    println!("- load/runtime failures: {}", failures);
    println!("- halted runs: {}", halted_runs);
    println!("- total unknown opcodes: {}", total_unknown);
    println!("- total cycles: {}", total_cycles);
    println!("- elapsed: {:.2}s", start.elapsed().as_secs_f32());

    if failures > 0 {
        anyhow::bail!("stress runner encountered {failures} failed iteration(s)");
    }
    Ok(())
}
