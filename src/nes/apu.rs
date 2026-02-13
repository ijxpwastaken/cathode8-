use std::f32::consts::PI;

const CPU_CLOCK_HZ: f64 = 1_789_772.727_272_727_3;
const DEFAULT_SAMPLE_RATE: u32 = 48_000;

const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];

const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 1, 1, 1],
];

const TRI_TABLE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
];

const NOISE_PERIOD_TABLE: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

const DMC_RATE_TABLE: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];

const FC_4STEP_Q1: u32 = 7_457;
const FC_4STEP_Q2_H2: u32 = 14_913;
const FC_4STEP_Q3: u32 = 22_371;
const FC_4STEP_Q4_H4_IRQ: u32 = 29_829;
const FC_4STEP_RESET: u32 = 29_830;

const FC_5STEP_Q1: u32 = 7_457;
const FC_5STEP_Q2_H2: u32 = 14_913;
const FC_5STEP_Q3: u32 = 22_371;
const FC_5STEP_Q4_H4: u32 = 37_281;
const FC_5STEP_RESET: u32 = 37_282;

pub struct Apu {
    pulse1: PulseChannel,
    pulse2: PulseChannel,
    triangle: TriangleChannel,
    noise: NoiseChannel,
    dmc: DmcChannel,

    frame_counter: u32,
    frame_mode_5_step: bool,
    frame_irq_inhibit: bool,
    frame_irq_flag: bool,
    frame_counter_write_pending: bool,
    frame_counter_write_value: u8,
    frame_counter_write_delay: u8,

    cpu_cycle: u64,
    sample_rate: u32,
    sample_phase: f64,
    samples: Vec<f32>,

    hp90_prev_in: f32,
    hp90_prev_out: f32,
    hp90_a: f32,
    hp440_prev_in: f32,
    hp440_prev_out: f32,
    hp440_a: f32,
    lp14k_prev_out: f32,
    lp14k_a: f32,
    dmc_dma_request: Option<u16>,
}

impl Apu {
    pub fn new() -> Self {
        let mut apu = Self {
            pulse1: PulseChannel::new(true),
            pulse2: PulseChannel::new(false),
            triangle: TriangleChannel::new(),
            noise: NoiseChannel::new(),
            dmc: DmcChannel::new(),
            frame_counter: 0,
            frame_mode_5_step: false,
            frame_irq_inhibit: false,
            frame_irq_flag: false,
            frame_counter_write_pending: false,
            frame_counter_write_value: 0,
            frame_counter_write_delay: 0,
            cpu_cycle: 0,
            sample_rate: DEFAULT_SAMPLE_RATE,
            sample_phase: 0.0,
            samples: Vec::with_capacity(2048),
            hp90_prev_in: 0.0,
            hp90_prev_out: 0.0,
            hp90_a: 0.0,
            hp440_prev_in: 0.0,
            hp440_prev_out: 0.0,
            hp440_a: 0.0,
            lp14k_prev_out: 0.0,
            lp14k_a: 0.0,
            dmc_dma_request: None,
        };
        apu.update_filter_coeffs();
        apu
    }

