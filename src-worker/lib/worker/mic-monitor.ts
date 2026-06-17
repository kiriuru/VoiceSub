import type { BrowserAsrState, MicrophoneMonitor } from "../asr/types";
import { MIC_ACTIVE_RMS_THRESHOLD, MIC_MONITOR_INTERVAL_MS, MIC_VOICE_RMS_THRESHOLD } from "./worker-defaults";

export async function ensureMicrophonePermission(state: BrowserAsrState, appendLog: (msg: string) => void): Promise<MediaStream> {
  const existingStream = state.microphoneMonitor?.stream || null;
  const existingTrack = state.microphoneMonitor?.track || null;
  if (existingStream && existingTrack && existingTrack.readyState === "live") {
    return existingStream;
  }
  if (state.microphoneMonitor) {
    state.mediaTrackLeakGuardCount = Number(state.mediaTrackLeakGuardCount || 0) + 1;
    appendLog("releasing stale microphone monitor before getUserMedia re-acquire");
    releaseMicrophoneMonitor(state);
  }
  appendLog("requesting microphone permission via getUserMedia");
  state.getUserMediaCount = Number(state.getUserMediaCount || 0) + 1;
  let stream: MediaStream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  } catch (error) {
    state.getUserMediaLastError = error instanceof Error ? error.message : String(error || "");
    throw error;
  }
  const [track] = stream.getAudioTracks();
  if (!track) {
    throw new Error("No audio track returned by getUserMedia.");
  }
  const AudioContextCtor = window.AudioContext || (window as Window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext || null;
  let audioContext: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let sourceNode: MediaStreamAudioSourceNode | null = null;
  let rmsBuffer: Uint8Array | null = null;
  if (AudioContextCtor) {
    try {
      audioContext = new AudioContextCtor();
      sourceNode = audioContext.createMediaStreamSource(stream);
      analyser = audioContext.createAnalyser();
      analyser.fftSize = 1024;
      analyser.smoothingTimeConstant = 0.15;
      sourceNode.connect(analyser);
      rmsBuffer = new Uint8Array(analyser.fftSize);
    } catch {
      audioContext = null;
      analyser = null;
      sourceNode = null;
      rmsBuffer = null;
    }
  }

  const updateMicHealth = () => {
    const monitor = state.microphoneMonitor;
    const monitorTrack = monitor?.track || null;
    if (!monitorTrack) {
      state.micTrackReadyState = "missing";
      state.micTrackMuted = false;
      state.micRms = 0;
      state.micHealthUpdatedAt = Date.now();
      return;
    }
    state.micTrackReadyState = String(monitorTrack.readyState || "unknown");
    state.micTrackMuted = Boolean(monitorTrack.muted);
    let micRms = 0;
    if (monitor?.analyser && monitor?.rmsBuffer) {
      try {
        monitor.analyser.getByteTimeDomainData(monitor.rmsBuffer as Uint8Array<ArrayBuffer>);
        let sum = 0;
        for (let index = 0; index < monitor.rmsBuffer.length; index += 1) {
          const centered = ((monitor.rmsBuffer[index] ?? 128) - 128) / 128;
          sum += centered * centered;
        }
        micRms = Math.sqrt(sum / monitor.rmsBuffer.length);
      } catch {
        micRms = 0;
      }
    }
    state.micRms = Number.isFinite(micRms) ? Number(micRms.toFixed(4)) : 0;
    const nowMs = Date.now();
    if (state.micRms >= MIC_ACTIVE_RMS_THRESHOLD && !state.micTrackMuted) {
      state.lastMicActivityAt = nowMs;
      state.micActiveRecentMs = 0;
    } else if (state.lastMicActivityAt > 0) {
      state.micActiveRecentMs = Math.max(0, nowMs - state.lastMicActivityAt);
    } else {
      state.micActiveRecentMs = null;
    }
    state.micHealthUpdatedAt = nowMs;
  };

  const monitor: MicrophoneMonitor = {
    stream,
    track,
    audioContext,
    analyser,
    sourceNode,
    rmsBuffer,
    intervalId: null,
  };
  state.microphoneMonitor = monitor;
  state.micStreamActive = true;
  state.getUserMediaLastError = null;
  state.micTrackReadyState = String(track.readyState || "unknown");
  state.micTrackMuted = Boolean(track.muted);
  updateMicHealth();
  monitor.intervalId = window.setInterval(updateMicHealth, MIC_MONITOR_INTERVAL_MS);
  track.addEventListener("ended", updateMicHealth);
  track.addEventListener("mute", updateMicHealth);
  track.addEventListener("unmute", updateMicHealth);
  appendLog(`microphone permission granted${track?.label ? `: ${track.label}` : ""}`);
  return stream;
}

export function releaseMicrophoneMonitor(state: BrowserAsrState): void {
  const monitor = state.microphoneMonitor;
  state.microphoneMonitor = null;
  if (!monitor) {
    state.micStreamActive = false;
    return;
  }
  if (monitor.intervalId) {
    window.clearInterval(monitor.intervalId);
  }
  try {
    monitor.sourceNode?.disconnect?.();
  } catch {
    // best effort
  }
  try {
    const tracks = monitor.stream?.getTracks?.() || [];
    state.mediaTracksStoppedCount = Number(state.mediaTracksStoppedCount || 0) + tracks.length;
    tracks.forEach((track) => {
      try {
        track.stop();
      } catch {
        // best effort
      }
    });
  } catch {
    // best effort
  }
  try {
    monitor.audioContext?.close?.();
  } catch {
    // best effort
  }
  state.micStreamActive = false;
}
