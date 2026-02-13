use super::mapper::{Mapper, Mirroring};
use super::palette::NES_PALETTE;

pub const FRAME_WIDTH: usize = 256;
pub const FRAME_HEIGHT: usize = 240;

const CTRL_NMI_ENABLE: u8 = 0x80;
const CTRL_VRAM_INC_32: u8 = 0x04;
const CTRL_SPRITE_TABLE: u8 = 0x08;
const CTRL_BG_TABLE: u8 = 0x10;
const CTRL_SPRITE_SIZE_16: u8 = 0x20;

const MASK_SHOW_BG_LEFT: u8 = 0x02;
const MASK_SHOW_SPRITE_LEFT: u8 = 0x04;
const MASK_SHOW_BG: u8 = 0x08;
const MASK_SHOW_SPRITES: u8 = 0x10;

const STATUS_SPRITE_OVERFLOW: u8 = 0x20;
const STATUS_SPRITE_ZERO_HIT: u8 = 0x40;
const STATUS_VBLANK: u8 = 0x80;
const NMI_DELAY_CYCLES: u8 = 0;

#[derive(Debug, Clone, Copy, Default)]
pub struct PpuDebugCounters {
    pub ticks: u64,
    pub vblank_entries: u64,
    pub nmi_edges: u64,
    pub nmi_fired: u64,
    pub sprite_overflow_events: u64,
    pub sprite_overflow_last_scanline: i16,
    pub sprite_overflow_last_cycle: i16,
    pub sprite0_hit_events: u64,
    pub sprite0_hit_last_scanline: i16,
    pub sprite0_hit_last_cycle: i16,
    pub sprite0_nonzero_events: u64,
    pub sprite0_nonzero_last_scanline: i16,
    pub sprite0_nonzero_last_cycle: i16,
    pub sprite0_nonzero_last_bg_pixel: u8,
    pub sprite0_nonzero_last_bg_opaque: bool,
    pub pattern_reads: u64,
    pub nametable_reads: u64,
    pub palette_reads: u64,
    pub pattern_writes: u64,
    pub nametable_writes: u64,
    pub palette_writes: u64,
    pub status_reads: u64,
    pub status_read_last_scanline: i16,
    pub status_read_last_cycle: i16,
    pub status_overflow_reads: u64,
    pub status_overflow_last_scanline: i16,
    pub status_overflow_last_cycle: i16,
    pub scroll_writes_2005: u64,
    pub scroll_write_2005_last_scanline: i16,
    pub scroll_write_2005_last_cycle: i16,
    pub scroll_write_2005_last_value: u8,
    pub scroll_write_2005_last_phase_second: bool,
    pub scroll_write_2005_prev_scanline: i16,
    pub scroll_write_2005_prev_cycle: i16,
    pub scroll_write_2005_prev_value: u8,
    pub scroll_write_2005_prev_phase_second: bool,
    pub addr_writes_2006: u64,
    pub addr_write_2006_last_scanline: i16,
    pub addr_write_2006_last_cycle: i16,
    pub addr_write_2006_last_value: u8,
    pub addr_write_2006_last_phase_second: bool,
    pub addr_write_2006_prev_scanline: i16,
    pub addr_write_2006_prev_cycle: i16,
    pub addr_write_2006_prev_value: u8,
    pub addr_write_2006_prev_phase_second: bool,
    pub last_read_addr: u16,
    pub last_write_addr: u16,
}

pub struct Ppu {
    ctrl: u8,
    mask: u8,
    status: u8,

    oam_addr: u8,
    oam: [u8; 256],

    vram: [u8; 4096],
    palette_ram: [u8; 32],

    write_toggle: bool,
    v: u16,
    t: u16,
    fine_x: u8,
    read_buffer: u8,
    open_bus: u8,
    ppuaddr_reload_pending: bool,
    ppuaddr_reload_delay: u8,

    scanline: i16,
    cycle: i16,
    odd_frame: bool,
    frame_complete: bool,
    nmi_pending: bool,
    vblank_suppress: bool,
    nmi_line: bool,
    nmi_delay: u8,

    debug_mask_write_count: u64,
    debug_last_mask_value: u8,

    next_tile_id: u8,
    next_tile_attr: u8,
    next_tile_lsb: u8,
    next_tile_msb: u8,
    bg_shift_pattern_lo: u16,
    bg_shift_pattern_hi: u16,
    bg_shift_attr_lo: u16,
    bg_shift_attr_hi: u16,

    sprite_count: usize,
    sprite_patterns_lo: [u8; 8],
    sprite_patterns_hi: [u8; 8],
    sprite_x: [u8; 8],
    sprite_attributes: [u8; 8],
    sprite_indices: [u8; 8],

    sprite_eval_active: bool,
    sprite_eval_n: u8,
    sprite_eval_m: u8,
    sprite_eval_found: u8,
    sprite_eval_copy_remaining: u8,
    sprite_eval_bug_mode: bool,
    sprite_eval_target_scanline: i16,
    sprite0_prev_bg_opaque: bool,
    allow_relaxed_sprite0_hit: bool,

