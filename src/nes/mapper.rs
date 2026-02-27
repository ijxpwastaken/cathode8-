use anyhow::{Result, bail};

use super::cartridge::Cartridge;

pub const DOCUMENTED_MAPPER_COUNT: u16 = 560;
pub const DOCUMENTED_MAPPER_MAX_ID: u16 = DOCUMENTED_MAPPER_COUNT - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    OneScreenLower,
    OneScreenUpper,
    FourScreen,
}

pub trait Mapper {
    fn cpu_read(&mut self, addr: u16) -> u8;
    fn cpu_write(&mut self, addr: u16, value: u8);
    fn ppu_read(&mut self, addr: u16) -> u8;
    fn ppu_write(&mut self, addr: u16, value: u8);
    fn mirroring(&self) -> Mirroring;
    fn tick_cpu_cycle(&mut self) {}
    fn tick_ppu_cycle(&mut self) {}
    fn ppu_nametable_read(&mut self, _addr: u16, _vram: &[u8; 4096]) -> Option<u8> {
        None
    }
    fn ppu_nametable_write(&mut self, _addr: u16, _value: u8, _vram: &mut [u8; 4096]) -> bool {
        false
    }
    fn notify_ppu_read_addr(&mut self, _addr: u16) {}
    fn notify_ppu_write_addr(&mut self, _addr: u16) {}
    fn suppress_a12_on_sprite_eval_reads(&self) -> bool {
        false
    }
    fn allow_relaxed_sprite0_hit(&self) -> bool {
        false
    }
    fn irq_pending(&self) -> bool {
        false
    }
    fn clear_irq(&mut self) {}
    fn debug_peek_chr(&self, _addr: u16) -> u8 {
        0
    }
    fn debug_state(&self) -> String {
        String::new()
    }
}

pub fn mapper_name(mapper_id: u16) -> &'static str {
    match mapper_id {
        0 => "NROM",
        1 => "MMC1",
        2 => "UxROM",
        3 => "CNROM",
        4 => "MMC3",
        5 => "MMC5",
        7 => "AxROM",
        9 => "MMC2",
        10 => "MMC4",
        15 => "100-in-1",
        19 => "Namco 163",
        21 => "Konami VRC4a",
        22 => "Konami VRC2a",
        23 => "Konami VRC2b/VRC4e",
        24 => "Konami VRC6a",
        25 => "Konami VRC4b/d",
        26 => "Konami VRC6b",
        37 => "PAL-ZZ",
        47 => "MMC3 variant",
        52 => "MMC3 variant",
        66 => "GxROM",
        69 => "FME-7 / Sunsoft 5B",
        71 => "Camerica",
        85 => "Konami VRC7",
        225 => "72-in-1",
        232 => "Quattro",
        342 => "COOLGIRL",
        365 => "NES 2.0 Mapper 365",
        _ if mapper_id <= DOCUMENTED_MAPPER_MAX_ID => "Documented Mapper (generic)",
        _ => "Unsupported",
    }
}

pub fn create_mapper(cart: Cartridge) -> Result<Box<dyn Mapper>> {
    let mapper: Box<dyn Mapper> = match cart.mapper_id {
        0 => Box::new(Mapper0::new(cart)),
        1 => Box::new(Mapper1::new(cart)),
        2 => Box::new(Mapper2::new(cart)),
        3 => Box::new(Mapper3::new(cart)),
        4 => Box::new(Mapper4::new(cart)),
        5 => Box::new(Mapper5::new(cart)),
        7 => Box::new(Mapper7::new(cart)),
        9 => Box::new(Mapper9::new(cart)),
        10 => Box::new(Mapper10::new(cart)),
        19 => Box::new(Mapper19::new(cart)),
        24 => Box::new(Mapper24::new(cart)),
        25 => Box::new(Mapper25::new(cart)),
        26 => Box::new(Mapper26::new(cart)),
        69 => Box::new(Mapper69::new(cart)),
        66 => Box::new(Mapper66::new(cart)),
        71 => Box::new(Mapper71::new(cart)),
        85 => Box::new(Mapper85::new(cart)),
        id if id <= DOCUMENTED_MAPPER_MAX_ID => Box::new(GenericMapper::new(cart)),
        id => {
            bail!(
                "mapper {id} exceeds max supported ({}). Try increasing DOCUMENTED_MAPPER_MAX_ID",
                DOCUMENTED_MAPPER_MAX_ID
            );
        }
    };
    Ok(mapper)
}

struct GenericMapper {
    mapper_id: u16,
    submapper_id: u8,
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    mirroring: Mirroring,
    prg_bank_select: u8,
    chr_bank_select: u8,
}

impl GenericMapper {
    fn new(cart: Cartridge) -> Self {
        Self {
            mapper_id: cart.mapper_id,
            submapper_id: cart.submapper_id,
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            mirroring: cart.mirroring,
            prg_bank_select: 0,
            chr_bank_select: 0,
        }
    }

    fn prg_bank_count_16k(&self) -> usize {
        (self.prg_rom.len() / 0x4000).max(1)
    }

    fn chr_bank_count_8k(&self) -> usize {
        (self.chr.len() / 0x2000).max(1)
    }

    fn read_prg_16k(&self, bank: usize, offset: usize) -> u8 {
        let bank = bank % self.prg_bank_count_16k();
        self.prg_rom[(bank * 0x4000 + offset) % self.prg_rom.len()]
    }
}

impl Mapper for GenericMapper {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0xBFFF => {
                let bank = self.prg_bank_select as usize % self.prg_bank_count_16k();
                self.read_prg_16k(bank, addr as usize - 0x8000)
            }
            0xC000..=0xFFFF => {
                let last = self.prg_bank_count_16k().saturating_sub(1);
                self.read_prg_16k(last, addr as usize - 0xC000)
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx] = value;
            }
            0x8000..=0xFFFF => {
                self.prg_bank_select = value & 0x1F;
                self.chr_bank_select = (value >> 4) & 0x0F;
            }
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        let bank = (self.chr_bank_select as usize) % self.chr_bank_count_8k();
        let offset = (addr as usize) & 0x1FFF;
        let idx = bank * 0x2000 + offset;
        self.chr[idx % self.chr.len()]
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if self.chr_is_ram {
            let bank = (self.chr_bank_select as usize) % self.chr_bank_count_8k();
            let offset = (addr as usize) & 0x1FFF;
            let idx = bank * 0x2000 + offset;
            let mapped = idx % self.chr.len();
            self.chr[mapped] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn debug_peek_chr(&self, addr: u16) -> u8 {
        self.chr[(addr as usize) % self.chr.len()]
    }

    fn debug_state(&self) -> String {
        format!(
            "generic mapper={} submapper={} prg_bank=${:02X} chr_bank=${:02X}",
            self.mapper_id, self.submapper_id, self.prg_bank_select, self.chr_bank_select
        )
    }
}

struct Mapper0 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    mirroring: Mirroring,
}

impl Mapper0 {
    fn new(cart: Cartridge) -> Self {
        let prg_ram_size = cart.prg_ram_size.max(8 * 1024);
        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; prg_ram_size],
            mirroring: cart.mirroring,
        }
    }
}

impl Mapper for Mapper0 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0xFFFF => {
                let mut idx = addr as usize - 0x8000;
                if self.prg_rom.len() == 0x4000 {
                    idx %= 0x4000;
                }
                self.prg_rom[idx]
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        if (0x6000..=0x7FFF).contains(&addr) {
            let idx = (addr as usize - 0x6000) % self.prg_ram.len();
            self.prg_ram[idx] = value;
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        self.chr[addr as usize % self.chr.len()]
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if self.chr_is_ram {
            let idx = addr as usize % self.chr.len();
            self.chr[idx] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn debug_peek_chr(&self, addr: u16) -> u8 {
        self.chr[addr as usize % self.chr.len()]
    }
}

struct Mapper1 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,

    shift_register: u8,
    control: u8,
    chr_bank0: u8,
    chr_bank1: u8,
    prg_bank: u8,
}

impl Mapper1 {
    fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            shift_register: 0x10,
            control: 0x0C,
            chr_bank0: 0,
            chr_bank1: 0,
            prg_bank: 0,
        }
    }

    fn prg_bank_count_16k(&self) -> usize {
        (self.prg_rom.len() / 0x4000).max(1)
    }

    fn chr_bank_count_4k(&self) -> usize {
        (self.chr.len() / 0x1000).max(1)
    }

    fn read_prg_bank(&self, bank: usize, offset: usize) -> u8 {
        let bank = bank % self.prg_bank_count_16k();
        let idx = bank * 0x4000 + offset;
        self.prg_rom[idx % self.prg_rom.len()]
    }

    fn write_shift_register(&mut self, addr: u16, value: u8) {
        if (value & 0x80) != 0 {
            self.shift_register = 0x10;
            self.control |= 0x0C;
            return;
        }

        let commit = (self.shift_register & 0x01) != 0;
        self.shift_register >>= 1;
        self.shift_register |= (value & 0x01) << 4;

        if commit {
            let data = self.shift_register;
            match addr {
                0x8000..=0x9FFF => self.control = data,
                0xA000..=0xBFFF => self.chr_bank0 = data,
                0xC000..=0xDFFF => self.chr_bank1 = data,
                0xE000..=0xFFFF => self.prg_bank = data & 0x0F,
                _ => {}
            }
            self.shift_register = 0x10;
        }
    }

    fn read_chr(&self, addr: u16) -> usize {
        let addr_usize = addr as usize;
        if (self.control & 0x10) == 0 {
            let bank = (self.chr_bank0 as usize & 0x1E) % self.chr_bank_count_4k();
            let base = bank * 0x1000;
            (base + addr_usize) % self.chr.len()
        } else if addr_usize < 0x1000 {
            let bank = (self.chr_bank0 as usize) % self.chr_bank_count_4k();
            (bank * 0x1000 + addr_usize) % self.chr.len()
        } else {
            let bank = (self.chr_bank1 as usize) % self.chr_bank_count_4k();
            (bank * 0x1000 + (addr_usize - 0x1000)) % self.chr.len()
        }
    }
}

impl Mapper for Mapper1 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0xFFFF => {
                let mode = (self.control >> 2) & 0x03;
                let bank = self.prg_bank as usize;
                let offset_16k = (addr as usize) & 0x3FFF;
                match mode {
                    0 | 1 => {
                        let bank32 = bank & !1;
                        let idx = bank32 * 0x4000 + (addr as usize - 0x8000);
                        self.prg_rom[idx % self.prg_rom.len()]
                    }
                    2 => {
                        if addr < 0xC000 {
                            self.read_prg_bank(0, offset_16k)
                        } else {
                            self.read_prg_bank(bank, offset_16k)
                        }
                    }
                    _ => {
                        if addr < 0xC000 {
                            self.read_prg_bank(bank, offset_16k)
                        } else {
                            let last = self.prg_bank_count_16k() - 1;
                            self.read_prg_bank(last, offset_16k)
                        }
                    }
                }
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx] = value;
            }
            0x8000..=0xFFFF => self.write_shift_register(addr, value),
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        let idx = self.read_chr(addr);
        self.chr[idx]
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if self.chr_is_ram {
            let idx = self.read_chr(addr);
            self.chr[idx] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        match self.control & 0x03 {
            0 => Mirroring::OneScreenLower,
            1 => Mirroring::OneScreenUpper,
            2 => Mirroring::Vertical,
            _ => Mirroring::Horizontal,
        }
    }
}

struct Mapper2 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    bank_select: u8,
    mirroring: Mirroring,
}