    pub fn reset(&mut self) {
        self.pulse1 = PulseChannel::new(true);
        self.pulse2 = PulseChannel::new(false);
        self.triangle = TriangleChannel::new();
        self.noise = NoiseChannel::new();
        self.dmc = DmcChannel::new();
        self.frame_counter = 0;
        self.frame_mode_5_step = false;
        self.frame_irq_inhibit = false;
        self.frame_irq_flag = false;
        self.frame_counter_write_pending = false;
        self.frame_counter_write_value = 0;
        self.frame_counter_write_delay = 0;
        self.cpu_cycle = 0;
        self.sample_phase = 0.0;
        self.samples.clear();
        self.hp90_prev_in = 0.0;
        self.hp90_prev_out = 0.0;
        self.hp440_prev_in = 0.0;
        self.hp440_prev_out = 0.0;
        self.lp14k_prev_out = 0.0;
        self.dmc_dma_request = None;
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate.max(8_000);
        self.update_filter_coeffs();
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn write_register(&mut self, addr: u16, value: u8) {
        match addr {
            0x4000 => self.pulse1.write_control(value),
            0x4001 => self.pulse1.write_sweep(value),
            0x4002 => self.pulse1.write_timer_low(value),
            0x4003 => self.pulse1.write_timer_high(value),

            0x4004 => self.pulse2.write_control(value),
            0x4005 => self.pulse2.write_sweep(value),
            0x4006 => self.pulse2.write_timer_low(value),
            0x4007 => self.pulse2.write_timer_high(value),

            0x4008 => self.triangle.write_linear(value),
            0x400A => self.triangle.write_timer_low(value),
            0x400B => self.triangle.write_timer_high(value),

            0x400C => self.noise.write_control(value),
            0x400E => self.noise.write_period(value),
            0x400F => self.noise.write_length(value),

            0x4010 => self.dmc.write_control(value),
            0x4011 => self.dmc.write_output_level(value),
            0x4012 => self.dmc.write_sample_addr(value),
            0x4013 => self.dmc.write_sample_length(value),

            0x4015 => self.write_status(value),
            0x4017 => self.write_frame_counter(value),
            _ => {}
        }
    }

    pub fn read_status(&mut self) -> u8 {
        let mut status = 0u8;
        if self.pulse1.length_counter > 0 {
            status |= 0x01;
        }
        if self.pulse2.length_counter > 0 {
            status |= 0x02;
        }
        if self.triangle.length_counter > 0 {
            status |= 0x04;
        }
        if self.noise.length_counter > 0 {
            status |= 0x08;
        }
        if self.dmc.playback_active() {
            status |= 0x10;
        }
        if self.frame_irq_flag {
            status |= 0x40;
        }
        if self.dmc.irq_flag {
            status |= 0x80;
        }

        self.frame_irq_flag = false;
        status
    }

    pub fn irq_pending(&self) -> bool {
        self.frame_irq_flag || self.dmc.irq_flag
    }

    pub fn tick(&mut self) {
        self.cpu_cycle = self.cpu_cycle.wrapping_add(1);

        if self.frame_counter_write_pending {
            if self.frame_counter_write_delay > 0 {
                self.frame_counter_write_delay = self.frame_counter_write_delay.saturating_sub(1);
            }
            if self.frame_counter_write_delay == 0 {
                self.apply_frame_counter_write(self.frame_counter_write_value);
                self.frame_counter_write_pending = false;
            }
        }

        if (self.cpu_cycle & 1) == 0 {
            self.pulse1.clock_timer();
            self.pulse2.clock_timer();
            self.noise.clock_timer();
        }
        self.triangle.clock_timer();
        // DMC timer runs every CPU cycle (unlike pulse/noise timers).
        self.dmc.clock_timer();
        if self.dmc.needs_dma() && self.dmc_dma_request.is_none() {
            self.dmc_dma_request = Some(self.dmc.current_dma_addr());
        }

        self.clock_frame_counter();

        self.sample_phase += self.sample_rate as f64;
        while self.sample_phase >= CPU_CLOCK_HZ {
            self.sample_phase -= CPU_CLOCK_HZ;
            let mixed = self.mix_sample();
            let filtered = self.apply_output_filters(mixed);
            self.samples.push(filtered);
        }
    }

    pub fn take_samples(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.samples)
    }

    pub fn take_dmc_dma_request(&mut self) -> Option<u16> {
        self.dmc_dma_request.take()
    }

    pub fn complete_dmc_dma(&mut self, value: u8) {
        self.dmc.consume_dma_byte(value);
        if self.dmc.needs_dma() && self.dmc_dma_request.is_none() {
            self.dmc_dma_request = Some(self.dmc.current_dma_addr());
        }
    }

