use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, ColorImage, Key, TextureHandle, TextureOptions};

use crate::audio::AudioOutput;
use crate::nes::{
    BUTTON_A, BUTTON_B, BUTTON_DOWN, BUTTON_LEFT, BUTTON_RIGHT, BUTTON_SELECT, BUTTON_START,
    BUTTON_UP, Nes,
};

const NTSC_FRAME_RATE_HZ: f64 = 60.098_813_897_440_515;
const HIGH_REFRESH_RATE_HZ: f64 = 240.0;
const MAX_FRAMES_PER_UPDATE: u32 = 2;
const AUDIO_TARGET_BUFFER_MS: usize = 8;
const AUDIO_MAX_BUFFER_MS: usize = 16;

pub struct NesApp {
    nes: Nes,
    frame_texture: Option<TextureHandle>,
    status_line: String,
    loaded_rom: Option<PathBuf>,
    last_screen_rect: Option<egui::Rect>,
    audio: Option<AudioOutput>,
    frame_interval: Duration,
    high_refresh_interval: Duration,
    next_frame_at: Option<Instant>,
}

impl NesApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let mut nes = Nes::new();
        let audio = AudioOutput::new().ok();
        if let Some(audio_out) = &audio {
            nes.set_audio_sample_rate(audio_out.sample_rate());
        } else {
            nes.set_audio_sample_rate(48_000);
        }

        Self {
            nes,
            frame_texture: None,
            status_line: "Drop a .nes file or click Open ROM".to_string(),
            loaded_rom: None,
            last_screen_rect: None,
            audio,
            frame_interval: Duration::from_secs_f64(1.0 / NTSC_FRAME_RATE_HZ),
            high_refresh_interval: Duration::from_secs_f64(1.0 / HIGH_REFRESH_RATE_HZ),
            next_frame_at: None,
        }
    }

    fn load_rom(&mut self, path: &Path) {
        match self.nes.load_rom_from_path(path) {
            Ok(()) => {
                self.loaded_rom = Some(path.to_path_buf());
                self.status_line = format!(
                    "Loaded {} using {}",
                    path.file_name().and_then(|f| f.to_str()).unwrap_or("ROM"),
                    self.nes.mapper_name()
                );
                self.frame_texture = None;
                self.next_frame_at = None;
            }
            Err(err) => {
                self.status_line = format!("Failed to load ROM: {err}");
            }
        }
    }

    fn open_rom_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("NES ROM", &["nes"])
            .set_title("Open NES ROM")
            .pick_file()
        {
            self.load_rom(&path);
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|input| input.raw.dropped_files.clone());
        for file in dropped {
            if let Some(path) = file.path {
                let is_nes = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("nes"))
                    .unwrap_or(false);

                if is_nes {
                    self.load_rom(&path);
                } else {
                    self.status_line = format!("Unsupported file: {}", path.display());
                }
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let open_shortcut = ctx.input(|i| i.modifiers.command && i.key_pressed(Key::O));
        if open_shortcut {
            self.open_rom_dialog();
        }

        let reset = ctx.input(|i| i.key_pressed(Key::R));
        if reset && self.nes.has_rom() {
            self.nes.reset();
            self.next_frame_at = None;
            self.status_line = "Reset complete".to_string();
        }
    }

    fn controller_state_from_input(ctx: &egui::Context) -> u8 {
        let mut state = 0u8;

        ctx.input(|input| {
            if input.key_down(Key::W) {
                state |= BUTTON_UP;
            }
            if input.key_down(Key::S) {
                state |= BUTTON_DOWN;
            }
            if input.key_down(Key::A) {
                state |= BUTTON_LEFT;
            }
            if input.key_down(Key::D) {
                state |= BUTTON_RIGHT;
            }
            if input.key_down(Key::ArrowUp) {
                state |= BUTTON_UP;
            }
            if input.key_down(Key::ArrowDown) {
                state |= BUTTON_DOWN;
            }
            if input.key_down(Key::ArrowLeft) {
                state |= BUTTON_LEFT;
            }
            if input.key_down(Key::ArrowRight) {
                state |= BUTTON_RIGHT;
            }
            if input.key_down(Key::Space) {
                state |= BUTTON_A;
            }
            if input.key_down(Key::Z) {
                state |= BUTTON_A;
            }
            if input.key_down(Key::X) {
                state |= BUTTON_B;
            }
            if input.key_down(Key::Enter) {
                state |= BUTTON_START;
            }
            if input.modifiers.shift {
                state |= BUTTON_SELECT;
            }
        });

        state
    }

    fn update_zapper(&mut self, ctx: &egui::Context) {
        let trigger = ctx.input(|input| input.pointer.primary_down());
        let pointer = ctx.input(|input| input.pointer.hover_pos());

        if let (Some(rect), Some(pos)) = (self.last_screen_rect, pointer) {
            if rect.contains(pos) && rect.width() > 0.0 && rect.height() > 0.0 {
                let nx = ((pos.x - rect.left()) / rect.width() * 256.0)
                    .floor()
                    .clamp(0.0, 255.0) as i16;
                let ny = ((pos.y - rect.top()) / rect.height() * 240.0)
                    .floor()
                    .clamp(0.0, 239.0) as i16;
                self.nes.set_zapper_state(nx, ny, trigger);
                return;
            }
        }

        self.nes.set_zapper_state(-1, -1, trigger);
    }

    fn update_texture(&mut self, ctx: &egui::Context) {
        let image = ColorImage::from_rgba_unmultiplied([256, 240], self.nes.frame_buffer());

        if let Some(texture) = self.frame_texture.as_mut() {
            texture.set(image, TextureOptions::NEAREST);
        } else {
            self.frame_texture =
                Some(ctx.load_texture("nes-frame", image, TextureOptions::NEAREST));
        }
    }

    fn run_frame_with_audio(&mut self, controller_state: u8) {
        self.nes.set_controller_state(controller_state);
        self.nes.run_frame();
        let audio_samples = self.nes.take_audio_samples();
        if let Some(audio) = &self.audio {
            audio.push_samples(&audio_samples);
        }
    }

    fn queued_audio_samples(&self) -> usize {
        if let Some(audio) = &self.audio {
            audio.queued_samples()
        } else {
            0
        }
    }
}

