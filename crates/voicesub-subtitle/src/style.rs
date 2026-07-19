use std::sync::OnceLock;

use serde_json::{Map, Value, json};

const LINE_SLOT_NAMES: [&str; 6] = [
    "source",
    "translation_1",
    "translation_2",
    "translation_3",
    "translation_4",
    "translation_5",
];

const EFFECT_IDS: [&str; 9] = [
    "none",
    "fade",
    "subtle_pop",
    "slide_up",
    "zoom_in",
    "blur_in",
    "glow",
    "pulse",
    "reveal",
];

fn parse_i64(raw: &Value) -> Option<i64> {
    raw.as_i64()
        .or_else(|| raw.as_u64().map(|v| v as i64))
        .or_else(|| raw.as_f64().map(|v| v as i64))
        .or_else(|| {
            raw.as_str()
                .and_then(|s| s.trim().parse::<f64>().ok())
                .map(|v| v as i64)
        })
}

fn parse_f64(raw: &Value) -> Option<f64> {
    raw.as_f64()
        .or_else(|| raw.as_i64().map(|v| v as f64))
        .or_else(|| raw.as_u64().map(|v| v as f64))
        .or_else(|| raw.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
}

fn clamp_i64(raw: &Value, default: i64, min: i64, max: i64) -> i64 {
    parse_i64(raw).unwrap_or(default).clamp(min, max)
}

fn clamp_f64(raw: &Value, default: f64, min: f64, max: f64) -> f64 {
    parse_f64(raw).unwrap_or(default).clamp(min, max)
}

fn normalize_str(raw: &Value, default: &str) -> String {
    let s = raw.as_str().unwrap_or(default).trim().to_string();
    if s.is_empty() { default.to_string() } else { s }
}

/// Align preset/config family tokens with `project_font_family_name` labels.
fn canonicalize_font_family_stack(stack: &str) -> String {
    stack
        .replace("\"JetBrains Mono Regular\"", "\"Jet Brains Mono Regular\"")
        .replace("\"JetBrains Mono Bold\"", "\"Jet Brains Mono Bold\"")
}

fn round_to(value: f64, decimals: u32) -> f64 {
    let factor = 10_f64.powi(decimals as i32);
    (value * factor).round() / factor
}

fn normalize_effect(raw: &Value, default: &str) -> String {
    let effect = raw.as_str().unwrap_or(default).trim().to_lowercase();
    if EFFECT_IDS.contains(&effect.as_str()) {
        effect
    } else {
        default.to_string()
    }
}

fn normalize_text_align(raw: &Value, default: &str) -> String {
    let align = raw.as_str().unwrap_or(default).trim().to_lowercase();
    if matches!(align.as_str(), "left" | "center" | "right") {
        align
    } else {
        default.to_string()
    }
}

fn normalize_base_style(raw_base: &Value) -> Value {
    let defaults = json!({
        "font_family": "\"Segoe UI\", Tahoma, Geneva, Verdana, sans-serif",
        "font_size_px": 30,
        "font_weight": 700,
        "fill_color": "#ffffff",
        "stroke_color": "#000000",
        "stroke_width_px": 2,
        "shadow_color": "#000000",
        "shadow_blur_px": 10,
        "shadow_offset_x_px": 0,
        "shadow_offset_y_px": 3,
        "background_color": "#000000",
        "background_opacity": 0,
        "background_padding_x_px": 12,
        "background_padding_y_px": 4,
        "background_radius_px": 10,
        "line_spacing_em": 1.15,
        "letter_spacing_em": 0.0,
        "text_align": "center",
        "line_gap_px": 8,
        "effect": "none"
    });

    let empty_base = Map::new();
    let base_obj = raw_base.as_object().unwrap_or(&empty_base);
    let get = |key: &str| {
        base_obj
            .get(key)
            .unwrap_or_else(|| defaults.get(key).unwrap())
    };

    let font_family = get("font_family");
    let font_size_px = get("font_size_px");
    let font_weight = get("font_weight");
    let fill_color = get("fill_color");
    let stroke_color = get("stroke_color");
    let stroke_width_px = get("stroke_width_px");

    let shadow_color = get("shadow_color");
    let shadow_blur_px = get("shadow_blur_px");
    let shadow_offset_x_px = get("shadow_offset_x_px");
    let shadow_offset_y_px = get("shadow_offset_y_px");

    let background_color = get("background_color");
    let background_opacity = get("background_opacity");
    let background_padding_x_px = get("background_padding_x_px");
    let background_padding_y_px = get("background_padding_y_px");
    let background_radius_px = get("background_radius_px");

    let line_spacing_em = get("line_spacing_em");
    let letter_spacing_em = get("letter_spacing_em");
    let text_align = get("text_align");
    let line_gap_px = get("line_gap_px");
    let effect = get("effect");

    let font_family = canonicalize_font_family_stack(&normalize_str(
        font_family,
        defaults["font_family"].as_str().unwrap_or_default(),
    ));
    let fill_color = normalize_str(
        fill_color,
        defaults["fill_color"].as_str().unwrap_or_default(),
    );
    let stroke_color = normalize_str(
        stroke_color,
        defaults["stroke_color"].as_str().unwrap_or_default(),
    );
    let shadow_color = normalize_str(
        shadow_color,
        defaults["shadow_color"].as_str().unwrap_or_default(),
    );
    let background_color = normalize_str(
        background_color,
        defaults["background_color"].as_str().unwrap_or_default(),
    );

    let font_size_px = clamp_i64(
        font_size_px,
        defaults["font_size_px"].as_i64().unwrap_or(30),
        12,
        96,
    );
    let font_weight = clamp_i64(
        font_weight,
        defaults["font_weight"].as_i64().unwrap_or(700),
        300,
        900,
    );

    // ASS/Aegisub outline: 0–4 px (step 0.1 in UI). Matches StyleFieldGroup + JS.
    let stroke_width_px = round_to(
        clamp_f64(
            stroke_width_px,
            defaults["stroke_width_px"].as_f64().unwrap_or(2.0),
            0.0,
            4.0,
        ),
        1,
    );
    // Clamp ranges must stay aligned with StyleFieldGroup.svelte + subtitle-style.js.
    let shadow_blur_px = round_to(
        clamp_f64(
            shadow_blur_px,
            defaults["shadow_blur_px"].as_f64().unwrap_or(10.0),
            0.0,
            40.0,
        ),
        2,
    );

    let shadow_offset_x_px = clamp_i64(
        shadow_offset_x_px,
        defaults["shadow_offset_x_px"].as_i64().unwrap_or(0),
        -24,
        24,
    );
    let shadow_offset_y_px = clamp_i64(
        shadow_offset_y_px,
        defaults["shadow_offset_y_px"].as_i64().unwrap_or(3),
        -24,
        24,
    );

    let background_opacity = clamp_i64(
        background_opacity,
        defaults["background_opacity"].as_i64().unwrap_or(0),
        0,
        100,
    );
    let background_padding_x_px = clamp_i64(
        background_padding_x_px,
        defaults["background_padding_x_px"].as_i64().unwrap_or(12),
        0,
        40,
    );
    let background_padding_y_px = clamp_i64(
        background_padding_y_px,
        defaults["background_padding_y_px"].as_i64().unwrap_or(4),
        0,
        24,
    );
    let background_radius_px = clamp_i64(
        background_radius_px,
        defaults["background_radius_px"].as_i64().unwrap_or(10),
        0,
        40,
    );

    let line_spacing_em = round_to(
        clamp_f64(
            line_spacing_em,
            defaults["line_spacing_em"].as_f64().unwrap_or(1.15),
            0.8,
            2.5,
        ),
        2,
    );
    let letter_spacing_em = round_to(
        clamp_f64(
            letter_spacing_em,
            defaults["letter_spacing_em"].as_f64().unwrap_or(0.0),
            -0.2,
            0.5,
        ),
        3,
    );
    let line_gap_px = clamp_i64(
        line_gap_px,
        defaults["line_gap_px"].as_i64().unwrap_or(8),
        0,
        40,
    );

    let text_align = normalize_text_align(
        text_align,
        defaults["text_align"].as_str().unwrap_or("center"),
    );
    let effect = normalize_effect(effect, defaults["effect"].as_str().unwrap_or("none"));

    json!({
        "font_family": font_family,
        "font_size_px": font_size_px,
        "font_weight": font_weight,
        "fill_color": fill_color,
        "stroke_color": stroke_color,
        "stroke_width_px": stroke_width_px,
        "shadow_color": shadow_color,
        "shadow_blur_px": shadow_blur_px,
        "shadow_offset_x_px": shadow_offset_x_px,
        "shadow_offset_y_px": shadow_offset_y_px,
        "background_color": background_color,
        "background_opacity": background_opacity,
        "background_padding_x_px": background_padding_x_px,
        "background_padding_y_px": background_padding_y_px,
        "background_radius_px": background_radius_px,
        "line_spacing_em": line_spacing_em,
        "letter_spacing_em": letter_spacing_em,
        "text_align": text_align,
        "line_gap_px": line_gap_px,
        "effect": effect,
    })
}

fn merge_line_style(base: &Value, override_style: &Value) -> Value {
    let enabled = override_style
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !enabled {
        return base.clone();
    }

    let empty_base = Map::new();
    let empty_override = Map::new();
    let base_obj = base.as_object().unwrap_or(&empty_base);
    let override_obj = override_style.as_object().unwrap_or(&empty_override);

    let normalized_override = normalize_base_style(override_style);
    let mut merged = base_obj.clone();
    for (key, base_value) in base_obj {
        if let Some(raw_override) = override_obj.get(key) {
            let should_override = match raw_override {
                Value::Null => false,
                Value::String(s) => !s.trim().is_empty(),
                _ => true,
            };
            if should_override {
                merged.insert(
                    key.clone(),
                    normalized_override
                        .get(key)
                        .cloned()
                        .unwrap_or(base_value.clone()),
                );
            }
        }
    }

    Value::Object(merged)
}

const BUILT_IN_PRESETS_JSON: &str = r##"{"clean_default":{"preset":"clean_default","label":"Clean Default","description":"Broadcast baseline: Inter + Noto Sans, crisp white, light outline and soft shadow — OBS-safe transparent plate.","built_in":true,"recommended_max_visible_lines":null,"base":{"font_family":"\"Inter Regular\", \"Noto Sans Regular\", \"Segoe UI\", Tahoma, sans-serif","font_size_px":32,"font_weight":600,"fill_color":"#ffffff","stroke_color":"#0b0d12","stroke_width_px":1.2,"shadow_color":"#000000","shadow_blur_px":10,"shadow_offset_x_px":0,"shadow_offset_y_px":2,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":8,"line_spacing_em":1.2,"letter_spacing_em":0,"text_align":"center","line_gap_px":8,"effect":"none"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"streamer_bold":{"preset":"streamer_bold","label":"Streamer Neon","description":"Gaming HUD: Oswald cyan with a restrained magenta halo; Montserrat covers Cyrillic without losing the neon punch.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Oswald Bold\", \"Montserrat Bold\", \"Impact\", \"Arial Narrow Bold\", sans-serif","font_size_px":36,"font_weight":800,"fill_color":"#00e8ff","stroke_color":"#07040f","stroke_width_px":3,"shadow_color":"#ff2bd6","shadow_blur_px":16,"shadow_offset_x_px":0,"shadow_offset_y_px":0,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":8,"line_spacing_em":1.15,"letter_spacing_em":0.015,"text_align":"center","line_gap_px":8,"effect":"glow"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"dual_tone":{"preset":"dual_tone","label":"Dual Color","description":"Slot-colored captions: Lato/Noto body with high-contrast fills per language so source and translations separate at a glance.","built_in":true,"recommended_max_visible_lines":null,"base":{"font_family":"\"Lato Regular\", \"Noto Sans Regular\", \"Montserrat Regular\", \"Verdana\", \"Segoe UI\", sans-serif","font_size_px":30,"font_weight":700,"fill_color":"#ffffff","stroke_color":"#101218","stroke_width_px":2,"shadow_color":"#000000","shadow_blur_px":6,"shadow_offset_x_px":0,"shadow_offset_y_px":2,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":8,"line_spacing_em":1.18,"letter_spacing_em":0,"text_align":"center","line_gap_px":8,"effect":"fade"},"line_slots":{"source":{"enabled":true,"fill_color":"#ffe566","stroke_color":"#3a2a00","stroke_width_px":2},"translation_1":{"enabled":true,"fill_color":"#7ad7ff","stroke_color":"#062433","stroke_width_px":2},"translation_2":{"enabled":true,"fill_color":"#8ff0a4","stroke_color":"#063816","stroke_width_px":2},"translation_3":{"enabled":true,"fill_color":"#ffb0cf","stroke_color":"#3a0b22","stroke_width_px":2},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"compact_overlay":{"preset":"compact_overlay","label":"Compact Bar","description":"YouTube-style caption bar: Noto/Source Sans on a dense near-opaque plate — small footprint, maximum legibility.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Noto Sans Regular\", \"Source Sans 3 Regular\", \"Source Sans 3 Bold\", \"Segoe UI\", sans-serif","font_size_px":24,"font_weight":600,"fill_color":"#ffffff","stroke_color":"#000000","stroke_width_px":0,"shadow_color":"#000000","shadow_blur_px":0,"shadow_offset_x_px":0,"shadow_offset_y_px":0,"background_color":"#0a0d12","background_opacity":92,"background_padding_x_px":16,"background_padding_y_px":6,"background_radius_px":6,"line_spacing_em":1.1,"letter_spacing_em":0,"text_align":"center","line_gap_px":4,"effect":"none"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"soft_shadow":{"preset":"soft_shadow","label":"Soft Cloud","description":"Airy Comfortaa with a wide diffused shadow and no outline — soft presence that still reads on busy gameplay.","built_in":true,"recommended_max_visible_lines":null,"base":{"font_family":"\"Comfortaa Regular\", \"Noto Sans Regular\", \"Segoe UI\", sans-serif","font_size_px":30,"font_weight":600,"fill_color":"#fff7ef","stroke_color":"#2a2018","stroke_width_px":0,"shadow_color":"#0a0806","shadow_blur_px":20,"shadow_offset_x_px":0,"shadow_offset_y_px":4,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":10,"line_spacing_em":1.2,"letter_spacing_em":0.005,"text_align":"center","line_gap_px":8,"effect":"subtle_pop"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"anime_stream":{"preset":"anime_stream","label":"Anime Stream","description":"Mochiy Pop One for Latin/Japanese + Comfortaa Bold for Cyrillic — classic anime fansub caption: white fill, crisp violet outline, soft dark drop shadow.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Mochiy Pop One Regular\", \"Comfortaa Bold\", \"Underdog Regular\", \"Bangers Regular\", \"Comic Relief Bold\", \"Poppins Bold\", \"Segoe UI\", sans-serif","font_size_px":40,"font_weight":800,"fill_color":"#ffffff","stroke_color":"#3a1a5c","stroke_width_px":1,"shadow_color":"#15071f","shadow_blur_px":8,"shadow_offset_x_px":0,"shadow_offset_y_px":3,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":10,"line_spacing_em":1.1,"letter_spacing_em":0.015,"text_align":"center","line_gap_px":6,"effect":"subtle_pop"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"accessibility_high_contrast":{"preset":"accessibility_high_contrast","label":"Max Contrast","description":"WCAG AAA solid caption box: pure white Montserrat Bold on fully opaque black — the only solid ink plate in the set.","built_in":true,"recommended_max_visible_lines":null,"base":{"font_family":"\"Montserrat Bold\", \"Noto Sans Bold\", \"Montserrat Regular\", \"Segoe UI\", sans-serif","font_size_px":38,"font_weight":800,"fill_color":"#ffffff","stroke_color":"#000000","stroke_width_px":0,"shadow_color":"#000000","shadow_blur_px":0,"shadow_offset_x_px":0,"shadow_offset_y_px":0,"background_color":"#000000","background_opacity":100,"background_padding_x_px":24,"background_padding_y_px":12,"background_radius_px":2,"line_spacing_em":1.15,"letter_spacing_em":0.02,"text_align":"center","line_gap_px":8,"effect":"none"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"dark_cinema":{"preset":"dark_cinema","label":"Cinema Plate","description":"Letterboxed cinema look: Playfair ivory on a warm sepia plate; translation slot slightly smaller for hierarchy.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Playfair Display Bold\", \"Playfair Display Regular\", \"Merriweather Bold\", Georgia, \"Times New Roman\", serif","font_size_px":30,"font_weight":700,"fill_color":"#f3e6c4","stroke_color":"#140a06","stroke_width_px":0,"shadow_color":"#08040a","shadow_blur_px":4,"shadow_offset_x_px":0,"shadow_offset_y_px":1,"background_color":"#160e0a","background_opacity":94,"background_padding_x_px":24,"background_padding_y_px":10,"background_radius_px":4,"line_spacing_em":1.2,"letter_spacing_em":0.015,"text_align":"center","line_gap_px":8,"effect":"fade"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":true,"fill_color":"#e4d2a4","stroke_color":"#08040a","font_size_px":24},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"meeting_soft":{"preset":"meeting_soft","label":"Podcast Subtle","description":"Warm parchment paper captions: Merriweather / Noto on a translucent cream plate — soft editorial podcast look, not a dark box.","built_in":true,"recommended_max_visible_lines":null,"base":{"font_family":"\"Merriweather Regular\", \"Merriweather Bold\", \"Noto Sans Regular\", \"Georgia\", serif","font_size_px":28,"font_weight":500,"fill_color":"#2a1f16","stroke_color":"#c4a882","stroke_width_px":0,"shadow_color":"#8a6a48","shadow_blur_px":12,"shadow_offset_x_px":0,"shadow_offset_y_px":2,"background_color":"#f4e8d6","background_opacity":82,"background_padding_x_px":20,"background_padding_y_px":9,"background_radius_px":10,"line_spacing_em":1.28,"letter_spacing_em":0.01,"text_align":"center","line_gap_px":6,"effect":"fade"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"retro_terminal":{"preset":"retro_terminal","label":"Retro Terminal","description":"Amber phosphor CRT: VT323 for Latin terminal texture, IBM Plex Serif for Cyrillic — dense near-black plate, restrained glow.","built_in":true,"recommended_max_visible_lines":3,"base":{"font_family":"\"VT323 Regular\", \"IBM Plex Serif Regular\", \"IBM Plex Serif Medium\", \"PT Mono Regular\", \"Consolas\", \"Courier New\", monospace","font_size_px":32,"font_weight":400,"fill_color":"#ffb84d","stroke_color":"#2a1600","stroke_width_px":0,"shadow_color":"#ff8a00","shadow_blur_px":10,"shadow_offset_x_px":0,"shadow_offset_y_px":0,"background_color":"#070604","background_opacity":93,"background_padding_x_px":18,"background_padding_y_px":7,"background_radius_px":2,"line_spacing_em":1.1,"letter_spacing_em":0.02,"text_align":"center","line_gap_px":4,"effect":"glow"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"fallout_pipboy":{"preset":"fallout_pipboy","label":"Fallout Pip-Boy","description":"Pip-Boy phosphor green: Share Tech Mono for CRT Latin, Ubuntu Mono + IBM Plex Mono for Cyrillic — deep CRT plate, controlled bloom.","built_in":true,"recommended_max_visible_lines":3,"base":{"font_family":"\"Share Tech Mono Regular\", \"Ubuntu Mono Bold\", \"Ubuntu Mono Regular\", \"IBM Plex Mono Medium\", \"PT Mono Regular\", \"Consolas\", monospace","font_size_px":30,"font_weight":400,"fill_color":"#3dff7a","stroke_color":"#001a08","stroke_width_px":0,"shadow_color":"#14ff55","shadow_blur_px":12,"shadow_offset_x_px":0,"shadow_offset_y_px":0,"background_color":"#020806","background_opacity":94,"background_padding_x_px":18,"background_padding_y_px":7,"background_radius_px":2,"line_spacing_em":1.12,"letter_spacing_em":0.022,"text_align":"center","line_gap_px":5,"effect":"glow"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"comic_burst":{"preset":"comic_burst","label":"Comic Burst","description":"Comic SFX energy: Bangers yellow with a chunky black outline; Comic Relief Bold covers Cyrillic.","built_in":true,"recommended_max_visible_lines":1,"base":{"font_family":"\"Bangers Regular\", \"Comic Relief Bold\", \"Comic Relief Regular\", \"Impact\", \"Arial Black\", sans-serif","font_size_px":42,"font_weight":400,"fill_color":"#ffd60a","stroke_color":"#0a0a0a","stroke_width_px":4,"shadow_color":"#d6172a","shadow_blur_px":3,"shadow_offset_x_px":3,"shadow_offset_y_px":5,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":8,"line_spacing_em":1.15,"letter_spacing_em":0.03,"text_align":"center","line_gap_px":8,"effect":"zoom_in"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"cyberpunk_neon":{"preset":"cyberpunk_neon","label":"Cyberpunk Neon","description":"Sci-fi HUD: Orbitron magenta with a cyan halo on a deep navy plate; Exo 2 covers Cyrillic geometry.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Orbitron Black\", \"Exo 2 Black\", \"Orbitron Regular\", \"Exo 2 Regular\", \"Montserrat Bold\", sans-serif","font_size_px":32,"font_weight":900,"fill_color":"#ff2bd6","stroke_color":"#03001a","stroke_width_px":2,"shadow_color":"#00f0ff","shadow_blur_px":18,"shadow_offset_x_px":0,"shadow_offset_y_px":0,"background_color":"#070416","background_opacity":90,"background_padding_x_px":20,"background_padding_y_px":8,"background_radius_px":4,"line_spacing_em":1.15,"letter_spacing_em":0.03,"text_align":"center","line_gap_px":7,"effect":"glow"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"noir_typewriter":{"preset":"noir_typewriter","label":"Film Noir","description":"1940s dossier captions: Special Elite for Latin typewriter texture, IBM Plex Mono for Cyrillic — warm ivory on deep ink, wide tracking, zero-radius plate, soft fade-in.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Special Elite Regular\", \"IBM Plex Mono Medium\", \"IBM Plex Mono Regular\", \"PT Mono Regular\", \"Courier New\", \"Consolas\", monospace","font_size_px":28,"font_weight":400,"fill_color":"#f2e6c8","stroke_color":"#120c08","stroke_width_px":0,"shadow_color":"#000000","shadow_blur_px":2,"shadow_offset_x_px":0,"shadow_offset_y_px":1,"background_color":"#0a0705","background_opacity":94,"background_padding_x_px":24,"background_padding_y_px":11,"background_radius_px":0,"line_spacing_em":1.28,"letter_spacing_em":0.055,"text_align":"center","line_gap_px":6,"effect":"fade"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"vlog_pastel":{"preset":"vlog_pastel","label":"Vlog Pastel","description":"Lifestyle pill: Poppins on a warm pastel plate with stronger contrast for bright vlog footage.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Poppins Regular\", \"Noto Sans Regular\", \"Poppins Bold\", \"Segoe UI\", sans-serif","font_size_px":28,"font_weight":600,"fill_color":"#2c1630","stroke_color":"#2c1630","stroke_width_px":0,"shadow_color":"#8f6aa8","shadow_blur_px":10,"shadow_offset_x_px":0,"shadow_offset_y_px":2,"background_color":"#ffd6e2","background_opacity":92,"background_padding_x_px":18,"background_padding_y_px":7,"background_radius_px":16,"line_spacing_em":1.18,"letter_spacing_em":0,"text_align":"center","line_gap_px":6,"effect":"slide_up"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"glass_frost":{"preset":"glass_frost","label":"Glass Frost","description":"Frosted ice glass: pale translucent ice plate (not a dark bar) with airy Raleway — dark ice-ink text floats on milky glass.","built_in":true,"recommended_max_visible_lines":null,"base":{"font_family":"\"Raleway Regular\", \"Raleway Bold\", \"Noto Sans Regular\", \"Segoe UI\", sans-serif","font_size_px":30,"font_weight":500,"fill_color":"#0b1a2e","stroke_color":"#9eb8d4","stroke_width_px":0,"shadow_color":"#7eb6e8","shadow_blur_px":18,"shadow_offset_x_px":0,"shadow_offset_y_px":2,"background_color":"#eef7ff","background_opacity":44,"background_padding_x_px":22,"background_padding_y_px":10,"background_radius_px":22,"line_spacing_em":1.22,"letter_spacing_em":0.03,"text_align":"center","line_gap_px":8,"effect":"blur_in"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"twitch_lower_third":{"preset":"twitch_lower_third","label":"Twitch Lower-Third","description":"Streamer lower-third chrome: condensed Oswald / Exo on saturated Twitch purple bar, left-aligned with magenta edge glow.","built_in":true,"recommended_max_visible_lines":null,"base":{"font_family":"\"Oswald Regular\", \"Oswald Bold\", \"Exo 2 Regular\", \"Noto Sans Bold\", \"Segoe UI\", sans-serif","font_size_px":28,"font_weight":700,"fill_color":"#ffffff","stroke_color":"#5a1fcf","stroke_width_px":0.8,"shadow_color":"#bf94ff","shadow_blur_px":16,"shadow_offset_x_px":0,"shadow_offset_y_px":0,"background_color":"#9146ff","background_opacity":78,"background_padding_x_px":28,"background_padding_y_px":7,"background_radius_px":0,"line_spacing_em":1.1,"letter_spacing_em":0.04,"text_align":"left","line_gap_px":3,"effect":"slide_up"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"warm_amber":{"preset":"warm_amber","label":"Warm Amber","description":"Night-stream warmth: Open Sans/Noto cream fill with a soft brown shadow — cozy Just Chatting energy.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Open Sans Regular\", \"Open Sans Bold\", \"Noto Sans Regular\", \"Segoe UI\", sans-serif","font_size_px":30,"font_weight":600,"fill_color":"#fff1d6","stroke_color":"#2a1a0c","stroke_width_px":1,"shadow_color":"#1a1008","shadow_blur_px":14,"shadow_offset_x_px":0,"shadow_offset_y_px":3,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":8,"line_spacing_em":1.2,"letter_spacing_em":0,"text_align":"center","line_gap_px":7,"effect":"subtle_pop"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"esports_hud":{"preset":"esports_hud","label":"Esports HUD","description":"Competitive HUD: Exo 2 / Oswald white with a thin cyan outline — sharp, no heavy neon bloom.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Exo 2 Black\", \"Exo 2 Regular\", \"Oswald Bold\", \"Montserrat Bold\", sans-serif","font_size_px":32,"font_weight":800,"fill_color":"#f4fbff","stroke_color":"#00d4ff","stroke_width_px":1.5,"shadow_color":"#001018","shadow_blur_px":6,"shadow_offset_x_px":0,"shadow_offset_y_px":1,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":4,"line_spacing_em":1.12,"letter_spacing_em":0.02,"text_align":"center","line_gap_px":6,"effect":"none"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"dual_caption_modern":{"preset":"dual_caption_modern","label":"Dual Caption Modern","description":"Calm dual-language plate: warm gold source + soft sky translation on a shared dark bar for bilingual streams.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Raleway Bold\", \"Raleway Regular\", \"Noto Sans Regular\", \"Segoe UI\", sans-serif","font_size_px":28,"font_weight":600,"fill_color":"#f0f3f8","stroke_color":"#0a0c10","stroke_width_px":0,"shadow_color":"#000000","shadow_blur_px":4,"shadow_offset_x_px":0,"shadow_offset_y_px":1,"background_color":"#12151c","background_opacity":90,"background_padding_x_px":18,"background_padding_y_px":8,"background_radius_px":8,"line_spacing_em":1.18,"letter_spacing_em":0,"text_align":"center","line_gap_px":6,"effect":"fade"},"line_slots":{"source":{"enabled":true,"fill_color":"#ffcc66","stroke_color":"#2a1e08","stroke_width_px":0},"translation_1":{"enabled":true,"fill_color":"#a8d8ff","stroke_color":"#0a2030","stroke_width_px":0,"font_size_px":26},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}}}"##;

fn built_in_preset_catalog() -> &'static Value {
    static CACHE: OnceLock<Value> = OnceLock::new();
    CACHE.get_or_init(|| {
        serde_json::from_str(BUILT_IN_PRESETS_JSON).expect("valid built-in subtitle presets JSON")
    })
}

