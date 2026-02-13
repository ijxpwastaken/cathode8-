use anyhow::{Context, Result, anyhow, bail};
use std::{fs, path::Path};

use super::mapper::Mirroring;

#[derive(Debug, Clone)]
pub struct Cartridge {
    pub mapper_id: u16,
    pub submapper_id: u8,
    pub mirroring: Mirroring,
    pub four_screen: bool,
    pub has_battery_backed_ram: bool,
    pub prg_rom: Vec<u8>,
    pub chr_data: Vec<u8>,
    pub chr_is_ram: bool,
    pub prg_ram_size: usize,
}

impl Cartridge {
    pub fn from_file(path: &Path) -> Result<Self> {
        let bytes =
            fs::read(path).with_context(|| format!("failed to read ROM: {}", path.display()))?;
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 16 {
            bail!("ROM is too small to contain an iNES header");
        }
        if &bytes[0..4] != b"NES\x1A" {
            bail!("invalid iNES header magic, expected NES<EOF>");
        }

        let flags6 = bytes[6];
        let flags7 = bytes[7];
        let is_nes2 = (flags7 & 0x0C) == 0x08;

        let mapper_id_low = ((flags6 as u16) >> 4) | ((flags7 as u16) & 0xF0);
        let mapper_id = if is_nes2 {
            mapper_id_low | (((bytes[8] as u16) & 0x0F) << 8)
        } else {
            mapper_id_low
        };
        let submapper_id = if is_nes2 { bytes[8] >> 4 } else { 0 };
        let four_screen = (flags6 & 0x08) != 0;
        let mirroring = if four_screen {
            Mirroring::FourScreen
        } else if (flags6 & 0x01) != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };

        let trainer_present = (flags6 & 0x04) != 0;
        let has_battery_backed_ram = (flags6 & 0x02) != 0;

        let (prg_rom_size, chr_rom_size, prg_ram_size) = if is_nes2 {
            let prg_msb = (bytes[9] & 0x0F) as usize;
            let chr_msb = (bytes[9] >> 4) as usize;
            if prg_msb == 0x0F || chr_msb == 0x0F {
                bail!("NES 2.0 exponent/multiplier ROM size encoding is not supported in v1");
            }

            let prg_units = ((prg_msb << 8) | bytes[4] as usize).max(1);
            let chr_units = (chr_msb << 8) | bytes[5] as usize;

            let prg_shift = bytes[10] & 0x0F;
            let prg_ram = if prg_shift == 0 {
                8 * 1024
            } else {
                64usize << prg_shift
            };

            (prg_units * 16 * 1024, chr_units * 8 * 1024, prg_ram)
        } else {
            let prg_units = (bytes[4] as usize).max(1);
            let chr_units = bytes[5] as usize;
            let prg_ram_units = if bytes[8] == 0 { 1 } else { bytes[8] as usize };
            (
                prg_units * 16 * 1024,
                chr_units * 8 * 1024,
                prg_ram_units * 8 * 1024,
            )
        };

        let mut cursor = 16usize;
        if trainer_present {
            cursor += 512;
        }

        if bytes.len() < cursor + prg_rom_size {
            bail!(
                "ROM truncated: expected {} PRG bytes but file ended early",
                prg_rom_size
            );
        }

        let prg_rom_end = cursor + prg_rom_size;
        let prg_rom = bytes[cursor..prg_rom_end].to_vec();
        cursor = prg_rom_end;

        let (chr_data, chr_is_ram) = if chr_rom_size == 0 {
            (vec![0; 8 * 1024], true)
        } else {
            if bytes.len() < cursor + chr_rom_size {
                bail!(
                    "ROM truncated: expected {} CHR bytes but file ended early",
                    chr_rom_size
                );
            }
            (bytes[cursor..cursor + chr_rom_size].to_vec(), false)
        };

        if prg_rom.is_empty() {
            return Err(anyhow!("invalid PRG ROM: empty payload"));
        }

        Ok(Self {
            mapper_id,
            submapper_id,
            mirroring,
            four_screen,
            has_battery_backed_ram,
            prg_rom,
            chr_data,
            chr_is_ram,
            prg_ram_size,
        })
    }
}
