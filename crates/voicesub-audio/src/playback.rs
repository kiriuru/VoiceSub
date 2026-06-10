//! Native MP3 playback for TTS dual-sink channels (`speech`, `twitch`).

use std::io::Cursor;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait};
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, Sink, Source};
use serde::Serialize;
use tracing::{debug, info, warn};

use crate::error::AudioError;
use crate::sonic_speed::change_speech_speed;

pub const CHANNEL_SPEECH: &str = "speech";
pub const CHANNEL_TWITCH: &str = "twitch";

const PLAYBACK_POLL_MS: u64 = 50;
const RATE_NEUTRAL_EPS: f32 = 0.02;

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
    while let Ok(cmd) = rx.recv() {
        match cmd {
            WorkerCommand::SetDeviceLabel(label) => {
                debug!(
                    target: "voicesub.tts.audio",
                    channel,
                    label = label.as_str(),
                    "playback device label updated"
                );
                device_label = label;
            }
            WorkerCommand::StopPending => {
                drain_pending_play_commands(&rx, &mut device_label);
                debug!(target: "voicesub.tts.audio", channel, "playback pending queue drained");
            }
            WorkerCommand::Play {
                item_id,
                bytes,
                volume,
                rate,
            } => {
                let result =
                    play_mp3_interruptible(&mut device_label, &bytes, volume, rate, &rx);
                let (ok, error) = match result {
                    PlayOutcome::Completed => (true, None),
                    PlayOutcome::Stopped => (false, Some("playback stopped".into())),
                    PlayOutcome::Failed(err) => {
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

fn drain_pending_play_commands(rx: &Receiver<WorkerCommand>, device_label: &mut String) {
    while let Ok(cmd) = rx.try_recv() {
        match cmd {
            WorkerCommand::StopPending => {}
            WorkerCommand::SetDeviceLabel(label) => {
                debug!(
                    target: "voicesub.tts.audio",
                    label = label.as_str(),
                    "playback device label updated (deferred while draining)"
                );
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
    bytes: &[u8],
    volume: f32,
    rate: f32,
    rx: &Receiver<WorkerCommand>,
) -> PlayOutcome {
    match play_mp3_blocking(device_label, bytes, volume, rate, rx) {
        Ok(()) => PlayOutcome::Completed,
        Err(err) if err.to_string() == "playback stopped" => PlayOutcome::Stopped,
        Err(err) => PlayOutcome::Failed(err),
    }
}

fn decode_mp3_to_pcm(bytes: &[u8]) -> Result<(Vec<f32>, u32, u16), AudioError> {
    let decoder = open_mp3_decoder(bytes)?;
    let sample_rate = decoder.sample_rate();
    let channels = decoder.channels();
    let pcm: Vec<f32> = decoder.convert_samples().collect();
    if pcm.is_empty() {
        return Err(AudioError::PlaybackFailed("decoded audio is empty".into()));
    }
    Ok((pcm, sample_rate, channels))
}

fn play_mp3_blocking(
    device_label: &mut String,
    bytes: &[u8],
    volume: f32,
    rate: f32,
    rx: &Receiver<WorkerCommand>,
) -> Result<(), AudioError> {
    if bytes.is_empty() {
        return Err(AudioError::PlaybackFailed("empty audio buffer".into()));
    }
    let rate = rate.clamp(0.5, 2.0);
    let volume = volume.clamp(0.0, 1.0);

    let device = resolve_output_device(device_label.as_str())?;
    let (_stream, stream_handle) = OutputStream::try_from_device(&device)
        .map_err(|e| AudioError::PlaybackFailed(e.to_string()))?;
    let sink =
        Sink::try_new(&stream_handle).map_err(|e| AudioError::PlaybackFailed(e.to_string()))?;

    if (rate - 1.0).abs() <= RATE_NEUTRAL_EPS {
        let decoder = open_mp3_decoder(bytes)?;
        sink.append(decoder.amplify(volume));
    } else {
        let (pcm, sample_rate, channels) = decode_mp3_to_pcm(bytes)?;
        let play_pcm = change_speech_speed(&pcm, sample_rate, channels, rate)?;
        let source = SamplesBuffer::new(channels, sample_rate, play_pcm);
        sink.append(source.amplify(volume));
    }

    let poll = Duration::from_millis(PLAYBACK_POLL_MS);
    loop {
        if sink.empty() {
            return Ok(());
        }
        match rx.recv_timeout(poll) {
            Ok(WorkerCommand::StopPending) => {
                sink.stop();
                drain_pending_play_commands(rx, device_label);
                return Err(AudioError::PlaybackFailed("playback stopped".into()));
            }
            Ok(WorkerCommand::SetDeviceLabel(label)) => {
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

/// Validate MP3 bytes and construct a decoder (no audio output).
pub fn open_mp3_decoder(bytes: &[u8]) -> Result<Decoder<Cursor<Vec<u8>>>, AudioError> {
    if bytes.is_empty() {
        return Err(AudioError::PlaybackFailed("empty audio buffer".into()));
    }
    Decoder::new(Cursor::new(bytes.to_vec()))
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
        if let Ok(name) = device.name() {
            if name.eq_ignore_ascii_case(trimmed) {
                return Ok(device.clone());
            }
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
                if let Ok(name) = device.name() {
                    if names_match(&name, &entry.label) {
                        return Ok(device.clone());
                    }
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

    #[test]
    fn names_match_partial() {
        assert!(names_match(
            "Speakers (Realtek Audio)",
            "Speakers (Realtek Audio) (default)"
        ));
    }

    #[test]
    fn resolve_default_when_label_empty() {
        let _ = resolve_output_device("").expect("default output device");
    }

    #[test]
    fn rejects_empty_audio_buffer() {
        let (tx, rx) = mpsc::channel();
        let mut label = String::new();
        let result = play_mp3_blocking(&mut label, &[], 1.0, 1.0, &rx);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .expect("error")
                .to_string()
                .contains("empty audio buffer")
        );
        drop(tx);
    }

    #[test]
    fn open_mp3_decoder_rejects_empty() {
        let result = open_mp3_decoder(&[]);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .expect("error")
                .to_string()
                .contains("empty audio buffer")
        );
    }

    #[test]
    fn decodes_minimal_mp3_fixture() {
        const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/minimal.mp3");
        open_mp3_decoder(FIXTURE).expect("minimal mp3 fixture should decode");
    }

    #[test]
    fn rejects_corrupt_mp3_bytes() {
        let result = open_mp3_decoder(&[0xFF, 0xFB, 0x00, 0x00]);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .expect("error")
                .to_string()
                .contains("mp3 decode failed")
        );
    }

    #[test]
    fn sonic_shortens_for_faster_rate() {
        let input: Vec<f32> = (0..4410)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let output = change_speech_speed(&input, 44100, 1, 1.5).expect("sonic");
        assert!(output.len() < input.len());
    }

    #[test]
    fn sonic_lengthens_for_slower_rate() {
        let input: Vec<f32> = (0..4410)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let output = change_speech_speed(&input, 44100, 1, 0.75).expect("sonic");
        assert!(output.len() > input.len());
    }
}
