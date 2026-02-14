use anyhow::Result;
use cathode8::nes::Nes;
use std::path::Path;

fn main() -> Result<()> {
    println!("Cathode8 NES Debugger");
    println!("=====================");
    println!();

    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("Usage: cathode8_debug <rom.nes>");
        println!();
        println!("Commands:");
        println!("  step         - Step one instruction");
        println!("  run          - Run continuously");
        println!("  bp <addr>   - Set breakpoint at address");
        println!("  regs        - Show CPU registers");
        println!("  mem <addr>  - Show memory at address");
        println!("  ppu         - Show PPU state");
        println!("  quit        - Exit debugger");
        return Ok(());
    }

    let rom_path = &args[1];
    println!("Loading ROM: {}", rom_path);

    let mut nes = Nes::new();
    nes.load_rom_from_path(Path::new(rom_path))?;

    println!("ROM loaded successfully!");
    println!("Mapper: {}", nes.mapper_name());
    println!();

    println!("Initial state:");
    println!(
        "PC: ${:04X}  A: {:02X}  X: {:02X}  Y: {:02X}  P: {:02X}  SP: {:02X}",
        nes.debug_pc(),
        nes.debug_cpu_regs().0,
        nes.debug_cpu_regs().1,
        nes.debug_cpu_regs().2,
        nes.debug_cpu_regs().3,
        nes.debug_cpu_regs().4
    );

    println!();
    println!("Type 'help' for commands, 'run' to start emulation");

    let mut running = false;

    loop {
        if running {
            nes.run_frame();
            let (nmi, irq, _dma) = nes.debug_interrupt_state();
            if nmi || irq {
                println!("Interrupt! NMI: {}, IRQ: {}", nmi, irq);
            }
        }

        print!("> ");
        std::io::Write::flush(&mut std::io::stdout()).ok();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "help" => {
                println!("Commands:");
                println!("  step, s     - Step one instruction");
                println!("  run, r     - Run continuously");
                println!("  stop       - Stop running");
                println!("  regs       - Show CPU registers");
                println!("  mem <addr> - Show memory bytes (hex)");
                println!("  ppu        - Show PPU state");
                println!(" apu         - Show APU state");
                println!("  mapper     - Show mapper state");
                println!("  quit, q    - Exit debugger");
            }
            "step" | "s" => {
                println!("Stepping not implemented in this build");
            }
            "run" | "r" => {
                running = true;
                println!("Running...");
            }
            "stop" => {
                running = false;
                println!("Stopped");
            }
            "regs" => {
                let (a, x, y, p, sp, pc) = nes.debug_cpu_regs();
                println!("A: ${:02X}  X: ${:02X}  Y: ${:02X}", a, x, y);
                println!("P: {:08b} (NVRBDIZC)", p);
                println!("SP: ${:02X}  PC: ${:04X}", sp, pc);
                println!(
                    "Flags: N={} V={} D={} I={} Z={} C={}",
                    (p & 0x80) != 0,
                    (p & 0x40) != 0,
                    (p & 0x08) != 0,
                    (p & 0x04) != 0,
                    (p & 0x02) != 0,
                    (p & 0x01) != 0
                );
            }
            "mem" => {
                if parts.len() >= 2 {
                    if let Ok(addr) = u16::from_str_radix(parts[1].trim_start_matches("0x"), 16) {
                        println!("Memory ${:04X}-${:04X}:", addr, addr.wrapping_add(15));
                        let mut s = String::new();
                        for i in 0..16 {
                            let a = addr.wrapping_add(i);
                            if i % 8 == 0 {
                                if i > 0 {
                                    println!("{}", s);
                                    s = String::new();
                                }
                                s.push_str(&format!("{:04X}: ", a));
                            }
                            s.push_str(&format!("{:02X} ", nes.debug_peek_internal_ram(a)));
                        }
                        println!("{}", s);
                    }
                } else {
                    println!("Usage: mem <addr>");
                }
            }
            "ppu" => {
                let (scanline, cycle) = nes.debug_ppu_scanline_cycle();
                let (ctrl, mask, status) = nes.debug_ppu_regs();
                println!("PPU State:");
                println!("  Scanline: {}, Cycle: {}", scanline, cycle);
                println!("  $2000 (ctrl):  {:08b}", ctrl);
                println!("  $2001 (mask):  {:08b}", mask);
                println!("  $2002 (status): {:08b}", status);
            }
            "apu" => {
                println!("APU: Use external tools for detailed state");
            }
            "mapper" => {
                println!("Mapper: {}", nes.debug_mapper_state());
            }
            "quit" | "q" => {
                println!("Goodbye!");
                break;
            }
            _ => {
                println!(
                    "Unknown command: {}. Type 'help' for available commands.",
                    parts[0]
                );
            }
        }
    }

    Ok(())
}