    fn write_status(&mut self, value: u8) {
        // Any write to $4015 clears pending DMC IRQ.
        self.dmc.irq_flag = false;

        self.pulse1.enabled = (value & 0x01) != 0;
        if !self.pulse1.enabled {
            self.pulse1.length_counter = 0;
        }

        self.pulse2.enabled = (value & 0x02) != 0;
        if !self.pulse2.enabled {
            self.pulse2.length_counter = 0;
        }

        self.triangle.enabled = (value & 0x04) != 0;
        if !self.triangle.enabled {
            self.triangle.length_counter = 0;
        }

        self.noise.enabled = (value & 0x08) != 0;
        if !self.noise.enabled {
            self.noise.length_counter = 0;
        }

        self.dmc.enabled = (value & 0x10) != 0;
        if !self.dmc.enabled {
            self.dmc.stop();
        } else if !self.dmc.playback_active() {
            self.dmc.restart_sample();
            if self.dmc.needs_dma() && self.dmc_dma_request.is_none() {
                self.dmc_dma_request = Some(self.dmc.current_dma_addr());
            }
        }
    }

    fn write_frame_counter(&mut self, value: u8) {
        if (value & 0x40) != 0 {
            self.frame_irq_flag = false;
        }
        self.frame_counter_write_pending = true;
        self.frame_counter_write_value = value;
        self.frame_counter_write_delay = if (self.cpu_cycle & 1) == 0 { 3 } else { 4 };
    }

    fn apply_frame_counter_write(&mut self, value: u8) {
        self.frame_mode_5_step = (value & 0x80) != 0;
        self.frame_irq_inhibit = (value & 0x40) != 0;
        if self.frame_irq_inhibit {
            self.frame_irq_flag = false;
        }
        self.frame_counter = 0;
        if self.frame_mode_5_step {
            self.clock_quarter_frame();
            self.clock_half_frame();
        }
    }

    fn clock_frame_counter(&mut self) {
        self.frame_counter = self.frame_counter.wrapping_add(1);

        if self.frame_mode_5_step {
            match self.frame_counter {
                FC_5STEP_Q1 | FC_5STEP_Q3 => self.clock_quarter_frame(),
                FC_5STEP_Q2_H2 | FC_5STEP_Q4_H4 => {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                }
                FC_5STEP_RESET => {
                    self.frame_counter = 0;
                }
                _ => {}
            }
        } else {
            match self.frame_counter {
                FC_4STEP_Q1 | FC_4STEP_Q3 => self.clock_quarter_frame(),
                FC_4STEP_Q2_H2 => {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                }
                FC_4STEP_Q4_H4_IRQ => {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                    if !self.frame_irq_inhibit {
                        self.frame_irq_flag = true;
                    }
                }
                FC_4STEP_RESET => {
                    if !self.frame_irq_inhibit {
                        self.frame_irq_flag = true;
                    }
                    self.frame_counter = 0;
                }
                _ => {}
            }
        }
    }

    fn clock_quarter_frame(&mut self) {
        self.pulse1.clock_envelope();
        self.pulse2.clock_envelope();
        self.triangle.clock_linear_counter();
        self.noise.clock_envelope();
    }

    fn clock_half_frame(&mut self) {
        self.pulse1.clock_length_and_sweep();
        self.pulse2.clock_length_and_sweep();
        self.triangle.clock_length_counter();
        self.noise.clock_length_counter();
    }

    fn mix_sample(&self) -> f32 {
        let p1 = self.pulse1.output() as f32;
        let p2 = self.pulse2.output() as f32;
        let t = self.triangle.output() as f32;
        let n = self.noise.output() as f32;
        let d = self.dmc.output() as f32;

        let pulse_sum = p1 + p2;
        let pulse_out = if pulse_sum > 0.0 {
            95.88 / ((8128.0 / pulse_sum) + 100.0)
        } else {
            0.0
        };

        let tnd_in = (t / 8227.0) + (n / 12241.0) + (d / 22638.0);
        let tnd_out = if tnd_in > 0.0 {
            159.79 / ((1.0 / tnd_in) + 100.0)
        } else {
            0.0
        };

        pulse_out + tnd_out
    }