impl eframe::App for NesApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_dropped_files(ctx);
        self.handle_shortcuts(ctx);
        self.update_zapper(ctx);

        if self.nes.has_rom() {
            let now = Instant::now();
            let mut next = self.next_frame_at.unwrap_or(now);
            let mut ran_frames = 0u32;

            let sample_rate = self
                .audio
                .as_ref()
                .map(|audio| audio.sample_rate() as usize);
            if let Some(sample_rate) = sample_rate {
                let target_samples = sample_rate * AUDIO_TARGET_BUFFER_MS / 1000;
                let max_samples = sample_rate * AUDIO_MAX_BUFFER_MS / 1000;

                while self.queued_audio_samples() < target_samples
                    && ran_frames < MAX_FRAMES_PER_UPDATE
                {
                    let state = Self::controller_state_from_input(ctx);
                    self.run_frame_with_audio(state);
                    ran_frames += 1;
                    next += self.frame_interval;
                }

                while Instant::now() >= next
                    && self.queued_audio_samples() < max_samples
                    && ran_frames < MAX_FRAMES_PER_UPDATE
                {
                    let state = Self::controller_state_from_input(ctx);
                    self.run_frame_with_audio(state);
                    ran_frames += 1;
                    next += self.frame_interval;
                }
            } else {
                while Instant::now() >= next && ran_frames < MAX_FRAMES_PER_UPDATE {
                    let state = Self::controller_state_from_input(ctx);
                    self.nes.set_controller_state(state);
                    self.nes.run_frame();
                    let _ = self.nes.take_audio_samples();
                    ran_frames += 1;
                    next += self.frame_interval;
                }
            }

            if ran_frames == 0 && now + self.frame_interval < next {
                next = now + self.frame_interval;
            }

            self.next_frame_at = Some(next);
        }

        self.update_texture(ctx);

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open ROM").clicked() {
                    self.open_rom_dialog();
                }

                let reset_enabled = self.nes.has_rom();
                if ui
                    .add_enabled(reset_enabled, egui::Button::new("Reset (R)"))
                    .clicked()
                {
                    self.nes.reset();
                    self.next_frame_at = None;
                    self.status_line = "Reset complete".to_string();
                }

                if let Some(path) = &self.loaded_rom {
                    ui.separator();
                    ui.label(path.display().to_string());
                }
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(&self.status_line);
                ui.separator();
                ui.label(format!("Mapper: {}", self.nes.mapper_name()));
                ui.separator();
                ui.label(format!("Core: {}", self.nes.accuracy_profile()));
                ui.separator();
                if let Some(audio) = &self.audio {
                    ui.label(format!(
                        "Audio: {} Hz (queue {} ms, target {}-{} ms)",
                        audio.sample_rate(),
                        (audio.queued_samples() * 1000) / audio.sample_rate() as usize,
                        AUDIO_TARGET_BUFFER_MS,
                        AUDIO_MAX_BUFFER_MS
                    ));
                } else {
                    ui.label("Audio: unavailable");
                }
                ui.separator();
                ui.label(
                    "Controls: WASD move, Space/Z jump (A), X=B, Enter=Start, Shift=Select, Mouse=Zapper",
                );
            });

            ui.separator();
            let (a, x, y, p, sp, pc) = self.nes.debug_cpu_regs();
            let (pnmi, pirq, dma) = self.nes.debug_interrupt_state();
            let (sl, cy) = self.nes.debug_ppu_scanline_cycle();
            let debug = self.nes.debug_counters();
            let ppu_debug = self.nes.debug_ppu_counters();
            ui.collapsing("Debug", |ui| {
                ui.monospace(format!(
                    "CPU A={:02X} X={:02X} Y={:02X} P={:02X} SP={:02X} PC={:04X} | pending_nmi={} pending_irq={} dma_cycles={}",
                    a, x, y, p, sp, pc, pnmi, pirq, dma
                ));
                ui.monospace(format!(
                    "Core frames={} cpu_steps={} cycles={} reads={} writes={} dma_transfers={} nmi_serviced={} irq_serviced={}",
                    debug.frame_count,
                    debug.cpu_steps,
                    self.nes.debug_total_cycles(),
                    debug.cpu_reads,
                    debug.cpu_writes,
                    debug.dma_transfers,
                    self.nes.debug_nmi_serviced_count(),
                    debug.irq_serviced_count
                ));
                ui.monospace(format!(
                    "Bus reads ram={} ppu={} apu/io={} cart={} | writes ram={} ppu={} apu/io={} cart={} | last read=${:04X} last write=${:04X}:${:02X}",
                    debug.cpu_reads_ram,
                    debug.cpu_reads_ppu_regs,
                    debug.cpu_reads_apu_io,
                    debug.cpu_reads_cart,
                    debug.cpu_writes_ram,
                    debug.cpu_writes_ppu_regs,
                    debug.cpu_writes_apu_io,
                    debug.cpu_writes_cart,
                    debug.last_cpu_read_addr,
                    debug.last_cpu_write_addr,
                    debug.last_cpu_write_value
                ));
                ui.monospace(format!(
                    "PPU sl={} cy={} ticks={} vblank_entries={} nmi_edges={} nmi_fired={} sprite_overflow={} last_ovf=({}, {}) status_reads={} last_status_read=({}, {}) pattern_rw={}/{} nametable_rw={}/{} palette_rw={}/{} last_rw=${:04X}/${:04X}",
                    sl,
                    cy,
                    ppu_debug.ticks,
                    ppu_debug.vblank_entries,
                    ppu_debug.nmi_edges,
                    ppu_debug.nmi_fired,
                    ppu_debug.sprite_overflow_events,
                    ppu_debug.sprite_overflow_last_scanline,
                    ppu_debug.sprite_overflow_last_cycle,
                    ppu_debug.status_reads,
                    ppu_debug.status_read_last_scanline,
                    ppu_debug.status_read_last_cycle,
                    ppu_debug.pattern_reads,
                    ppu_debug.pattern_writes,
                    ppu_debug.nametable_reads,
                    ppu_debug.nametable_writes,
                    ppu_debug.palette_reads,
                    ppu_debug.palette_writes,
                    ppu_debug.last_read_addr,
                    ppu_debug.last_write_addr
                ));
                ui.monospace(format!("Mapper detail: {}", self.nes.debug_mapper_state()));

                let events = self.nes.debug_recent_events(8);
                if !events.is_empty() {
                    ui.separator();
                    ui.label("Recent events:");
                    for event in events {
                        ui.monospace(event);
                    }
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                let available = ui.available_size();
                let scale_x = (available.x / 256.0).max(1.0);
                let scale_y = (available.y / 240.0).max(1.0);
                let scale = scale_x.min(scale_y).floor().max(1.0);
                let target = egui::vec2(256.0 * scale, 240.0 * scale);

                if let Some(texture) = &self.frame_texture {
                    let response = ui.add(egui::Image::new(texture).fit_to_exact_size(target));
                    self.last_screen_rect = Some(response.rect);
                }

                ui.add_space(8.0);
                ui.label(
                    "Drag/drop ROM. For Zapper games, aim with mouse and hold left click to fire.",
                );
            });
        });

        if let Some(next) = self.next_frame_at {
            let wait = next.saturating_duration_since(Instant::now());
            ctx.request_repaint_after(wait.min(self.high_refresh_interval));
        } else {
            ctx.request_repaint_after(self.high_refresh_interval);
        }
    }
}
