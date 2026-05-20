"""Backward-compatible import path; low-latency streaming NeMo is the only product backend."""

from backend.core.parakeet_provider import OfficialEuParakeetRealtimeProvider

# Historical name (file-based non-streaming provider) removed in favor of realtime.
OfficialEuParakeetProvider = OfficialEuParakeetRealtimeProvider

__all__ = ["OfficialEuParakeetProvider", "OfficialEuParakeetRealtimeProvider"]