    fn update_filter_coeffs(&mut self) {
        let dt = 1.0f32 / self.sample_rate as f32;
        self.hp90_a = high_pass_alpha(90.0, dt);
        self.hp440_a = high_pass_alpha(440.0, dt);
        self.lp14k_a = low_pass_alpha(14_000.0, dt);
    }

    fn apply_output_filters(&mut self, mut sample: f32) -> f32 {
        let hp90 = self.hp90_a * (self.hp90_prev_out + sample - self.hp90_prev_in);
        self.hp90_prev_in = sample;
        self.hp90_prev_out = hp90;
        sample = hp90;

        let hp440 = self.hp440_a * (self.hp440_prev_out + sample - self.hp440_prev_in);
        self.hp440_prev_in = sample;
        self.hp440_prev_out = hp440;
        sample = hp440;

        self.lp14k_prev_out += self.lp14k_a * (sample - self.lp14k_prev_out);
        self.lp14k_prev_out.clamp(-1.0, 1.0)
    }
}

fn high_pass_alpha(cutoff_hz: f32, dt: f32) -> f32 {
    let rc = 1.0 / (2.0 * PI * cutoff_hz);
    rc / (rc + dt)
}

fn low_pass_alpha(cutoff_hz: f32, dt: f32) -> f32 {
    let rc = 1.0 / (2.0 * PI * cutoff_hz);
    dt / (rc + dt)
}

#[derive(Clone, Copy)]
struct PulseChannel {
    enabled: bool,
    channel1: bool,
    duty: u8,
    duty_step: u8,

    timer_period: u16,
    timer_counter: u16,
    length_counter: u8,

    length_halt: bool,
    constant_volume: bool,
    volume: u8,
    envelope_period: u8,
    envelope_start: bool,
    envelope_divider: u8,
    envelope_decay: u8,

    sweep_enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_reload: bool,
    sweep_divider: u8,
}

impl PulseChannel {
    fn new(channel1: bool) -> Self {
        Self {
            enabled: false,
            channel1,
            duty: 0,
            duty_step: 0,
            timer_period: 0,
            timer_counter: 0,
            length_counter: 0,
            length_halt: false,
            constant_volume: false,
            volume: 0,
            envelope_period: 0,
            envelope_start: false,
            envelope_divider: 0,
            envelope_decay: 0,
            sweep_enabled: false,
            sweep_period: 1,
            sweep_negate: false,
            sweep_shift: 0,
            sweep_reload: false,
            sweep_divider: 0,
        }
    }

    fn write_control(&mut self, value: u8) {
        self.duty = (value >> 6) & 0x03;
        self.length_halt = (value & 0x20) != 0;
        self.constant_volume = (value & 0x10) != 0;
        self.volume = value & 0x0F;
        self.envelope_period = value & 0x0F;
        self.envelope_start = true;
    }

    fn write_sweep(&mut self, value: u8) {
        self.sweep_enabled = (value & 0x80) != 0;
        self.sweep_period = ((value >> 4) & 0x07) + 1;
        self.sweep_negate = (value & 0x08) != 0;
        self.sweep_shift = value & 0x07;
        self.sweep_reload = true;
    }

    fn write_timer_low(&mut self, value: u8) {
        self.timer_period = (self.timer_period & 0xFF00) | value as u16;
    }

    fn write_timer_high(&mut self, value: u8) {
        self.timer_period = (self.timer_period & 0x00FF) | (((value & 0x07) as u16) << 8);
        if self.enabled {
            self.length_counter = LENGTH_TABLE[(value >> 3) as usize];
        }
        self.duty_step = 0;
        self.envelope_start = true;
    }

