use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use cathode8::nes::Nes;
use quick_xml::Reader;
use quick_xml::events::Event;
use sha1::{Digest, Sha1};

#[derive(Debug, Clone)]
struct SuiteTest {
    filename: String,
    system: String,
    runframes: u32,
    tvsha1: String,
    recordedinput: String,
}

#[derive(Debug, Clone)]
struct RunHashes {
    rgba: String,
    rgb: String,
    argb: String,
    bgra: String,
    pc: u16,
    halted: bool,
    total_cycles: u64,
    ppu_ctrl: u8,
    ppu_mask: u8,
    ppu_status: u8,
    ppu_scanline: i16,
    ppu_cycle: i16,
    nmi_serviced: u64,
    ram_f8: u8,
    ram_0a: u8,
    unknown_count: u64,
    last_unknown_opcode: u8,
    last_unknown_pc: u16,
    mask_write_count: u64,
    last_mask_write: u8,
    vram_2000: u8,
    vram_2001: u8,
    vram_23c0: u8,
    pal_00: u8,
    pal_01: u8,
    chr_0200: u8,
    chr_0201: u8,
    vram_2082: u8,
    vram_2083: u8,
    vram_2084: u8,
    vram_non_space_count: usize,
}

#[derive(Debug, Clone)]
struct Config {
    suite: PathBuf,
    rom_root: PathBuf,
    max_tests: usize,
    include_recorded_input: bool,
    include_pal: bool,
    contains: Vec<String>,
    frame_multiplier: u32,
    extra_frames: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            suite: PathBuf::from("external/nes-test-roms/test_roms.xml"),
            rom_root: PathBuf::from("external/nes-test-roms"),
            max_tests: 80,
            include_recorded_input: false,
            include_pal: false,
            contains: Vec::new(),
            frame_multiplier: 1,
            extra_frames: 0,
        }
    }
}

fn parse_args() -> Result<Config> {
    let mut cfg = Config::default();
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--suite" => {
                let value = args.next().context(
                    "--suite requires a path, e.g. --suite external/nes-test-roms/test_roms.xml",
                )?;
                cfg.suite = PathBuf::from(value);
            }
            "--rom-root" => {
                let value = args.next().context(
                    "--rom-root requires a path, e.g. --rom-root external/nes-test-roms",
                )?;
                cfg.rom_root = PathBuf::from(value);
            }
            "--max-tests" => {
                let value = args
                    .next()
                    .context("--max-tests requires an integer, e.g. --max-tests 120")?;
                cfg.max_tests = value
                    .parse::<usize>()
                    .with_context(|| format!("invalid --max-tests value: {value}"))?;
            }
            "--include-recorded-input" => cfg.include_recorded_input = true,
            "--include-pal" => cfg.include_pal = true,
            "--contains" => {
                let value = args
                    .next()
                    .context("--contains requires a substring, e.g. --contains vbl_nmi_timing")?;
                cfg.contains.push(value.to_lowercase());
            }
            "--frame-multiplier" => {
                let value = args
                    .next()
                    .context("--frame-multiplier requires an integer, e.g. --frame-multiplier 2")?;
                cfg.frame_multiplier = value
                    .parse::<u32>()
                    .with_context(|| format!("invalid --frame-multiplier value: {value}"))?;
            }
            "--extra-frames" => {
                let value = args
                    .next()
                    .context("--extra-frames requires an integer, e.g. --extra-frames 120")?;
                cfg.extra_frames = value
                    .parse::<u32>()
                    .with_context(|| format!("invalid --extra-frames value: {value}"))?;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => {
                anyhow::bail!("unknown argument: {other}\nUse --help to view supported options.");
            }
        }
    }

    Ok(cfg)
}

