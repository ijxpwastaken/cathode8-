use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct AudioOutput {
    queue: Arc<Mutex<VecDeque<f32>>>,
    _stream: cpal::Stream,
    sample_rate: u32,
    max_queue_samples: usize,
}

impl AudioOutput {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("no default audio output device"))?;
        let supported = device
            .default_output_config()
            .context("failed to query default audio config")?;

        let stream_config: cpal::StreamConfig = supported.config();
        let sample_rate = stream_config.sample_rate.0;
        let channels = stream_config.channels as usize;
        let max_queue_samples = ((sample_rate as usize) * 96) / 1000;
        let queue = Arc::new(Mutex::new(VecDeque::<f32>::with_capacity(
            max_queue_samples,
        )));

        let err_fn = |err| {
            eprintln!("audio stream error: {err}");
        };

        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => {
                let queue = Arc::clone(&queue);
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _| fill_output_f32(data, channels, &queue),
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::I16 => {
                let queue = Arc::clone(&queue);
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [i16], _| fill_output_i16(data, channels, &queue),
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::U16 => {
                let queue = Arc::clone(&queue);
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [u16], _| fill_output_u16(data, channels, &queue),
                    err_fn,
                    None,
                )?
            }
            other => {
                return Err(anyhow!("unsupported audio sample format: {other:?}"));
            }
        };

        stream
            .play()
            .context("failed to start audio output stream")?;

        Ok(Self {
            queue,
            _stream: stream,
            sample_rate,
            max_queue_samples,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn push_samples(&self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        let Ok(mut queue) = self.queue.lock() else {
            return;
        };

        let incoming = samples.len();
        let future_len = queue.len().saturating_add(incoming);
        if future_len > self.max_queue_samples {
            let drop_count = future_len - self.max_queue_samples;
            for _ in 0..drop_count.min(queue.len()) {
                queue.pop_front();
            }
        }

        queue.extend(samples.iter().map(|s| s.clamp(-1.0, 1.0)));
    }

    pub fn queued_samples(&self) -> usize {
        if let Ok(queue) = self.queue.lock() {
            queue.len()
        } else {
            0
        }
    }
}

fn next_sample(queue: &Arc<Mutex<VecDeque<f32>>>) -> f32 {
    if let Ok(mut q) = queue.lock() {
        q.pop_front().unwrap_or(0.0)
    } else {
        0.0
    }
}

fn fill_output_f32(data: &mut [f32], channels: usize, queue: &Arc<Mutex<VecDeque<f32>>>) {
    for frame in data.chunks_mut(channels) {
        let sample = next_sample(queue);
        for out in frame {
            *out = sample;
        }
    }
}

fn fill_output_i16(data: &mut [i16], channels: usize, queue: &Arc<Mutex<VecDeque<f32>>>) {
    for frame in data.chunks_mut(channels) {
        let sample = (next_sample(queue).clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        for out in frame {
            *out = sample;
        }
    }
}

fn fill_output_u16(data: &mut [u16], channels: usize, queue: &Arc<Mutex<VecDeque<f32>>>) {
    for frame in data.chunks_mut(channels) {
        let sample = (((next_sample(queue).clamp(-1.0, 1.0) * 0.5) + 0.5) * u16::MAX as f32) as u16;
        for out in frame {
            *out = sample;
        }
    }
}