    fn clock_timer(&mut self) {
        if self.timer_counter == 0 {
            self.timer_counter = self.timer_period;
            self.duty_step = (self.duty_step + 1) & 0x07;
        } else {
            self.timer_counter = self.timer_counter.saturating_sub(1);
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_start {
            self.envelope_start = false;
            self.envelope_decay = 15;
            self.envelope_divider = self.envelope_period;
            return;
        }

        if self.envelope_divider == 0 {
            self.envelope_divider = self.envelope_period;
            if self.envelope_decay == 0 {
                if self.length_halt {
                    self.envelope_decay = 15;
                }
            } else {
                self.envelope_decay = self.envelope_decay.saturating_sub(1);
            }
        } else {
            self.envelope_divider = self.envelope_divider.saturating_sub(1);
        }
    }

    fn clock_length_and_sweep(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter = self.length_counter.saturating_sub(1);
        }

        if self.sweep_reload {
            if self.sweep_enabled && self.sweep_divider == 0 {
                self.apply_sweep();
            }
            self.sweep_divider = self.sweep_period;
            self.sweep_reload = false;
            return;
        }

        if self.sweep_divider == 0 {
            if self.sweep_enabled {
                self.apply_sweep();
            }
            self.sweep_divider = self.sweep_period;
        } else {
            self.sweep_divider = self.sweep_divider.saturating_sub(1);
        }
    }

    fn apply_sweep(&mut self) {
        if self.sweep_shift == 0 {
            return;
        }

        let target = self.sweep_target_period();
        if target <= 0x07FF {
            self.timer_period = target;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled || self.length_counter == 0 {
            return 0;
        }
        if DUTY_TABLE[self.duty as usize][self.duty_step as usize] == 0 {
            return 0;
        }
        if self.timer_period < 8 {
            return 0;
        }

        let target = self.sweep_target_period();
        if target > 0x07FF {
            return 0;
        }

        if self.constant_volume {
            self.volume
        } else {
            self.envelope_decay
        }
    }

    fn sweep_target_period(&self) -> u16 {
        if self.sweep_shift == 0 {
            return self.timer_period;
        }
        let change = self.timer_period >> self.sweep_shift;
        if self.sweep_negate {
            let extra = if self.channel1 { 1 } else { 0 };
            self.timer_period.wrapping_sub(change + extra)
        } else {
            self.timer_period.wrapping_add(change)
        }
    }
}

#[derive(Clone, Copy)]
struct TriangleChannel {
    enabled: bool,
    control_flag: bool,
    linear_reload_value: u8,
    linear_counter: u8,
    linear_reload_flag: bool,

    timer_period: u16,
    timer_counter: u16,
    length_counter: u8,
    seq_step: u8,
}

impl TriangleChannel {
    fn new() -> Self {
        Self {
            enabled: false,
            control_flag: false,
            linear_reload_value: 0,
            linear_counter: 0,
            linear_reload_flag: false,
            timer_period: 0,
            timer_counter: 0,
            length_counter: 0,
            seq_step: 0,
        }
    }

    fn write_linear(&mut self, value: u8) {
        self.control_flag = (value & 0x80) != 0;
        self.linear_reload_value = value & 0x7F;
    }

    fn write_timer_low(&mut self, value: u8) {
        self.timer_period = (self.timer_period & 0xFF00) | value as u16;
    }

    fn write_timer_high(&mut self, value: u8) {
        self.timer_period = (self.timer_period & 0x00FF) | (((value & 0x07) as u16) << 8);
        if self.enabled {
            self.length_counter = LENGTH_TABLE[(value >> 3) as usize];
        }
        self.linear_reload_flag = true;
    }

    fn clock_linear_counter(&mut self) {
        if self.linear_reload_flag {
            self.linear_counter = self.linear_reload_value;
        } else if self.linear_counter > 0 {
            self.linear_counter = self.linear_counter.saturating_sub(1);
        }

        if !self.control_flag {
            self.linear_reload_flag = false;
        }
    }

    fn clock_length_counter(&mut self) {
        if !self.control_flag && self.length_counter > 0 {
            self.length_counter = self.length_counter.saturating_sub(1);
        }
    }