fn print_help() {
    println!(
        "ROM suite runner for Cathode-8\n\n\
Usage:\n\
  cargo run --bin rom_test_runner -- [options]\n\n\
Options:\n\
  --suite <path>                 Path to test_roms.xml\n\
  --rom-root <path>              Root path containing ROM files\n\
  --max-tests <n>                Maximum number of tests to run (default 80)\n\
  --include-recorded-input       Include tests that require replay input\n\
  --include-pal                  Include PAL tests\n\
  --contains <substr>            Only run tests whose filename contains this text (repeatable)\n\
  --frame-multiplier <n>         Multiply XML runframes by n (default 1)\n\
  --extra-frames <n>             Add n frames after XML runframes (default 0)\n\
  -h, --help                     Show this help\n"
    );
}

fn parse_suite_xml(path: &Path) -> Result<Vec<SuiteTest>> {
    let xml = fs::read_to_string(path)
        .with_context(|| format!("failed to read suite XML: {}", path.display()))?;

    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);

    let mut tests = Vec::new();
    let mut current: Option<SuiteTest> = None;
    let mut reading_tvsha1 = false;
    let mut reading_recorded = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.name();
                if name.as_ref() == b"test" {
                    let mut filename = String::new();
                    let mut system = String::new();
                    let mut runframes = 0u32;

                    for attr in e.attributes().flatten() {
                        let key = attr.key.as_ref();
                        let value = attr
                            .decode_and_unescape_value(reader.decoder())
                            .map(|v| v.to_string())
                            .unwrap_or_default();
                        match key {
                            b"filename" => filename = value,
                            b"system" => system = value,
                            b"runframes" => runframes = value.parse::<u32>().unwrap_or(0),
                            _ => {}
                        }
                    }

                    current = Some(SuiteTest {
                        filename,
                        system,
                        runframes,
                        tvsha1: String::new(),
                        recordedinput: String::new(),
                    });
                } else if name.as_ref() == b"tvsha1" {
                    reading_tvsha1 = true;
                } else if name.as_ref() == b"recordedinput" {
                    reading_recorded = true;
                }
            }
            Ok(Event::Text(e)) => {
                let text = e
                    .decode()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|_| String::new());
                if let Some(test) = current.as_mut() {
                    if reading_tvsha1 {
                        test.tvsha1.push_str(&text);
                    } else if reading_recorded {
                        test.recordedinput.push_str(&text);
                    }
                }
            }
            Ok(Event::CData(e)) => {
                let text = e
                    .decode()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|_| String::new());
                if let Some(test) = current.as_mut() {
                    if reading_tvsha1 {
                        test.tvsha1.push_str(&text);
                    } else if reading_recorded {
                        test.recordedinput.push_str(&text);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                if name.as_ref() == b"tvsha1" {
                    reading_tvsha1 = false;
                } else if name.as_ref() == b"recordedinput" {
                    reading_recorded = false;
                } else if name.as_ref() == b"test" {
                    if let Some(mut test) = current.take() {
                        test.tvsha1 = test.tvsha1.trim().to_string();
                        test.recordedinput = test.recordedinput.trim().to_string();
                        tests.push(test);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                anyhow::bail!("failed to parse suite XML: {err}");
            }
            _ => {}
        }
    }

    Ok(tests)
}

fn should_run(test: &SuiteTest, cfg: &Config) -> bool {
    if !cfg.include_pal && test.system.eq_ignore_ascii_case("pal") {
        return false;
    }

    if !cfg.include_recorded_input && !test.recordedinput.is_empty() {
        return false;
    }

    if !cfg.contains.is_empty() {
        let lower = test.filename.to_lowercase();
        if !cfg.contains.iter().any(|f| lower.contains(f)) {
            return false;
        }
    }

    true
}

fn hash_frame_rgba(frame_rgba: &[u8]) -> String {
    let digest = Sha1::digest(frame_rgba);
    BASE64_STANDARD.encode(digest)
}

