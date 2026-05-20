from __future__ import annotations

from collections import deque
from dataclasses import dataclass
import math
from typing import Literal

import numpy as np
import webrtcvad


@dataclass
class VadSegment:
    kind: Literal["partial", "final"]
    audio: bytes
    duration_ms: int
    voiced_ratio: float = 1.0
    average_rms: float = 0.0


class VadEngine:
    """
    WebRTC VAD + energy pre-gate + light noise adaptation (median + EMA floor).

    Best-practice additions over raw frame VAD:
    - **Speech attack**: require N consecutive "admitted speech" frames before starting
      a segment (reduces impulsive false starts).
    - **Pre-roll ring buffer**: retain a short tail of pre-speech frames so plosives
      and word onsets are less likely to be clipped for the ASR path.
    - **EMA noise tracking**: complements the median ambient buffer for faster response
      to slowly rising room noise when computing the adaptive pre-partial RMS floor.
    """

    def __init__(
        self,
        *,
        sample_rate: int = 16000,
        frame_duration_ms: int = 30,
        mode: int = 2,
        silence_hold_ms: int = 300,
        finalization_hold_ms: int | None = None,
        min_speech_ms: int = 0,
        partial_emit_interval_ms: int = 450,
        max_segment_ms: int = 6000,
        energy_gate_enabled: bool = True,
        min_rms_for_recognition: float = 0.0018,
        min_voiced_ratio: float = 0.45,
        first_partial_min_speech_ms: int = 300,
        speech_attack_frames: int = 2,
        speech_preroll_frames: int = 5,
    ) -> None:
        self.sample_rate = sample_rate
        self.frame_duration_ms = frame_duration_ms
        self.frame_bytes = int(sample_rate * frame_duration_ms / 1000) * 2
        self.vad_mode = max(0, min(3, int(mode)))
        self.vad = webrtcvad.Vad(self.vad_mode)
        self._pending_audio: bytearray = bytearray()
        self._speech_frames: list[bytes] = []
        self._speech_rms_values: list[float] = []
        self._ambient_rms_values: deque[float] = deque(maxlen=64)
        self._silence_frames = 0
        self._last_partial_frame_count = 0
        self._segment_total_frames = 0
        self._segment_voiced_frames = 0
        self._segment_dropped_count = 0
        self._ambient_rms_ema = 0.0
        self._preroll: deque[tuple[bytes, float]] = deque()
        self._pending_attack: list[tuple[bytes, float]] = []
        self.configure(
            mode=self.vad_mode,
            silence_hold_ms=silence_hold_ms,
            finalization_hold_ms=finalization_hold_ms if finalization_hold_ms is not None else silence_hold_ms,
            min_speech_ms=min_speech_ms,
            partial_emit_interval_ms=partial_emit_interval_ms,
            max_segment_ms=max_segment_ms,
            energy_gate_enabled=energy_gate_enabled,
            min_rms_for_recognition=min_rms_for_recognition,
            min_voiced_ratio=min_voiced_ratio,
            first_partial_min_speech_ms=first_partial_min_speech_ms,
            speech_attack_frames=speech_attack_frames,
            speech_preroll_frames=speech_preroll_frames,
        )

    def configure(
        self,
        *,
        mode: int | None = None,
        silence_hold_ms: int,
        finalization_hold_ms: int,
        min_speech_ms: int,
        partial_emit_interval_ms: int,
        max_segment_ms: int,
        energy_gate_enabled: bool,
        min_rms_for_recognition: float,
        min_voiced_ratio: float,
        first_partial_min_speech_ms: int,
        speech_attack_frames: int = 2,
        speech_preroll_frames: int = 5,
    ) -> None:
        if mode is not None:
            self.vad_mode = max(0, min(3, int(mode)))
            self.vad.set_mode(self.vad_mode)
        self.silence_hold_ms = max(self.frame_duration_ms, int(silence_hold_ms))
        self.finalization_hold_ms = max(self.silence_hold_ms, int(finalization_hold_ms))
        self.min_speech_ms = max(0, int(min_speech_ms))
        self.partial_emit_interval_ms = max(self.frame_duration_ms, int(partial_emit_interval_ms))
        self.max_segment_ms = max(self.frame_duration_ms, int(max_segment_ms))
        self.energy_gate_enabled = bool(energy_gate_enabled)
        self.min_rms_for_recognition = max(0.0, float(min_rms_for_recognition))
        self.min_voiced_ratio = max(0.0, min(1.0, float(min_voiced_ratio)))
        self.first_partial_min_speech_ms = max(self.min_speech_ms, int(first_partial_min_speech_ms))
        self.speech_attack_frames = max(1, int(speech_attack_frames))
        pr_n = max(0, int(speech_preroll_frames))
        self.speech_preroll_frames = pr_n
        if pr_n > 0:
            kept = list(self._preroll)[-pr_n:]
            self._preroll = deque(kept, maxlen=pr_n)
        else:
            self._preroll = deque()
        # Use ceil so effective timing never resolves *shorter* than requested.
        self.silence_hold_frames = max(1, int(math.ceil(self.silence_hold_ms / self.frame_duration_ms)))
        self.finalization_hold_frames = max(1, int(math.ceil(self.finalization_hold_ms / self.frame_duration_ms)))
        self.min_speech_frames = max(0, int(math.ceil(self.min_speech_ms / self.frame_duration_ms)))
        self.first_partial_min_speech_frames = max(0, int(math.ceil(self.first_partial_min_speech_ms / self.frame_duration_ms)))
        self.partial_interval_frames = max(1, int(math.ceil(self.partial_emit_interval_ms / self.frame_duration_ms)))
        self.max_segment_frames = max(1, int(math.ceil(self.max_segment_ms / self.frame_duration_ms)))

    def reset(self) -> None:
        self._pending_audio = bytearray()
        self._speech_frames = []
        self._speech_rms_values = []
        self._silence_frames = 0
        self._last_partial_frame_count = 0
        self._segment_total_frames = 0
        self._segment_voiced_frames = 0
        self._pending_attack.clear()
        self._preroll.clear()
        # Keep dropped counter cumulative across segments for diagnostics.
        # Ambient deque + EMA carry over for smoother adaptation across segments.

    def _duration_ms(self, frame_count: int) -> int:
        return frame_count * self.frame_duration_ms

    def _frame_rms(self, frame: bytes) -> float:
        samples = np.frombuffer(frame, dtype=np.int16).astype(np.float32)
        if samples.size == 0:
            return 0.0
        normalized = samples / 32768.0
        return float(np.sqrt(np.mean(np.square(normalized)) + 1e-12))

    def _segment_voiced_ratio(self) -> float:
        if self._segment_total_frames <= 0:
            return 0.0
        return float(self._segment_voiced_frames) / float(self._segment_total_frames)

    def _segment_average_rms(self) -> float:
        if not self._speech_rms_values:
            return 0.0
        return float(sum(self._speech_rms_values) / len(self._speech_rms_values))

    def _remember_ambient_rms(self, frame_rms: float) -> None:
        if frame_rms <= 0.0:
            return
        value = float(frame_rms)
        self._ambient_rms_values.append(value)
        if self._ambient_rms_ema <= 0.0:
            self._ambient_rms_ema = value
        else:
            # Slightly faster than median-only tracking so slowly rising noise lifts the floor.
            self._ambient_rms_ema = 0.88 * self._ambient_rms_ema + 0.12 * value

    def _ambient_rms_floor(self) -> float:
        median = 0.0
        if self._ambient_rms_values:
            median = float(np.median(np.asarray(list(self._ambient_rms_values), dtype=np.float32)))
        ema = self._ambient_rms_ema
        if median > 0.0 and ema > 0.0:
            return max(median, ema * 0.98)
        if median > 0.0:
            return median
        return ema if ema > 0.0 else 0.0

    def _adaptive_pre_partial_rms_threshold(self) -> float:
        ambient_floor = self._ambient_rms_floor()
        static_floor = self.min_rms_for_recognition if self.energy_gate_enabled else 0.0
        if ambient_floor <= 0.0:
            return static_floor

        # Mode 0/1 is intentionally more permissive, so it needs a firmer
        # pre-partial floor to keep room noise from becoming fake speech.
        multiplier = 2.15 if self.vad_mode <= 1 else 1.85
        padding = 0.00065 if self.vad_mode <= 1 else 0.00045
        threshold = max(static_floor, ambient_floor * multiplier, ambient_floor + padding)
        return min(threshold, 0.0028)

    def _segment_passes_admission(self, *, for_partial: bool) -> bool:
        speech_frames = len(self._speech_frames)
        if speech_frames < self.min_speech_frames:
            return False
        if for_partial and self._last_partial_frame_count == 0 and speech_frames < self.first_partial_min_speech_frames:
            return False
        if self.min_voiced_ratio > 0.0 and self._segment_total_frames > 0:
            if self._segment_voiced_ratio() < self.min_voiced_ratio:
                return False
        return True

    def _build_segment(self, kind: Literal["partial", "final"]) -> VadSegment | None:
        if not self._speech_frames:
            return None
        if not self._segment_passes_admission(for_partial=kind == "partial"):
            return None
        return VadSegment(
            kind=kind,
            audio=b"".join(self._speech_frames),
            duration_ms=self._duration_ms(len(self._speech_frames)),
            voiced_ratio=self._segment_voiced_ratio(),
            average_rms=self._segment_average_rms(),
        )

    def _flush_speech_onset_from_preroll_and_attack(self) -> None:
        """Promote preroll + confirmed attack frames into an active speech segment."""
        for frame_bytes, rms in self._preroll:
            self._speech_frames.append(frame_bytes)
            self._speech_rms_values.append(rms)
            self._segment_total_frames += 1
        for frame_bytes, rms in self._pending_attack:
            self._speech_frames.append(frame_bytes)
            self._speech_rms_values.append(rms)
            self._segment_total_frames += 1
            self._segment_voiced_frames += 1
        self._preroll.clear()
        self._pending_attack.clear()
        self._silence_frames = 0

    def _try_grow_partial_or_finalize_max_segment(self, segments: list[VadSegment]) -> None:
        frames_since_last_partial = len(self._speech_frames) - self._last_partial_frame_count
        target_partial_frames = (
            self.first_partial_min_speech_frames
            if self._last_partial_frame_count == 0
            else self.partial_interval_frames
        )
        if (
            len(self._speech_frames) >= self.min_speech_frames
            and frames_since_last_partial >= target_partial_frames
        ):
            partial_segment = self._build_segment("partial")
            if partial_segment is not None:
                segments.append(partial_segment)
                self._last_partial_frame_count = len(self._speech_frames)

        if len(self._speech_frames) >= self.max_segment_frames:
            final_segment = self._build_segment("final")
            if final_segment is not None:
                segments.append(final_segment)
            else:
                self._segment_dropped_count += 1
            self.reset()

    def _append_voiced_frame(self, frame: bytes, frame_rms: float, segments: list[VadSegment]) -> None:
        self._speech_frames.append(frame)
        self._speech_rms_values.append(frame_rms)
        self._silence_frames = 0
        self._segment_total_frames += 1
        self._segment_voiced_frames += 1
        self._try_grow_partial_or_finalize_max_segment(segments)

    def process_chunk(self, audio_chunk: bytes) -> list[VadSegment]:
        if not audio_chunk:
            return []

        segments: list[VadSegment] = []
        self._pending_audio.extend(audio_chunk)
        total_len = len(self._pending_audio)
        if total_len < self.frame_bytes:
            return []
        usable = total_len - (total_len % self.frame_bytes)
        chunk = bytes(self._pending_audio[:usable])
        for start in range(0, usable, self.frame_bytes):
            frame = chunk[start : start + self.frame_bytes]

            frame_rms = self._frame_rms(frame)
            is_speech = self.vad.is_speech(frame, self.sample_rate)
            admitted_speech = is_speech and (
                not self.energy_gate_enabled or frame_rms >= self.min_rms_for_recognition
            )
            if admitted_speech and self._last_partial_frame_count == 0:
                adaptive_threshold = self._adaptive_pre_partial_rms_threshold()
                if adaptive_threshold > 0.0 and frame_rms < adaptive_threshold:
                    admitted_speech = False

            if admitted_speech:
                if not self._speech_frames:
                    self._pending_attack.append((frame, frame_rms))
                    if len(self._pending_attack) < self.speech_attack_frames:
                        continue
                    self._flush_speech_onset_from_preroll_and_attack()
                    self._try_grow_partial_or_finalize_max_segment(segments)
                else:
                    self._append_voiced_frame(frame, frame_rms, segments)

            elif self._speech_frames:
                self._remember_ambient_rms(frame_rms)
                self._segment_total_frames += 1
                self._silence_frames += 1
                if (
                    self._silence_frames >= self.silence_hold_frames
                    and len(self._speech_frames) >= self.min_speech_frames
                    and self._last_partial_frame_count != len(self._speech_frames)
                ):
                    partial_segment = self._build_segment("partial")
                    if partial_segment is not None:
                        segments.append(partial_segment)
                        self._last_partial_frame_count = len(self._speech_frames)

                if self._silence_frames >= self.finalization_hold_frames:
                    final_segment = self._build_segment("final")
                    if final_segment is not None:
                        segments.append(final_segment)
                    else:
                        self._segment_dropped_count += 1
                    self.reset()
            else:
                self._pending_attack.clear()
                self._remember_ambient_rms(frame_rms)
                if self.speech_preroll_frames > 0:
                    self._preroll.append((frame, frame_rms))

        if usable:
            del self._pending_audio[:usable]
        return segments