    frame_buffer: [u8; FRAME_WIDTH * FRAME_HEIGHT * 4],
    debug: PpuDebugCounters,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            ctrl: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            oam: [0; 256],
            vram: [0; 4096],
            palette_ram: [0x0F; 32],
            write_toggle: false,
            v: 0,
            t: 0,
            fine_x: 0,
            read_buffer: 0,
            open_bus: 0,
            ppuaddr_reload_pending: false,
            ppuaddr_reload_delay: 0,
            scanline: 261,
            cycle: 0,
            odd_frame: false,
            frame_complete: false,
            nmi_pending: false,
            vblank_suppress: false,
            nmi_line: false,
            nmi_delay: 0,
            debug_mask_write_count: 0,
            debug_last_mask_value: 0,
            next_tile_id: 0,
            next_tile_attr: 0,
            next_tile_lsb: 0,
            next_tile_msb: 0,
            bg_shift_pattern_lo: 0,
            bg_shift_pattern_hi: 0,
            bg_shift_attr_lo: 0,
            bg_shift_attr_hi: 0,
            sprite_count: 0,
            sprite_patterns_lo: [0; 8],
            sprite_patterns_hi: [0; 8],
            sprite_x: [0; 8],
            sprite_attributes: [0; 8],
            sprite_indices: [0; 8],
            sprite_eval_active: false,
            sprite_eval_n: 0,
            sprite_eval_m: 0,
            sprite_eval_found: 0,
            sprite_eval_copy_remaining: 0,
            sprite_eval_bug_mode: false,
            sprite_eval_target_scanline: 0,
            sprite0_prev_bg_opaque: false,
            allow_relaxed_sprite0_hit: false,
            frame_buffer: [0; FRAME_WIDTH * FRAME_HEIGHT * 4],
            debug: PpuDebugCounters::default(),
        }
    }

    pub fn reset(&mut self) {
        self.ctrl = 0;
        self.mask = 0;
        self.status = 0;
        self.oam_addr = 0;
        self.write_toggle = false;
        self.v = 0;
        self.t = 0;
        self.fine_x = 0;
        self.read_buffer = 0;
        self.open_bus = 0;
        self.ppuaddr_reload_pending = false;
        self.ppuaddr_reload_delay = 0;
        self.scanline = 261;
        self.cycle = 0;
        self.odd_frame = false;
        self.frame_complete = false;
        self.nmi_pending = false;
        self.vblank_suppress = false;
        self.nmi_line = false;
        self.nmi_delay = 0;
        self.debug_mask_write_count = 0;
        self.debug_last_mask_value = self.mask;

        self.next_tile_id = 0;
        self.next_tile_attr = 0;
        self.next_tile_lsb = 0;
        self.next_tile_msb = 0;
        self.bg_shift_pattern_lo = 0;
        self.bg_shift_pattern_hi = 0;
        self.bg_shift_attr_lo = 0;
        self.bg_shift_attr_hi = 0;

        self.sprite_count = 0;
        self.sprite_patterns_lo = [0; 8];
        self.sprite_patterns_hi = [0; 8];
        self.sprite_x = [0; 8];
        self.sprite_attributes = [0; 8];
        self.sprite_indices = [0; 8];
        self.sprite_eval_active = false;
        self.sprite_eval_n = 0;
        self.sprite_eval_m = 0;
        self.sprite_eval_found = 0;
        self.sprite_eval_copy_remaining = 0;
        self.sprite_eval_bug_mode = false;
        self.sprite_eval_target_scanline = 0;
        self.sprite0_prev_bg_opaque = false;
        self.allow_relaxed_sprite0_hit = false;
        self.debug = PpuDebugCounters::default();

        // Keep startup background black for deterministic test behavior.
        self.palette_ram = [0x0F; 32];
    }

    pub fn frame_buffer(&self) -> &[u8] {
        &self.frame_buffer
    }

    pub fn debug_ctrl(&self) -> u8 {
        self.ctrl
    }

    pub fn debug_mask(&self) -> u8 {
        self.mask
    }

    pub fn debug_status(&self) -> u8 {
        self.status
    }

    pub fn debug_scanline_cycle(&self) -> (i16, i16) {
        (self.scanline, self.cycle)
    }

    pub fn debug_mask_write_stats(&self) -> (u64, u8) {
        (self.debug_mask_write_count, self.debug_last_mask_value)
    }

    pub fn debug_peek_vram(&self, index: usize) -> u8 {
        self.vram[index % self.vram.len()]
    }

    pub fn debug_peek_palette(&self, index: usize) -> u8 {
        self.palette_ram[index % self.palette_ram.len()]
    }

    pub fn debug_peek_oam(&self, index: usize) -> u8 {
        self.oam[index % self.oam.len()]
    }

    pub fn debug_counters(&self) -> PpuDebugCounters {
        self.debug
    }

    pub fn zapper_light_sensed(&self, x: i16, y: i16) -> bool {
        if x < 0 || y < 0 || x >= FRAME_WIDTH as i16 || y >= FRAME_HEIGHT as i16 {
            return false;
        }

        let mut max_luma: u16 = 0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let sx = (x + dx).clamp(0, FRAME_WIDTH as i16 - 1) as usize;
                let sy = (y + dy).clamp(0, FRAME_HEIGHT as i16 - 1) as usize;
                let idx = (sy * FRAME_WIDTH + sx) * 4;
                let r = self.frame_buffer[idx] as u16;
                let g = self.frame_buffer[idx + 1] as u16;
                let b = self.frame_buffer[idx + 2] as u16;
                let luma = r + g + b;
                if luma > max_luma {
                    max_luma = luma;
                }
            }
        }

        max_luma >= 620
    }

    pub fn clear_frame_complete(&mut self) {
        self.frame_complete = false;
    }

    pub fn frame_complete(&self) -> bool {
        self.frame_complete
    }

    pub fn take_nmi(&mut self) -> bool {
        let pending = self.nmi_pending;
        self.nmi_pending = false;
        pending
    }

    pub fn cpu_read_register(&mut self, addr: u16, mapper: &mut dyn Mapper) -> u8 {
        let value = match addr {
            0x2002 => {
                self.debug.status_reads = self.debug.status_reads.wrapping_add(1);
                self.debug.status_read_last_scanline = self.scanline;
                self.debug.status_read_last_cycle = self.cycle;
                if (self.status & STATUS_SPRITE_OVERFLOW) != 0 {
                    self.debug.status_overflow_reads =
                        self.debug.status_overflow_reads.wrapping_add(1);
                    self.debug.status_overflow_last_scanline = self.scanline;
                    self.debug.status_overflow_last_cycle = self.cycle;
                }

                // Reading $2002 around VBL start suppresses VBL/NMI for this frame.
                if self.scanline == 241 && self.cycle == 0 {
                    self.vblank_suppress = true;
                    self.nmi_delay = 0;
                    self.nmi_pending = false;
                }

                let value = (self.status & 0xE0) | (self.open_bus & 0x1F);
                self.status &= !STATUS_VBLANK;
                self.write_toggle = false;
                self.update_nmi_line();
                value
            }
            0x2004 => self.oam[self.oam_addr as usize],
            0x2007 => {
                let ppu_addr = self.v & 0x3FFF;
                let value = self.ppu_read(ppu_addr, mapper);
                let result = if ppu_addr >= 0x3F00 {
                    self.read_buffer = self.ppu_read((ppu_addr - 0x1000) & 0x3FFF, mapper);
                    value
                } else {
                    let buffered = self.read_buffer;
                    self.read_buffer = value;
                    buffered
                };

                self.increment_vram_addr_cpu_access();
                result
            }
            _ => self.open_bus,
        };
        self.open_bus = value;
        value
    }

    pub fn cpu_write_register(&mut self, addr: u16, value: u8, mapper: &mut dyn Mapper) {
        self.open_bus = value;
        match addr {
            0x2000 => {
                self.ctrl = value;
                self.t = (self.t & !0x0C00) | (((value as u16) & 0x03) << 10);
                self.update_nmi_line();
            }
            0x2001 => {
                self.mask = value;
                self.debug_mask_write_count = self.debug_mask_write_count.wrapping_add(1);
                self.debug_last_mask_value = value;
            }
            0x2003 => {
                self.oam_addr = value;
            }
            0x2004 => {
                self.oam[self.oam_addr as usize] = value;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            0x2005 => {
                let second_phase = self.write_toggle;
                self.debug.scroll_writes_2005 = self.debug.scroll_writes_2005.wrapping_add(1);
                self.debug.scroll_write_2005_prev_scanline =
                    self.debug.scroll_write_2005_last_scanline;
                self.debug.scroll_write_2005_prev_cycle = self.debug.scroll_write_2005_last_cycle;
                self.debug.scroll_write_2005_prev_value = self.debug.scroll_write_2005_last_value;
                self.debug.scroll_write_2005_prev_phase_second =
                    self.debug.scroll_write_2005_last_phase_second;
                self.debug.scroll_write_2005_last_scanline = self.scanline;
                self.debug.scroll_write_2005_last_cycle = self.cycle;
                self.debug.scroll_write_2005_last_value = value;
                self.debug.scroll_write_2005_last_phase_second = second_phase;
                if !self.write_toggle {
                    self.fine_x = value & 0x07;
                    self.t = (self.t & !0x001F) | (((value as u16) >> 3) & 0x001F);
                } else {
                    self.t = (self.t & !0x03E0) | ((((value as u16) >> 3) & 0x001F) << 5);
                    self.t = (self.t & !0x7000) | (((value as u16) & 0x07) << 12);
                }
                self.write_toggle = !self.write_toggle;
            }
            0x2006 => {
                let second_phase = self.write_toggle;
                self.debug.addr_writes_2006 = self.debug.addr_writes_2006.wrapping_add(1);
                self.debug.addr_write_2006_prev_scanline = self.debug.addr_write_2006_last_scanline;
                self.debug.addr_write_2006_prev_cycle = self.debug.addr_write_2006_last_cycle;
                self.debug.addr_write_2006_prev_value = self.debug.addr_write_2006_last_value;
                self.debug.addr_write_2006_prev_phase_second =
                    self.debug.addr_write_2006_last_phase_second;
                self.debug.addr_write_2006_last_scanline = self.scanline;
                self.debug.addr_write_2006_last_cycle = self.cycle;
                self.debug.addr_write_2006_last_value = value;
                self.debug.addr_write_2006_last_phase_second = second_phase;
                if !self.write_toggle {
                    self.t = (self.t & 0x00FF) | (((value as u16) & 0x3F) << 8);
                } else {
                    self.t = (self.t & 0x7F00) | (value as u16);
                    if mapper.allow_relaxed_sprite0_hit() {
                        // Keep Bee52 compatibility timing path isolated to Mapper71.
                        self.ppuaddr_reload_pending = true;
                        self.ppuaddr_reload_delay = 1;
                    } else {
                        self.v = self.t;
                        self.ppuaddr_reload_pending = false;
                        self.ppuaddr_reload_delay = 0;
                    }
                }
                self.write_toggle = !self.write_toggle;
            }
            0x2007 => {
                let ppu_addr = self.v & 0x3FFF;
                self.ppu_write(ppu_addr, value, mapper);
                self.increment_vram_addr_cpu_access();
            }
            _ => {}
        }
    }

    pub fn write_oam_dma(&mut self, bytes: &[u8; 256]) {
        for byte in bytes {
            self.oam[self.oam_addr as usize] = *byte;
            self.oam_addr = self.oam_addr.wrapping_add(1);
        }
    }

    pub fn tick(&mut self, mapper: &mut dyn Mapper) {
        self.debug.ticks = self.debug.ticks.wrapping_add(1);
        self.allow_relaxed_sprite0_hit = mapper.allow_relaxed_sprite0_hit();

        if self.nmi_delay > 0 {
            self.nmi_delay = self.nmi_delay.saturating_sub(1);
            if self.nmi_delay == 0 && self.nmi_line {
                self.nmi_pending = true;
                self.debug.nmi_fired = self.debug.nmi_fired.wrapping_add(1);
            }
        }

        if self.ppuaddr_reload_pending {
            if self.ppuaddr_reload_delay > 0 {
                self.ppuaddr_reload_delay = self.ppuaddr_reload_delay.saturating_sub(1);
            }
            if self.ppuaddr_reload_delay == 0 {
                self.v = self.t;
                self.ppuaddr_reload_pending = false;
            }
        }

        let visible_line = (0..240).contains(&self.scanline);
        let pre_render = self.scanline == 261;
        let render_line = visible_line || pre_render;
        let rendering_enabled = self.rendering_enabled();

        if pre_render && self.cycle == 1 {
            self.status &= !(STATUS_VBLANK | STATUS_SPRITE_ZERO_HIT | STATUS_SPRITE_OVERFLOW);
            self.frame_complete = false;
            self.vblank_suppress = false;
            self.update_nmi_line();
        }

        if self.scanline == 241 && self.cycle == 1 {
            self.frame_complete = true;
            self.debug.vblank_entries = self.debug.vblank_entries.wrapping_add(1);
            if !self.vblank_suppress {
                self.status |= STATUS_VBLANK;
            }
            self.vblank_suppress = false;
            self.update_nmi_line();
        }

        if visible_line && self.cycle == 65 {
            self.begin_sprite_overflow_evaluation(rendering_enabled);
        }
        if visible_line && (65..=256).contains(&self.cycle) {
            self.clock_sprite_overflow_evaluation(rendering_enabled);
        }

        if visible_line && self.cycle == 0 {
            self.evaluate_sprites(self.scanline as usize, mapper);
        }

        if visible_line && (1..=256).contains(&self.cycle) {
            if self.cycle == 1 {
                self.sprite0_prev_bg_opaque = false;
            }
            self.render_pixel((self.cycle - 1) as usize, self.scanline as usize);
        }

        if render_line && rendering_enabled {
            if (1..=256).contains(&self.cycle) || (321..=336).contains(&self.cycle) {
                self.shift_background_registers();

                let phase = (self.cycle - 1) & 0x07;
                match phase {
                    0 => {
                        self.load_background_shifters();
                        self.next_tile_id = self.ppu_read(0x2000 | (self.v & 0x0FFF), mapper);
                    }
                    2 => {
                        let addr = 0x23C0
                            | (self.v & 0x0C00)
                            | ((self.v >> 4) & 0x0038)
                            | ((self.v >> 2) & 0x0007);
                        let attr = self.ppu_read(addr, mapper);
                        let shift = ((self.v >> 4) & 0x04) | (self.v & 0x02);
                        self.next_tile_attr = (attr >> shift) & 0x03;
                    }
                    4 => {
                        let fine_y = (self.v >> 12) & 0x07;
                        let table = if (self.ctrl & CTRL_BG_TABLE) != 0 {
                            0x1000
                        } else {
                            0x0000
                        };
                        let addr = table + (self.next_tile_id as u16) * 16 + fine_y;
                        self.next_tile_lsb = self.ppu_read(addr, mapper);
                    }
                    6 => {
                        let fine_y = (self.v >> 12) & 0x07;
                        let table = if (self.ctrl & CTRL_BG_TABLE) != 0 {
                            0x1000
                        } else {
                            0x0000
                        };
                        let addr = table + (self.next_tile_id as u16) * 16 + fine_y + 8;
                        self.next_tile_msb = self.ppu_read(addr, mapper);
                    }
                    7 => self.increment_coarse_x(),
                    _ => {}
                }
            }

            if visible_line && (1..=256).contains(&self.cycle) {
                self.shift_sprite_registers();
            }

            if self.cycle == 256 {
                self.increment_y();
            }

            if self.cycle == 257 {
                self.load_background_shifters();
                self.copy_horizontal_bits();
            }

            if pre_render && (280..=304).contains(&self.cycle) {
                self.copy_vertical_bits();
            }

            if self.cycle == 338 || self.cycle == 340 {
                self.next_tile_id = self.ppu_read(0x2000 | (self.v & 0x0FFF), mapper);
            }
        }

        if visible_line
            && rendering_enabled
            && self.cycle == 260
            && mapper.suppress_a12_on_sprite_eval_reads()
            && (self.ctrl & CTRL_SPRITE_TABLE) != 0
            && (self.ctrl & CTRL_BG_TABLE) == 0
        {
            // MMC3 IRQ timing approximation for renderers that do not run the
            // 257-320 sprite fetch pipeline cycle-by-cycle.
            mapper.notify_ppu_read_addr(0x0000);
            mapper.notify_ppu_read_addr(0x1000);
        }

        // NTSC odd-frame cycle skip: pre-render line drops one PPU cycle when rendering is on.
        if pre_render && rendering_enabled && self.odd_frame && self.cycle == 339 {
            self.cycle = 0;
            self.scanline = 0;
            self.odd_frame = false;
            return;
        }

        self.cycle += 1;
        if self.cycle > 340 {
            self.cycle = 0;
            self.scanline += 1;
            if self.scanline > 261 {
                self.scanline = 0;
                self.odd_frame = !self.odd_frame;
            }
        }
    }

    fn rendering_enabled(&self) -> bool {
        (self.mask & (MASK_SHOW_BG | MASK_SHOW_SPRITES)) != 0
    }

    fn update_nmi_line(&mut self) {
        let line = (self.ctrl & CTRL_NMI_ENABLE) != 0 && (self.status & STATUS_VBLANK) != 0;
        if line && !self.nmi_line {
            // NMI edge is not observed by CPU immediately; short delay is suppressible.
            self.nmi_delay = NMI_DELAY_CYCLES;
            self.debug.nmi_edges = self.debug.nmi_edges.wrapping_add(1);
            if self.nmi_delay == 0 {
                self.nmi_pending = true;
                self.debug.nmi_fired = self.debug.nmi_fired.wrapping_add(1);
            }
        } else if !line {
            self.nmi_delay = 0;
        }
        self.nmi_line = line;
    }

    fn render_pixel(&mut self, x: usize, y: usize) {
        let (bg_pixel, bg_palette, bg_opaque) = self.background_sample(x);
        let (spr_pixel, spr_palette, spr_behind_bg) = self.sprite_sample(x);
        let sprite0_pixel = self.sprite0_pixel(x);

        if sprite0_pixel != 0 && x < 255 {
            self.debug.sprite0_nonzero_events = self.debug.sprite0_nonzero_events.wrapping_add(1);
            self.debug.sprite0_nonzero_last_scanline = self.scanline;
            self.debug.sprite0_nonzero_last_cycle = self.cycle;
            self.debug.sprite0_nonzero_last_bg_pixel = bg_pixel;
            self.debug.sprite0_nonzero_last_bg_opaque = bg_opaque;

            let relaxed_overlap = self.allow_relaxed_sprite0_hit
                && (self.status & STATUS_SPRITE_OVERFLOW) != 0
                && (200..=239).contains(&self.scanline);
            if bg_opaque || self.sprite0_prev_bg_opaque || relaxed_overlap {
                if (self.status & STATUS_SPRITE_ZERO_HIT) == 0 {
                    self.debug.sprite0_hit_events = self.debug.sprite0_hit_events.wrapping_add(1);
                    self.debug.sprite0_hit_last_scanline = self.scanline;
                    self.debug.sprite0_hit_last_cycle = self.cycle;
                }
                self.status |= STATUS_SPRITE_ZERO_HIT;
            }
        }
        self.sprite0_prev_bg_opaque = bg_opaque;

        let palette_index = if bg_opaque {
            if spr_pixel != 0 && !spr_behind_bg {
                0x10 | ((spr_palette << 2) | spr_pixel)
            } else {
                (bg_palette << 2) | bg_pixel
            }
        } else if spr_pixel != 0 {
            0x10 | ((spr_palette << 2) | spr_pixel)
        } else {
            0
        };

        let rgba = self.palette_rgba(palette_index);
        let pixel = (y * FRAME_WIDTH + x) * 4;
        self.frame_buffer[pixel] = rgba[0];
        self.frame_buffer[pixel + 1] = rgba[1];
        self.frame_buffer[pixel + 2] = rgba[2];
        self.frame_buffer[pixel + 3] = 0xFF;
    }

    fn background_sample(&self, x: usize) -> (u8, u8, bool) {
        if (self.mask & MASK_SHOW_BG) == 0 {
            return (0, 0, false);
        }
        if x < 8 && (self.mask & MASK_SHOW_BG_LEFT) == 0 {
            return (0, 0, false);
        }

        let bit = 0x8000u16 >> self.fine_x;

        let p0 = ((self.bg_shift_pattern_lo & bit) != 0) as u8;
        let p1 = ((self.bg_shift_pattern_hi & bit) != 0) as u8;
        let pixel = (p1 << 1) | p0;

        let a0 = ((self.bg_shift_attr_lo & bit) != 0) as u8;
        let a1 = ((self.bg_shift_attr_hi & bit) != 0) as u8;
        let palette = (a1 << 1) | a0;

        (pixel, palette, pixel != 0)
    }

    fn sprite_sample(&self, x: usize) -> (u8, u8, bool) {
        if (self.mask & MASK_SHOW_SPRITES) == 0 {
            return (0, 0, false);
        }
        if x < 8 && (self.mask & MASK_SHOW_SPRITE_LEFT) == 0 {
            return (0, 0, false);
        }

        for i in 0..self.sprite_count {
            if self.sprite_x[i] != 0 {
                continue;
            }

            let p0 = (self.sprite_patterns_lo[i] & 0x80) >> 7;
            let p1 = (self.sprite_patterns_hi[i] & 0x80) >> 6;
            let pixel = p0 | p1;
            if pixel == 0 {
                continue;
            }

            let palette = self.sprite_attributes[i] & 0x03;
            let behind_bg = (self.sprite_attributes[i] & 0x20) != 0;
            return (pixel, palette, behind_bg);
        }

        (0, 0, false)
    }

    fn sprite0_pixel(&self, x: usize) -> u8 {
        if (self.mask & MASK_SHOW_SPRITES) == 0 {
            return 0;
        }
        if x < 8 && (self.mask & MASK_SHOW_SPRITE_LEFT) == 0 {
            return 0;
        }

        for i in 0..self.sprite_count {
            if self.sprite_indices[i] != 0 || self.sprite_x[i] != 0 {
                continue;
            }

            let p0 = (self.sprite_patterns_lo[i] & 0x80) >> 7;
            let p1 = (self.sprite_patterns_hi[i] & 0x80) >> 6;
            return p0 | p1;
        }

        0
    }

    fn shift_background_registers(&mut self) {
        self.bg_shift_pattern_lo <<= 1;
        self.bg_shift_pattern_hi <<= 1;
        self.bg_shift_attr_lo <<= 1;
        self.bg_shift_attr_hi <<= 1;
    }

    fn shift_sprite_registers(&mut self) {
        for i in 0..self.sprite_count {
            if self.sprite_x[i] > 0 {
                self.sprite_x[i] = self.sprite_x[i].wrapping_sub(1);
            } else {
                self.sprite_patterns_lo[i] <<= 1;
                self.sprite_patterns_hi[i] <<= 1;
            }
        }
    }

    fn load_background_shifters(&mut self) {
        self.bg_shift_pattern_lo = (self.bg_shift_pattern_lo & 0xFF00) | self.next_tile_lsb as u16;
        self.bg_shift_pattern_hi = (self.bg_shift_pattern_hi & 0xFF00) | self.next_tile_msb as u16;

        let attr_lo = if (self.next_tile_attr & 0x01) != 0 {
            0xFF
        } else {
            0x00
        };
        let attr_hi = if (self.next_tile_attr & 0x02) != 0 {
            0xFF
        } else {
            0x00
        };

        self.bg_shift_attr_lo = (self.bg_shift_attr_lo & 0xFF00) | attr_lo;
        self.bg_shift_attr_hi = (self.bg_shift_attr_hi & 0xFF00) | attr_hi;
    }

    fn increment_coarse_x(&mut self) {
        if (self.v & 0x001F) == 31 {
            self.v &= !0x001F;
            self.v ^= 0x0400;
        } else {
            self.v = self.v.wrapping_add(1);
        }
    }

    fn increment_y(&mut self) {
        if (self.v & 0x7000) != 0x7000 {
            self.v = self.v.wrapping_add(0x1000);
            return;
        }

        self.v &= !0x7000;
        let mut y = (self.v & 0x03E0) >> 5;
        if y == 29 {
            y = 0;
            self.v ^= 0x0800;
        } else if y == 31 {
            y = 0;
        } else {
            y += 1;
        }

        self.v = (self.v & !0x03E0) | (y << 5);
    }

    fn copy_horizontal_bits(&mut self) {
        self.v = (self.v & !0x041F) | (self.t & 0x041F);
    }

    fn copy_vertical_bits(&mut self) {
        self.v = (self.v & !0x7BE0) | (self.t & 0x7BE0);
    }

    fn begin_sprite_overflow_evaluation(&mut self, rendering_enabled: bool) {
        self.sprite_eval_active = false;
        self.sprite_eval_n = 0;
        self.sprite_eval_m = 0;
        self.sprite_eval_found = 0;
        self.sprite_eval_copy_remaining = 0;
        self.sprite_eval_bug_mode = false;

        if !rendering_enabled {
            return;
        }
        if !(0..=239).contains(&self.scanline) {
            return;
        }

        self.sprite_eval_target_scanline = self.scanline + 1;
        self.sprite_eval_active = true;
    }

    fn set_sprite_overflow_flag(&mut self) {
        if (self.status & STATUS_SPRITE_OVERFLOW) == 0 {
            self.debug.sprite_overflow_events = self.debug.sprite_overflow_events.wrapping_add(1);
            self.debug.sprite_overflow_last_scanline = self.scanline;
            self.debug.sprite_overflow_last_cycle = self.cycle;
        }
        self.status |= STATUS_SPRITE_OVERFLOW;
    }

    fn sprite_match_scanline(byte: u8, scanline: i16, sprite_height: i16) -> bool {
        let row = scanline - (byte as i16 + 1);
        row >= 0 && row < sprite_height
    }

    fn clock_sprite_overflow_evaluation(&mut self, rendering_enabled: bool) {
        if !self.sprite_eval_active {
            return;
        }
        if !rendering_enabled {
            self.sprite_eval_active = false;
            return;
        }
        if ((self.cycle - 65) & 1) != 0 {
            return;
        }
        if self.sprite_eval_n >= 64 {
            self.sprite_eval_active = false;
            return;
        }

        if self.sprite_eval_copy_remaining > 0 {
            self.sprite_eval_copy_remaining = self.sprite_eval_copy_remaining.saturating_sub(1);
            if self.sprite_eval_copy_remaining == 0 {
                self.sprite_eval_n = self.sprite_eval_n.saturating_add(1);
            }
            return;
        }

        let n = self.sprite_eval_n as usize;
        let m = self.sprite_eval_m as usize;
        let byte = self.oam[n * 4 + m];
        let sprite_height = if (self.ctrl & CTRL_SPRITE_SIZE_16) != 0 {
            16
        } else {
            8
        };
        let in_range =
            Self::sprite_match_scanline(byte, self.sprite_eval_target_scanline, sprite_height);

        if !self.sprite_eval_bug_mode {
            if in_range {
                if self.sprite_eval_found < 8 {
                    self.sprite_eval_found = self.sprite_eval_found.saturating_add(1);
                    self.sprite_eval_copy_remaining = 3;
                    self.sprite_eval_m = 0;
                    return;
                }

                self.set_sprite_overflow_flag();
                self.sprite_eval_active = false;
                return;
            }

            if self.sprite_eval_found < 8 {
                self.sprite_eval_n = self.sprite_eval_n.saturating_add(1);
                self.sprite_eval_m = 0;
                return;
            }

            // After 8 sprites, hardware begins diagonal OAM scan (sprite overflow bug):
            // m increments without carry with n, causing tile/attr/X bytes to be
            // interpreted as Y for subsequent sprites.
            self.sprite_eval_bug_mode = true;
            self.sprite_eval_m = 1;
            self.sprite_eval_n = self.sprite_eval_n.saturating_add(1);
            if self.sprite_eval_n >= 64 {
                self.sprite_eval_active = false;
            }
            return;
        }

        if in_range {
            self.set_sprite_overflow_flag();
            self.sprite_eval_active = false;
            return;
        }

        self.sprite_eval_n = self.sprite_eval_n.saturating_add(1);
        self.sprite_eval_m = (self.sprite_eval_m.wrapping_add(1)) & 0x03;
        if self.sprite_eval_n >= 64 {
            self.sprite_eval_active = false;
        }
    }

    fn evaluate_sprites(&mut self, scanline: usize, mapper: &mut dyn Mapper) {
        self.sprite_count = 0;

        let sprite_height = if (self.ctrl & CTRL_SPRITE_SIZE_16) != 0 {
            16usize
        } else {
            8usize
        };

        for i in 0..64 {
            let base = i * 4;
            let y = self.oam[base] as i16 + 1;
            let row = scanline as i16 - y;

            if row < 0 || row >= sprite_height as i16 {
                continue;
            }

            if self.sprite_count >= 8 {
                break;
            }

            let tile_index = self.oam[base + 1];
            let attributes = self.oam[base + 2];
            let x = self.oam[base + 3];

            let mut sprite_row = row as u16;
            if (attributes & 0x80) != 0 {
                sprite_row = (sprite_height as u16 - 1) - sprite_row;
            }

            let (table, tile, row_in_tile) = if sprite_height == 16 {
                let table = ((tile_index & 0x01) as u16) * 0x1000;
                let tile = ((tile_index & 0xFE) as u16) + (sprite_row / 8);
                (table, tile, sprite_row & 0x07)
            } else {
                let table = if (self.ctrl & CTRL_SPRITE_TABLE) != 0 {
                    0x1000
                } else {
                    0x0000
                };
                (table, tile_index as u16, sprite_row & 0x07)
            };

            let addr = table + tile * 16 + row_in_tile;
            let mut low = self.sprite_eval_pattern_read(addr, mapper);
            let mut high = self.sprite_eval_pattern_read(addr + 8, mapper);

            if (attributes & 0x40) != 0 {
                low = low.reverse_bits();
                high = high.reverse_bits();
            }

            let idx = self.sprite_count;
            self.sprite_patterns_lo[idx] = low;
            self.sprite_patterns_hi[idx] = high;
            self.sprite_x[idx] = x;
            self.sprite_attributes[idx] = attributes;
            self.sprite_indices[idx] = i as u8;
            self.sprite_count += 1;
        }

        for i in self.sprite_count..8 {
            self.sprite_patterns_lo[i] = 0;
            self.sprite_patterns_hi[i] = 0;
            self.sprite_x[i] = 0;
            self.sprite_attributes[i] = 0;
            self.sprite_indices[i] = 0;
        }
    }

    fn sprite_eval_pattern_read(&mut self, addr: u16, mapper: &mut dyn Mapper) -> u8 {
        if mapper.suppress_a12_on_sprite_eval_reads() {
            // Mapper 4 IRQ timing is approximated elsewhere from scanline timing.
            // Skip A12 edge notifications for software sprite-eval reads.
            let mapped = addr & 0x1FFF;
            self.debug.last_read_addr = mapped;
            self.debug.pattern_reads = self.debug.pattern_reads.wrapping_add(1);
            mapper.ppu_read(mapped)
        } else {
            self.ppu_read(addr, mapper)
        }
    }

    fn increment_vram_addr(&mut self) {
        if (self.ctrl & CTRL_VRAM_INC_32) != 0 {
            self.v = self.v.wrapping_add(32);
        } else {
            self.v = self.v.wrapping_add(1);
        }
    }

    fn increment_vram_addr_cpu_access(&mut self) {
        // $2007 accesses during rendering use the rendering increment path.
        if self.rendering_enabled() && ((0..240).contains(&self.scanline) || self.scanline == 261) {
            self.increment_coarse_x();
            self.increment_y();
        } else {
            self.increment_vram_addr();
        }
    }

    fn palette_rgba(&self, palette_index: u8) -> [u8; 4] {
        let mut idx = (palette_index as usize) & 0x1F;
        if idx >= 16 && (idx & 0x03) == 0 {
            idx -= 16;
        }
        let color = self.palette_ram[idx] & 0x3F;
        let rgb = NES_PALETTE[color as usize % 64];
        [rgb[0], rgb[1], rgb[2], 0xFF]
    }

    fn ppu_read(&mut self, addr: u16, mapper: &mut dyn Mapper) -> u8 {
        let addr = addr & 0x3FFF;
        self.debug.last_read_addr = addr;
        let value = match addr {
            0x0000..=0x1FFF => {
                self.debug.pattern_reads = self.debug.pattern_reads.wrapping_add(1);
                mapper.ppu_read(addr)
            }
            0x2000..=0x3EFF => {
                self.debug.nametable_reads = self.debug.nametable_reads.wrapping_add(1);
                let mirrored = 0x2000 + ((addr - 0x2000) % 0x1000);
                if let Some(value) = mapper.ppu_nametable_read(mirrored, &self.vram) {
                    value
                } else {
                    let index = self.mirrored_vram_index(mirrored, mapper.mirroring());
                    self.vram[index]
                }
            }
            0x3F00..=0x3FFF => {
                self.debug.palette_reads = self.debug.palette_reads.wrapping_add(1);
                let index = self.palette_index(addr);
                self.palette_ram[index]
            }
            _ => 0,
        };

        mapper.notify_ppu_read_addr(addr);
        value
    }

    fn ppu_write(&mut self, addr: u16, value: u8, mapper: &mut dyn Mapper) {
        let addr = addr & 0x3FFF;
        self.debug.last_write_addr = addr;
        match addr {
            0x0000..=0x1FFF => {
                self.debug.pattern_writes = self.debug.pattern_writes.wrapping_add(1);
                mapper.ppu_write(addr, value);
            }
            0x2000..=0x3EFF => {
                self.debug.nametable_writes = self.debug.nametable_writes.wrapping_add(1);
                let mirrored = 0x2000 + ((addr - 0x2000) % 0x1000);
                if !mapper.ppu_nametable_write(mirrored, value, &mut self.vram) {
                    let index = self.mirrored_vram_index(mirrored, mapper.mirroring());
                    self.vram[index] = value;
                }
            }
            0x3F00..=0x3FFF => {
                self.debug.palette_writes = self.debug.palette_writes.wrapping_add(1);
                let index = self.palette_index(addr);
                self.palette_ram[index] = value;
            }
            _ => {}
        }

        mapper.notify_ppu_write_addr(addr);
    }

    fn palette_index(&self, addr: u16) -> usize {
        let mut index = ((addr - 0x3F00) % 0x20) as usize;
        if index >= 16 && (index & 0x03) == 0 {
            index -= 16;
        }
        index
    }

    fn mirrored_vram_index(&self, addr: u16, mirroring: Mirroring) -> usize {
        let index = (addr - 0x2000) as usize;
        let table = index / 0x400;
        let offset = index % 0x400;

        let mapped_table = match mirroring {
            Mirroring::Horizontal => match table {
                0 | 1 => 0,
                _ => 1,
            },
            Mirroring::Vertical => table & 1,
            Mirroring::OneScreenLower => 0,
            Mirroring::OneScreenUpper => 1,
            Mirroring::FourScreen => table & 3,
        };

        mapped_table * 0x400 + offset
    }
}
