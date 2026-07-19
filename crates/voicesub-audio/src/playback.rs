//! Native MP3 playback for TTS dual-sink channels (`speech`, `twitch`).
//!
//! - **Native @ 1.0×:** stream MP3 directly (minimal latency, fixed pitch).
//! - **Sonic:** decode → libsonic tempo → rodio @ 1.0× (pitch-preserving rate).

use std::io::Cursor;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait};
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use serde::Serialize;
use tracing::{debug, info, warn};

use crate::error::AudioError;
use crate::sonic_speed::SonicProcessor;

pub const CHANNEL_SPEECH: &str = "speech";
pub const CHANNEL_TWITCH: &str = "twitch";

pub const SPEECH_VOLUME_MAX: f32 = 1.5;

/// Clamp module/TTS playback gain (0–150%).
pub fn clamp_speech_volume(volume: f32) -> f32 {
    if !volume.is_finite() {
        return 1.0;
    }
    volume.clamp(0.0, SPEECH_VOLUME_MAX)
}

/// Apply UI volume on decoded `f32` PCM before cpal conversion.
///
/// ≤100%: linear attenuation. >100%: gentle compression + makeup gain + brick-wall limit
/// at 0 dBFS (standard limiter/input-gain pattern; see Web Audio / mastering docs).
pub fn apply_speech_volume_to_pcm(samples: &mut [f32], volume: f32) {
    let volume = clamp_speech_volume(volume);
    if samples.is_empty() {
        return;
    }

    if volume <= 1.0 {
        for sample in samples.iter_mut() {
            *sample = (*sample * volume).clamp(-1.0, 1.0);
        }
        return;
    }

    const COMPRESS_THRESHOLD: f32 = 0.70;
    const COMPRESS_RATIO: f32 = 3.0;

    for sample in samples.iter_mut() {
        let abs = sample.abs();
        if abs <= COMPRESS_THRESHOLD {
            continue;
        }
        let sign = sample.signum();
        let excess = abs - COMPRESS_THRESHOLD;
        *sample = sign * (COMPRESS_THRESHOLD + excess / COMPRESS_RATIO);
    }

    for sample in samples.iter_mut() {
        *sample *= volume;
    }

    let peak = samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max);
    if peak > 1.0 {
        let limit_gain = 1.0 / peak;
        for sample in samples.iter_mut() {
            *sample *= limit_gain;
        }
    }
}

const PLAYBACK_POLL_MS: u64 = 10;
const RATE_NEUTRAL_EPS: f32 = 0.02;
const SONIC_DRAIN_SAMPLES: i32 = 16_384;
#[derive(Debug, Clone, Serialize)]
pub struct PlaybackFinished {
    pub channel: String,
    pub item_id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

enum WorkerCommand {
    Play {
        item_id: String,
        bytes: Vec<u8>,
        volume: f32,
        rate: f32,
    },
    SetDeviceLabel(String),
    StopPending,
}

enum PlayOutcome {
    Completed,
    Stopped,
    Failed(AudioError),
}

/// Reused cpal output stream for a playback worker thread (one per channel).
struct OutputCache {
    device_label: String,
    _stream: OutputStream,
    handle: OutputStreamHandle,
}

/// Two independent playback workers (speech + twitch).
pub struct PlaybackHub {
    speech_tx: Sender<WorkerCommand>,
    twitch_tx: Sender<WorkerCommand>,
    _speech_thread: JoinHandle<()>,
    _twitch_thread: JoinHandle<()>,
}

impl PlaybackHub {
    pub fn spawn() -> (Self, Receiver<PlaybackFinished>) {
        let (completion_tx, completion_rx) = mpsc::channel();
        let (speech_tx, speech_rx) = mpsc::channel();
        let (twitch_tx, twitch_rx) = mpsc::channel();

        let speech_completion = completion_tx.clone();
        let speech_thread = thread::Builder::new()
            .name("voicesub-tts-speech".into())
            .spawn(move || channel_worker_loop(CHANNEL_SPEECH, speech_rx, speech_completion))
            .expect("spawn speech playback thread");

        let twitch_thread = thread::Builder::new()
            .name("voicesub-tts-twitch".into())
            .spawn(move || channel_worker_loop(CHANNEL_TWITCH, twitch_rx, completion_tx))
            .expect("spawn twitch playback thread");

        (
            Self {
                speech_tx,
                twitch_tx,
                _speech_thread: speech_thread,
                _twitch_thread: twitch_thread,
            },
            completion_rx,
        )
    }