fn prettify_custom_preset_name(name: &str) -> String {
    let normalized = name.replace(['_', '-'], " ");
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clone_slot_overrides(overrides: Option<&Value>) -> Value {
    let empty_overrides = Map::new();
    let empty_slot = Map::new();
    let obj = overrides
        .and_then(|v| v.as_object())
        .unwrap_or(&empty_overrides);

    let mut out = Map::new();
    for slot_name in LINE_SLOT_NAMES {
        let slot_raw = obj.get(slot_name).and_then(|v| v.as_object());
        let slot_raw = slot_raw.unwrap_or(&empty_slot);
        let enabled = slot_raw
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut slot_map = Map::new();
        slot_map.insert("enabled".to_string(), json!(enabled));
        for (k, v) in slot_raw {
            slot_map.insert(k.clone(), v.clone());
        }
        out.insert(slot_name.to_string(), Value::Object(slot_map));
    }
    Value::Object(out)
}

fn merge_style_presets_with_custom(custom_presets: Option<&Value>) -> Value {
    let mut catalog = match built_in_preset_catalog().as_object() {
        Some(obj) => obj.clone(),
        None => Map::new(),
    };

    let Some(custom_obj) = custom_presets.and_then(|v| v.as_object()) else {
        return Value::Object(catalog);
    };

    for (preset_name, preset_payload) in custom_obj {
        let Some(payload_obj) = preset_payload.as_object() else {
            continue;
        };
        let label = payload_obj
            .get("label")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| prettify_custom_preset_name(preset_name));
        let description = payload_obj
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "User-created local subtitle style.".to_string());

        let base = payload_obj
            .get("base")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let line_slots = clone_slot_overrides(payload_obj.get("line_slots"));

        catalog.insert(
            preset_name.clone(),
            json!({
                "preset": preset_name,
                "label": label,
                "description": description,
                "built_in": false,
                "recommended_max_visible_lines": payload_obj.get("recommended_max_visible_lines").cloned().unwrap_or(Value::Null),
                "base": base,
                "line_slots": line_slots,
            }),
        );
    }

    Value::Object(catalog)
}

