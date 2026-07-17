use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use serde::Serialize;
use thiserror::Error;
use tracing::warn;

pub const PARAKEET_SAMPLE_RATE: u32 = 16_000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InputDeviceInfo {
    pub id: String,
    pub label: String,
    pub is_default: bool,
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("no input device available")]
    NoInputDevice,
    #[error("input device not found: {0}")]
    DeviceNotFound(String),
    #[error("unsupported sample format: {0:?}")]
    UnsupportedFormat(SampleFormat),
    #[error("cpal error: {0}")]
    Cpal(String),
    #[error("no audio samples captured")]
    EmptyCapture,
}

pub fn list_input_devices() -> Result<Vec<InputDeviceInfo>, CaptureError> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|device| device.name().ok());

    let mut devices = vec![InputDeviceInfo {
        id: String::new(),
        label: "Default".into(),
        is_default: true,
    }];

    let inputs = host
        .input_devices()
        .map_err(|e| CaptureError::Cpal(e.to_string()))?;
    for device in inputs {
        let name = device
            .name()
            .map_err(|e| CaptureError::Cpal(e.to_string()))?;
        if devices.iter().any(|entry| entry.id == name) {
            continue;
        }
        devices.push(InputDeviceInfo {
            id: name.clone(),
            label: name.clone(),
            is_default: default_name.as_ref().is_some_and(|default| default == &name),
        });
    }
    Ok(devices)
}

pub fn resolve_input_device(device_id: &str) -> Result<cpal::Device, CaptureError> {
    let host = cpal::default_host();
    let trimmed = device_id.trim();
    if trimmed.is_empty() {
        return host
            .default_input_device()
            .ok_or(CaptureError::NoInputDevice);
    }

    let inputs: Vec<cpal::Device> = host
        .input_devices()
        .map_err(|e| CaptureError::Cpal(e.to_string()))?
        .collect();

    let needle = trimmed.to_lowercase();
    for device in &inputs {
        if let Ok(name) = device.name()
            && name.eq_ignore_ascii_case(trimmed)
        {
            return Ok(device.clone());
        }
    }
    for device in &inputs {
        if let Ok(name) = device.name() {
            let hay = name.to_lowercase();
            if hay.contains(&needle) || needle.contains(&hay) {
                return Ok(device.clone());
            }
        }
    }

    warn!(
        target: "voicesub.asr_local.capture",
        device_id = trimmed,
        "input device not found — falling back to default"
    );
    host.default_input_device()
        .ok_or_else(|| CaptureError::DeviceNotFound(trimmed.to_string()))
}

pub fn record_input(
    duration_ms: u64,
    device_id: &str,
) -> Result<(Vec<f32>, u32, String), CaptureError> {
    let device = resolve_input_device(device_id)?;
    let device_label = device
        .name()
        .unwrap_or_else(|_| if device_id.trim().is_empty() {
            "Default".into()
        } else {
            device_id.to_string()
        });
    let config = device
        .default_input_config()
        .map_err(|e| CaptureError::Cpal(e.to_string()))?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let stream_config: cpal::StreamConfig = config.clone().into();

    let buffer = Arc::new(parking_lot::Mutex::new(Vec::<f32>::new()));
    let writer = Arc::clone(&buffer);

    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _| writer.lock().extend_from_slice(data),
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _| {
                writer
                    .lock()
                    .extend(data.iter().map(|sample| *sample as f32 / i16::MAX as f32));
            },
            err_fn,
            None,
        ),
        SampleFormat::I32 => device.build_input_stream(
            &stream_config,
            move |data: &[i32], _| {
                writer
                    .lock()
                    .extend(data.iter().map(|sample| *sample as f32 / i32::MAX as f32));
            },
            err_fn,
            None,
        ),
        other => return Err(CaptureError::UnsupportedFormat(other)),
    }
    .map_err(|e| CaptureError::Cpal(e.to_string()))?;

    stream
        .play()
        .map_err(|e| CaptureError::Cpal(e.to_string()))?;
    std::thread::sleep(Duration::from_millis(duration_ms.max(250)));
    drop(stream);

    let interleaved = buffer.lock().clone();
    if interleaved.is_empty() {
        return Err(CaptureError::EmptyCapture);
    }
    let mono = downmix_to_mono(interleaved, channels);
    let (samples, sample_rate) = if sample_rate == PARAKEET_SAMPLE_RATE {
        (mono, sample_rate)
    } else {
        (
            resample_linear(&mono, sample_rate, PARAKEET_SAMPLE_RATE),
            PARAKEET_SAMPLE_RATE,
        )
    };
    Ok((samples, sample_rate, device_label))
}