    pub fn play(
        &self,
        channel: &str,
        item_id: String,
        audio_bytes: Vec<u8>,
        volume: f32,
        rate: f32,
    ) -> Result<(), AudioError> {
        let tx = self.sender_for(channel)?;
        tx.send(WorkerCommand::Play {
            item_id,
            bytes: audio_bytes,
            volume,
            rate,
        })
        .map_err(|_| AudioError::PlaybackFailed("playback worker channel closed".into()))?;
        Ok(())
    }

    pub fn set_device_label(&self, channel: &str, label: String) -> Result<(), AudioError> {
        let tx = self.sender_for(channel)?;
        tx.send(WorkerCommand::SetDeviceLabel(label))
            .map_err(|_| AudioError::PlaybackFailed("playback worker channel closed".into()))?;
        Ok(())
    }

    /// Stops the current clip (if any) and drops queued items for the channel.
    pub fn stop_channel(&self, channel: &str) -> Result<(), AudioError> {
        let tx = self.sender_for(channel)?;
        tx.send(WorkerCommand::StopPending)
            .map_err(|_| AudioError::PlaybackFailed("playback worker channel closed".into()))?;
        Ok(())
    }

    fn sender_for(&self, channel: &str) -> Result<&Sender<WorkerCommand>, AudioError> {
        match channel {
            CHANNEL_SPEECH => Ok(&self.speech_tx),
            CHANNEL_TWITCH => Ok(&self.twitch_tx),
            other => Err(AudioError::PlaybackFailed(format!(
                "unknown playback channel: {other}"
            ))),
        }
    }
}

fn channel_worker_loop(
    channel: &'static str,
    rx: Receiver<WorkerCommand>,
    completion_tx: Sender<PlaybackFinished>,
) {
    let mut device_label = String::new();
    let mut output_cache: Option<OutputCache> = None;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            WorkerCommand::SetDeviceLabel(label) => {
                debug!(
                    target: "voicesub.tts.audio",
                    channel,
                    label = label.as_str(),
                    "playback device label updated"
                );
                if device_label != label {
                    output_cache = None;
                }
                device_label = label;
            }
            WorkerCommand::StopPending => {
                drain_pending_play_commands(&rx, &mut device_label, &mut output_cache);
                debug!(target: "voicesub.tts.audio", channel, "playback pending queue drained");
            }
            WorkerCommand::Play {
                item_id,
                bytes,
                volume,
                rate,
            } => {
                let result = play_mp3_interruptible(
                    &mut device_label,
                    &mut output_cache,
                    bytes,
                    volume,
                    rate,
                    &rx,
                );
                let (ok, error) = match result {
                    PlayOutcome::Completed => (true, None),
                    PlayOutcome::Stopped => (false, Some("playback stopped".into())),
                    PlayOutcome::Failed(err) => {
                        output_cache = None;
                        warn!(
                            target: "voicesub.tts.audio",
                            channel,
                            item_id = item_id.as_str(),
                            error = %err,
                            "native playback failed"
                        );
                        (false, Some(err.to_string()))
                    }
                };
                let _ = completion_tx.send(PlaybackFinished {
                    channel: channel.to_string(),
                    item_id,
                    ok,
                    error,
                });
            }
        }
    }
    info!(target: "voicesub.tts.audio", channel, "playback worker exited");
}

fn drain_pending_play_commands(
    rx: &Receiver<WorkerCommand>,
    device_label: &mut String,
    output_cache: &mut Option<OutputCache>,
) {
    while let Ok(cmd) = rx.try_recv() {
        match cmd {
            WorkerCommand::StopPending => {}
            WorkerCommand::SetDeviceLabel(label) => {
                debug!(
                    target: "voicesub.tts.audio",
                    label = label.as_str(),
                    "playback device label updated (deferred while draining)"
                );
                if *device_label != label {
                    *output_cache = None;
                }
                *device_label = label;
            }
            WorkerCommand::Play { .. } => {
                debug!(target: "voicesub.tts.audio", "dropped queued play command");
            }
        }
    }
}

fn play_mp3_interruptible(
    device_label: &mut String,
    output_cache: &mut Option<OutputCache>,
    bytes: Vec<u8>,
    volume: f32,
    rate: f32,
    rx: &Receiver<WorkerCommand>,
) -> PlayOutcome {
    match play_mp3_blocking(device_label, output_cache, bytes, volume, rate, rx) {
        Ok(()) => PlayOutcome::Completed,
        Err(err) if err.to_string() == "playback stopped" => PlayOutcome::Stopped,
        Err(err) => PlayOutcome::Failed(err),
    }
}

