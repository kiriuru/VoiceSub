from __future__ import annotations

import unittest

from backend.core.runtime.local_asr_pipeline import LocalAsrPipeline


class LocalAsrPipelineTests(unittest.TestCase):
    def test_pipeline_exposes_capture_and_asr_entrypoints(self) -> None:
        pipeline = LocalAsrPipeline(host=object())
        self.assertEqual(pipeline.name, "local_asr_pipeline")
        self.assertTrue(callable(pipeline.run_capture_loop))
        self.assertTrue(callable(pipeline.run_asr_loop))