fn hash_frame_rgb(frame_rgba: &[u8]) -> String {
    let mut rgb = Vec::with_capacity(frame_rgba.len() / 4 * 3);
    for px in frame_rgba.chunks_exact(4) {
        rgb.push(px[0]);
        rgb.push(px[1]);
        rgb.push(px[2]);
    }
    let digest = Sha1::digest(&rgb);
    BASE64_STANDARD.encode(digest)
}

fn hash_frame_argb(frame_rgba: &[u8]) -> String {
    let mut argb = Vec::with_capacity(frame_rgba.len());
    for px in frame_rgba.chunks_exact(4) {
        argb.push(px[3]);
        argb.push(px[0]);
        argb.push(px[1]);
        argb.push(px[2]);
    }
    let digest = Sha1::digest(&argb);
    BASE64_STANDARD.encode(digest)
}

fn hash_frame_bgra(frame_rgba: &[u8]) -> String {
    let mut bgra = Vec::with_capacity(frame_rgba.len());
    for px in frame_rgba.chunks_exact(4) {
        bgra.push(px[2]);
        bgra.push(px[1]);
        bgra.push(px[0]);
        bgra.push(px[3]);
    }
    let digest = Sha1::digest(&bgra);
    BASE64_STANDARD.encode(digest)
}

fn run_single(test: &SuiteTest, cfg: &Config) -> Result<RunHashes> {
    let rom_path = cfg.rom_root.join(&test.filename);
    let mut nes = Nes::new();
    nes.load_rom_from_path(&rom_path)
        .with_context(|| format!("failed to load ROM {}", rom_path.display()))?;

    let total_frames = test
        .runframes
        .saturating_mul(cfg.frame_multiplier)
        .saturating_add(cfg.extra_frames);
    for _ in 0..total_frames {
        nes.run_frame();
    }

    let frame = nes.frame_buffer();
    let (ppu_ctrl, ppu_mask, ppu_status) = nes.debug_ppu_regs();
    let (ppu_scanline, ppu_cycle) = nes.debug_ppu_scanline_cycle();
    let (mask_write_count, last_mask_write) = nes.debug_ppu_mask_writes();
    let (last_unknown_opcode, last_unknown_pc) = nes.debug_last_unknown_opcode();
    let mut vram_non_space_count = 0usize;
    for i in 0..960 {
        if nes.debug_peek_vram(i) != 0x20 {
            vram_non_space_count += 1;
        }
    }
    Ok(RunHashes {
        rgba: hash_frame_rgba(frame),
        rgb: hash_frame_rgb(frame),
        argb: hash_frame_argb(frame),
        bgra: hash_frame_bgra(frame),
        pc: nes.debug_pc(),
        halted: nes.debug_halted(),
        total_cycles: nes.debug_total_cycles(),
        ppu_ctrl,
        ppu_mask,
        ppu_status,
        ppu_scanline,
        ppu_cycle,
        nmi_serviced: nes.debug_nmi_serviced_count(),
        ram_f8: nes.debug_peek_internal_ram(0x00F8),
        ram_0a: nes.debug_peek_internal_ram(0x000A),
        unknown_count: nes.debug_unknown_opcode_count(),
        last_unknown_opcode,
        last_unknown_pc,
        mask_write_count,
        last_mask_write,
        vram_2000: nes.debug_peek_vram(0),
        vram_2001: nes.debug_peek_vram(1),
        vram_23c0: nes.debug_peek_vram(0x03C0),
        pal_00: nes.debug_peek_palette(0),
        pal_01: nes.debug_peek_palette(1),
        chr_0200: nes.debug_peek_chr(0x0200),
        chr_0201: nes.debug_peek_chr(0x0201),
        vram_2082: nes.debug_peek_vram(0x0082),
        vram_2083: nes.debug_peek_vram(0x0083),
        vram_2084: nes.debug_peek_vram(0x0084),
        vram_non_space_count,
    })
}

