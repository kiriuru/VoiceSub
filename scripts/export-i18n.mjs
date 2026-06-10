/**
 * Extracts SST locale objects from scripts/i18n-source/locales/*.js into src/lib/i18n/locales/*.json
 * Merges dynamic-locales.js fallbacks and VoiceSub dashboard keys.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import { voicesubLocaleOverrides } from "./voicesub-locale-overrides.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const srcDir = path.join(root, "scripts", "i18n-source", "locales");
const outDir = path.join(root, "src", "lib", "i18n", "locales");

const dynamicPath = path.join(root, "scripts", "i18n-source", "dynamic-locales.js");
const dynamicCode = fs.readFileSync(dynamicPath, "utf8");
const dynamicSandbox = { window: { __SST_I18N_DYNAMIC: {} } };
vm.runInNewContext(dynamicCode, dynamicSandbox);
const dynamicEn = dynamicSandbox.window.__SST_I18N_DYNAMIC?.en || {};
const dynamicRu = dynamicSandbox.window.__SST_I18N_DYNAMIC?.ru || {};

const voicesubExtras = {
  "document.title.dashboard": "VoiceSub",
  "header.title": "VoiceSub",
  "save.status.saved": "Settings saved.",
  "obs.overlay.copy": "Copy URL",
  "obs.overlay.copied": "Copied",
  "obs.overlay.instructions": "Add this URL as an OBS Browser Source. Update the URL when VoiceSub bind address changes.",
  "subtitles.display_order": "Display order (comma-separated slot ids)",
  "style.font_size.source": "Source font size (px)",
  "style.font_size.translation": "Translation font size (px)",
  "tools.runtime.note": "Runtime logs and diagnostics are written to the user data folder.",
  "overlay.preview.waiting": "Waiting for subtitle payload…",
  "translation.dispatcher.eyebrow": "Queue & timeouts",
  "translation.dispatcher.title": "Translation dispatcher",
  "translation.dispatcher.timeout_ms": "Provider request timeout (ms)",
  "translation.dispatcher.queue_max_size": "Maximum queued jobs",
  "translation.dispatcher.max_concurrent_jobs": "Maximum parallel jobs",
  "translation.dispatcher.note":
    "Increase timeout for slow providers. Higher parallelism uses more API quota.",
  "translation.provider_limits.eyebrow": "Provider limits",
  "translation.provider_limits.title": "Per-provider dispatcher limits",
  "translation.provider_limits.max_concurrent_targets": "Max concurrent targets",
  "translation.provider_limits.min_interval_ms": "Min interval between calls (ms)",
  "translation.provider_limits.note":
    "Leave empty to use provider defaults. Limits apply per provider name in the dispatcher.",
  "translation.cache.max_entries": "Maximum cache entries",
  "tuning.source_lang": "Source language hint (translation)",
  "tuning.source_lang.auto": "Auto (from recognition)",
  "tuning.source_lang.note":
    "Hint for translation providers. Use auto to detect from recognition, or pick a fixed language code.",
  "overview.recognition.hint.summary": "Browser recognition notes",
  "settings.webspeech.advanced.summary": "Advanced Web Speech settings",
  "settings.webspeech.advanced.summary_hint":
    "Worker lifecycle and partial filtering. Reopen the worker after changes.",
  "settings.webspeech.advanced.hint":
    "These options map to asr.browser and partial filters in the backend. Save config and restart recognition when needed.",
  "settings.webspeech.advanced.section.final_thresholds": "Forced-final thresholds",
  "settings.webspeech.advanced.section.restart": "Restart & recovery",
  "settings.webspeech.advanced.section.network": "Network reconnect",
  "settings.webspeech.advanced.section.session": "Session rotation",
  "settings.webspeech.advanced.section.partial": "Partial filtering",
  "settings.webspeech.advanced.force_final_min_chars": "Minimum chars before forced final",
  "settings.webspeech.advanced.force_final_min_stable_ms": "Minimum stable partial time (ms)",
  "settings.webspeech.advanced.minimum_reconnect_interval_ms": "Minimum reconnect interval (ms)",
  "settings.webspeech.advanced.normal_restart_delay_ms": "Normal restart delay (ms)",
  "settings.webspeech.advanced.no_speech_restart_delay_ms": "No-speech restart delay (ms)",
  "settings.webspeech.advanced.stuck_stopping_timeout_ms": "Stuck stopping timeout (ms)",
  "settings.webspeech.advanced.network_reconnect_initial_ms": "Network reconnect initial delay (ms)",
  "settings.webspeech.advanced.network_reconnect_max_ms": "Network reconnect max delay (ms)",
  "settings.webspeech.advanced.max_browser_session_age_ms": "Max browser session age (ms)",
  "settings.webspeech.advanced.prepare_cycle_before_ms": "Prepare cycle before max age (ms)",
  "settings.webspeech.advanced.partial_min_delta_chars": "Minimum partial char delta",
  "settings.webspeech.advanced.partial_coalescing_ms": "Partial coalescing window (ms)",
  "settings.webspeech.advanced.partial.note":
    "Filters tiny partial rewrites before they reach subtitles and translation.",
  "worker.force_finalization_timeout_ms": "Forced-final idle timeout (ms)",
  "worker.force_finalization_timeout_ms.note":
    "How long to wait without partial updates before sending the current live text as a final segment.",
  "style.ui_theme.aurora": "Aurora",
  "style.ui_theme.preview.cta": "Accent button",
  "app.chrome.search": "Search commands…",
  "command_palette.title": "Command palette",
  "command_palette.placeholder": "Search commands…",
  "command_palette.no_results": "No matching commands",
  "command_palette.navigate": "navigate",
  "command_palette.select": "select",
  "command_palette.start": "Start recognition",
  "command_palette.stop": "Stop recognition",
  "command_palette.save": "Save settings",
  "command_palette.toggle_theme": "Toggle dark / light theme",
  "command_palette.toggle_layout": "Toggle compact layout",
  "command_palette.export_diagnostics": "Export diagnostics bundle",
  "command_palette.group.runtime": "Runtime",
  "command_palette.group.settings": "Settings",
  "command_palette.group.tools": "Tools",
  "command_palette.group.navigation": "Navigation",
  "command_palette.tab.translation": "Go to Translation",
  "command_palette.tab.subtitles": "Go to Subtitles",
  "command_palette.tab.style": "Go to Style",
  "command_palette.tab.theme": "Go to Theme",
  "command_palette.tab.obs": "Go to OBS",
  "command_palette.tab.replacement": "Go to Word Replacement",
  "command_palette.tab.tools": "Go to Tools & Data",
  "command_palette.tab.settings": "Go to Settings",
  "command_palette.tab.help": "Go to Help",
};

const voicesubUiRedesignLocalized = {
  ru: {
    "tuning.source_lang.auto": "Авто (из распознавания)",
    "overview.recognition.hint.summary": "Заметки о браузерном распознавании",
    "style.ui_theme.aurora": "Аврора",
    "style.ui_theme.preview.cta": "Акцентная кнопка",
    "app.chrome.search": "Поиск команд…",
    "command_palette.title": "Палитра команд",
    "command_palette.placeholder": "Поиск команд…",
    "command_palette.no_results": "Команды не найдены",
    "command_palette.navigate": "навигация",
    "command_palette.select": "выбор",
    "command_palette.start": "Запустить распознавание",
    "command_palette.stop": "Остановить распознавание",
    "command_palette.save": "Сохранить настройки",
    "command_palette.toggle_theme": "Переключить тёмную / светлую тему",
    "command_palette.toggle_layout": "Переключить компактный макет",
    "command_palette.export_diagnostics": "Экспорт диагностики",
    "command_palette.group.runtime": "Рантайм",
    "command_palette.group.settings": "Настройки",
    "command_palette.group.tools": "Инструменты",
    "command_palette.group.navigation": "Навигация",
    "command_palette.tab.translation": "Перейти: Перевод",
    "command_palette.tab.subtitles": "Перейти: Субтитры",
    "command_palette.tab.style": "Перейти: Стиль",
    "command_palette.tab.theme": "Перейти: Тема",
    "command_palette.tab.obs": "Перейти: OBS",
    "command_palette.tab.replacement": "Перейти: Замена слов",
    "command_palette.tab.tools": "Перейти: Инструменты",
    "command_palette.tab.settings": "Перейти: Настройки",
    "command_palette.tab.help": "Перейти: Справка",
  },
  ja: {
    "tuning.source_lang.auto": "自動（認識から）",
    "overview.recognition.hint.summary": "ブラウザ認識の注意事項",
    "style.ui_theme.aurora": "オーロラ",
    "style.ui_theme.preview.cta": "アクセントボタン",
    "app.chrome.search": "コマンドを検索…",
    "command_palette.title": "コマンドパレット",
    "command_palette.placeholder": "コマンドを検索…",
    "command_palette.no_results": "一致するコマンドがありません",
    "command_palette.navigate": "移動",
    "command_palette.select": "選択",
    "command_palette.start": "認識を開始",
    "command_palette.stop": "認識を停止",
    "command_palette.save": "設定を保存",
    "command_palette.toggle_theme": "ダーク / ライトテーマを切り替え",
    "command_palette.toggle_layout": "コンパクトレイアウトを切り替え",
    "command_palette.export_diagnostics": "診断バンドルをエクスポート",
    "command_palette.group.runtime": "ランタイム",
    "command_palette.group.settings": "設定",
    "command_palette.group.tools": "ツール",
    "command_palette.group.navigation": "ナビゲーション",
    "command_palette.tab.translation": "翻訳へ移動",
    "command_palette.tab.subtitles": "字幕へ移動",
    "command_palette.tab.style": "スタイルへ移動",
    "command_palette.tab.theme": "テーマへ移動",
    "command_palette.tab.obs": "OBSへ移動",
    "command_palette.tab.replacement": "単語置換へ移動",
    "command_palette.tab.tools": "ツールとデータへ移動",
    "command_palette.tab.settings": "設定へ移動",
    "command_palette.tab.help": "ヘルプへ移動",
  },
  ko: {
    "tuning.source_lang.auto": "자동 (인식 기준)",
    "overview.recognition.hint.summary": "브라우저 인식 참고",
    "style.ui_theme.aurora": "오로라",
    "style.ui_theme.preview.cta": "강조 버튼",
    "app.chrome.search": "명령 검색…",
    "command_palette.title": "명령 팔레트",
    "command_palette.placeholder": "명령 검색…",
    "command_palette.no_results": "일치하는 명령이 없습니다",
    "command_palette.navigate": "이동",
    "command_palette.select": "선택",
    "command_palette.start": "인식 시작",
    "command_palette.stop": "인식 중지",
    "command_palette.save": "설정 저장",
    "command_palette.toggle_theme": "다크 / 라이트 테마 전환",
    "command_palette.toggle_layout": "컴팩트 레이아웃 전환",
    "command_palette.export_diagnostics": "진단 번들보내기",
    "command_palette.group.runtime": "런타임",
    "command_palette.group.settings": "설정",
    "command_palette.group.tools": "도구",
    "command_palette.group.navigation": "탐색",
    "command_palette.tab.translation": "번역으로 이동",
    "command_palette.tab.subtitles": "자막으로 이동",
    "command_palette.tab.style": "스타일로 이동",
    "command_palette.tab.theme": "테마로 이동",
    "command_palette.tab.obs": "OBS로 이동",
    "command_palette.tab.replacement": "단어 치환으로 이동",
    "command_palette.tab.tools": "도구 및 데이터로 이동",
    "command_palette.tab.settings": "설정으로 이동",
    "command_palette.tab.help": "도움말로 이동",
  },
  zh: {
    "tuning.source_lang.auto": "自动（来自识别）",
    "overview.recognition.hint.summary": "浏览器识别说明",
    "style.ui_theme.aurora": "极光",
    "style.ui_theme.preview.cta": "强调按钮",
    "app.chrome.search": "搜索命令…",
    "command_palette.title": "命令面板",
    "command_palette.placeholder": "搜索命令…",
    "command_palette.no_results": "没有匹配的命令",
    "command_palette.navigate": "导航",
    "command_palette.select": "选择",
    "command_palette.start": "开始识别",
    "command_palette.stop": "停止识别",
    "command_palette.save": "保存设置",
    "command_palette.toggle_theme": "切换深色 / 浅色主题",
    "command_palette.toggle_layout": "切换紧凑布局",
    "command_palette.export_diagnostics": "导出诊断包",
    "command_palette.group.runtime": "运行时",
    "command_palette.group.settings": "设置",
    "command_palette.group.tools": "工具",
    "command_palette.group.navigation": "导航",
    "command_palette.tab.translation": "转到：翻译",
    "command_palette.tab.subtitles": "转到：字幕",
    "command_palette.tab.style": "转到：样式",
    "command_palette.tab.theme": "转到：主题",
    "command_palette.tab.obs": "转到：OBS",
    "command_palette.tab.replacement": "转到：词语替换",
    "command_palette.tab.tools": "转到：工具与数据",
    "command_palette.tab.settings": "转到：设置",
    "command_palette.tab.help": "转到：帮助",
  },
};

const webspeechAdvancedLocalized = {
  ru: {
    "settings.webspeech.advanced.summary": "Расширенные настройки Web Speech",
    "settings.webspeech.advanced.summary_hint":
      "Жизненный цикл воркера и фильтрация partial. После изменений переоткройте окно воркера.",
    "settings.webspeech.advanced.hint":
      "Эти параметры соответствуют asr.browser и фильтрам partial на бэкенде. Сохраните config и при необходимости перезапустите распознавание.",
    "settings.webspeech.advanced.section.final_thresholds": "Пороги принудительного final",
    "settings.webspeech.advanced.section.restart": "Перезапуск и восстановление",
    "settings.webspeech.advanced.section.network": "Сеть",
    "settings.webspeech.advanced.section.session": "Ротация сессии",
    "settings.webspeech.advanced.section.partial": "Фильтрация partial",
    "settings.webspeech.advanced.force_final_min_chars": "Минимум символов перед forced final",
    "settings.webspeech.advanced.force_final_min_stable_ms": "Минимальное время стабильного partial (мс)",
    "settings.webspeech.advanced.minimum_reconnect_interval_ms": "Минимальный интервал переподключения (мс)",
    "settings.webspeech.advanced.normal_restart_delay_ms": "Задержка обычного перезапуска (мс)",
    "settings.webspeech.advanced.no_speech_restart_delay_ms": "Задержка перезапуска при отсутствии речи (мс)",
    "settings.webspeech.advanced.stuck_stopping_timeout_ms": "Таймаут зависшего stopping (мс)",
    "settings.webspeech.advanced.network_reconnect_initial_ms": "Начальная задержка сетевого reconnect (мс)",
    "settings.webspeech.advanced.network_reconnect_max_ms": "Максимальная задержка сетевого reconnect (мс)",
    "settings.webspeech.advanced.max_browser_session_age_ms": "Максимальный возраст сессии браузера (мс)",
    "settings.webspeech.advanced.prepare_cycle_before_ms": "Подготовка цикла до max age (мс)",
    "settings.webspeech.advanced.partial_min_delta_chars": "Минимальная дельта символов partial",
    "settings.webspeech.advanced.partial_coalescing_ms": "Окно слияния partial (мс)",
    "settings.webspeech.advanced.partial.note":
      "Отсекает мелкие переписывания partial до субтитров и перевода.",
  },
  ja: {
    "settings.webspeech.advanced.summary": "Web Speech 詳細設定",
    "settings.webspeech.advanced.summary_hint":
      "ワーカーのライフサイクルと partial フィルタ。変更後はワーカーウィンドウを開き直してください。",
    "settings.webspeech.advanced.hint":
      "asr.browser とバックエンドの partial フィルタに対応します。必要に応じて config を保存し、認識を再起動してください。",
    "settings.webspeech.advanced.section.final_thresholds": "強制 final のしきい値",
    "settings.webspeech.advanced.section.restart": "再起動と復旧",
    "settings.webspeech.advanced.section.network": "ネットワーク再接続",
    "settings.webspeech.advanced.section.session": "セッションローテーション",
    "settings.webspeech.advanced.section.partial": "Partial フィルタ",
    "settings.webspeech.advanced.force_final_min_chars": "強制 final 前の最小文字数",
    "settings.webspeech.advanced.force_final_min_stable_ms": "安定 partial の最小時間 (ms)",
    "settings.webspeech.advanced.minimum_reconnect_interval_ms": "最小再接続間隔 (ms)",
    "settings.webspeech.advanced.normal_restart_delay_ms": "通常再起動の遅延 (ms)",
    "settings.webspeech.advanced.no_speech_restart_delay_ms": "無音時の再起動遅延 (ms)",
    "settings.webspeech.advanced.stuck_stopping_timeout_ms": "stopping 固着タイムアウト (ms)",
    "settings.webspeech.advanced.network_reconnect_initial_ms": "ネットワーク再接続の初期待機 (ms)",
    "settings.webspeech.advanced.network_reconnect_max_ms": "ネットワーク再接続の最大待機 (ms)",
    "settings.webspeech.advanced.max_browser_session_age_ms": "ブラウザセッション最大存続時間 (ms)",
    "settings.webspeech.advanced.prepare_cycle_before_ms": "最大存続前の準備サイクル (ms)",
    "settings.webspeech.advanced.partial_min_delta_chars": "partial の最小文字差分",
    "settings.webspeech.advanced.partial_coalescing_ms": "partial 統合ウィンドウ (ms)",
    "settings.webspeech.advanced.partial.note":
      "字幕と翻訳に届く前に、小さな partial の書き換えを抑えます。",
  },
  ko: {
    "settings.webspeech.advanced.summary": "Web Speech 고급 설정",
    "settings.webspeech.advanced.summary_hint":
      "워커 수명 주기 및 partial 필터. 변경 후 워커 창을 다시 여세요.",
    "settings.webspeech.advanced.hint":
      "asr.browser 및 백엔드 partial 필터에 해당합니다. 필요 시 config를 저장하고 인식을 다시 시작하세요.",
    "settings.webspeech.advanced.section.final_thresholds": "강제 final 임계값",
    "settings.webspeech.advanced.section.restart": "재시작 및 복구",
    "settings.webspeech.advanced.section.network": "네트워크 재연결",
    "settings.webspeech.advanced.section.session": "세션 로테이션",
    "settings.webspeech.advanced.section.partial": "Partial 필터",
    "settings.webspeech.advanced.force_final_min_chars": "강제 final 전 최소 문자 수",
    "settings.webspeech.advanced.force_final_min_stable_ms": "안정 partial 최소 시간 (ms)",
    "settings.webspeech.advanced.minimum_reconnect_interval_ms": "최소 재연결 간격 (ms)",
    "settings.webspeech.advanced.normal_restart_delay_ms": "일반 재시작 지연 (ms)",
    "settings.webspeech.advanced.no_speech_restart_delay_ms": "무음 시 재시작 지연 (ms)",
    "settings.webspeech.advanced.stuck_stopping_timeout_ms": "stopping 고착 타임아웃 (ms)",
    "settings.webspeech.advanced.network_reconnect_initial_ms": "네트워크 재연결 초기 지연 (ms)",
    "settings.webspeech.advanced.network_reconnect_max_ms": "네트워크 재연결 최대 지연 (ms)",
    "settings.webspeech.advanced.max_browser_session_age_ms": "브라우저 세션 최대 수명 (ms)",
    "settings.webspeech.advanced.prepare_cycle_before_ms": "최대 수명 전 준비 주기 (ms)",
    "settings.webspeech.advanced.partial_min_delta_chars": "partial 최소 문자 델타",
    "settings.webspeech.advanced.partial_coalescing_ms": "partial 병합 창 (ms)",
    "settings.webspeech.advanced.partial.note":
      "자막과 번역에 도달하기 전에 작은 partial 재작성을 걸러냅니다.",
  },
  zh: {
    "settings.webspeech.advanced.summary": "Web Speech 高级设置",
    "settings.webspeech.advanced.summary_hint":
      "Worker 生命周期与 partial 过滤。更改后请重新打开 worker 窗口。",
    "settings.webspeech.advanced.hint":
      "这些选项对应 asr.browser 和后端 partial 过滤器。请保存 config 并在需要时重启识别。",
    "settings.webspeech.advanced.section.final_thresholds": "强制 final 阈值",
    "settings.webspeech.advanced.section.restart": "重启与恢复",
    "settings.webspeech.advanced.section.network": "网络重连",
    "settings.webspeech.advanced.section.session": "会话轮换",
    "settings.webspeech.advanced.section.partial": "Partial 过滤",
    "settings.webspeech.advanced.force_final_min_chars": "强制 final 前最少字符数",
    "settings.webspeech.advanced.force_final_min_stable_ms": "稳定 partial 最短时间 (ms)",
    "settings.webspeech.advanced.minimum_reconnect_interval_ms": "最小重连间隔 (ms)",
    "settings.webspeech.advanced.normal_restart_delay_ms": "正常重启延迟 (ms)",
    "settings.webspeech.advanced.no_speech_restart_delay_ms": "无语音时重启延迟 (ms)",
    "settings.webspeech.advanced.stuck_stopping_timeout_ms": "stopping 卡住超时 (ms)",
    "settings.webspeech.advanced.network_reconnect_initial_ms": "网络重连初始延迟 (ms)",
    "settings.webspeech.advanced.network_reconnect_max_ms": "网络重连最大延迟 (ms)",
    "settings.webspeech.advanced.max_browser_session_age_ms": "浏览器会话最大时长 (ms)",
    "settings.webspeech.advanced.prepare_cycle_before_ms": "达到最大时长前的准备周期 (ms)",
    "settings.webspeech.advanced.partial_min_delta_chars": "partial 最小字符差",
    "settings.webspeech.advanced.partial_coalescing_ms": "partial 合并窗口 (ms)",
    "settings.webspeech.advanced.partial.note":
      "在 partial 到达字幕和翻译之前过滤微小的重写。",
  },
};

fs.mkdirSync(outDir, { recursive: true });

for (const file of fs.readdirSync(srcDir).filter((f) => f.endsWith(".js"))) {
  const locale = file.replace(/\.js$/, "");
  const code = fs.readFileSync(path.join(srcDir, file), "utf8");
  const sandbox = { window: { __SST_I18N_LOCALES: {} } };
  vm.runInNewContext(code, sandbox);
  const messages = sandbox.window.__SST_I18N_LOCALES[locale];
  if (!messages) {
    console.warn(`skip ${file}: no locale object`);
    continue;
  }
  const dynamic = locale === "ru" ? dynamicRu : dynamicEn;
  const localized = webspeechAdvancedLocalized[locale] || {};
  const uiRedesign = voicesubUiRedesignLocalized[locale] || {};
  const voicesubOverrides = voicesubLocaleOverrides(locale);
  const merged = {
    ...voicesubExtras,
    ...dynamic,
    ...localized,
    ...uiRedesign,
    ...messages,
    ...voicesubOverrides,
  };
  const outPath = path.join(outDir, `${locale}.json`);
  fs.writeFileSync(outPath, `${JSON.stringify(merged, null, 2)}\n`);
  console.log(`wrote ${outPath} (${Object.keys(merged).length} keys)`);
}