impl Mapper2 {
    fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            bank_select: 0,
            mirroring: cart.mirroring,
        }
    }

    fn prg_banks(&self) -> usize {
        (self.prg_rom.len() / 0x4000).max(1)
    }

    fn read_prg(&self, bank: usize, offset: usize) -> u8 {
        let bank = bank % self.prg_banks();
        self.prg_rom[bank * 0x4000 + offset]
    }
}

impl Mapper for Mapper2 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0xBFFF => self.read_prg(self.bank_select as usize, addr as usize - 0x8000),
            0xC000..=0xFFFF => self.read_prg(self.prg_banks() - 1, addr as usize - 0xC000),
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx] = value;
            }
            0x8000..=0xFFFF => {
                self.bank_select = value & 0x0F;
            }
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        self.chr[addr as usize % self.chr.len()]
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if self.chr_is_ram {
            let idx = addr as usize % self.chr.len();
            self.chr[idx] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}

struct Mapper3 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    chr_bank_select: u8,
    mirroring: Mirroring,
}

impl Mapper3 {
    fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            chr_bank_select: 0,
            mirroring: cart.mirroring,
        }
    }

    fn prg_read(&self, addr: u16) -> u8 {
        let mut idx = addr as usize - 0x8000;
        if self.prg_rom.len() == 0x4000 {
            idx %= 0x4000;
        }
        self.prg_rom[idx]
    }

    fn chr_bank_count(&self) -> usize {
        (self.chr.len() / 0x2000).max(1)
    }
}

impl Mapper for Mapper3 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0xFFFF => self.prg_read(addr),
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx] = value;
            }
            0x8000..=0xFFFF => self.chr_bank_select = value,
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        let bank = (self.chr_bank_select as usize) % self.chr_bank_count();
        let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
        self.chr[idx % self.chr.len()]
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if self.chr_is_ram {
            let bank = (self.chr_bank_select as usize) % self.chr_bank_count();
            let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
            let mapped = idx % self.chr.len();
            self.chr[mapped] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}

struct Mapper7 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    prg_bank_select: u8,
    mirroring: Mirroring,
}

impl Mapper7 {
    fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            prg_bank_select: 0,
            mirroring: cart.mirroring,
        }
    }

    fn prg_bank_count_32k(&self) -> usize {
        (self.prg_rom.len() / 0x8000).max(1)
    }
}

impl Mapper for Mapper7 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0xFFFF => {
                let bank = (self.prg_bank_select as usize) % self.prg_bank_count_32k();
                let offset = (addr as usize) & 0x7FFF;
                let idx = bank * 0x8000 + offset;
                self.prg_rom[idx % self.prg_rom.len()]
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx] = value;
            }
            0x8000..=0xFFFF => {
                self.prg_bank_select = value & 0x0F;
                self.mirroring = if (value & 0x10) != 0 {
                    Mirroring::OneScreenUpper
                } else {
                    Mirroring::OneScreenLower
                };
            }
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        self.chr[(addr as usize) % self.chr.len()]
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if self.chr_is_ram {
            let idx = (addr as usize) % self.chr.len();
            self.chr[idx] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn debug_peek_chr(&self, addr: u16) -> u8 {
        self.chr[(addr as usize) % self.chr.len()]
    }

    fn debug_state(&self) -> String {
        format!(
            "AxROM prg_bank=${:02X} prg_32k_banks={} mirroring={:?}",
            self.prg_bank_select,
            self.prg_bank_count_32k(),
            self.mirroring
        )
    }
}

struct Mapper10 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    prg_bank: u8,
    chr_fd_0000: u8,
    chr_fe_0000: u8,
    chr_fd_1000: u8,
    chr_fe_1000: u8,
    latch0_is_fe: bool,
    latch1_is_fe: bool,
    mirroring: Mirroring,
}

impl Mapper10 {
    fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            prg_bank: 0,
            chr_fd_0000: 0,
            chr_fe_0000: 0,
            chr_fd_1000: 0,
            chr_fe_1000: 0,
            latch0_is_fe: true,
            latch1_is_fe: true,
            mirroring: cart.mirroring,
        }
    }

    fn prg_bank_count_16k(&self) -> usize {
        (self.prg_rom.len() / 0x4000).max(1)
    }

    fn read_prg_16k(&self, bank: usize, offset: usize) -> u8 {
        let bank = bank % self.prg_bank_count_16k();
        self.prg_rom[(bank * 0x4000 + offset) % self.prg_rom.len()]
    }

    fn chr_bank_count_4k(&self) -> usize {
        (self.chr.len() / 0x1000).max(1)
    }

    fn map_chr_addr(&self, addr: u16) -> usize {
        let bank = if addr < 0x1000 {
            if self.latch0_is_fe {
                self.chr_fe_0000
            } else {
                self.chr_fd_0000
            }
        } else if self.latch1_is_fe {
            self.chr_fe_1000
        } else {
            self.chr_fd_1000
        } as usize
            % self.chr_bank_count_4k();

        bank * 0x1000 + (addr as usize & 0x0FFF)
    }

    fn update_latches(&mut self, addr: u16) {
        match addr {
            0x0FD8 => self.latch0_is_fe = false,
            0x0FE8 => self.latch0_is_fe = true,
            0x1FD8..=0x1FDF => self.latch1_is_fe = false,
            0x1FE8..=0x1FEF => self.latch1_is_fe = true,
            _ => {}
        }
    }
}