    fn clock_timer(&mut self) {
        if self.timer_counter == 0 {
            self.timer_counter = self.timer_period;
            if self.length_counter > 0 && self.linear_counter > 0 && self.timer_period > 1 {
                self.seq_step = (self.seq_step + 1) & 0x1F;
            }
        } else {
            self.timer_counter = self.timer_counter.saturating_sub(1);
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled
            || self.length_counter == 0
            || self.linear_counter == 0
            || self.timer_period < 2
        {
            0
        } else {
            TRI_TABLE[self.seq_step as usize]
        }
    }
}

#[derive(Clone, Copy)]
struct NoiseChannel {
    enabled: bool,
    length_halt: bool,
    constant_volume: bool,
    volume: u8,
    envelope_period: u8,
    envelope_start: bool,
    envelope_divider: u8,
    envelope_decay: u8,

    mode: bool,
    timer_period: u16,
    timer_counter: u16,
    shift_register: u16,
    length_counter: u8,
}

impl NoiseChannel {
    fn new() -> Self {
        Self {
            enabled: false,
            length_halt: false,
            constant_volume: false,
            volume: 0,
            envelope_period: 0,
            envelope_start: false,
            envelope_divider: 0,
            envelope_decay: 0,
            mode: false,
            timer_period: NOISE_PERIOD_TABLE[0],
            timer_counter: 0,
            shift_register: 1,
            length_counter: 0,
        }
    }

    fn write_control(&mut self, value: u8) {
        self.length_halt = (value & 0x20) != 0;
        self.constant_volume = (value & 0x10) != 0;
        self.volume = value & 0x0F;
        self.envelope_period = value & 0x0F;
        self.envelope_start = true;
    }

    fn write_period(&mut self, value: u8) {
        self.mode = (value & 0x80) != 0;
        self.timer_period = NOISE_PERIOD_TABLE[(value & 0x0F) as usize];
    }

    fn write_length(&mut self, value: u8) {
        if self.enabled {
            self.length_counter = LENGTH_TABLE[(value >> 3) as usize];
        }
        self.envelope_start = true;
    }

    fn clock_timer(&mut self) {
        if self.timer_counter == 0 {
            self.timer_counter = self.timer_period;
            let tap = if self.mode { 6 } else { 1 };
            let feedback = (self.shift_register ^ (self.shift_register >> tap)) & 0x0001;
            self.shift_register >>= 1;
            self.shift_register |= feedback << 14;
        } else {
            self.timer_counter = self.timer_counter.saturating_sub(1);
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_start {
            self.envelope_start = false;
            self.envelope_decay = 15;
            self.envelope_divider = self.envelope_period;
            return;
        }

        if self.envelope_divider == 0 {
            self.envelope_divider = self.envelope_period;
            if self.envelope_decay == 0 {
                if self.length_halt {
                    self.envelope_decay = 15;
                }
            } else {
                self.envelope_decay = self.envelope_decay.saturating_sub(1);
            }
        } else {
            self.envelope_divider = self.envelope_divider.saturating_sub(1);
        }
    }

    fn clock_length_counter(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter = self.length_counter.saturating_sub(1);
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled || self.length_counter == 0 || (self.shift_register & 0x0001) != 0 {
            return 0;
        }
        if self.constant_volume {
            self.volume
        } else {
            self.envelope_decay
        }
    }
}

#[derive(Clone, Copy)]
struct DmcChannel {
    enabled: bool,
    irq_enabled: bool,
    irq_flag: bool,
    loop_flag: bool,
    rate_index: u8,
    timer_period: u16,
    timer_counter: u16,
    output_level: u8,
    sample_addr: u8,
    sample_length: u8,
    current_addr: u16,
    bytes_remaining: u16,
    sample_buffer: Option<u8>,
    shift_register: u8,
    bits_remaining: u8,
    silence: bool,
    dma_pending: bool,
    dma_delay: u8,
}

impl DmcChannel {
    fn new() -> Self {
        Self {
            enabled: false,
            irq_enabled: false,
            irq_flag: false,
            loop_flag: false,
            rate_index: 0,
            timer_period: DMC_RATE_TABLE[0],
            timer_counter: DMC_RATE_TABLE[0],
            output_level: 0,
            sample_addr: 0,
            sample_length: 0,
            current_addr: 0xC000,
            bytes_remaining: 0,
            sample_buffer: None,
            shift_register: 0,
            bits_remaining: 8,
            silence: true,
            dma_pending: false,
            dma_delay: 0,
        }
    }