/// Live microphone stream — pushes resampled 16 kHz mono chunks to the receiver.
pub struct MicStream {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl MicStream {
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for MicStream {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn start_mic_stream(
    device_id: &str,
) -> Result<(MicStream, Receiver<Vec<f32>>, u32, String), CaptureError> {
    let device = resolve_input_device(device_id)?;
    let device_label = device
        .name()
        .unwrap_or_else(|_| if device_id.trim().is_empty() {
            "Default".into()
        } else {
            device_id.to_string()
        });
    let config = device
        .default_input_config()
        .map_err(|e| CaptureError::Cpal(e.to_string()))?;
    let input_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let stream_config: cpal::StreamConfig = config.clone().into();

    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let device_for_thread = device;

    let worker = thread::spawn(move || {
        let pending = Arc::new(parking_lot::Mutex::new(Vec::<f32>::new()));
        let writer = Arc::clone(&pending);
        let sender = tx;

        let stream = match config.sample_format() {
            SampleFormat::F32 => device_for_thread.build_input_stream(
                &stream_config,
                move |data: &[f32], _| writer.lock().extend_from_slice(data),
                err_fn,
                None,
            ),
            SampleFormat::I16 => device_for_thread.build_input_stream(
                &stream_config,
                move |data: &[i16], _| {
                    writer.lock().extend(
                        data.iter()
                            .map(|sample| *sample as f32 / i16::MAX as f32),
                    );
                },
                err_fn,
                None,
            ),
            SampleFormat::I32 => device_for_thread.build_input_stream(
                &stream_config,
                move |data: &[i32], _| {
                    writer.lock().extend(
                        data.iter()
                            .map(|sample| *sample as f32 / i32::MAX as f32),
                    );
                },
                err_fn,
                None,
            ),
            other => {
                let _ = sender.send(Vec::new());
                warn!(
                    target: "voicesub.asr_local.capture",
                    ?other,
                    "unsupported mic format for streaming"
                );
                return;
            }
        };

        let stream = match stream {
            Ok(stream) => stream,
            Err(err) => {
                warn!(
                    target: "voicesub.asr_local.capture",
                    error = %err,
                    "failed to open mic stream"
                );
                return;
            }
        };

        if let Err(err) = stream.play() {
            warn!(
                target: "voicesub.asr_local.capture",
                error = %err,
                "failed to start mic stream"
            );
            return;
        }

        while !stop_flag.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(40));
            let chunk = {
                let mut guard = pending.lock();
                if guard.is_empty() {
                    continue;
                }
                let taken = guard.clone();
                guard.clear();
                taken
            };
            let mono = downmix_to_mono(chunk, channels);
            let samples = if input_rate == PARAKEET_SAMPLE_RATE {
                mono
            } else {
                resample_linear(&mono, input_rate, PARAKEET_SAMPLE_RATE)
            };
            if !samples.is_empty() && sender.send(samples).is_err() {
                break;
            }
        }
        drop(stream);
    });

    Ok((
        MicStream {
            stop,
            worker: Some(worker),
        },
        rx,
        PARAKEET_SAMPLE_RATE,
        device_label,
    ))
}

fn downmix_to_mono(samples: Vec<f32>, channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples;
    }
    samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Linear resampling — adequate for ASR; matches Higgs-Ultimate parakeet path.
fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || input.is_empty() {
        return input.to_vec();
    }
    let ratio = to_rate as f64 / from_rate as f64;
    let out_len = ((input.len() as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let idx = src.floor() as usize;
        let frac = (src - idx as f64) as f32;
        let a = input[idx.min(input.len() - 1)];
        let b = input[(idx + 1).min(input.len() - 1)];
        out.push(a + (b - a) * frac);
    }
    out
}

fn err_fn(err: cpal::StreamError) {
    warn!(target: "voicesub.asr_local.capture", error = %err, "microphone stream error");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_linear_downsamples_48k_to_16k() {
        let input: Vec<f32> = (0..48_000).map(|i| (i as f32).sin()).collect();
        let out = resample_linear(&input, 48_000, PARAKEET_SAMPLE_RATE);
        assert_eq!(out.len(), 16_000);
    }

    #[test]
    fn resample_linear_is_noop_at_target_rate() {
        let input = vec![0.25, -0.5, 0.75];
        let out = resample_linear(&input, PARAKEET_SAMPLE_RATE, PARAKEET_SAMPLE_RATE);
        assert_eq!(out, input);
    }

    #[test]
    fn list_input_devices_includes_default_row() {
        let devices = list_input_devices().unwrap_or_default();
        assert!(
            devices.first().is_some_and(|entry| entry.is_default && entry.id.is_empty()),
            "expected synthetic default row"
        );
    }
}