impl Mapper for Mapper10 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0xBFFF => self.read_prg_16k(self.prg_bank as usize, addr as usize - 0x8000),
            0xC000..=0xFFFF => {
                let last = self.prg_bank_count_16k().saturating_sub(1);
                self.read_prg_16k(last, addr as usize - 0xC000)
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx] = value;
            }
            0xA000..=0xAFFF => self.prg_bank = value & 0x0F,
            0xB000..=0xBFFF => self.chr_fd_0000 = value & 0x1F,
            0xC000..=0xCFFF => self.chr_fe_0000 = value & 0x1F,
            0xD000..=0xDFFF => self.chr_fd_1000 = value & 0x1F,
            0xE000..=0xEFFF => self.chr_fe_1000 = value & 0x1F,
            0xF000..=0xFFFF => {
                self.mirroring = if (value & 0x01) == 0 {
                    Mirroring::Vertical
                } else {
                    Mirroring::Horizontal
                };
            }
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        let idx = self.map_chr_addr(addr & 0x1FFF) % self.chr.len();
        self.chr[idx]
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if self.chr_is_ram {
            let idx = self.map_chr_addr(addr & 0x1FFF) % self.chr.len();
            self.chr[idx] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn notify_ppu_read_addr(&mut self, addr: u16) {
        self.update_latches(addr & 0x1FFF);
    }

    fn debug_state(&self) -> String {
        format!(
            "MMC4 prg=${:02X} latch0_fe={} latch1_fe={} chr_fd=({:02X},{:02X}) chr_fe=({:02X},{:02X})",
            self.prg_bank,
            self.latch0_is_fe,
            self.latch1_is_fe,
            self.chr_fd_0000,
            self.chr_fd_1000,
            self.chr_fe_0000,
            self.chr_fe_1000
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mapper5PrgTarget {
    Rom,
    Ram,
}

struct Mapper5 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    exram: [u8; 0x400],
    nametable_map: [u8; 4],
    prg_mode: u8,
    chr_mode: u8,
    exram_mode: u8,
    fill_tile: u8,
    fill_attr: u8,
    prg_ram_protect_1: u8,
    prg_ram_protect_2: u8,
    prg_regs: [u8; 5],
    chr_regs: [u16; 12],
    chr_upper_bits: u8,
    irq_scanline_compare: u8,
    irq_enabled: bool,
    irq_pending: bool,
    in_frame: bool,
    scanline_counter: u8,
    last_nametable_probe: u16,
    repeated_nametable_reads: u8,
    scanline_detect_armed: bool,
    cpu_cycles_since_ppu_read: u8,
    mul_a: u8,
    mul_b: u8,
}

impl Mapper5 {
    fn new(cart: Cartridge) -> Self {
        let mut chr_regs = [0u16; 12];
        for (idx, reg) in chr_regs.iter_mut().enumerate() {
            *reg = idx as u16;
        }

        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            exram: [0; 0x400],
            nametable_map: Self::default_nametable_map(cart.mirroring),
            prg_mode: 3,
            chr_mode: 3,
            exram_mode: 0,
            fill_tile: 0,
            fill_attr: 0,
            prg_ram_protect_1: 0,
            prg_ram_protect_2: 0,
            prg_regs: [0, 0, 0, 0, 0xFF],
            chr_regs,
            chr_upper_bits: 0,
            irq_scanline_compare: 0,
            irq_enabled: false,
            irq_pending: false,
            in_frame: false,
            scanline_counter: 0,
            last_nametable_probe: 0,
            repeated_nametable_reads: 0,
            scanline_detect_armed: false,
            cpu_cycles_since_ppu_read: 3,
            mul_a: 0,
            mul_b: 0,
        }
    }

    fn default_nametable_map(mirroring: Mirroring) -> [u8; 4] {
        match mirroring {
            Mirroring::Horizontal => [0, 0, 1, 1],
            Mirroring::Vertical => [0, 1, 0, 1],
            Mirroring::OneScreenLower => [0, 0, 0, 0],
            Mirroring::OneScreenUpper => [1, 1, 1, 1],
            Mirroring::FourScreen => [0, 1, 0, 1],
        }
    }

    fn prg_rom_bank_count_8k(&self) -> usize {
        (self.prg_rom.len() / 0x2000).max(1)
    }

    fn prg_ram_bank_count_8k(&self) -> usize {
        (self.prg_ram.len() / 0x2000).max(1)
    }

    fn chr_bank_count_1k(&self) -> usize {
        (self.chr.len() / 0x0400).max(1)
    }

    fn prg_ram_write_enabled(&self) -> bool {
        (self.prg_ram_protect_1 & 0x03) == 0x02 && (self.prg_ram_protect_2 & 0x03) == 0x01
    }

    fn read_prg_rom_8k(&self, bank: usize, offset: usize) -> u8 {
        let bank = bank % self.prg_rom_bank_count_8k();
        self.prg_rom[(bank * 0x2000 + offset) % self.prg_rom.len()]
    }

    fn read_prg_ram_8k(&self, bank: usize, offset: usize) -> u8 {
        let bank = bank % self.prg_ram_bank_count_8k();
        self.prg_ram[(bank * 0x2000 + offset) % self.prg_ram.len()]
    }

    fn write_prg_ram_8k(&mut self, bank: usize, offset: usize, value: u8) {
        let bank = bank % self.prg_ram_bank_count_8k();
        let idx = (bank * 0x2000 + offset) % self.prg_ram.len();
        self.prg_ram[idx] = value;
    }

    fn decode_window_bank(reg: u8, window_size_kb: u8, window_offset: usize) -> usize {
        match window_size_kb {
            8 => (reg & 0x7F) as usize,
            16 => ((reg & 0x7E) as usize) + ((window_offset >> 13) & 0x01),
            32 => ((reg & 0x7C) as usize) + ((window_offset >> 13) & 0x03),
            _ => 0,
        }
    }

    fn map_prg_addr(&self, addr: u16) -> Option<(Mapper5PrgTarget, usize, usize)> {
        if (0x6000..=0x7FFF).contains(&addr) {
            let bank = (self.prg_regs[0] & 0x7F) as usize;
            let offset = (addr as usize) - 0x6000;
            return Some((Mapper5PrgTarget::Ram, bank, offset));
        }

        if !(0x8000..=0xFFFF).contains(&addr) {
            return None;
        }

        let (reg, window_size_kb, window_offset, allows_ram, force_rom) = match self.prg_mode & 0x03
        {
            0 => (self.prg_regs[4], 32, (addr as usize) - 0x8000, false, true),
            1 => {
                if addr < 0xC000 {
                    (self.prg_regs[2], 16, (addr as usize) - 0x8000, true, false)
                } else {
                    (self.prg_regs[4], 16, (addr as usize) - 0xC000, false, true)
                }
            }
            2 => {
                if addr < 0xC000 {
                    (self.prg_regs[2], 16, (addr as usize) - 0x8000, true, false)
                } else if addr < 0xE000 {
                    (self.prg_regs[3], 8, (addr as usize) - 0xC000, true, false)
                } else {
                    (self.prg_regs[4], 8, (addr as usize) - 0xE000, false, true)
                }
            }
            _ => {
                if addr < 0xA000 {
                    (self.prg_regs[1], 8, (addr as usize) - 0x8000, true, false)
                } else if addr < 0xC000 {
                    (self.prg_regs[2], 8, (addr as usize) - 0xA000, true, false)
                } else if addr < 0xE000 {
                    (self.prg_regs[3], 8, (addr as usize) - 0xC000, true, false)
                } else {
                    (self.prg_regs[4], 8, (addr as usize) - 0xE000, false, true)
                }
            }
        };

        let target = if force_rom {
            Mapper5PrgTarget::Rom
        } else if allows_ram && (reg & 0x80) == 0 {
            Mapper5PrgTarget::Ram
        } else {
            Mapper5PrgTarget::Rom
        };

        let bank = Self::decode_window_bank(reg, window_size_kb, window_offset);
        let offset = window_offset & 0x1FFF;
        Some((target, bank, offset))
    }

    fn map_chr_addr(&self, addr: u16) -> usize {
        let slot = ((addr as usize) & 0x1FFF) / 0x0400;
        let slot_offset = (addr as usize) & 0x03FF;

        let bank_1k = match self.chr_mode & 0x03 {
            0 => {
                let base = self.chr_regs[7] as usize * 8;
                base + slot
            }
            1 => {
                let reg = if slot < 4 {
                    self.chr_regs[3]
                } else {
                    self.chr_regs[7]
                };
                let base = reg as usize * 4;
                base + (slot & 0x03)
            }
            2 => {
                let reg = match slot {
                    0 | 1 => self.chr_regs[1],
                    2 | 3 => self.chr_regs[3],
                    4 | 5 => self.chr_regs[5],
                    _ => self.chr_regs[7],
                };
                let base = reg as usize * 2;
                base + (slot & 0x01)
            }
            _ => self.chr_regs[slot] as usize,
        };

        let bank = bank_1k % self.chr_bank_count_1k();
        bank * 0x0400 + slot_offset
    }

    fn fill_attribute_byte(&self) -> u8 {
        let bits = self.fill_attr & 0x03;
        bits | (bits << 2) | (bits << 4) | (bits << 6)
    }

    fn clock_scanline_detector(&mut self) {
        if !self.in_frame {
            self.in_frame = true;
            self.scanline_counter = 0;
            return;
        }

        self.scanline_counter = self.scanline_counter.wrapping_add(1);
        if self.irq_scanline_compare != 0 && self.scanline_counter == self.irq_scanline_compare {
            self.irq_pending = true;
        }
    }
}

impl Mapper for Mapper5 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x5C00..=0x5FFF => self.exram[(addr as usize) - 0x5C00],
            0x5204 => {
                let status = ((self.irq_pending as u8) << 7) | ((self.in_frame as u8) << 6);
                self.irq_pending = false;
                status
            }
            0x5205 => {
                let product = (self.mul_a as u16) * (self.mul_b as u16);
                (product & 0xFF) as u8
            }
            0x5206 => {
                let product = (self.mul_a as u16) * (self.mul_b as u16);
                (product >> 8) as u8
            }
            _ => {
                if let Some((target, bank, offset)) = self.map_prg_addr(addr) {
                    match target {
                        Mapper5PrgTarget::Rom => self.read_prg_rom_8k(bank, offset),
                        Mapper5PrgTarget::Ram => self.read_prg_ram_8k(bank, offset),
                    }
                } else {
                    0
                }
            }
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x5100 => self.prg_mode = value & 0x03,
            0x5101 => self.chr_mode = value & 0x03,
            0x5102 => self.prg_ram_protect_1 = value,
            0x5103 => self.prg_ram_protect_2 = value,
            0x5104 => self.exram_mode = value & 0x03,
            0x5105 => {
                for (idx, slot) in self.nametable_map.iter_mut().enumerate() {
                    *slot = (value >> (idx * 2)) & 0x03;
                }
            }
            0x5106 => self.fill_tile = value,
            0x5107 => self.fill_attr = value & 0x03,
            0x5113 => self.prg_regs[0] = value,
            0x5114 => self.prg_regs[1] = value,
            0x5115 => self.prg_regs[2] = value,
            0x5116 => self.prg_regs[3] = value,
            0x5117 => self.prg_regs[4] = value,
            0x5120..=0x512B => {
                let idx = (addr as usize) - 0x5120;
                self.chr_regs[idx] = ((self.chr_upper_bits as u16) << 8) | value as u16;
            }
            0x5130 => self.chr_upper_bits = value & 0x03,
            0x5203 => self.irq_scanline_compare = value,
            0x5204 => self.irq_enabled = (value & 0x80) != 0,
            0x5205 => self.mul_a = value,
            0x5206 => self.mul_b = value,
            0x5C00..=0x5FFF => {
                if self.exram_mode != 3 {
                    self.exram[(addr as usize) - 0x5C00] = value;
                }
            }
            0x6000..=0xFFFF => {
                if !self.prg_ram_write_enabled() {
                    return;
                }
                if let Some((target, bank, offset)) = self.map_prg_addr(addr)
                    && target == Mapper5PrgTarget::Ram
                {
                    self.write_prg_ram_8k(bank, offset, value);
                }
            }
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        let idx = self.map_chr_addr(addr & 0x1FFF);
        self.chr[idx % self.chr.len()]
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if self.chr_is_ram {
            let idx = self.map_chr_addr(addr & 0x1FFF) % self.chr.len();
            self.chr[idx] = value;
        }
    }

    fn ppu_nametable_read(&mut self, addr: u16, vram: &[u8; 4096]) -> Option<u8> {
        let mirrored = 0x2000 + ((addr - 0x2000) % 0x1000);
        let table = ((mirrored - 0x2000) / 0x400) as usize;
        let offset = ((mirrored - 0x2000) % 0x400) as usize;

        let value = match self.nametable_map[table] & 0x03 {
            0 | 1 => {
                let page = (self.nametable_map[table] & 0x01) as usize;
                vram[page * 0x400 + offset]
            }
            2 => {
                if self.exram_mode >= 2 {
                    0
                } else {
                    self.exram[offset]
                }
            }
            _ => {
                if offset < 0x3C0 {
                    self.fill_tile
                } else {
                    self.fill_attribute_byte()
                }
            }
        };

        Some(value)
    }

    fn ppu_nametable_write(&mut self, addr: u16, value: u8, vram: &mut [u8; 4096]) -> bool {
        let mirrored = 0x2000 + ((addr - 0x2000) % 0x1000);
        let table = ((mirrored - 0x2000) / 0x400) as usize;
        let offset = ((mirrored - 0x2000) % 0x400) as usize;

        match self.nametable_map[table] & 0x03 {
            0 | 1 => {
                let page = (self.nametable_map[table] & 0x01) as usize;
                vram[page * 0x400 + offset] = value;
            }
            2 => {
                if self.exram_mode != 3 {
                    self.exram[offset] = value;
                }
            }
            _ => {}
        }

        true
    }

    fn mirroring(&self) -> Mirroring {
        Mirroring::FourScreen
    }

    fn tick_cpu_cycle(&mut self) {
        self.cpu_cycles_since_ppu_read = self.cpu_cycles_since_ppu_read.saturating_add(1).min(3);
        if self.cpu_cycles_since_ppu_read >= 3 {
            self.in_frame = false;
            self.scanline_counter = 0;
            self.irq_pending = false;
            self.scanline_detect_armed = false;
            self.repeated_nametable_reads = 0;
        }
    }

    fn notify_ppu_read_addr(&mut self, addr: u16) {
        self.cpu_cycles_since_ppu_read = 0;

        if self.scanline_detect_armed {
            self.clock_scanline_detector();
            self.scanline_detect_armed = false;
        }

        if (0x2000..=0x3EFF).contains(&addr) {
            let probe = 0x2000 + ((addr - 0x2000) % 0x1000);
            if probe < 0x3000 {
                if probe == self.last_nametable_probe {
                    self.repeated_nametable_reads = self.repeated_nametable_reads.saturating_add(1);
                } else {
                    self.last_nametable_probe = probe;
                    self.repeated_nametable_reads = 1;
                }

                if self.repeated_nametable_reads >= 3 {
                    self.scanline_detect_armed = true;
                    self.repeated_nametable_reads = 0;
                }
                return;
            }
        }

        self.repeated_nametable_reads = 0;
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending && self.irq_enabled
    }

    fn clear_irq(&mut self) {
        self.irq_pending = false;
    }

    fn debug_state(&self) -> String {
        format!(
            "MMC5 prg_mode={} chr_mode={} exram_mode={} prg=[{:02X},{:02X},{:02X},{:02X},{:02X}] nt=[{},{},{},{}] scanline={}/{} irq={}/{}",
            self.prg_mode,
            self.chr_mode,
            self.exram_mode,
            self.prg_regs[0],
            self.prg_regs[1],
            self.prg_regs[2],
            self.prg_regs[3],
            self.prg_regs[4],
            self.nametable_map[0],
            self.nametable_map[1],
            self.nametable_map[2],
            self.nametable_map[3],
            self.scanline_counter,
            self.irq_scanline_compare,
            self.irq_pending,
            self.irq_enabled
        )
    }
}

struct Mapper19 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    chr_nt_banks: [u8; 12],
    prg_bank_8000: u8,
    prg_bank_a000: u8,
    prg_bank_c000: u8,
    disable_chrram_low: bool,
    disable_chrram_high: bool,
    ram_write_protect: u8,
    irq_counter: u16,
    irq_enabled: bool,
    irq_pending: bool,
    ciram_shadow: [u8; 0x800],
    internal_ram: [u8; 128],
    internal_addr: u8,
    internal_auto_inc: bool,
}