fn ensure_output_cache<'a>(
    device_label: &str,
    output_cache: &'a mut Option<OutputCache>,
) -> Result<&'a OutputStreamHandle, AudioError> {
    let needs_open = output_cache
        .as_ref()
        .map(|cache| cache.device_label != device_label)
        .unwrap_or(true);
    if needs_open {
        let device = resolve_output_device(device_label)?;
        let (stream, handle) = OutputStream::try_from_device(&device)
            .map_err(|e| AudioError::PlaybackFailed(e.to_string()))?;
        *output_cache = Some(OutputCache {
            device_label: device_label.to_string(),
            _stream: stream,
            handle,
        });
        debug!(
            target: "voicesub.tts.audio",
            label = device_label,
            "opened reusable output stream"
        );
    }
    Ok(&output_cache
        .as_ref()
        .expect("output cache populated")
        .handle)
}

fn play_mp3_blocking(
    device_label: &mut String,
    output_cache: &mut Option<OutputCache>,
    bytes: Vec<u8>,
    volume: f32,
    rate: f32,
    rx: &Receiver<WorkerCommand>,
) -> Result<(), AudioError> {
    if bytes.is_empty() {
        return Err(AudioError::PlaybackFailed("empty audio buffer".into()));
    }
    let volume = clamp_speech_volume(volume);
    let rate = rate.clamp(0.5, 2.0);

    let stream_handle = ensure_output_cache(device_label.as_str(), output_cache)?;
    let sink =
        Sink::try_new(stream_handle).map_err(|e| AudioError::PlaybackFailed(e.to_string()))?;

    if (rate - 1.0).abs() <= RATE_NEUTRAL_EPS {
        let (mut pcm, sample_rate, channels) = decode_mp3_to_pcm(bytes)?;
        apply_speech_volume_to_pcm(&mut pcm, volume);
        sink.append(SamplesBuffer::new(channels, sample_rate, pcm));
    } else {
        let (mut pcm, sample_rate, channels) = decode_mp3_to_pcm(bytes)?;
        apply_speech_volume_to_pcm(&mut pcm, volume);
        play_pcm_sonic_streaming(SonicStreamContext {
            sink: &sink,
            pcm,
            sample_rate,
            channels,
            rate,
            rx,
            device_label,
            output_cache,
        })?;
    }

    let poll = Duration::from_millis(PLAYBACK_POLL_MS);
    loop {
        if sink.empty() {
            return Ok(());
        }
        match rx.recv_timeout(poll) {
            Ok(WorkerCommand::StopPending) => {
                sink.stop();
                drain_pending_play_commands(rx, device_label, output_cache);
                return Err(AudioError::PlaybackFailed("playback stopped".into()));
            }
            Ok(WorkerCommand::SetDeviceLabel(label)) => {
                if *device_label != label {
                    *output_cache = None;
                }
                *device_label = label;
            }
            Ok(WorkerCommand::Play { .. }) => {
                debug!(target: "voicesub.tts.audio", "queued play while busy — dropped");
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                sink.stop();
                return Err(AudioError::PlaybackFailed("playback stopped".into()));
            }
        }
    }
}

fn poll_worker_control(
    rx: &Receiver<WorkerCommand>,
    device_label: &mut String,
    output_cache: &mut Option<OutputCache>,
) -> Result<(), AudioError> {
    match rx.try_recv() {
        Ok(WorkerCommand::StopPending) => {
            drain_pending_play_commands(rx, device_label, output_cache);
            Err(AudioError::PlaybackFailed("playback stopped".into()))
        }
        Ok(WorkerCommand::SetDeviceLabel(label)) => {
            if *device_label != label {
                *output_cache = None;
            }
            *device_label = label;
            Ok(())
        }
        Ok(WorkerCommand::Play { .. }) => Ok(()),
        Err(TryRecvError::Empty) => Ok(()),
        Err(TryRecvError::Disconnected) => {
            Err(AudioError::PlaybackFailed("playback stopped".into()))
        }
    }
}

fn append_sonic_pcm_to_sink(sink: &Sink, pcm: Vec<f32>, channels: u16, sample_rate: u32) {
    if pcm.is_empty() {
        return;
    }
    sink.append(SamplesBuffer::new(channels, sample_rate, pcm));
}

fn drain_sonic_to_sink(
    sonic: &mut SonicProcessor,
    sink: &Sink,
    channels: u16,
    sample_rate: u32,
) -> Result<(), AudioError> {
    loop {
        let chunk = sonic.drain_up_to(SONIC_DRAIN_SAMPLES)?;
        if chunk.is_empty() {
            break;
        }
        append_sonic_pcm_to_sink(sink, chunk, channels, sample_rate);
    }
    Ok(())
}