pub fn subtitle_style_presets(subtitle_style: Option<&Value>) -> Value {
    let custom_presets = subtitle_style.and_then(|v| v.get("custom_presets"));
    merge_style_presets_with_custom(custom_presets)
}

fn migrate_unknown_preset_name(name: &str, catalog: &Value) -> String {
    let Some(obj) = catalog.as_object() else {
        return "clean_default".to_string();
    };
    if obj.contains_key(name) {
        return name.to_string();
    }
    let migrated = match name {
        "jp_stream_single" | "jp_dual_caption" => "anime_stream",
        // Collapsed similar “black plate” family into distinct material presets.
        "sakura_soft" => "meeting_soft",
        "minimal_mono" => "glass_frost",
        "editorial_news" => "dark_cinema",
        _ => return "clean_default".to_string(),
    };
    if obj.contains_key(migrated) {
        migrated.to_string()
    } else {
        "clean_default".to_string()
    }
}

fn preset_lookup(catalog: &Value, preset_name: &str) -> Value {
    if let Some(obj) = catalog.as_object() {
        if let Some(p) = obj.get(preset_name) {
            return p.clone();
        }
        if let Some(p) = obj.get("clean_default") {
            return p.clone();
        }
        if let Some((_k, v)) = obj.iter().next() {
            return v.clone();
        }
    }

    // Should never happen, but keep it total.
    json!({"preset":"clean_default","label":"Clean Default","description":"","built_in":true,"recommended_max_visible_lines":null,"base":{},"line_slots":{}})
}