impl Mapper19 {
    fn new(cart: Cartridge) -> Self {
        let mut chr_nt_banks = [0u8; 12];
        for (idx, bank) in chr_nt_banks.iter_mut().enumerate() {
            *bank = idx as u8;
        }

        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            chr_nt_banks,
            prg_bank_8000: 0,
            prg_bank_a000: 1,
            prg_bank_c000: 2,
            disable_chrram_low: false,
            disable_chrram_high: false,
            ram_write_protect: 0x4F,
            irq_counter: 0,
            irq_enabled: false,
            irq_pending: false,
            ciram_shadow: [0; 0x800],
            internal_ram: [0; 128],
            internal_addr: 0,
            internal_auto_inc: false,
        }
    }

    fn prg_bank_count_8k(&self) -> usize {
        (self.prg_rom.len() / 0x2000).max(1)
    }

    fn chr_bank_count_1k(&self) -> usize {
        (self.chr.len() / 0x0400).max(1)
    }

    fn read_prg_rom_8k(&self, bank: usize, offset: usize) -> u8 {
        let bank = bank % self.prg_bank_count_8k();
        self.prg_rom[(bank * 0x2000 + offset) % self.prg_rom.len()]
    }

    fn map_chr_bank(&self, bank: u8, offset: usize) -> usize {
        let mapped = (bank as usize) % self.chr_bank_count_1k();
        mapped * 0x0400 + offset
    }

    fn pattern_slot_uses_ciram(&self, slot: usize, bank: u8) -> bool {
        if bank < 0xE0 {
            return false;
        }
        if slot < 4 {
            !self.disable_chrram_low
        } else {
            !self.disable_chrram_high
        }
    }

    fn ciram_index(bank: u8, offset: usize) -> usize {
        ((bank as usize) & 0x01) * 0x0400 + offset
    }

    fn prg_ram_write_enabled_for_addr(&self, addr: u16) -> bool {
        let key = (self.ram_write_protect >> 4) & 0x0F;
        if key != 0x04 {
            return false;
        }
        let window = ((addr - 0x6000) / 0x0800) as usize;
        ((self.ram_write_protect >> window) & 0x01) == 0
    }

    fn read_internal_ram(&mut self) -> u8 {
        let idx = (self.internal_addr & 0x7F) as usize;
        let value = self.internal_ram[idx];
        if self.internal_auto_inc {
            self.internal_addr = (self.internal_addr.wrapping_add(1)) & 0x7F;
        }
        value
    }

    fn write_internal_ram(&mut self, value: u8) {
        let idx = (self.internal_addr & 0x7F) as usize;
        self.internal_ram[idx] = value;
        if self.internal_auto_inc {
            self.internal_addr = (self.internal_addr.wrapping_add(1)) & 0x7F;
        }
    }
}

impl Mapper for Mapper19 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x4800 => self.read_internal_ram(),
            0x5000 => (self.irq_counter & 0x00FF) as u8,
            0x5800 => ((self.irq_enabled as u8) << 7) | ((self.irq_counter >> 8) as u8 & 0x7F),
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0x9FFF => {
                self.read_prg_rom_8k(self.prg_bank_8000 as usize, addr as usize - 0x8000)
            }
            0xA000..=0xBFFF => {
                self.read_prg_rom_8k(self.prg_bank_a000 as usize, addr as usize - 0xA000)
            }
            0xC000..=0xDFFF => {
                self.read_prg_rom_8k(self.prg_bank_c000 as usize, addr as usize - 0xC000)
            }
            0xE000..=0xFFFF => {
                let last = self.prg_bank_count_8k().saturating_sub(1);
                self.read_prg_rom_8k(last, addr as usize - 0xE000)
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x4800 => self.write_internal_ram(value),
            0x5000 => {
                self.irq_counter = (self.irq_counter & 0x7F00) | value as u16;
                self.irq_pending = false;
            }
            0x5800 => {
                self.irq_counter = (self.irq_counter & 0x00FF) | (((value as u16) & 0x7F) << 8);
                self.irq_enabled = (value & 0x80) != 0;
                self.irq_pending = false;
            }
            0x6000..=0x7FFF => {
                if self.prg_ram_write_enabled_for_addr(addr) {
                    let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                    self.prg_ram[idx] = value;
                }
            }
            0x8000..=0xDFFF => {
                let idx = ((addr - 0x8000) / 0x0800) as usize;
                if idx < self.chr_nt_banks.len() {
                    self.chr_nt_banks[idx] = value;
                }
            }
            0xE000..=0xE7FF => {
                self.prg_bank_8000 = value & 0x3F;
            }
            0xE800..=0xEFFF => {
                self.prg_bank_a000 = value & 0x3F;
                self.disable_chrram_low = (value & 0x40) != 0;
                self.disable_chrram_high = (value & 0x80) != 0;
            }
            0xF000..=0xF7FF => {
                self.prg_bank_c000 = value & 0x3F;
            }
            0xF800..=0xFFFF => {
                self.ram_write_protect = value;
                self.internal_addr = value & 0x7F;
                self.internal_auto_inc = (value & 0x80) != 0;
            }
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        let slot = ((addr as usize) & 0x1FFF) / 0x0400;
        let offset = (addr as usize) & 0x03FF;
        let bank = self.chr_nt_banks[slot];

        if self.pattern_slot_uses_ciram(slot, bank) {
            let idx = Self::ciram_index(bank, offset);
            self.ciram_shadow[idx]
        } else {
            let idx = self.map_chr_bank(bank, offset);
            self.chr[idx % self.chr.len()]
        }
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        let slot = ((addr as usize) & 0x1FFF) / 0x0400;
        let offset = (addr as usize) & 0x03FF;
        let bank = self.chr_nt_banks[slot];

        if self.pattern_slot_uses_ciram(slot, bank) {
            let idx = Self::ciram_index(bank, offset);
            self.ciram_shadow[idx] = value;
            return;
        }

        if self.chr_is_ram {
            let idx = self.map_chr_bank(bank, offset) % self.chr.len();
            self.chr[idx] = value;
        }
    }

    fn ppu_nametable_read(&mut self, addr: u16, vram: &[u8; 4096]) -> Option<u8> {
        let mirrored = 0x2000 + ((addr - 0x2000) % 0x1000);
        let slot = ((mirrored - 0x2000) / 0x0400) as usize;
        let offset = ((mirrored - 0x2000) % 0x0400) as usize;
        let bank = self.chr_nt_banks[8 + slot];

        if bank >= 0xE0 {
            let idx = Self::ciram_index(bank, offset);
            self.ciram_shadow[idx] = vram[idx];
            Some(vram[idx])
        } else {
            let idx = self.map_chr_bank(bank, offset);
            Some(self.chr[idx % self.chr.len()])
        }
    }

    fn ppu_nametable_write(&mut self, addr: u16, value: u8, vram: &mut [u8; 4096]) -> bool {
        let mirrored = 0x2000 + ((addr - 0x2000) % 0x1000);
        let slot = ((mirrored - 0x2000) / 0x0400) as usize;
        let offset = ((mirrored - 0x2000) % 0x0400) as usize;
        let bank = self.chr_nt_banks[8 + slot];

        if bank >= 0xE0 {
            let idx = Self::ciram_index(bank, offset);
            vram[idx] = value;
            self.ciram_shadow[idx] = value;
        } else if self.chr_is_ram {
            let idx = self.map_chr_bank(bank, offset) % self.chr.len();
            self.chr[idx] = value;
        }

        true
    }

    fn mirroring(&self) -> Mirroring {
        Mirroring::FourScreen
    }

    fn tick_cpu_cycle(&mut self) {
        if !self.irq_enabled || self.irq_pending {
            return;
        }

        if self.irq_counter < 0x7FFF {
            self.irq_counter = self.irq_counter.wrapping_add(1);
            if self.irq_counter == 0x7FFF {
                self.irq_pending = true;
            }
        }
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn clear_irq(&mut self) {
        self.irq_pending = false;
    }

    fn debug_state(&self) -> String {
        format!(
            "Namco163 prg=[{:02X},{:02X},{:02X},FF] nt=[{:02X},{:02X},{:02X},{:02X}] irq={:04X}/{}{} wp={:02X}",
            self.prg_bank_8000,
            self.prg_bank_a000,
            self.prg_bank_c000,
            self.chr_nt_banks[8],
            self.chr_nt_banks[9],
            self.chr_nt_banks[10],
            self.chr_nt_banks[11],
            self.irq_counter,
            self.irq_enabled,
            if self.irq_pending { " pending" } else { "" },
            self.ram_write_protect
        )
    }
}

struct Mapper69 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    mirroring: Mirroring,
    command: u8,
    chr_banks: [u8; 8],
    prg_banks: [u8; 3],
    prg_bank_6000: u8,
    map_6000_to_ram: bool,
    ram_enable: bool,
    irq_counter: u16,
    irq_enabled: bool,
    irq_counter_enabled: bool,
    irq_pending: bool,
}

impl Mapper69 {
    fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            mirroring: cart.mirroring,
            command: 0,
            chr_banks: [0, 1, 2, 3, 4, 5, 6, 7],
            prg_banks: [0, 1, 2],
            prg_bank_6000: 0,
            map_6000_to_ram: true,
            ram_enable: true,
            irq_counter: 0,
            irq_enabled: false,
            irq_counter_enabled: false,
            irq_pending: false,
        }
    }

    fn prg_bank_count_8k(&self) -> usize {
        (self.prg_rom.len() / 0x2000).max(1)
    }

    fn chr_bank_count_1k(&self) -> usize {
        (self.chr.len() / 0x0400).max(1)
    }

    fn prg_ram_bank_count_8k(&self) -> usize {
        (self.prg_ram.len() / 0x2000).max(1)
    }

    fn read_prg_8k(&self, bank: usize, offset: usize) -> u8 {
        let bank = bank % self.prg_bank_count_8k();
        let idx = bank * 0x2000 + offset;
        self.prg_rom[idx % self.prg_rom.len()]
    }

    fn map_chr_addr(&self, addr: u16) -> usize {
        let slot = ((addr as usize) & 0x1FFF) / 0x0400;
        let bank = (self.chr_banks[slot] as usize) % self.chr_bank_count_1k();
        bank * 0x0400 + ((addr as usize) & 0x03FF)
    }

    fn write_command_param(&mut self, value: u8) {
        match self.command & 0x0F {
            0x0..=0x7 => self.chr_banks[(self.command & 0x07) as usize] = value,
            0x8 => {
                self.prg_bank_6000 = value & 0x3F;
                self.map_6000_to_ram = (value & 0x40) != 0;
                self.ram_enable = (value & 0x80) != 0;
            }
            0x9 => self.prg_banks[0] = value & 0x3F,
            0xA => self.prg_banks[1] = value & 0x3F,
            0xB => self.prg_banks[2] = value & 0x3F,
            0xC => {
                self.mirroring = match value & 0x03 {
                    0 => Mirroring::Vertical,
                    1 => Mirroring::Horizontal,
                    2 => Mirroring::OneScreenLower,
                    _ => Mirroring::OneScreenUpper,
                };
            }
            0xD => {
                // On FME-7, writes to IRQ control acknowledge pending IRQ.
                self.irq_pending = false;
                self.irq_enabled = (value & 0x01) != 0;
                self.irq_counter_enabled = (value & 0x80) != 0;
            }
            0xE => {
                self.irq_counter = (self.irq_counter & 0xFF00) | value as u16;
            }
            0xF => {
                self.irq_counter = (self.irq_counter & 0x00FF) | ((value as u16) << 8);
            }
            _ => {}
        }
    }
}