    fn write_control(&mut self, value: u8) {
        self.irq_enabled = (value & 0x80) != 0;
        if !self.irq_enabled {
            self.irq_flag = false;
        }
        self.loop_flag = (value & 0x40) != 0;
        self.rate_index = value & 0x0F;
        self.timer_period = DMC_RATE_TABLE[self.rate_index as usize];
        if self.timer_counter == 0 || self.timer_counter > self.timer_period {
            self.timer_counter = self.timer_period;
        }
    }

    fn write_output_level(&mut self, value: u8) {
        self.output_level = value & 0x7F;
    }

    fn write_sample_addr(&mut self, value: u8) {
        self.sample_addr = value;
    }

    fn write_sample_length(&mut self, value: u8) {
        self.sample_length = value;
    }

    fn restart_sample(&mut self) {
        self.current_addr = 0xC000 | ((self.sample_addr as u16) << 6);
        self.bytes_remaining = ((self.sample_length as u16) << 4) | 0x0001;
        if self.sample_buffer.is_none() && self.bytes_remaining > 0 {
            // Load DMA after enabling playback is delayed by roughly 2 CPU cycles.
            self.schedule_dma(2);
        }
    }

    fn playback_active(&self) -> bool {
        self.bytes_remaining > 0 || self.sample_buffer.is_some()
    }

    fn needs_dma(&self) -> bool {
        self.enabled && self.dma_pending && self.dma_delay == 0
    }

    fn current_dma_addr(&self) -> u16 {
        self.current_addr
    }

    fn stop(&mut self) {
        self.bytes_remaining = 0;
        self.dma_pending = false;
        self.dma_delay = 0;
    }

    fn consume_dma_byte(&mut self, byte: u8) {
        self.dma_pending = false;
        self.dma_delay = 0;
        self.sample_buffer = Some(byte);
        if self.bytes_remaining > 0 {
            self.current_addr = if self.current_addr == 0xFFFF {
                0x8000
            } else {
                self.current_addr.wrapping_add(1)
            };
            self.bytes_remaining = self.bytes_remaining.saturating_sub(1);

            if self.bytes_remaining == 0 {
                if self.loop_flag {
                    self.restart_sample();
                } else if self.irq_enabled {
                    self.irq_flag = true;
                }
            }
        }
    }

    fn clock_output_unit(&mut self) {
        if !self.silence {
            if (self.shift_register & 0x01) != 0 {
                if self.output_level <= 125 {
                    self.output_level = self.output_level.saturating_add(2);
                }
            } else if self.output_level >= 2 {
                self.output_level = self.output_level.saturating_sub(2);
            }
        }

        self.shift_register >>= 1;
        if self.bits_remaining > 0 {
            self.bits_remaining = self.bits_remaining.saturating_sub(1);
        }

        if self.bits_remaining == 0 {
            self.bits_remaining = 8;
            if let Some(sample) = self.sample_buffer.take() {
                self.shift_register = sample;
                self.silence = false;
                if self.bytes_remaining > 0 {
                    // Reload DMA after buffer is consumed occurs one cycle later.
                    self.schedule_dma(1);
                }
            } else {
                self.silence = true;
            }
        }
    }

    fn clock_timer(&mut self) {
        if self.dma_pending && self.dma_delay > 0 {
            self.dma_delay = self.dma_delay.saturating_sub(1);
        }

        if self.timer_counter == 0 {
            self.timer_counter = self.timer_period;
        }
        self.timer_counter = self.timer_counter.saturating_sub(1);
        if self.timer_counter == 0 {
            self.clock_output_unit();
        }
    }

    fn schedule_dma(&mut self, delay: u8) {
        if self.enabled && self.sample_buffer.is_none() && self.bytes_remaining > 0 {
            self.dma_pending = true;
            self.dma_delay = delay;
        }
    }

    fn output(&self) -> u8 {
        self.output_level
    }
}