fn value_to_string(v: Option<&Value>, default: &str) -> String {
    v.and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| default.to_string())
}

/// Port of SST `resolve_effective_subtitle_style` (JS renderer-compatible shape).
pub(crate) fn resolve_effective_subtitle_style(subtitle_style: &Value) -> Value {
    let empty = json!({});
    let subtitle_style = if subtitle_style.is_object() {
        subtitle_style
    } else {
        &empty
    };

    let raw_preset = value_to_string(
        subtitle_style
            .get("active_preset")
            .or_else(|| subtitle_style.get("preset")),
        "clean_default",
    );

    let catalog = subtitle_style_presets(Some(subtitle_style));
    let active_preset = migrate_unknown_preset_name(&raw_preset, &catalog);
    let preset = preset_lookup(&catalog, &active_preset);

    let preset_base = preset.get("base").cloned().unwrap_or_else(|| json!({}));
    let raw_base = subtitle_style.get("base").cloned().unwrap_or(preset_base);
    let base = normalize_base_style(&raw_base);

    // UILabels are mostly for tooling; overlay rendering doesn't depend on them.
    let label = subtitle_style
        .get("label")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            preset
                .get("label")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| active_preset.clone());

    let description = subtitle_style
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            preset
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    let built_in = preset
        .get("built_in")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let recommended_max_visible_lines = preset.get("recommended_max_visible_lines").cloned();

    let preset_line_slots = preset.get("line_slots").and_then(|v| v.as_object());

    let mut line_slots: Map<String, Value> = Map::new();
    let nested_line_slots = subtitle_style.get("line_slots").and_then(|v| v.as_object());

    for slot_name in LINE_SLOT_NAMES {
        let mut candidate: Option<&Value> = None;
        if let Some(obj) = nested_line_slots {
            candidate = obj.get(slot_name);
        }
        if candidate.is_none() {
            candidate = subtitle_style.get(slot_name);
        }

        // Compatibility rule:
        // our minimal TS style editor stores `source`/`translation_1` without
        // an explicit `enabled` flag. Treat such objects as "not set" so
        // built-in preset line_slot overrides (dual_tone/dark_cinema) still apply.
        let use_candidate = candidate.and_then(|v| {
            if !v.is_object() || v.get("enabled").is_none() {
                None
            } else {
                Some(v)
            }
        });

        let override_style = if let Some(v) = use_candidate {
            v.clone()
        } else if let Some(preset_obj) = preset_line_slots {
            preset_obj
                .get(slot_name)
                .cloned()
                .unwrap_or_else(|| json!({ "enabled": false }))
        } else {
            json!({ "enabled": false })
        };

        let merged = merge_line_style(&base, &override_style);
        line_slots.insert(slot_name.to_string(), merged);
    }

    // Roles match the JS contract: `roles.source` and `roles.translation` (translation_1).
    let roles = json!({
        "source": line_slots.get("source").cloned().unwrap_or_else(|| base.clone()),
        "translation": line_slots.get("translation_1").cloned().unwrap_or_else(|| base.clone()),
    });

    json!({
        "preset": active_preset,
        "label": label,
        "description": description,
        "built_in": built_in,
        "recommended_max_visible_lines": recommended_max_visible_lines.unwrap_or(Value::Null),
        "effect": base.get("effect").cloned().unwrap_or(Value::String("none".into())),
        "container": {
            "text_align": base.get("text_align").cloned().unwrap_or(Value::String("center".into())),
            "line_gap_px": base.get("line_gap_px").cloned().unwrap_or(Value::from(8)),
        },
        "base": base,
        "line_slots": line_slots,
        "roles": roles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn preserves_extended_web_effects() {
        for effect in [
            "slide_up",
            "zoom_in",
            "blur_in",
            "glow",
            "fade",
            "subtle_pop",
            "pulse",
            "reveal",
            "none",
        ] {
            let payload = json!({
                "preset": "clean_default",
                "base": { "effect": effect }
            });
            let effective = resolve_effective_subtitle_style(&payload);
            assert_eq!(effective["base"]["effect"], effect, "effect {effect}");
        }
    }

    #[test]
    fn rejects_unknown_effect_to_none() {
        let payload = json!({
            "preset": "clean_default",
            "base": { "effect": "spin_wildly" }
        });
        let effective = resolve_effective_subtitle_style(&payload);
        assert_eq!(effective["base"]["effect"], "none");
    }

    #[test]
    fn style_clamps_match_dashboard_ui_limits() {
        let payload = json!({
            "preset": "clean_default",
            "base": {
                "shadow_blur_px": 40,
                "line_spacing_em": 2.5,
                "letter_spacing_em": 0.5,
                "shadow_offset_x_px": -24,
                "stroke_width_px": 4
            }
        });
        let effective = resolve_effective_subtitle_style(&payload);
        let base = &effective["base"];
        assert_eq!(base["shadow_blur_px"].as_f64().unwrap(), 40.0);
        assert_eq!(base["line_spacing_em"].as_f64().unwrap(), 2.5);
        assert_eq!(base["letter_spacing_em"].as_f64().unwrap(), 0.5);
        assert_eq!(base["shadow_offset_x_px"].as_i64().unwrap(), -24);
        assert_eq!(base["stroke_width_px"].as_f64().unwrap(), 4.0);

        let clipped = resolve_effective_subtitle_style(&json!({
            "preset": "clean_default",
            "base": { "stroke_width_px": 12 }
        }));
        assert_eq!(
            clipped["base"]["stroke_width_px"].as_f64().unwrap(),
            4.0,
            "outline width clamps to ASS scale 0–4"
        );
    }

    #[test]
    fn stroke_width_accepts_numeric_strings_including_zero() {
        for (raw, expected) in [
            ("0", 0.0),
            ("1.2", 1.2),
            ("1.25", 1.3),
            ("4.5", 4.0),
            ("12", 4.0),
        ] {
            let payload = json!({
                "preset": "clean_default",
                "base": { "stroke_width_px": raw }
            });
            let effective = resolve_effective_subtitle_style(&payload);
            assert_eq!(
                effective["base"]["stroke_width_px"].as_f64().unwrap(),
                expected,
                "raw stroke {raw}"
            );
        }

        let slot_payload = json!({
            "preset": "clean_default",
            "base": { "stroke_width_px": 2 },
            "line_slots": {
                "source": {
                    "enabled": true,
                    "stroke_width_px": "0"
                }
            }
        });
        let effective = resolve_effective_subtitle_style(&slot_payload);
        assert_eq!(
            effective["line_slots"]["source"]["stroke_width_px"]
                .as_f64()
                .unwrap(),
            0.0
        );
    }

    #[test]
    fn line_slot_override_applies_font_family_and_size() {
        let payload = json!({
            "preset": "clean_default",
            "base": { "font_family": "Base Font", "font_size_px": 30 },
            "line_slots": {
                "source": {
                    "enabled": true,
                    "font_family": "Source Font",
                    "font_size_px": 40
                },
                "translation_1": {
                    "enabled": true,
                    "font_size_px": 24
                }
            }
        });
        let effective = resolve_effective_subtitle_style(&payload);
        assert_eq!(
            effective["line_slots"]["source"]["font_family"],
            "Source Font"
        );
        assert_eq!(effective["line_slots"]["source"]["font_size_px"], 40);
        assert_eq!(
            effective["line_slots"]["translation_1"]["font_family"],
            "Base Font"
        );
        assert_eq!(effective["line_slots"]["translation_1"]["font_size_px"], 24);
    }

    #[test]
    fn builtin_presets_include_accessibility_dark_cinema_meeting_soft() {
        let catalog = subtitle_style_presets(None);
        for key in ["accessibility_high_contrast", "dark_cinema", "meeting_soft"] {
            assert!(catalog.get(key).is_some(), "missing preset {key}");
            assert_eq!(catalog[key]["built_in"], true);
        }
    }

    #[test]
    fn legacy_jp_presets_are_removed() {
        let catalog = subtitle_style_presets(None);
        assert!(catalog.get("jp_dual_caption").is_none());
        assert!(catalog.get("jp_stream_single").is_none());
    }

    #[test]
    fn legacy_jp_preset_migrates_to_anime_stream() {
        for legacy in ["jp_stream_single", "jp_dual_caption"] {
            let payload = json!({ "preset": legacy });
            let effective = resolve_effective_subtitle_style(&payload);
            assert_eq!(effective["preset"], "anime_stream", "legacy {legacy}");
            assert_eq!(effective["label"], "Anime Stream");
        }
    }

    #[test]
    fn builtin_presets_include_distinct_themed_looks() {
        let catalog = subtitle_style_presets(None);
        for key in [
            "retro_terminal",
            "comic_burst",
            "vlog_pastel",
            "anime_stream",
            "glass_frost",
            "twitch_lower_third",
            "meeting_soft",
            "esports_hud",
            "dual_caption_modern",
        ] {
            assert!(catalog.get(key).is_some(), "missing preset {key}");
            assert_eq!(catalog[key]["built_in"], true);
        }
        for removed in ["sakura_soft", "minimal_mono", "editorial_news"] {
            assert!(
                catalog.get(removed).is_none(),
                "collapsed plate preset {removed} should be removed"
            );
        }
    }

    #[test]
    fn collapsed_plate_presets_migrate() {
        for (legacy, target) in [
            ("sakura_soft", "meeting_soft"),
            ("minimal_mono", "glass_frost"),
            ("editorial_news", "dark_cinema"),
        ] {
            let effective = resolve_effective_subtitle_style(&json!({ "preset": legacy }));
            assert_eq!(effective["preset"], target, "legacy {legacy}");
        }
    }

    #[test]
    fn readable_plate_family_uses_distinct_materials() {
        let catalog = subtitle_style_presets(None);
        let max_bg = catalog["accessibility_high_contrast"]["base"]["background_color"]
            .as_str()
            .unwrap_or("");
        let podcast_bg = catalog["meeting_soft"]["base"]["background_color"]
            .as_str()
            .unwrap_or("");
        let glass_bg = catalog["glass_frost"]["base"]["background_color"]
            .as_str()
            .unwrap_or("");
        let twitch_bg = catalog["twitch_lower_third"]["base"]["background_color"]
            .as_str()
            .unwrap_or("");
        assert_eq!(max_bg.to_ascii_lowercase(), "#000000");
        assert_ne!(podcast_bg.to_ascii_lowercase(), "#000000");
        assert_ne!(glass_bg.to_ascii_lowercase(), "#000000");
        assert_ne!(twitch_bg.to_ascii_lowercase(), "#000000");
        assert_ne!(podcast_bg, glass_bg);
        assert_ne!(glass_bg, twitch_bg);
        assert_eq!(catalog["twitch_lower_third"]["base"]["text_align"], "left");
        // Light translucent materials — not dark charcoal boxes.
        assert_eq!(
            catalog["meeting_soft"]["base"]["background_color"],
            "#f4e8d6"
        );
        assert_eq!(
            catalog["glass_frost"]["base"]["background_color"],
            "#eef7ff"
        );
        assert_eq!(
            catalog["twitch_lower_third"]["base"]["background_color"],
            "#9146ff"
        );
        let glass_op = catalog["glass_frost"]["base"]["background_opacity"]
            .as_i64()
            .unwrap_or(100);
        assert!(
            (30..=55).contains(&glass_op),
            "glass_frost must stay milky-translucent, got {glass_op}%"
        );
        assert_eq!(
            catalog["glass_frost"]["base"]["fill_color"]
                .as_str()
                .unwrap_or("")
                .to_ascii_lowercase(),
            "#0b1a2e",
            "frosted ice plate uses dark ice-ink text"
        );
    }

    #[test]
    fn themed_presets_use_dedicated_font_tokens() {
        let catalog = subtitle_style_presets(None);
        for (preset, token) in [
            ("anime_stream", "Mochiy Pop One"),
            ("retro_terminal", "VT323"),
            ("fallout_pipboy", "Share Tech Mono"),
            ("comic_burst", "Bangers"),
            ("cyberpunk_neon", "Orbitron"),
            ("noir_typewriter", "Special Elite"),
            ("dark_cinema", "Playfair Display"),
            ("accessibility_high_contrast", "Montserrat"),
        ] {
            let family = catalog[preset]["base"]["font_family"]
                .as_str()
                .unwrap_or("");
            assert!(
                family.contains(token),
                "preset {preset} expected font token {token} in {family}"
            );
        }
    }

    #[test]
    fn themed_presets_include_cyrillic_capable_fallback() {
        let catalog = subtitle_style_presets(None);
        for (preset, fallback_token) in [
            ("anime_stream", "Comfortaa Bold"),
            ("retro_terminal", "IBM Plex Serif"),
            ("fallout_pipboy", "Ubuntu Mono"),
            ("comic_burst", "Comic Relief"),
            ("cyberpunk_neon", "Exo 2"),
            ("noir_typewriter", "IBM Plex Mono"),
            ("dual_tone", "Noto Sans"),
            ("vlog_pastel", "Noto Sans"),
            ("clean_default", "Noto Sans"),
            ("streamer_bold", "Montserrat"),
            ("compact_overlay", "Noto Sans"),
            ("meeting_soft", "Noto Sans"),
            ("glass_frost", "Noto Sans"),
            ("twitch_lower_third", "Noto Sans"),
        ] {
            let family = catalog[preset]["base"]["font_family"]
                .as_str()
                .unwrap_or("");
            assert!(
                family.contains(fallback_token),
                "preset {preset} missing Cyrillic fallback {fallback_token} in {family}"
            );
        }
    }

    #[test]
    fn latin_only_primaries_put_cyrillic_project_face_before_system_generics() {
        let catalog = subtitle_style_presets(None);
        // Faces shipped without a Cyrillic cmap — must not be the only project face
        // before Consolas/Segoe steal the script with the wrong look.
        let cases = [
            ("retro_terminal", "VT323 Regular", "IBM Plex Serif"),
            ("fallout_pipboy", "Share Tech Mono Regular", "Ubuntu Mono"),
            ("noir_typewriter", "Special Elite Regular", "IBM Plex Mono"),
            ("comic_burst", "Bangers Regular", "Comic Relief"),
            ("cyberpunk_neon", "Orbitron Black", "Exo 2"),
            ("anime_stream", "Mochiy Pop One Regular", "Comfortaa Bold"),
            ("dual_tone", "Lato Regular", "Noto Sans"),
            ("vlog_pastel", "Poppins Regular", "Noto Sans"),
        ];
        for (preset, primary, cyr_face) in cases {
            let family = catalog[preset]["base"]["font_family"]
                .as_str()
                .unwrap_or("");
            let primary_at = family.find(primary).expect("primary face");
            let cyr_at = family.find(cyr_face).expect("cyrillic face");
            assert!(
                cyr_at > primary_at,
                "{preset}: {cyr_face} must follow {primary} in {family}"
            );
            for system in [
                "Consolas",
                "Segoe UI",
                "Courier New",
                "sans-serif",
                "monospace",
            ] {
                if let Some(sys_at) = family.find(system) {
                    assert!(
                        cyr_at < sys_at,
                        "{preset}: {cyr_face} must precede system generic {system} in {family}"
                    );
                }
            }
        }
    }

    #[test]
    fn redesigned_mono_noir_family_uses_plex_stacks() {
        let catalog = subtitle_style_presets(None);
        let noir = catalog["noir_typewriter"]["base"]["font_family"]
            .as_str()
            .unwrap_or("");
        assert!(noir.contains("Special Elite Regular"));
        assert!(noir.contains("IBM Plex Mono Medium"));
        assert!(
            (noir.find("Special Elite Regular").unwrap())
                < (noir.find("IBM Plex Mono Medium").unwrap())
        );
        assert_eq!(
            catalog["noir_typewriter"]["base"]["background_radius_px"],
            0
        );
        assert!(
            catalog["noir_typewriter"]["base"]["letter_spacing_em"]
                .as_f64()
                .unwrap()
                >= 0.05
        );

        let retro = catalog["retro_terminal"]["base"]["font_family"]
            .as_str()
            .unwrap_or("");
        assert!(retro.contains("VT323 Regular"));
        assert!(retro.contains("IBM Plex Serif Regular"));
    }

    #[test]
    fn jetbrains_font_alias_canonicalizes_to_registered_family() {
        let payload = json!({
            "preset": "glass_frost",
            "base": {
                "font_family": "\"JetBrains Mono Regular\", \"PT Mono Regular\", monospace"
            }
        });
        let effective = resolve_effective_subtitle_style(&payload);
        let family = effective["base"]["font_family"].as_str().unwrap_or("");
        assert!(family.contains("Jet Brains Mono Regular"));
        assert!(!family.contains("\"JetBrains Mono Regular\""));
    }

    #[test]
    fn anime_stream_keeps_dual_script_font_stack() {
        let catalog = subtitle_style_presets(None);
        let family = catalog["anime_stream"]["base"]["font_family"]
            .as_str()
            .unwrap_or("");
        assert!(family.contains("Mochiy Pop One"), "latin/jp face");
        assert!(family.contains("Comfortaa Bold"), "cyrillic-matching face");
        assert!(
            family.contains("Comic Relief Bold"),
            "cyrillic comic fallback"
        );
        assert!(
            !family.contains("Noto Sans"),
            "Noto must not steal anime Cyrillic fallback styling"
        );
        assert_eq!(catalog["anime_stream"]["base"]["font_size_px"], 40);
        assert_eq!(catalog["anime_stream"]["base"]["stroke_width_px"], 1.0);
    }

    #[test]
    fn meeting_soft_uses_readable_plate_on_bright_backgrounds() {
        let catalog = subtitle_style_presets(None);
        let base = &catalog["meeting_soft"]["base"];
        let opacity = base["background_opacity"].as_i64().unwrap_or(0);
        assert!(
            opacity >= 75,
            "meeting_soft parchment plate needs enough body for bright slides"
        );
        assert_eq!(
            base["background_color"]
                .as_str()
                .unwrap_or("")
                .to_ascii_lowercase(),
            "#f4e8d6"
        );
        // Dark ink on cream paper — not white-on-charcoal.
        assert_ne!(
            base["fill_color"]
                .as_str()
                .unwrap_or("")
                .to_ascii_lowercase(),
            "#ffffff"
        );
    }

    #[test]
    fn plate_backed_presets_use_opaque_enough_plates() {
        let catalog = subtitle_style_presets(None);
        // Intentionally translucent materials (frosted glass / stream chrome).
        const TRANSLUCENT_OK: &[&str] = &["glass_frost", "twitch_lower_third", "meeting_soft"];
        for (key, preset) in catalog.as_object().expect("catalog object") {
            if preset.get("built_in").and_then(|v| v.as_bool()) != Some(true) {
                continue;
            }
            let opacity = preset["base"]["background_opacity"].as_i64().unwrap_or(0);
            if opacity <= 0 {
                continue;
            }
            if TRANSLUCENT_OK.contains(&key.as_str()) {
                continue;
            }
            assert!(
                opacity >= 88,
                "built-in preset {key} uses plate opacity {opacity}% (< 88)"
            );
        }
    }

    #[test]
    fn max_contrast_is_truly_high_contrast() {
        let catalog = subtitle_style_presets(None);
        let base = &catalog["accessibility_high_contrast"]["base"];
        assert_eq!(
            base["fill_color"]
                .as_str()
                .unwrap_or("")
                .to_ascii_lowercase(),
            "#ffffff"
        );
        assert_eq!(
            base["background_color"]
                .as_str()
                .unwrap_or("")
                .to_ascii_lowercase(),
            "#000000"
        );
        assert_eq!(base["background_opacity"].as_i64(), Some(100));
    }

    #[test]
    fn builtin_presets_use_visually_distinct_fonts_and_fills() {
        let catalog = subtitle_style_presets(None);
        let mut seen_signatures = std::collections::HashSet::new();
        for (key, preset) in catalog.as_object().expect("catalog object") {
            if preset.get("built_in").and_then(|v| v.as_bool()) != Some(true) {
                continue;
            }
            let font_family = preset["base"]["font_family"]
                .as_str()
                .unwrap_or("")
                .split(',')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            let fill_color = preset["base"]["fill_color"]
                .as_str()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            let signature = (font_family.clone(), fill_color.clone());
            assert!(
                seen_signatures.insert(signature),
                "built-in preset {key} reuses ({font_family}, {fill_color})"
            );
        }
    }

    #[test]
    fn new_modern_presets_use_expected_font_tokens() {
        let catalog = subtitle_style_presets(None);
        for (preset, token) in [
            ("glass_frost", "Raleway"),
            ("twitch_lower_third", "Oswald"),
            ("meeting_soft", "Merriweather"),
            ("warm_amber", "Open Sans"),
            ("esports_hud", "Exo 2"),
            ("dual_caption_modern", "Raleway"),
            ("retro_terminal", "IBM Plex Serif"),
        ] {
            let family = catalog[preset]["base"]["font_family"]
                .as_str()
                .unwrap_or("");
            assert!(
                family.contains(token),
                "preset {preset} expected font token {token} in {family}"
            );
        }
        assert_eq!(
            catalog["dual_caption_modern"]["line_slots"]["source"]["enabled"],
            true
        );
        assert_eq!(
            catalog["dual_caption_modern"]["line_slots"]["translation_1"]["enabled"],
            true
        );
    }

    #[test]
    fn custom_preset_merges_into_catalog() {
        let payload = json!({
            "custom_presets": {
                "stream": {
                    "label": "Stream",
                    "base": { "font_size_px": 44 }
                }
            }
        });
        let catalog = subtitle_style_presets(Some(&payload));
        assert_eq!(catalog["stream"]["label"], "Stream");
        assert_eq!(catalog["stream"]["built_in"], false);
    }
}
