from __future__ import annotations

import re
import subprocess
import sys
import unittest
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
JS_ROOT = PROJECT_ROOT / "frontend" / "js"


class FrontendModularVanillaTests(unittest.TestCase):
    def test_core_modular_primitives_exist(self) -> None:
        expected = [
            JS_ROOT / "core" / "dom.js",
            JS_ROOT / "core" / "panel-mount.js",
            JS_ROOT / "core" / "selectors.js",
            JS_ROOT / "core" / "store.js",
        ]
        missing = [str(path.relative_to(PROJECT_ROOT)) for path in expected if not path.exists()]
        self.assertEqual(missing, [])

    def test_store_exposes_selector_subscription(self) -> None:
        store_js = (JS_ROOT / "core" / "store.js").read_text(encoding="utf-8")
        self.assertIn("export function subscribeSelector", store_js)
        self.assertIn("export function patchUi", store_js)

    def test_dashboard_actions_are_composed_from_modules(self) -> None:
        actions_js = (JS_ROOT / "dashboard" / "actions.js").read_text(encoding="utf-8")
        self.assertIn('./actions/index.js', actions_js)
        index_js = (JS_ROOT / "dashboard" / "actions" / "index.js").read_text(encoding="utf-8")
        self.assertIn("createConfigActions", index_js)
        self.assertIn("createRuntimeActions", index_js)
        self.assertIn("createWsHandlers", index_js)

    def test_shell_modules_and_help_partial_exist(self) -> None:
        expected = [
            JS_ROOT / "shell" / "help-content-loader.js",
            JS_ROOT / "shell" / "help-topics.js",
            JS_ROOT / "shell" / "locale-switcher.js",
            JS_ROOT / "shell" / "tabs.js",
            PROJECT_ROOT / "frontend" / "partials" / "dashboard-help-topics.html",
        ]
        missing = [str(path.relative_to(PROJECT_ROOT)) for path in expected if not path.exists()]
        self.assertEqual(missing, [])

    def test_main_mounts_panels_before_help_fetch(self) -> None:
        main_js = (JS_ROOT / "main.js").read_text(encoding="utf-8")
        self.assertIn("loadDashboardHelpContent", main_js)
        self.assertIn("initializeHelpTopics", main_js)
        mounts_index = main_js.index("mountRuntimePanel")
        help_index = main_js.index("loadDashboardHelpContent")
        self.assertLess(mounts_index, help_index)
        loader_js = (JS_ROOT / "shell" / "help-content-loader.js").read_text(encoding="utf-8")
        self.assertIn("/static/partials/dashboard-help-topics.html", loader_js)

    def test_profiles_panel_uses_panel_mount_and_dom_helpers(self) -> None:
        panel_js = (JS_ROOT / "panels" / "profiles-panel.js").read_text(encoding="utf-8")
        self.assertIn("createPanelMount", panel_js)
        self.assertIn("fillSelectOptions", panel_js)

    def test_dashboard_entry_modules_parse_in_node(self) -> None:
        """Catch JS syntax errors that prevent main.js from loading in the browser."""
        rel_paths = [
            "frontend/js/panels/translation-panel.js",
            "frontend/js/panels/runtime-panel.js",
            "frontend/js/panels/asr-panel.js",
            "frontend/js/panels/style-editor-panel.js",
            "frontend/js/panels/source-text-replacement-panel.js",
            "frontend/js/browser-asr-session-manager.js",
            "frontend/js/panels/style/style-editor-panel-render.js",
            "frontend/js/dashboard/actions/index.js",
        ]
        for rel_path in rel_paths:
            abs_path = (PROJECT_ROOT / rel_path).resolve()
            completed = subprocess.run(
                ["node", "--check", str(abs_path)],
                cwd=PROJECT_ROOT,
                capture_output=True,
                text=True,
            )
            self.assertEqual(
                completed.returncode,
                0,
                msg=f"{rel_path} failed node --check: {completed.stderr.strip()}",
            )

    def test_translation_results_view_is_isolated(self) -> None:
        panel_js = (JS_ROOT / "panels" / "translation-panel.js").read_text(encoding="utf-8")
        view_js = (JS_ROOT / "panels" / "translation" / "translation-results-view.js").read_text(encoding="utf-8")
        self.assertIn("translation-results-view.js", panel_js)
        self.assertIn("export function renderTranslationResults", view_js)
        self.assertIn("export function buildTranslationResultsKey", view_js)

    def test_runtime_panel_uses_panel_mount(self) -> None:
        panel_js = (JS_ROOT / "panels" / "runtime-panel.js").read_text(encoding="utf-8")
        self.assertIn("createPanelMount", panel_js)
        self.assertIn("collectElements", panel_js)

    def test_overlay_panel_uses_panel_mount_and_display_order_view(self) -> None:
        panel_js = (JS_ROOT / "panels" / "overlay-panel.js").read_text(encoding="utf-8")
        self.assertIn("createPanelMount", panel_js)
        self.assertIn("overlay-display-order-view.js", panel_js)

    def test_asr_panel_uses_panel_mount_and_render_module(self) -> None:
        panel_js = (JS_ROOT / "panels" / "asr-panel.js").read_text(encoding="utf-8")
        render_js = (JS_ROOT / "panels" / "asr" / "asr-panel-render.js").read_text(encoding="utf-8")
        self.assertIn("createPanelMount", panel_js)
        self.assertIn("renderAsrPanel", render_js)
        self.assertIn("fillAudioInputDevices", render_js)

    def test_diagnostics_obs_remote_use_panel_mount(self) -> None:
        for name in ("diagnostics-panel.js", "obs-captions-panel.js", "remote-panel.js"):
            panel_js = (JS_ROOT / "panels" / name).read_text(encoding="utf-8")
            self.assertIn("createPanelMount", panel_js, name)

    def test_style_and_replacement_panels_use_panel_mount(self) -> None:
        style_js = (JS_ROOT / "panels" / "style-editor-panel.js").read_text(encoding="utf-8")
        render_js = (JS_ROOT / "panels" / "style" / "style-editor-panel-render.js").read_text(encoding="utf-8")
        repl_js = (JS_ROOT / "panels" / "source-text-replacement-panel.js").read_text(encoding="utf-8")
        self.assertIn("createPanelMount", style_js)
        self.assertIn("renderStyleEditorPanel", render_js)
        self.assertIn("createPanelMount", repl_js)
        self.assertNotIn("subscribe(", style_js)
        self.assertNotIn("subscribe(", repl_js)

    def test_dom_exposes_set_select_markup(self) -> None:
        dom_js = (JS_ROOT / "core" / "dom.js").read_text(encoding="utf-8")
        self.assertIn("export function setSelectMarkup", dom_js)

    def test_remote_panel_uses_subscribe_selector(self) -> None:
        panel_js = (JS_ROOT / "panels" / "remote-panel.js").read_text(encoding="utf-8")
        self.assertIn("subscribeSelector", panel_js)

    def test_translation_panel_shared_module_exists(self) -> None:
        shared_js = (JS_ROOT / "panels" / "translation" / "translation-panel-shared.js").read_text(encoding="utf-8")
        self.assertIn("export function getLineCards", shared_js)
        line_editor_js = (JS_ROOT / "panels" / "translation" / "translation-line-editor-view.js").read_text(encoding="utf-8")
        self.assertIn("export function createTranslationLineEditor", line_editor_js)
        panel_js = (JS_ROOT / "panels" / "translation-panel.js").read_text(encoding="utf-8")
        self.assertIn("createTranslationLineEditor", panel_js)
        self.assertIn("translation-panel-shared.js", panel_js)
        self.assertNotIn("function renderLineEditor(snapshot)", panel_js)

    def test_translation_panel_imports_all_shared_symbols_used(self) -> None:
        """Catch the runtime ReferenceError class of bug: helpers referenced
        inside the panel but missing from the import list."""
        panel_js = (JS_ROOT / "panels" / "translation-panel.js").read_text(encoding="utf-8")
        import_block_match = re.search(
            r'import\s*\{([^}]+)\}\s*from\s*"\./translation/translation-panel-shared\.js";',
            panel_js,
        )
        self.assertIsNotNone(
            import_block_match,
            "translation-panel.js must import its helpers from translation-panel-shared.js",
        )
        imported_names = {
            name.strip()
            for name in import_block_match.group(1).split(",")  # type: ignore[union-attr]
            if name.strip()
        }
        # Add-language click handler depends on getLineMap (regression check
        # for the "translation_1 add button explodes" runtime bug).
        for required_symbol in ("getLineMap", "ensureLine", "CANONICAL_TRANSLATION_SLOTS"):
            self.assertIn(
                required_symbol,
                imported_names,
                f"translation-panel.js must import {required_symbol} from translation-panel-shared.js",
            )

    def test_store_emit_isolates_listener_errors(self) -> None:
        """A throwing listener must not stop the rest of the panel subscribers
        from receiving the store snapshot. The store iterates over a snapshot
        of the listener set so unsubscribes during dispatch are safe too."""
        store_js = (JS_ROOT / "core" / "store.js").read_text(encoding="utf-8")
        self.assertIn("Array.from(listeners).forEach", store_js)
        self.assertIn("try {", store_js)
        self.assertIn("listener(snapshot);", store_js)
        # Defensive: console.error path is wired so we surface listener bugs
        # in dev tools without crashing the store.
        self.assertIn("console.error(\"[store] listener error\"", store_js)

    def test_dom_helpers_expose_idempotent_input_setters(self) -> None:
        """Panels must rely on setInputValueIfChanged / setCheckedIfChanged so
        background ws/runtime ticks never reset the user's caret/selection or
        write redundant DOM updates."""
        dom_js = (JS_ROOT / "core" / "dom.js").read_text(encoding="utf-8")
        self.assertIn("export function setInputValueIfChanged", dom_js)
        self.assertIn("export function setCheckedIfChanged", dom_js)
        # Focus guard is what protects the active editor (e.g. JSON textarea,
        # API key fields) from caret reset during background renders.
        self.assertIn("document.activeElement === element", dom_js)

    def test_diagnostics_panel_protects_config_json_textarea(self) -> None:
        """JSON.stringify(snapshot.config) is expensive and the textarea is
        actively edited — render() must funnel writes through
        setInputValueIfChanged so unrelated state ticks never blow away the
        user's edits."""
        panel_js = (JS_ROOT / "panels" / "diagnostics-panel.js").read_text(encoding="utf-8")
        self.assertIn("setInputValueIfChanged(elements.configJson", panel_js)
        self.assertNotIn("elements.configJson.value = JSON.stringify", panel_js)

    def test_asr_and_obs_panels_bind_checkbox_change_once(self) -> None:
        """Checkbox and <select> only emit 'change'. Binding both
        `eventName === 'change'` and an extra 'change' listener fires the
        mutator twice per toggle and emits a double store update across all
        panels."""
        asr_js = (JS_ROOT / "panels" / "asr-panel.js").read_text(encoding="utf-8")
        obs_js = (JS_ROOT / "panels" / "obs-captions-panel.js").read_text(encoding="utf-8")
        # The new branches must short-circuit for checkbox/select so we never
        # bind two 'change' listeners.
        self.assertIn("isCheckboxOrSelect", asr_js)
        self.assertIn("isCheckboxOrSelect", obs_js)
        # The previous double-bind pattern (eventName + explicit change with
        # the same handler) must be gone.
        self.assertNotRegex(
            asr_js,
            r"add\(element,\s*eventName,\s*mutateRealtimeFromControls\);\s*add\(element,\s*\"change\"",
        )
        self.assertNotRegex(
            obs_js,
            r"add\(element,\s*eventName,\s*syncConfig\);\s*add\(element,\s*\"change\"",
        )

    def test_remote_panel_does_not_double_subscribe_to_config(self) -> None:
        """createPanelMount already subscribes renderRemotePanel. The legacy
        subscribeSelector(selectConfig, ...) duplicated the per-tick fill;
        keep the single subscription so config updates run once per snapshot."""
        panel_js = (JS_ROOT / "panels" / "remote-panel.js").read_text(encoding="utf-8")
        self.assertNotIn("subscribeSelector(selectConfig", panel_js)
        self.assertNotIn("unsubscribeConfig()", panel_js)

    def test_config_normalizer_browser_defaults_match_backend(self) -> None:
        """Frontend config-normalizer must keep backend-positive defaults
        (interim/continuous/force_finalization) without flipping them to false
        when the payload omits the key. The previous implementation used
        ``=== true`` for continuous_results which silently downgraded the
        backend default."""
        normalizer_js = (JS_ROOT / "normalizers" / "config-normalizer.js").read_text(encoding="utf-8")
        for snippet in (
            "normalized.asr.browser.interim_results = normalized.asr.browser.interim_results !== false;",
            "normalized.asr.browser.continuous_results = normalized.asr.browser.continuous_results !== false;",
            "normalized.asr.browser.force_finalization_enabled = normalized.asr.browser.force_finalization_enabled !== false;",
        ):
            self.assertIn(snippet, normalizer_js)
        self.assertNotIn(
            "normalized.asr.browser.continuous_results = normalized.asr.browser.continuous_results === true;",
            normalizer_js,
        )

    def test_index_html_help_mount_is_compact(self) -> None:
        index_html = (PROJECT_ROOT / "frontend" / "index.html").read_text(encoding="utf-8")
        self.assertIn("data-help-content-mount", index_html)
        self.assertLess(index_html.count('data-help-topic-panel="overview"'), 1)


if __name__ == "__main__":
    unittest.main()