impl Mapper for Mapper69 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let offset = (addr as usize) - 0x6000;
                if self.map_6000_to_ram {
                    if !self.ram_enable {
                        return 0;
                    }
                    let bank = (self.prg_bank_6000 as usize) % self.prg_ram_bank_count_8k();
                    let idx = bank * 0x2000 + offset;
                    self.prg_ram[idx % self.prg_ram.len()]
                } else {
                    self.read_prg_8k(self.prg_bank_6000 as usize, offset)
                }
            }
            0x8000..=0x9FFF => self.read_prg_8k(self.prg_banks[0] as usize, addr as usize - 0x8000),
            0xA000..=0xBFFF => self.read_prg_8k(self.prg_banks[1] as usize, addr as usize - 0xA000),
            0xC000..=0xDFFF => self.read_prg_8k(self.prg_banks[2] as usize, addr as usize - 0xC000),
            0xE000..=0xFFFF => {
                let last = self.prg_bank_count_8k().saturating_sub(1);
                self.read_prg_8k(last, addr as usize - 0xE000)
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                if self.map_6000_to_ram && self.ram_enable {
                    let offset = (addr as usize) - 0x6000;
                    let bank = (self.prg_bank_6000 as usize) % self.prg_ram_bank_count_8k();
                    let idx = bank * 0x2000 + offset;
                    let mapped = idx % self.prg_ram.len();
                    self.prg_ram[mapped] = value;
                }
            }
            0x8000..=0x9FFF => self.command = value & 0x0F,
            0xA000..=0xBFFF => self.write_command_param(value),
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        let idx = self.map_chr_addr(addr) % self.chr.len();
        self.chr[idx]
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if self.chr_is_ram {
            let idx = self.map_chr_addr(addr) % self.chr.len();
            self.chr[idx] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn tick_cpu_cycle(&mut self) {
        if !self.irq_counter_enabled {
            return;
        }
        let previous = self.irq_counter;
        self.irq_counter = self.irq_counter.wrapping_sub(1);
        if previous == 0 && self.irq_enabled {
            self.irq_pending = true;
        }
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn clear_irq(&mut self) {
        self.irq_pending = false;
    }

    fn debug_state(&self) -> String {
        format!(
            "FME7 cmd={} prg=[{:02X},{:02X},{:02X}] 6000={:02X} ram={} en={} irq={:04X}/{}{}",
            self.command,
            self.prg_banks[0],
            self.prg_banks[1],
            self.prg_banks[2],
            self.prg_bank_6000,
            self.map_6000_to_ram,
            self.ram_enable,
            self.irq_counter,
            self.irq_counter_enabled,
            if self.irq_pending { " pending" } else { "" }
        )
    }
}

struct Mapper9 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    prg_bank: u8,
    chr_fd_0000: u8,
    chr_fe_0000: u8,
    chr_fd_1000: u8,
    chr_fe_1000: u8,
    latch0_is_fe: bool,
    latch1_is_fe: bool,
    mirroring: Mirroring,
}

impl Mapper9 {
    fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            prg_bank: 0,
            chr_fd_0000: 0,
            chr_fe_0000: 0,
            chr_fd_1000: 0,
            chr_fe_1000: 0,
            latch0_is_fe: true,
            latch1_is_fe: true,
            mirroring: cart.mirroring,
        }
    }

    fn prg_bank_count_8k(&self) -> usize {
        (self.prg_rom.len() / 0x2000).max(1)
    }

    fn read_prg_8k(&self, bank: usize, offset: usize) -> u8 {
        let bank = bank % self.prg_bank_count_8k();
        self.prg_rom[(bank * 0x2000 + offset) % self.prg_rom.len()]
    }

    fn map_chr_addr(&self, addr: u16) -> usize {
        let bank = if addr < 0x1000 {
            if self.latch0_is_fe {
                self.chr_fe_0000
            } else {
                self.chr_fd_0000
            }
        } else if self.latch1_is_fe {
            self.chr_fe_1000
        } else {
            self.chr_fd_1000
        } as usize;

        let base = bank * 0x1000;
        let offset = (addr as usize) & 0x0FFF;
        (base + offset) % self.chr.len()
    }

    fn update_latches(&mut self, addr: u16) {
        // MMC2 latch trigger addresses selected by PPU pattern fetches.
        match addr {
            0x0FD8 => self.latch0_is_fe = false,
            0x0FE8 => self.latch0_is_fe = true,
            0x1FD8..=0x1FDF => self.latch1_is_fe = false,
            0x1FE8..=0x1FEF => self.latch1_is_fe = true,
            _ => {}
        }
    }
}

impl Mapper for Mapper9 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0x9FFF => self.read_prg_8k(self.prg_bank as usize, addr as usize - 0x8000),
            0xA000..=0xBFFF => {
                let banks = self.prg_bank_count_8k();
                self.read_prg_8k(banks.saturating_sub(3), addr as usize - 0xA000)
            }
            0xC000..=0xDFFF => {
                let banks = self.prg_bank_count_8k();
                self.read_prg_8k(banks.saturating_sub(2), addr as usize - 0xC000)
            }
            0xE000..=0xFFFF => {
                let banks = self.prg_bank_count_8k();
                self.read_prg_8k(banks.saturating_sub(1), addr as usize - 0xE000)
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx] = value;
            }
            0xA000..=0xAFFF => self.prg_bank = value & 0x0F,
            0xB000..=0xBFFF => self.chr_fd_0000 = value & 0x1F,
            0xC000..=0xCFFF => self.chr_fe_0000 = value & 0x1F,
            0xD000..=0xDFFF => self.chr_fd_1000 = value & 0x1F,
            0xE000..=0xEFFF => self.chr_fe_1000 = value & 0x1F,
            0xF000..=0xFFFF => {
                self.mirroring = if (value & 0x01) == 0 {
                    Mirroring::Vertical
                } else {
                    Mirroring::Horizontal
                };
            }
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        let idx = self.map_chr_addr(addr & 0x1FFF);
        self.chr[idx]
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if self.chr_is_ram {
            let idx = self.map_chr_addr(addr & 0x1FFF);
            self.chr[idx] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn notify_ppu_read_addr(&mut self, addr: u16) {
        self.update_latches(addr & 0x1FFF);
    }
}

struct Mapper66 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_bank: u8,
    chr_bank: u8,
    mirroring: Mirroring,
}

impl Mapper66 {
    fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_bank: 0,
            chr_bank: 0,
            mirroring: cart.mirroring,
        }
    }

    fn prg_bank_count_32k(&self) -> usize {
        (self.prg_rom.len() / 0x8000).max(1)
    }

    fn chr_bank_count_8k(&self) -> usize {
        (self.chr.len() / 0x2000).max(1)
    }
}

impl Mapper for Mapper66 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xFFFF => {
                let bank = (self.prg_bank as usize) % self.prg_bank_count_32k();
                let offset = (addr as usize) & 0x7FFF;
                let idx = bank * 0x8000 + offset;
                self.prg_rom[idx % self.prg_rom.len()]
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        if (0x8000..=0xFFFF).contains(&addr) {
            self.chr_bank = value & 0x03;
            self.prg_bank = (value >> 4) & 0x03;
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        let bank = (self.chr_bank as usize) % self.chr_bank_count_8k();
        let offset = (addr as usize) & 0x1FFF;
        let idx = bank * 0x2000 + offset;
        self.chr[idx % self.chr.len()]
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if self.chr_is_ram {
            let bank = (self.chr_bank as usize) % self.chr_bank_count_8k();
            let offset = (addr as usize) & 0x1FFF;
            let idx = bank * 0x2000 + offset;
            let mapped = idx % self.chr.len();
            self.chr[mapped] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}

struct Mapper71 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    prg_ram: Vec<u8>,
    bank_select: u8,
    bank_mask: u8,
    mirroring: Mirroring,
    submapper_id: u8,
    mirroring_control_supported: bool,
    debug_bank_write_count: u64,
    debug_mirroring_write_count: u64,
    debug_last_bank_write_addr: u16,
    debug_last_bank_value: u8,
    debug_last_mirroring_value: u8,
}

impl Mapper71 {
    fn new(cart: Cartridge) -> Self {
        // Mapper 71 uses CHR-RAM in hardware; honor writes even if a dump
        // incorrectly advertises CHR-ROM.
        let mut chr = cart.chr_data;
        if chr.len() < 0x2000 {
            chr.resize(0x2000, 0);
        }

        // NES 2.0 submapper 1 (Fire Hawk) has 3-bit banking and
        // one-screen mirroring control at $9000-$9FFF.
        let bank_mask = if cart.submapper_id == 1 { 0x07 } else { 0x0F };
        Self {
            prg_rom: cart.prg_rom,
            chr,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            bank_select: 0,
            bank_mask,
            mirroring: cart.mirroring,
            submapper_id: cart.submapper_id,
            mirroring_control_supported: cart.submapper_id == 1,
            debug_bank_write_count: 0,
            debug_mirroring_write_count: 0,
            debug_last_bank_write_addr: 0,
            debug_last_bank_value: 0,
            debug_last_mirroring_value: 0,
        }
    }

    fn prg_bank_count_16k(&self) -> usize {
        (self.prg_rom.len() / 0x4000).max(1)
    }

    fn read_prg_16k(&self, bank: usize, offset: usize) -> u8 {
        let bank = bank % self.prg_bank_count_16k();
        self.prg_rom[(bank * 0x4000 + offset) % self.prg_rom.len()]
    }
}

impl Mapper for Mapper71 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0xBFFF => self.read_prg_16k(self.bank_select as usize, addr as usize - 0x8000),
            0xC000..=0xFFFF => {
                let last = self.prg_bank_count_16k().saturating_sub(1);
                self.read_prg_16k(last, addr as usize - 0xC000)
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx] = value;
            }
            0x9000..=0x9FFF => {
                if self.mirroring_control_supported {
                    self.mirroring = if (value & 0x10) != 0 {
                        Mirroring::OneScreenUpper
                    } else {
                        Mirroring::OneScreenLower
                    };
                    self.debug_mirroring_write_count =
                        self.debug_mirroring_write_count.wrapping_add(1);
                    self.debug_last_mirroring_value = value;
                }
            }
            0xC000..=0xFFFF => {
                self.bank_select = value & self.bank_mask;
                self.debug_bank_write_count = self.debug_bank_write_count.wrapping_add(1);
                self.debug_last_bank_write_addr = addr;
                self.debug_last_bank_value = value;
            }
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        self.chr[(addr as usize) % self.chr.len()]
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        let idx = (addr as usize) % self.chr.len();
        self.chr[idx] = value;
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn debug_peek_chr(&self, addr: u16) -> u8 {
        self.chr[(addr as usize) % self.chr.len()]
    }

    fn allow_relaxed_sprite0_hit(&self) -> bool {
        true
    }

    fn debug_state(&self) -> String {
        format!(
            "submapper={} bank_select=${:02X} bank_mask=${:02X} prg_16k_banks={} chr_ram_kib={} mirroring={:?} bank_writes={} mirror_writes={} last_bank=${:04X}:${:02X} last_mirror=${:02X}",
            self.submapper_id,
            self.bank_select,
            self.bank_mask,
            self.prg_bank_count_16k(),
            self.chr.len() / 1024,
            self.mirroring,
            self.debug_bank_write_count,
            self.debug_mirroring_write_count,
            self.debug_last_bank_write_addr,
            self.debug_last_bank_value,
            self.debug_last_mirroring_value
        )
    }
}

struct Mapper4 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    bank_select: u8,
    bank_regs: [u8; 8],
    mirroring: Mirroring,
    four_screen: bool,

    irq_latch: u8,
    irq_counter: u8,
    irq_reload: bool,
    irq_enabled: bool,
    irq_pending: bool,
    last_a12: bool,
    a12_low_cycles: u8,
    debug_a12_high_samples: u64,
    debug_irq_clocks: u64,
}