struct SonicStreamContext<'a> {
    sink: &'a Sink,
    pcm: Vec<f32>,
    sample_rate: u32,
    channels: u16,
    rate: f32,
    rx: &'a Receiver<WorkerCommand>,
    device_label: &'a mut String,
    output_cache: &'a mut Option<OutputCache>,
}

/// Tempo-stretch decoded PCM via libsonic and append output to sink incrementally.
fn play_pcm_sonic_streaming(ctx: SonicStreamContext<'_>) -> Result<(), AudioError> {
    poll_worker_control(ctx.rx, ctx.device_label, ctx.output_cache)?;
    let mut sonic = SonicProcessor::new(ctx.sample_rate, ctx.channels, ctx.rate)?;
    sonic.write(&ctx.pcm)?;
    drop(ctx.pcm);
    sonic.flush()?;
    drain_sonic_to_sink(&mut sonic, ctx.sink, ctx.channels, ctx.sample_rate)?;
    if ctx.sink.empty() {
        return Err(AudioError::PlaybackFailed(
            "sonic produced no output".into(),
        ));
    }
    Ok(())
}

/// Decode MP3 bytes to interleaved `f32` PCM (rodio/minimp3).
fn decode_mp3_to_pcm(bytes: Vec<u8>) -> Result<(Vec<f32>, u32, u16), AudioError> {
    let decoder = open_mp3_decoder(bytes)?;
    let sample_rate = decoder.sample_rate();
    let channels = decoder.channels();
    let samples: Vec<f32> = decoder.convert_samples().collect();
    if samples.is_empty() {
        return Err(AudioError::PlaybackFailed("decoded audio is empty".into()));
    }
    Ok((samples, sample_rate, channels))
}

/// Validate MP3 bytes and construct a decoder (no audio output).
pub fn open_mp3_decoder(bytes: Vec<u8>) -> Result<Decoder<Cursor<Vec<u8>>>, AudioError> {
    if bytes.is_empty() {
        return Err(AudioError::PlaybackFailed("empty audio buffer".into()));
    }
    Decoder::new(Cursor::new(bytes))
        .map_err(|e| AudioError::PlaybackFailed(format!("mp3 decode failed: {e}")))
}

/// Resolve a cpal output device from a user-visible label (empty = system default).
pub fn resolve_output_device(label: &str) -> Result<cpal::Device, AudioError> {
    let host = cpal::default_host();
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return host
            .default_output_device()
            .ok_or_else(|| AudioError::DeviceNotFound("default output device".into()));
    }

    let devices: Vec<cpal::Device> = host
        .output_devices()
        .map_err(|e| AudioError::PlaybackFailed(e.to_string()))?
        .collect();

    let needle = trimmed.to_lowercase();
    for device in &devices {
        if let Ok(name) = device.name()
            && name.eq_ignore_ascii_case(trimmed)
        {
            return Ok(device.clone());
        }
    }
    for device in &devices {
        if let Ok(name) = device.name() {
            let hay = name.to_lowercase();
            if hay.contains(&needle) || needle.contains(&hay) {
                return Ok(device.clone());
            }
        }
    }

    if let Ok(list) = crate::list_output_devices() {
        for entry in list {
            if entry.is_default {
                continue;
            }
            let entry_label = entry.label.to_lowercase();
            if !entry_label.contains(&needle) && !needle.contains(&entry_label) {
                continue;
            }
            for device in &devices {
                if let Ok(name) = device.name()
                    && names_match(&name, &entry.label)
                {
                    return Ok(device.clone());
                }
            }
        }
    }

    warn!(
        target: "voicesub.tts.audio",
        label = trimmed,
        "output device label not found; falling back to default"
    );
    host.default_output_device()
        .ok_or_else(|| AudioError::DeviceNotFound(trimmed.to_string()))
}