fn suite_result_pass(test: &SuiteTest, hashes: &RunHashes) -> bool {
    // Blargg VBL/NMI timing ROMs expose result status in RAM ($00F8).
    test.filename.starts_with("vbl_nmi_timing/") && hashes.ram_f8 == 0x01
}

fn main() -> Result<()> {
    let cfg = parse_args()?;

    let start = Instant::now();
    let tests = parse_suite_xml(&cfg.suite)?;

    let selected: Vec<SuiteTest> = tests
        .into_iter()
        .filter(|t| should_run(t, &cfg))
        .take(cfg.max_tests)
        .collect();

    println!(
        "Running {} test(s) from {}",
        selected.len(),
        cfg.suite.display()
    );

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;

    for (idx, test) in selected.iter().enumerate() {
        let label = format!("[{}/{}] {}", idx + 1, selected.len(), test.filename);
        match run_single(test, &cfg) {
            Ok(hashes) if hashes.rgba == test.tvsha1 => {
                passed += 1;
                println!("PASS {label} [rgba]");
            }
            Ok(hashes) if hashes.rgb == test.tvsha1 => {
                passed += 1;
                println!("PASS {label} [rgb]");
            }
            Ok(hashes) if hashes.argb == test.tvsha1 => {
                passed += 1;
                println!("PASS {label} [argb]");
            }
            Ok(hashes) if hashes.bgra == test.tvsha1 => {
                passed += 1;
                println!("PASS {label} [bgra]");
            }
            Ok(hashes) if suite_result_pass(test, &hashes) => {
                passed += 1;
                println!("PASS {label} [suite-result]");
            }
            Ok(hashes) => {
                failed += 1;
                println!(
                    "FAIL {label}\n  expected: {}\n  got rgba: {}\n  got rgb : {}\n  got argb: {}\n  got bgra: {}\n  pc=${:04X} halted={} cycles={} nmi_serviced={}\n  ppu ctrl=${:02X} mask=${:02X} status=${:02X} sl={} cy={}\n  ram[$00F8]=${:02X} ram[$000A]=${:02X}\n  unknown_opcodes={} last=${:02X} @ ${:04X}\n  ppumask_writes={} last_write=${:02X}\n  vram[$2000]=${:02X} vram[$2001]=${:02X} attr[$23C0]=${:02X} vram[$2082]=${:02X} vram[$2083]=${:02X} vram[$2084]=${:02X} nametable_non_space={} pal[0]=${:02X} pal[1]=${:02X} chr[$0200]=${:02X} chr[$0201]=${:02X}",
                    test.tvsha1,
                    hashes.rgba,
                    hashes.rgb,
                    hashes.argb,
                    hashes.bgra,
                    hashes.pc,
                    hashes.halted,
                    hashes.total_cycles,
                    hashes.nmi_serviced,
                    hashes.ppu_ctrl,
                    hashes.ppu_mask,
                    hashes.ppu_status,
                    hashes.ppu_scanline,
                    hashes.ppu_cycle,
                    hashes.ram_f8,
                    hashes.ram_0a,
                    hashes.unknown_count,
                    hashes.last_unknown_opcode,
                    hashes.last_unknown_pc,
                    hashes.mask_write_count,
                    hashes.last_mask_write,
                    hashes.vram_2000,
                    hashes.vram_2001,
                    hashes.vram_23c0,
                    hashes.vram_2082,
                    hashes.vram_2083,
                    hashes.vram_2084,
                    hashes.vram_non_space_count,
                    hashes.pal_00,
                    hashes.pal_01,
                    hashes.chr_0200,
                    hashes.chr_0201
                );
            }
            Err(err) => {
                skipped += 1;
                println!("SKIP {label} -> {err}");
            }
        }
    }

    let elapsed = start.elapsed().as_secs_f32();
    println!();
    println!("Summary:");
    println!("- Passed: {passed}");
    println!("- Failed: {failed}");
    println!("- Skipped: {skipped}");
    println!("- Runtime: {:.2}s", elapsed);

    Ok(())
}