impl Mapper4 {
    fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            bank_select: 0,
            bank_regs: [0; 8],
            mirroring: cart.mirroring,
            four_screen: cart.four_screen,
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enabled: false,
            irq_pending: false,
            last_a12: false,
            a12_low_cycles: 0,
            debug_a12_high_samples: 0,
            debug_irq_clocks: 0,
        }
    }

    fn prg_bank_count_8k(&self) -> usize {
        (self.prg_rom.len() / 0x2000).max(1)
    }

    fn chr_bank_count_1k(&self) -> usize {
        (self.chr.len() / 0x0400).max(1)
    }

    fn read_prg_bank_8k(&self, bank: usize, offset: usize) -> u8 {
        let bank = bank % self.prg_bank_count_8k();
        let idx = bank * 0x2000 + offset;
        self.prg_rom[idx % self.prg_rom.len()]
    }

    fn map_chr_addr(&self, addr: u16) -> usize {
        let r0 = self.bank_regs[0] & 0xFE;
        let r1 = self.bank_regs[1] & 0xFE;
        let r2 = self.bank_regs[2];
        let r3 = self.bank_regs[3];
        let r4 = self.bank_regs[4];
        let r5 = self.bank_regs[5];

        let banks = if (self.bank_select & 0x80) == 0 {
            [
                r0,
                r0.wrapping_add(1),
                r1,
                r1.wrapping_add(1),
                r2,
                r3,
                r4,
                r5,
            ]
        } else {
            [
                r2,
                r3,
                r4,
                r5,
                r0,
                r0.wrapping_add(1),
                r1,
                r1.wrapping_add(1),
            ]
        };

        let slot = (addr as usize) / 0x0400;
        let bank = banks[slot] as usize % self.chr_bank_count_1k();
        bank * 0x0400 + (addr as usize & 0x03FF)
    }

    fn clock_irq_counter(&mut self) {
        self.debug_irq_clocks = self.debug_irq_clocks.wrapping_add(1);
        if self.irq_counter == 0 || self.irq_reload {
            self.irq_counter = self.irq_latch;
            self.irq_reload = false;
        } else {
            self.irq_counter = self.irq_counter.wrapping_sub(1);
        }

        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_pending = true;
        }
    }

    fn monitor_ppu_a12(&mut self, addr: u16) {
        // MMC3 IRQ counter clocks on filtered A12 rising edges.
        let a12 = (addr & 0x1000) != 0;
        if a12 {
            self.debug_a12_high_samples = self.debug_a12_high_samples.wrapping_add(1);
        }

        if !a12 {
            self.a12_low_cycles = self.a12_low_cycles.saturating_add(1);
        } else if !self.last_a12 && self.a12_low_cycles >= 8 {
            self.clock_irq_counter();
            self.a12_low_cycles = 0;
        } else if a12 {
            self.a12_low_cycles = 0;
        }

        self.last_a12 = a12;
    }
}

impl Mapper for Mapper4 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0xFFFF => {
                let prg_mode = (self.bank_select >> 6) & 0x01;
                let last = self.prg_bank_count_8k() - 1;
                let second_last = self.prg_bank_count_8k().saturating_sub(2);

                let offset = (addr as usize) & 0x1FFF;
                let bank = match addr {
                    0x8000..=0x9FFF => {
                        if prg_mode == 0 {
                            self.bank_regs[6] as usize
                        } else {
                            second_last
                        }
                    }
                    0xA000..=0xBFFF => self.bank_regs[7] as usize,
                    0xC000..=0xDFFF => {
                        if prg_mode == 0 {
                            second_last
                        } else {
                            self.bank_regs[6] as usize
                        }
                    }
                    _ => last,
                };

                self.read_prg_bank_8k(bank, offset)
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx] = value;
            }
            0x8000..=0x9FFF => {
                if (addr & 1) == 0 {
                    self.bank_select = value;
                } else {
                    let target = (self.bank_select & 0x07) as usize;
                    self.bank_regs[target] = if target <= 1 { value & 0xFE } else { value };
                }
            }
            0xA000..=0xBFFF => {
                if (addr & 1) == 0 && !self.four_screen {
                    self.mirroring = if (value & 1) == 0 {
                        Mirroring::Vertical
                    } else {
                        Mirroring::Horizontal
                    };
                }
            }
            0xC000..=0xDFFF => {
                if (addr & 1) == 0 {
                    self.irq_latch = value;
                } else {
                    self.irq_reload = true;
                }
            }
            0xE000..=0xFFFF => {
                if (addr & 1) == 0 {
                    self.irq_enabled = false;
                    self.irq_pending = false;
                } else {
                    self.irq_enabled = true;
                }
            }
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        let mapped = self.map_chr_addr(addr & 0x1FFF);
        self.chr[mapped % self.chr.len()]
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if self.chr_is_ram {
            let mapped = self.map_chr_addr(addr & 0x1FFF) % self.chr.len();
            self.chr[mapped] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        if self.four_screen {
            Mirroring::FourScreen
        } else {
            self.mirroring
        }
    }

    fn notify_ppu_read_addr(&mut self, addr: u16) {
        self.monitor_ppu_a12(addr);
    }

    fn notify_ppu_write_addr(&mut self, addr: u16) {
        self.monitor_ppu_a12(addr);
    }

    fn suppress_a12_on_sprite_eval_reads(&self) -> bool {
        true
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn clear_irq(&mut self) {
        self.irq_pending = false;
    }

    fn debug_state(&self) -> String {
        format!(
            "MMC3 bank_select=${:02X} prg=[{:02X},{:02X}] chr=[{:02X},{:02X},{:02X},{:02X},{:02X},{:02X}] irq_latch=${:02X} irq_counter=${:02X} reload={} en={} pending={} a12_low={} last_a12={} a12_high_samples={} irq_clocks={}",
            self.bank_select,
            self.bank_regs[6],
            self.bank_regs[7],
            self.bank_regs[0],
            self.bank_regs[1],
            self.bank_regs[2],
            self.bank_regs[3],
            self.bank_regs[4],
            self.bank_regs[5],
            self.irq_latch,
            self.irq_counter,
            self.irq_reload,
            self.irq_enabled,
            self.irq_pending,
            self.a12_low_cycles,
            self.last_a12,
            self.debug_a12_high_samples,
            self.debug_irq_clocks
        )
    }
}

struct Mapper24 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    mirroring: Mirroring,
    prg_banks: [u8; 4],
    chr_banks: [u8; 8],
    irq_enabled: bool,
    irq_counter: u16,
    irq_pending: bool,
    control: u8,
}

impl Mapper24 {
    fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            mirroring: cart.mirroring,
            prg_banks: [0, 1, 0xFE, 0xFF],
            chr_banks: [0; 8],
            irq_enabled: false,
            irq_counter: 0,
            irq_pending: false,
            control: 0xC0,
        }
    }

    fn prg_bank_count_8k(&self) -> usize {
        (self.prg_rom.len() / 0x2000).max(1)
    }

    fn chr_bank_count_1k(&self) -> usize {
        (self.chr.len() / 0x0400).max(1)
    }
}