fn names_match(cpal_name: &str, list_label: &str) -> bool {
    let a = cpal_name.to_lowercase();
    let b = list_label.to_lowercase();
    a == b || a.contains(&b) || b.contains(&a)
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::{
        SONIC_DRAIN_SAMPLES, decode_mp3_to_pcm, names_match, open_mp3_decoder, play_mp3_blocking,
        resolve_output_device,
    };
    use crate::sonic_speed::{SonicProcessor, change_speech_speed};

    #[test]
    fn names_match_partial() {
        assert!(names_match(
            "Speakers (Realtek Audio)",
            "Speakers (Realtek Audio) (default)"
        ));
    }

    #[test]
    fn resolve_default_when_label_empty() {
        let host = cpal::default_host();
        let result = resolve_output_device("");
        if host.default_output_device().is_some() {
            result.expect("default output device");
        } else {
            assert!(matches!(result, Err(AudioError::DeviceNotFound(_))));
        }
    }

    #[test]
    fn rejects_empty_audio_buffer() {
        let (tx, rx) = mpsc::channel();
        let mut label = String::new();
        let mut cache = None;
        let result = play_mp3_blocking(&mut label, &mut cache, vec![], 1.0, 1.0, &rx);
        assert!(result.is_err());
        match result {
            Err(err) => assert!(err.to_string().contains("empty audio buffer")),
            Ok(()) => panic!("expected empty audio buffer error"),
        }
        drop(tx);
    }

    #[test]
    fn open_mp3_decoder_rejects_empty() {
        let result = open_mp3_decoder(vec![]);
        assert!(result.is_err());
        match result {
            Err(err) => assert!(err.to_string().contains("empty audio buffer")),
            Ok(_) => panic!("expected empty audio buffer error"),
        }
    }

    #[test]
    fn decodes_minimal_mp3_fixture() {
        const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/minimal.mp3");
        open_mp3_decoder(FIXTURE.to_vec()).expect("minimal mp3 fixture should decode");
    }

    #[test]
    fn decode_mp3_to_pcm_from_fixture() {
        const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/minimal.mp3");
        let (pcm, sample_rate, channels) =
            decode_mp3_to_pcm(FIXTURE.to_vec()).expect("decode fixture");
        assert!(!pcm.is_empty());
        assert!(sample_rate > 0);
        assert!(channels > 0);
    }

    #[test]
    fn streaming_drain_matches_batch_output() {
        const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/minimal.mp3");
        let (pcm, sample_rate, channels) =
            decode_mp3_to_pcm(FIXTURE.to_vec()).expect("decode fixture");
        let batch = change_speech_speed(&pcm, sample_rate, channels, 1.5).expect("batch");

        let mut sonic = SonicProcessor::new(sample_rate, channels, 1.5).expect("sonic");
        sonic.write(&pcm).expect("write");
        sonic.flush().expect("flush");
        let mut streamed = Vec::new();
        loop {
            let piece = sonic.drain_up_to(SONIC_DRAIN_SAMPLES).expect("drain");
            if piece.is_empty() {
                break;
            }
            streamed.extend(piece);
        }

        assert_eq!(batch.len(), streamed.len());
        for (a, b) in batch.iter().zip(streamed.iter()) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn rejects_corrupt_mp3_bytes() {
        let result = open_mp3_decoder(vec![0xFF, 0xFB, 0x00, 0x00]);
        assert!(result.is_err());
        match result {
            Err(err) => assert!(err.to_string().contains("mp3 decode failed")),
            Ok(_) => panic!("expected mp3 decode error"),
        }
    }

    #[test]
    fn apply_speech_volume_attenuates_below_unity() {
        let mut samples = vec![0.5, -0.5];
        apply_speech_volume_to_pcm(&mut samples, 0.5);
        assert!((samples[0] - 0.25).abs() < 1e-6);
        assert!((samples[1] + 0.25).abs() < 1e-6);
    }

    #[test]
    fn apply_speech_volume_boosts_quiet_pcm_above_unity() {
        let baseline = vec![0.2, -0.2, 0.1];
        let mut at_100 = baseline.clone();
        apply_speech_volume_to_pcm(&mut at_100, 1.0);
        let peak_100 = at_100.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);

        let mut at_150 = baseline;
        apply_speech_volume_to_pcm(&mut at_150, 1.5);
        let peak_150 = at_150.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);

        assert!(peak_150 > peak_100);
    }

    #[test]
    fn apply_speech_volume_one_fifty_is_louder_than_one_on_hot_signal() {
        let sine: Vec<f32> = (0..4410)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let peak = sine.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        let baseline: Vec<f32> = sine.iter().map(|s| s * (0.95 / peak)).collect();

        let mut at_100 = baseline.clone();
        apply_speech_volume_to_pcm(&mut at_100, 1.0);
        let rms_100 = at_100.iter().map(|s| s * s).sum::<f32>() / at_100.len() as f32;

        let mut at_150 = baseline;
        apply_speech_volume_to_pcm(&mut at_150, 1.5);
        let rms_150 = at_150.iter().map(|s| s * s).sum::<f32>() / at_150.len() as f32;

        assert!(rms_150 > rms_100 * 1.05);
    }
}
