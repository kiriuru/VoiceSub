from __future__ import annotations

from typing import Any

from backend.core.translation_engine import TranslationEngine
from backend.models import TranslationDiagnostics


def summarize_translation_readiness(
    engine: TranslationEngine,
    translation_config: dict[str, Any],
) -> TranslationDiagnostics:
    return engine.summarize_readiness(translation_config)


__all__ = ["summarize_translation_readiness"]