impl Mapper for Mapper24 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0x9FFF => {
                let bank = self.prg_banks[0] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            0xA000..=0xBFFF => {
                let bank = self.prg_banks[1] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            0xC000..=0xDFFF => {
                let bank = self.prg_banks[2] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            0xE000..=0xFFFF => {
                let bank = self.prg_banks[3] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx] = value;
            }
            0x8000..=0x8FFF => {
                let reg = addr & 0x0F;
                match reg {
                    0x0 => self.prg_banks[0] = value & 0x0F,
                    0x2 => self.prg_banks[1] = value & 0x0F,
                    0x4 => self.prg_banks[2] = value & 0x0F,
                    0x6 => self.prg_banks[3] = value & 0x0F,
                    0x8 => {
                        self.control = value;
                        self.mirroring = if (value & 0x01) != 0 {
                            Mirroring::Vertical
                        } else {
                            Mirroring::Horizontal
                        };
                    }
                    0xA => {
                        self.irq_counter = (self.irq_counter & 0xFF00) | (value as u16);
                    }
                    0xE => self.irq_enabled = (value & 0x01) != 0,
                    _ => {}
                }
            }
            0x9000..=0x9FFF => {
                let reg = addr & 0x0F;
                match reg {
                    0x0 => self.chr_banks[0] = value,
                    0x2 => self.chr_banks[1] = value,
                    0x4 => self.chr_banks[2] = value,
                    0x6 => self.chr_banks[3] = value,
                    0x8 => self.chr_banks[4] = value,
                    0xA => self.chr_banks[5] = value,
                    0xC => self.chr_banks[6] = value,
                    0xE => self.chr_banks[7] = value,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        if addr < 0x2000 {
            let bank = (self.chr_banks[(addr >> 10) as usize] as usize) % self.chr_bank_count_1k();
            let idx = bank * 0x0400 + (addr as usize & 0x03FF);
            self.chr[idx % self.chr.len()]
        } else {
            0
        }
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if addr < 0x2000 && self.chr_is_ram {
            let chr_len = self.chr.len();
            let bank = (self.chr_banks[(addr >> 10) as usize] as usize) % self.chr_bank_count_1k();
            let idx = (bank * 0x0400 + (addr as usize & 0x03FF)) % chr_len;
            self.chr[idx] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn tick_cpu_cycle(&mut self) {
        if self.irq_enabled {
            if self.irq_counter == 0 {
                self.irq_counter = 0xFFFF;
                self.irq_pending = true;
            } else {
                self.irq_counter = self.irq_counter.wrapping_sub(1);
            }
        }
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn clear_irq(&mut self) {
        self.irq_pending = false;
    }
}

struct Mapper25 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    mirroring: Mirroring,
    prg_banks: [u8; 4],
    chr_banks: [u8; 8],
    irq_enabled: bool,
    irq_counter: u8,
    irq_pending: bool,
    control: u8,
}

impl Mapper25 {
    fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            mirroring: cart.mirroring,
            prg_banks: [0, 1, 0xFE, 0xFF],
            chr_banks: [0; 8],
            irq_enabled: false,
            irq_counter: 0,
            irq_pending: false,
            control: 0xC0,
        }
    }

    fn prg_bank_count_8k(&self) -> usize {
        (self.prg_rom.len() / 0x2000).max(1)
    }

    fn chr_bank_count_1k(&self) -> usize {
        (self.chr.len() / 0x0400).max(1)
    }
}

impl Mapper for Mapper25 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0x9FFF => {
                let bank = self.prg_banks[0] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            0xA000..=0xBFFF => {
                let bank = self.prg_banks[1] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            0xC000..=0xDFFF => {
                let bank = self.prg_banks[2] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            0xE000..=0xFFFF => {
                let bank = self.prg_banks[3] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx] = value;
            }
            0x8000..=0x8FFF => {
                let reg = addr & 0x0F;
                match reg {
                    0x0 => self.prg_banks[0] = value & 0x0F,
                    0x2 => self.prg_banks[1] = value & 0x0F,
                    0x4 => self.prg_banks[2] = value & 0x0F,
                    0x6 => self.prg_banks[3] = value & 0x0F,
                    0x8 => {
                        self.control = value;
                        self.mirroring = if (value & 0x01) != 0 {
                            Mirroring::Vertical
                        } else {
                            Mirroring::Horizontal
                        };
                    }
                    0xA => self.irq_counter = value,
                    0xE => self.irq_enabled = (value & 0x01) != 0,
                    _ => {}
                }
            }
            0x9000..=0x9FFF => {
                let reg = addr & 0x0F;
                match reg {
                    0x0 => self.chr_banks[0] = value,
                    0x2 => self.chr_banks[1] = value,
                    0x4 => self.chr_banks[2] = value,
                    0x6 => self.chr_banks[3] = value,
                    0x8 => self.chr_banks[4] = value,
                    0xA => self.chr_banks[5] = value,
                    0xC => self.chr_banks[6] = value,
                    0xE => self.chr_banks[7] = value,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        if addr < 0x2000 {
            let bank = (self.chr_banks[(addr >> 10) as usize] as usize) % self.chr_bank_count_1k();
            let idx = bank * 0x0400 + (addr as usize & 0x03FF);
            self.chr[idx % self.chr.len()]
        } else {
            0
        }
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if addr < 0x2000 && self.chr_is_ram {
            let chr_len = self.chr.len();
            let bank = (self.chr_banks[(addr >> 10) as usize] as usize) % self.chr_bank_count_1k();
            let idx = (bank * 0x0400 + (addr as usize & 0x03FF)) % chr_len;
            self.chr[idx] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn tick_cpu_cycle(&mut self) {
        if self.irq_enabled {
            if self.irq_counter == 0 {
                self.irq_counter = 0xFF;
                self.irq_pending = true;
            } else {
                self.irq_counter = self.irq_counter.wrapping_sub(1);
            }
        }
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn clear_irq(&mut self) {
        self.irq_pending = false;
    }
}

struct Mapper26 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    mirroring: Mirroring,
    prg_banks: [u8; 4],
    chr_banks: [u8; 8],
    irq_enabled: bool,
    irq_counter: u16,
    irq_pending: bool,
    control: u8,
}

impl Mapper26 {
    fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            mirroring: cart.mirroring,
            prg_banks: [0, 1, 0xFE, 0xFF],
            chr_banks: [0; 8],
            irq_enabled: false,
            irq_counter: 0,
            irq_pending: false,
            control: 0xC0,
        }
    }

    fn prg_bank_count_8k(&self) -> usize {
        (self.prg_rom.len() / 0x2000).max(1)
    }

    fn chr_bank_count_1k(&self) -> usize {
        (self.chr.len() / 0x0400).max(1)
    }
}

impl Mapper for Mapper26 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0x9FFF => {
                let bank = self.prg_banks[0] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            0xA000..=0xBFFF => {
                let bank = self.prg_banks[1] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            0xC000..=0xDFFF => {
                let bank = self.prg_banks[2] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            0xE000..=0xFFFF => {
                let bank = self.prg_banks[3] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx] = value;
            }
            0x8000..=0x8FFF => {
                let reg = addr & 0x0F;
                match reg {
                    0x0 => self.prg_banks[0] = value & 0x0F,
                    0x2 => self.prg_banks[1] = value & 0x0F,
                    0x4 => self.prg_banks[2] = value & 0x0F,
                    0x6 => self.prg_banks[3] = value & 0x0F,
                    0x8 => {
                        self.control = value;
                        self.mirroring = if (value & 0x01) != 0 {
                            Mirroring::Vertical
                        } else {
                            Mirroring::Horizontal
                        };
                    }
                    0xA => {
                        self.irq_counter = (self.irq_counter & 0xFF00) | (value as u16);
                    }
                    0xE => self.irq_enabled = (value & 0x01) != 0,
                    _ => {}
                }
            }
            0x9000..=0x9FFF => {
                let reg = addr & 0x0F;
                match reg {
                    0x0 => self.chr_banks[0] = value,
                    0x2 => self.chr_banks[1] = value,
                    0x4 => self.chr_banks[2] = value,
                    0x6 => self.chr_banks[3] = value,
                    0x8 => self.chr_banks[4] = value,
                    0xA => self.chr_banks[5] = value,
                    0xC => self.chr_banks[6] = value,
                    0xE => self.chr_banks[7] = value,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        if addr < 0x2000 {
            let bank = (self.chr_banks[(addr >> 10) as usize] as usize) % self.chr_bank_count_1k();
            let idx = bank * 0x0400 + (addr as usize & 0x03FF);
            self.chr[idx % self.chr.len()]
        } else {
            0
        }
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if addr < 0x2000 && self.chr_is_ram {
            let chr_len = self.chr.len();
            let bank = (self.chr_banks[(addr >> 10) as usize] as usize) % self.chr_bank_count_1k();
            let idx = (bank * 0x0400 + (addr as usize & 0x03FF)) % chr_len;
            self.chr[idx] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn tick_cpu_cycle(&mut self) {
        if self.irq_enabled {
            if self.irq_counter == 0 {
                self.irq_counter = 0xFFFF;
                self.irq_pending = true;
            } else {
                self.irq_counter = self.irq_counter.wrapping_sub(1);
            }
        }
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn clear_irq(&mut self) {
        self.irq_pending = false;
    }
}

struct Mapper85 {
    prg_rom: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    prg_ram: Vec<u8>,
    mirroring: Mirroring,
    prg_banks: [u8; 4],
    chr_banks: [u8; 8],
    irq_enabled: bool,
    irq_counter: u8,
    irq_pending: bool,
    control: u8,
}

impl Mapper85 {
    fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
            chr: cart.chr_data,
            chr_is_ram: cart.chr_is_ram,
            prg_ram: vec![0; cart.prg_ram_size.max(8 * 1024)],
            mirroring: cart.mirroring,
            prg_banks: [0, 1, 0xFE, 0xFF],
            chr_banks: [0; 8],
            irq_enabled: false,
            irq_counter: 0,
            irq_pending: false,
            control: 0xC0,
        }
    }

    fn prg_bank_count_8k(&self) -> usize {
        (self.prg_rom.len() / 0x2000).max(1)
    }

    fn chr_bank_count_1k(&self) -> usize {
        (self.chr.len() / 0x0400).max(1)
    }
}

impl Mapper for Mapper85 {
    fn cpu_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx]
            }
            0x8000..=0x9FFF => {
                let bank = self.prg_banks[0] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            0xA000..=0xBFFF => {
                let bank = self.prg_banks[1] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            0xC000..=0xDFFF => {
                let bank = self.prg_banks[2] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            0xE000..=0xFFFF => {
                let bank = self.prg_banks[3] as usize % self.prg_bank_count_8k();
                let idx = bank * 0x2000 + (addr as usize & 0x1FFF);
                self.prg_rom[idx % self.prg_rom.len()]
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                let idx = (addr as usize - 0x6000) % self.prg_ram.len();
                self.prg_ram[idx] = value;
            }
            0x8000..=0x8FFF => {
                let reg = addr & 0x0F;
                match reg {
                    0x0 => self.prg_banks[0] = value & 0x0F,
                    0x2 => self.prg_banks[1] = value & 0x0F,
                    0x4 => self.prg_banks[2] = value & 0x0F,
                    0x6 => self.prg_banks[3] = value & 0x0F,
                    0x8 => {
                        self.control = value;
                        self.mirroring = if (value & 0x01) != 0 {
                            Mirroring::Vertical
                        } else {
                            Mirroring::Horizontal
                        };
                    }
                    0xA => self.irq_counter = value,
                    0xE => self.irq_enabled = (value & 0x01) != 0,
                    _ => {}
                }
            }
            0x9000..=0x9FFF => {
                let reg = addr & 0x0F;
                match reg {
                    0x0 => self.chr_banks[0] = value,
                    0x2 => self.chr_banks[1] = value,
                    0x4 => self.chr_banks[2] = value,
                    0x6 => self.chr_banks[3] = value,
                    0x8 => self.chr_banks[4] = value,
                    0xA => self.chr_banks[5] = value,
                    0xC => self.chr_banks[6] = value,
                    0xE => self.chr_banks[7] = value,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn ppu_read(&mut self, addr: u16) -> u8 {
        if addr < 0x2000 {
            let bank = (self.chr_banks[(addr >> 10) as usize] as usize) % self.chr_bank_count_1k();
            let idx = bank * 0x0400 + (addr as usize & 0x03FF);
            self.chr[idx % self.chr.len()]
        } else {
            0
        }
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        if addr < 0x2000 && self.chr_is_ram {
            let chr_len = self.chr.len();
            let bank = (self.chr_banks[(addr >> 10) as usize] as usize) % self.chr_bank_count_1k();
            let idx = (bank * 0x0400 + (addr as usize & 0x03FF)) % chr_len;
            self.chr[idx] = value;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn tick_cpu_cycle(&mut self) {
        if self.irq_enabled {
            if self.irq_counter == 0 {
                self.irq_counter = 0xFF;
                self.irq_pending = true;
            } else {
                self.irq_counter = self.irq_counter.wrapping_sub(1);
            }
        }
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn clear_irq(&mut self) {
        self.irq_pending = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nes::ppu::Ppu;

    fn patterned_banks(total_size: usize, bank_size: usize) -> Vec<u8> {
        let mut data = vec![0u8; total_size];
        for (bank, chunk) in data.chunks_mut(bank_size).enumerate() {
            chunk.fill((bank as u8).wrapping_add(1));
        }
        data
    }

    fn make_cart(
        mapper_id: u16,
        submapper_id: u8,
        prg_rom: Vec<u8>,
        chr_data: Vec<u8>,
        chr_is_ram: bool,
    ) -> Cartridge {
        Cartridge {
            mapper_id,
            submapper_id,
            mirroring: Mirroring::Horizontal,
            four_screen: false,
            has_battery_backed_ram: false,
            prg_rom,
            chr_data,
            chr_is_ram,
            prg_ram_size: 8 * 1024,
        }
    }

    fn run_mmc3_irq_approximation_cycle(ctrl: u8) -> u64 {
        let prg = patterned_banks(4 * 0x2000, 0x2000);
        let chr = patterned_banks(8 * 0x0400, 0x0400);
        let mut mapper = Mapper4::new(make_cart(4, 0, prg, chr, false));
        let mut ppu = Ppu::new();

        ppu.cpu_write_register(0x2000, ctrl, &mut mapper);
        ppu.cpu_write_register(0x2001, 0x18, &mut mapper);

        for _ in 0..700 {
            ppu.tick(&mut mapper);
        }

        mapper.debug_irq_clocks
    }

    #[test]
    fn mapper2_keeps_last_bank_fixed() {
        let prg = patterned_banks(3 * 0x4000, 0x4000);
        let chr = vec![0; 0x2000];
        let mut mapper = Mapper2::new(make_cart(2, 0, prg, chr, false));

        mapper.cpu_write(0x8000, 1);
        assert_eq!(mapper.cpu_read(0x8000), 2);
        assert_eq!(mapper.cpu_read(0xC000), 3);
    }

    #[test]
    fn mapper3_switches_chr_bank() {
        let prg = patterned_banks(0x8000, 0x4000);
        let chr = patterned_banks(2 * 0x2000, 0x2000);
        let mut mapper = Mapper3::new(make_cart(3, 0, prg, chr, false));

        mapper.cpu_write(0x8000, 1);
        assert_eq!(mapper.ppu_read(0x0000), 2);
    }

    #[test]
    fn mapper3_allows_chr_ram_writes_when_present() {
        let prg = patterned_banks(0x8000, 0x4000);
        let chr = vec![0; 2 * 0x2000];
        let mut mapper = Mapper3::new(make_cart(3, 0, prg, chr, true));

        mapper.cpu_write(0x8000, 1);
        mapper.ppu_write(0x0010, 0xAB);
        assert_eq!(mapper.ppu_read(0x0010), 0xAB);
    }

    #[test]
    fn mapper4_irq_a12_edge_filtering() {
        let prg = patterned_banks(4 * 0x2000, 0x2000);
        let chr = patterned_banks(8 * 0x0400, 0x0400);
        let mut mapper = Mapper4::new(make_cart(4, 0, prg, chr, false));

        mapper.cpu_write(0xC000, 0x01);
        mapper.cpu_write(0xC001, 0x00);
        mapper.cpu_write(0xE001, 0x00);

        for _ in 0..8 {
            mapper.notify_ppu_read_addr(0x0000);
        }
        mapper.notify_ppu_read_addr(0x1000);
        assert!(!mapper.irq_pending());

        for _ in 0..8 {
            mapper.notify_ppu_read_addr(0x0000);
        }
        mapper.notify_ppu_read_addr(0x1000);
        assert!(mapper.irq_pending());
    }

    #[test]
    fn mapper4_ppu_irq_approximation_supports_both_table_polarities() {
        // BG=$0000, sprites=$1000.
        let sprite_high_clocks = run_mmc3_irq_approximation_cycle(0x08);
        assert!(sprite_high_clocks > 0);

        // BG=$1000, sprites=$0000.
        let bg_high_clocks = run_mmc3_irq_approximation_cycle(0x10);
        assert!(bg_high_clocks > 0);
    }

    #[test]
    fn mapper5_prg_banking_and_ram_protection() {
        let prg = patterned_banks(16 * 0x2000, 0x2000);
        let chr = patterned_banks(8 * 0x0400, 0x0400);
        let mut mapper = Mapper5::new(make_cart(5, 0, prg, chr, false));

        mapper.cpu_write(0x5100, 0x03);
        mapper.cpu_write(0x5114, 0x82);
        mapper.cpu_write(0x5115, 0x83);
        mapper.cpu_write(0x5116, 0x84);
        mapper.cpu_write(0x5117, 0x8F);

        assert_eq!(mapper.cpu_read(0x8000), 3);
        assert_eq!(mapper.cpu_read(0xA000), 4);
        assert_eq!(mapper.cpu_read(0xC000), 5);
        assert_eq!(mapper.cpu_read(0xE000), 16);

        mapper.cpu_write(0x5102, 0x02);
        mapper.cpu_write(0x5103, 0x01);
        mapper.cpu_write(0x5113, 0x00);
        mapper.cpu_write(0x6000, 0xAA);
        assert_eq!(mapper.cpu_read(0x6000), 0xAA);

        mapper.cpu_write(0x5102, 0x00);
        mapper.cpu_write(0x6000, 0x55);
        assert_eq!(mapper.cpu_read(0x6000), 0xAA);
    }

    #[test]
    fn mapper5_nametable_modes_and_scanline_irq() {
        let prg = patterned_banks(8 * 0x2000, 0x2000);
        let chr = patterned_banks(8 * 0x0400, 0x0400);
        let mut mapper = Mapper5::new(make_cart(5, 0, prg, chr, false));
        let mut vram = [0u8; 4096];

        mapper.cpu_write(0x5105, 0b11_10_01_00);
        mapper.cpu_write(0x5104, 0x00);
        mapper.cpu_write(0x5106, 0x2A);
        mapper.cpu_write(0x5107, 0x03);
        mapper.cpu_write(0x5C12, 0x77);

        vram[0x0005] = 0x11;
        vram[0x0406] = 0x22;
        assert_eq!(mapper.ppu_nametable_read(0x2005, &vram), Some(0x11));
        assert_eq!(mapper.ppu_nametable_read(0x2406, &vram), Some(0x22));
        assert_eq!(mapper.ppu_nametable_read(0x2812, &vram), Some(0x77));
        assert_eq!(mapper.ppu_nametable_read(0x2C00, &vram), Some(0x2A));
        assert_eq!(mapper.ppu_nametable_read(0x2FC0, &vram), Some(0xFF));

        mapper.cpu_write(0x5104, 0x02);
        assert_eq!(mapper.ppu_nametable_read(0x2812, &vram), Some(0x00));

        mapper.cpu_write(0x5203, 0x01);
        mapper.cpu_write(0x5204, 0x80);
        for _ in 0..3 {
            mapper.notify_ppu_read_addr(0x2000);
        }
        mapper.notify_ppu_read_addr(0x23C0);
        assert!(!mapper.irq_pending());

        for _ in 0..3 {
            mapper.notify_ppu_read_addr(0x2000);
        }
        mapper.notify_ppu_read_addr(0x23C0);
        assert!(mapper.irq_pending());

        let status = mapper.cpu_read(0x5204);
        assert_eq!(status & 0x80, 0x80);
        assert!(!mapper.irq_pending());
    }

    #[test]
    fn mapper7_switches_prg_and_onescreen_mirroring() {
        let prg = patterned_banks(2 * 0x8000, 0x8000);
        let chr = patterned_banks(0x2000, 0x2000);
        let mut mapper = Mapper7::new(make_cart(7, 0, prg, chr, false));

        mapper.cpu_write(0x8000, 0x11);
        assert_eq!(mapper.cpu_read(0x8000), 2);
        assert_eq!(mapper.mirroring(), Mirroring::OneScreenUpper);
    }

    #[test]
    fn mapper9_latches_control_chr_windows() {
        let prg = patterned_banks(4 * 0x2000, 0x2000);
        let chr = patterned_banks(8 * 0x1000, 0x1000);
        let mut mapper = Mapper9::new(make_cart(9, 0, prg, chr, false));

        mapper.cpu_write(0xB000, 0x01);
        mapper.cpu_write(0xC000, 0x02);
        mapper.cpu_write(0xD000, 0x03);
        mapper.cpu_write(0xE000, 0x04);

        assert_eq!(mapper.ppu_read(0x0000), 3);
        mapper.notify_ppu_read_addr(0x0FD8);
        assert_eq!(mapper.ppu_read(0x0000), 2);

        assert_eq!(mapper.ppu_read(0x1000), 5);
        mapper.notify_ppu_read_addr(0x1FD8);
        assert_eq!(mapper.ppu_read(0x1000), 4);
    }

    #[test]
    fn mapper10_switches_prg_and_chr_latches() {
        let prg = patterned_banks(3 * 0x4000, 0x4000);
        let chr = patterned_banks(8 * 0x1000, 0x1000);
        let mut mapper = Mapper10::new(make_cart(10, 0, prg, chr, false));

        mapper.cpu_write(0xA000, 0x01);
        assert_eq!(mapper.cpu_read(0x8000), 2);
        assert_eq!(mapper.cpu_read(0xC000), 3);

        mapper.cpu_write(0xB000, 0x00);
        mapper.cpu_write(0xC000, 0x01);
        assert_eq!(mapper.ppu_read(0x0000), 2);
        mapper.notify_ppu_read_addr(0x0FD8);
        assert_eq!(mapper.ppu_read(0x0000), 1);
    }

    #[test]
    fn mapper19_nametable_chr_mapping_and_irq_counter() {
        let prg = patterned_banks(8 * 0x2000, 0x2000);
        let chr = patterned_banks(16 * 0x0400, 0x0400);
        let mut mapper = Mapper19::new(make_cart(19, 0, prg, chr, false));
        let mut vram = [0u8; 4096];

        mapper.cpu_write(0xC000, 0xE0);
        vram[0x0010] = 0x33;
        assert_eq!(mapper.ppu_nametable_read(0x2010, &vram), Some(0x33));
        assert!(mapper.ppu_nametable_write(0x2020, 0x44, &mut vram));
        assert_eq!(vram[0x0020], 0x44);

        mapper.cpu_write(0xC000, 0x02);
        assert_eq!(mapper.ppu_nametable_read(0x2010, &vram), Some(3));

        mapper.cpu_write(0x5000, 0xFE);
        mapper.cpu_write(0x5800, 0xFF);
        mapper.tick_cpu_cycle();
        assert!(mapper.irq_pending());
        mapper.cpu_write(0x5000, 0x00);
        assert!(!mapper.irq_pending());

        mapper.cpu_write(0xF800, 0x40);
        mapper.cpu_write(0x6000, 0xA5);
        assert_eq!(mapper.cpu_read(0x6000), 0xA5);
        mapper.cpu_write(0xF800, 0x4F);
        mapper.cpu_write(0x6000, 0x5A);
        assert_eq!(mapper.cpu_read(0x6000), 0xA5);
    }

    #[test]
    fn mapper66_switches_prg_and_chr() {
        let prg = patterned_banks(2 * 0x8000, 0x8000);
        let chr = patterned_banks(2 * 0x2000, 0x2000);
        let mut mapper = Mapper66::new(make_cart(66, 0, prg, chr, false));

        mapper.cpu_write(0x8000, 0x11);
        assert_eq!(mapper.cpu_read(0x8000), 2);
        assert_eq!(mapper.ppu_read(0x0000), 2);
    }

    #[test]
    fn mapper69_prg_registers_and_irq_counter() {
        let prg = patterned_banks(8 * 0x2000, 0x2000);
        let chr = patterned_banks(8 * 0x0400, 0x0400);
        let mut mapper = Mapper69::new(make_cart(69, 0, prg, chr, false));

        mapper.cpu_write(0x8000, 0x09);
        mapper.cpu_write(0xA000, 0x03);
        mapper.cpu_write(0x8000, 0x0A);
        mapper.cpu_write(0xA000, 0x04);
        mapper.cpu_write(0x8000, 0x0B);
        mapper.cpu_write(0xA000, 0x05);
        assert_eq!(mapper.cpu_read(0x8000), 4);
        assert_eq!(mapper.cpu_read(0xA000), 5);
        assert_eq!(mapper.cpu_read(0xC000), 6);

        mapper.cpu_write(0x8000, 0x0E);
        mapper.cpu_write(0xA000, 0x00);
        mapper.cpu_write(0x8000, 0x0F);
        mapper.cpu_write(0xA000, 0x00);
        mapper.cpu_write(0x8000, 0x0D);
        mapper.cpu_write(0xA000, 0x81);
        mapper.tick_cpu_cycle();
        assert!(mapper.irq_pending());
        mapper.cpu_write(0x8000, 0x0D);
        mapper.cpu_write(0xA000, 0x81);
        assert!(!mapper.irq_pending());
        mapper.cpu_write(0x8000, 0x0F);
        mapper.cpu_write(0xA000, 0x00);
        mapper.cpu_write(0x8000, 0x0E);
        mapper.cpu_write(0xA000, 0x01);
        mapper.tick_cpu_cycle();
        assert!(!mapper.irq_pending());
        mapper.tick_cpu_cycle();
        assert!(mapper.irq_pending());
        mapper.clear_irq();
        assert!(!mapper.irq_pending());
    }

    #[test]
    fn mapper71_submapper1_masks_bank_to_three_bits() {
        let prg = patterned_banks(16 * 0x4000, 0x4000);
        let chr = patterned_banks(0x2000, 0x2000);
        let mut mapper = Mapper71::new(make_cart(71, 1, prg, chr, false));

        mapper.cpu_write(0xC000, 0x0F);
        assert_eq!(mapper.cpu_read(0x8000), 8);
        mapper.cpu_write(0x9000, 0x10);
        assert_eq!(mapper.mirroring(), Mirroring::OneScreenUpper);
    }

    #[test]
    fn mapper71_submapper0_ignores_mirroring_writes_and_allows_chr_writes() {
        let prg = patterned_banks(4 * 0x4000, 0x4000);
        let chr = vec![0; 0x2000];
        let mut mapper = Mapper71::new(make_cart(71, 0, prg, chr, false));

        assert_eq!(mapper.mirroring(), Mirroring::Horizontal);
        mapper.cpu_write(0x9000, 0x10);
        assert_eq!(mapper.mirroring(), Mirroring::Horizontal);

        mapper.ppu_write(0x0010, 0xA5);
        assert_eq!(mapper.ppu_read(0x0010), 0xA5);
    }

    #[test]
    fn mapper1_shift_register_programs_prg_bank() {
        let prg = patterned_banks(4 * 0x4000, 0x4000);
        let chr = patterned_banks(0x2000, 0x1000);
        let mut mapper = Mapper1::new(make_cart(1, 0, prg, chr, false));

        for bit in [1u8, 0, 0, 0, 0] {
            mapper.cpu_write(0xE000, bit);
        }

        assert_eq!(mapper.cpu_read(0x8000), 2);
        assert_eq!(mapper.cpu_read(0xC000), 4);
    }
}
