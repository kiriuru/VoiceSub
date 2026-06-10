use std::sync::OnceLock;

use serde_json::{json, Map, Value};

const LINE_SLOT_NAMES: [&str; 6] = [
    "source",
    "translation_1",
    "translation_2",
    "translation_3",
    "translation_4",
    "translation_5",
];

const EFFECT_IDS: [&str; 7] = [
    "none",
    "fade",
    "subtle_pop",
    "slide_up",
    "zoom_in",
    "blur_in",
    "glow",
];

fn clamp_i64(raw: &Value, default: i64, min: i64, max: i64) -> i64 {
    let parsed = raw
        .as_i64()
        .or_else(|| raw.as_u64().map(|v| v as i64))
        .or_else(|| raw.as_f64().map(|v| v as i64));
    let value = parsed.unwrap_or(default);
    value.clamp(min, max)
}

fn clamp_f64(raw: &Value, default: f64, min: f64, max: f64) -> f64 {
    let parsed = raw.as_f64().or_else(|| {
        raw.as_i64()
            .map(|v| v as f64)
            .or_else(|| raw.as_u64().map(|v| v as f64))
    });
    let value = parsed.unwrap_or(default);
    value.clamp(min, max)
}

fn normalize_str(raw: &Value, default: &str) -> String {
    let s = raw.as_str().unwrap_or(default).trim().to_string();
    if s.is_empty() {
        default.to_string()
    } else {
        s
    }
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

    let font_family = normalize_str(
        font_family,
        defaults["font_family"].as_str().unwrap_or_default(),
    );
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

    let stroke_width_px = round_to(
        clamp_f64(
            stroke_width_px,
            defaults["stroke_width_px"].as_f64().unwrap_or(2.0),
            0.0,
            8.0,
        ),
        2,
    );
    let shadow_blur_px = round_to(
        clamp_f64(
            shadow_blur_px,
            defaults["shadow_blur_px"].as_f64().unwrap_or(10.0),
            0.0,
            32.0,
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
            2.2,
        ),
        2,
    );
    let letter_spacing_em = round_to(
        clamp_f64(
            letter_spacing_em,
            defaults["letter_spacing_em"].as_f64().unwrap_or(0.0),
            -0.08,
            0.2,
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
    for (key, base_value) in base_obj.iter() {
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

const BUILT_IN_PRESETS_JSON: &str = r##"{"clean_default":{"preset":"clean_default","label":"Clean Default","description":"Neutral baseline: Inter on a transparent background with a minimal black outline.","built_in":true,"recommended_max_visible_lines":null,"base":{"font_family":"\"Inter Regular\", \"Segoe UI\", Tahoma, sans-serif","font_size_px":30,"font_weight":500,"fill_color":"#ffffff","stroke_color":"#1c1f25","stroke_width_px":1.5,"shadow_color":"#000000","shadow_blur_px":8,"shadow_offset_x_px":0,"shadow_offset_y_px":2,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":10,"line_spacing_em":1.15,"letter_spacing_em":0,"text_align":"center","line_gap_px":8,"effect":"none"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"streamer_bold":{"preset":"streamer_bold","label":"Streamer Neon","description":"Loud display look: Oswald with a cyan fill and a hot-magenta glow for live gameplay.","built_in":true,"recommended_max_visible_lines":null,"base":{"font_family":"\"Oswald Bold\", \"Impact\", \"Arial Narrow Bold\", sans-serif","font_size_px":38,"font_weight":800,"fill_color":"#00f0ff","stroke_color":"#0a0612","stroke_width_px":3.5,"shadow_color":"#ff2bd6","shadow_blur_px":24,"shadow_offset_x_px":0,"shadow_offset_y_px":0,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":10,"line_spacing_em":1.15,"letter_spacing_em":0.02,"text_align":"center","line_gap_px":10,"effect":"glow"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"dual_tone":{"preset":"dual_tone","label":"Dual Color","description":"Lato body with distinct fill colors per slot so source and each translation read at a glance.","built_in":true,"recommended_max_visible_lines":null,"base":{"font_family":"\"Lato Regular\", \"Verdana\", \"Segoe UI\", sans-serif","font_size_px":30,"font_weight":700,"fill_color":"#ffffff","stroke_color":"#13151a","stroke_width_px":2,"shadow_color":"#000000","shadow_blur_px":6,"shadow_offset_x_px":0,"shadow_offset_y_px":2,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":10,"line_spacing_em":1.15,"letter_spacing_em":0.0,"text_align":"center","line_gap_px":8,"effect":"fade"},"line_slots":{"source":{"enabled":true,"fill_color":"#ffd60a","stroke_color":"#4a3000"},"translation_1":{"enabled":true,"fill_color":"#7be2ff","stroke_color":"#0b2c3a"},"translation_2":{"enabled":true,"fill_color":"#a8ffb8","stroke_color":"#0b3a18"},"translation_3":{"enabled":true,"fill_color":"#ff9fc5","stroke_color":"#3a0b22"},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"compact_overlay":{"preset":"compact_overlay","label":"Compact Bar","description":"Source Sans 3 inside a tight semi-opaque black bar � small footprint, maximum legibility.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Source Sans 3 Regular\", \"Source Sans 3 Bold\", \"Segoe UI\", sans-serif","font_size_px":22,"font_weight":600,"fill_color":"#ffffff","stroke_color":"#000000","stroke_width_px":0,"shadow_color":"#000000","shadow_blur_px":0,"shadow_offset_x_px":0,"shadow_offset_y_px":0,"background_color":"#0a0d12","background_opacity":90,"background_padding_x_px":14,"background_padding_y_px":4,"background_radius_px":4,"line_spacing_em":1.05,"letter_spacing_em":0,"text_align":"center","line_gap_px":4,"effect":"none"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"soft_shadow":{"preset":"soft_shadow","label":"Soft Cloud","description":"Comfortaa with a wide diffused shadow and zero outline � feels airy, no edge crunch.","built_in":true,"recommended_max_visible_lines":null,"base":{"font_family":"\"Comfortaa Regular\", \"Segoe UI\", sans-serif","font_size_px":30,"font_weight":500,"fill_color":"#fff8eb","stroke_color":"#3a2a18","stroke_width_px":0,"shadow_color":"#1d1410","shadow_blur_px":22,"shadow_offset_x_px":0,"shadow_offset_y_px":6,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":10,"line_spacing_em":1.15,"letter_spacing_em":0.01,"text_align":"center","line_gap_px":9,"effect":"subtle_pop"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"anime_stream":{"preset":"anime_stream","label":"Anime Stream","description":"Mochiy Pop One for Latin/Japanese + Comfortaa Bold for Cyrillic � classic anime fansub caption: white fill, crisp violet outline, soft dark drop shadow.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Mochiy Pop One Regular\", \"Comfortaa Bold\", \"Underdog Regular\", \"Bangers Regular\", \"Comic Relief Bold\", \"Poppins Bold\", \"Segoe UI\", sans-serif","font_size_px":40,"font_weight":800,"fill_color":"#ffffff","stroke_color":"#3a1a5c","stroke_width_px":1,"shadow_color":"#15071f","shadow_blur_px":8,"shadow_offset_x_px":0,"shadow_offset_y_px":3,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":10,"line_spacing_em":1.1,"letter_spacing_em":0.015,"text_align":"center","line_gap_px":6,"effect":"subtle_pop"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"accessibility_high_contrast":{"preset":"accessibility_high_contrast","label":"Max Contrast","description":"Pure white Montserrat Bold on a solid 100%-opaque black plate � WCAG AAA contrast in any environment.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Montserrat Bold\", \"Montserrat Regular\", \"Segoe UI\", sans-serif","font_size_px":36,"font_weight":800,"fill_color":"#ffffff","stroke_color":"#000000","stroke_width_px":0,"shadow_color":"#000000","shadow_blur_px":0,"shadow_offset_x_px":0,"shadow_offset_y_px":0,"background_color":"#000000","background_opacity":100,"background_padding_x_px":24,"background_padding_y_px":10,"background_radius_px":6,"line_spacing_em":1.2,"letter_spacing_em":0.02,"text_align":"center","line_gap_px":8,"effect":"none"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"dark_cinema":{"preset":"dark_cinema","label":"Cinema Plate","description":"Playfair Display ivory on a solid warm sepia plate � letterboxed art-house aesthetic, readable on any background.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Playfair Display Bold\", \"Playfair Display Regular\", Georgia, \"Times New Roman\", serif","font_size_px":30,"font_weight":700,"fill_color":"#f4e3b8","stroke_color":"#1a0a05","stroke_width_px":0,"shadow_color":"#08040a","shadow_blur_px":6,"shadow_offset_x_px":0,"shadow_offset_y_px":2,"background_color":"#1a0d08","background_opacity":95,"background_padding_x_px":26,"background_padding_y_px":10,"background_radius_px":4,"line_spacing_em":1.18,"letter_spacing_em":0.02,"text_align":"center","line_gap_px":8,"effect":"fade"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":true,"fill_color":"#e8d4a0","stroke_color":"#08040a","font_size_px":24},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"meeting_soft":{"preset":"meeting_soft","label":"Podcast Subtle","description":"Roboto Regular in light grey with no stroke and no plate � minimal, talking-head friendly.","built_in":true,"recommended_max_visible_lines":3,"base":{"font_family":"\"Roboto Regular\", \"Segoe UI\", \"Calibri\", sans-serif","font_size_px":24,"font_weight":400,"fill_color":"#e8edf5","stroke_color":"#000000","stroke_width_px":0,"shadow_color":"#0b1018","shadow_blur_px":8,"shadow_offset_x_px":0,"shadow_offset_y_px":1,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":10,"line_spacing_em":1.18,"letter_spacing_em":0,"text_align":"center","line_gap_px":5,"effect":"none"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"retro_terminal":{"preset":"retro_terminal","label":"Retro Terminal","description":"VT323 amber phosphor on a dark CRT panel � DEC VT320 / Apple ][ vibe with PT Mono for Cyrillic.","built_in":true,"recommended_max_visible_lines":3,"base":{"font_family":"\"VT323 Regular\", \"PT Mono Regular\", \"Share Tech Mono Regular\", \"Consolas\", \"Courier New\", monospace","font_size_px":36,"font_weight":400,"fill_color":"#ffb000","stroke_color":"#3a1c00","stroke_width_px":0,"shadow_color":"#ff8800","shadow_blur_px":14,"shadow_offset_x_px":0,"shadow_offset_y_px":0,"background_color":"#0a0805","background_opacity":92,"background_padding_x_px":18,"background_padding_y_px":6,"background_radius_px":2,"line_spacing_em":1.05,"letter_spacing_em":0.04,"text_align":"center","line_gap_px":4,"effect":"glow"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"fallout_pipboy":{"preset":"fallout_pipboy","label":"Fallout Pip-Boy","description":"Share Tech Mono in Pip-Boy phosphor green with a strong scanline glow; Ubuntu Mono Bold covers Cyrillic.","built_in":true,"recommended_max_visible_lines":3,"base":{"font_family":"\"Share Tech Mono Regular\", \"Ubuntu Mono Bold\", \"Ubuntu Mono Regular\", \"PT Mono Regular\", \"VT323 Regular\", \"Consolas\", \"Courier New\", monospace","font_size_px":30,"font_weight":400,"fill_color":"#4cff79","stroke_color":"#001f0a","stroke_width_px":0,"shadow_color":"#16ff3c","shadow_blur_px":18,"shadow_offset_x_px":0,"shadow_offset_y_px":0,"background_color":"#020806","background_opacity":95,"background_padding_x_px":18,"background_padding_y_px":6,"background_radius_px":2,"line_spacing_em":1.08,"letter_spacing_em":0.05,"text_align":"center","line_gap_px":5,"effect":"glow"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"comic_burst":{"preset":"comic_burst","label":"Comic Burst","description":"Bangers in comic-yellow with a chunky black outline and a hot-red shadow � Marvel SFX panel energy; Comic Relief Bold covers Cyrillic.","built_in":true,"recommended_max_visible_lines":1,"base":{"font_family":"\"Bangers Regular\", \"Comic Relief Bold\", \"Comic Relief Regular\", \"Impact\", \"Arial Black\", sans-serif","font_size_px":46,"font_weight":400,"fill_color":"#ffd60a","stroke_color":"#0a0a0a","stroke_width_px":5,"shadow_color":"#d6172a","shadow_blur_px":4,"shadow_offset_x_px":4,"shadow_offset_y_px":6,"background_color":"#000000","background_opacity":0,"background_padding_x_px":12,"background_padding_y_px":4,"background_radius_px":10,"line_spacing_em":1.15,"letter_spacing_em":0.045,"text_align":"center","line_gap_px":8,"effect":"zoom_in"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"cyberpunk_neon":{"preset":"cyberpunk_neon","label":"Cyberpunk Neon","description":"Orbitron Black in hot magenta with a cyan halo glow on a deep navy plate; Exo 2 Black handles Cyrillic with the same sci-fi geometry.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Orbitron Black\", \"Exo 2 Black\", \"Orbitron Regular\", \"Exo 2 Regular\", \"Audiowide\", sans-serif","font_size_px":32,"font_weight":900,"fill_color":"#ff2bd6","stroke_color":"#03001a","stroke_width_px":2,"shadow_color":"#00f0ff","shadow_blur_px":22,"shadow_offset_x_px":0,"shadow_offset_y_px":0,"background_color":"#070416","background_opacity":88,"background_padding_x_px":20,"background_padding_y_px":8,"background_radius_px":4,"line_spacing_em":1.15,"letter_spacing_em":0.06,"text_align":"center","line_gap_px":7,"effect":"glow"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"noir_typewriter":{"preset":"noir_typewriter","label":"Film Noir","description":"Special Elite typewriter on a deep ink plate � 1940s detective / typewritten dossier mood; Cutive Mono carries the same vibe for Cyrillic.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Special Elite Regular\", \"Cutive Mono Regular\", \"PT Mono Regular\", \"Courier New\", \"Consolas\", monospace","font_size_px":28,"font_weight":400,"fill_color":"#ece1c4","stroke_color":"#1a1208","stroke_width_px":0,"shadow_color":"#000000","shadow_blur_px":4,"shadow_offset_x_px":0,"shadow_offset_y_px":2,"background_color":"#100a06","background_opacity":92,"background_padding_x_px":22,"background_padding_y_px":10,"background_radius_px":0,"line_spacing_em":1.18,"letter_spacing_em":0.03,"text_align":"center","line_gap_px":6,"effect":"fade"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}},"vlog_pastel":{"preset":"vlog_pastel","label":"Vlog Pastel","description":"Poppins on a warm pastel pill � cozy lifestyle / vlog look, plays nicely with soft backgrounds.","built_in":true,"recommended_max_visible_lines":2,"base":{"font_family":"\"Poppins Regular\", \"Poppins Bold\", \"Segoe UI\", sans-serif","font_size_px":28,"font_weight":600,"fill_color":"#3a1e3d","stroke_color":"#3a1e3d","stroke_width_px":0,"shadow_color":"#a37fc3","shadow_blur_px":14,"shadow_offset_x_px":0,"shadow_offset_y_px":3,"background_color":"#ffdce5","background_opacity":90,"background_padding_x_px":18,"background_padding_y_px":7,"background_radius_px":22,"line_spacing_em":1.15,"letter_spacing_em":0.0,"text_align":"center","line_gap_px":6,"effect":"slide_up"},"line_slots":{"source":{"enabled":false},"translation_1":{"enabled":false},"translation_2":{"enabled":false},"translation_3":{"enabled":false},"translation_4":{"enabled":false},"translation_5":{"enabled":false}}}}"##;

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
        for (k, v) in slot_raw.iter() {
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
            "slide_up", "zoom_in", "blur_in", "glow", "fade", "subtle_pop", "none",
        ] {
            let payload = json!({
                "preset": "clean_default",
                "base": { "effect": effect }
            });
            let effective = resolve_effective_subtitle_style(&payload);
            assert_eq!(
                effective["base"]["effect"], effect,
                "effect {effect}"
            );
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
        assert_eq!(effective["line_slots"]["source"]["font_family"], "Source Font");
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
        for key in [
            "accessibility_high_contrast",
            "dark_cinema",
            "meeting_soft",
        ] {
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
        ] {
            assert!(catalog.get(key).is_some(), "missing preset {key}");
            assert_eq!(catalog[key]["built_in"], true);
        }
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
            ("anime_stream", "Underdog Regular"),
            ("retro_terminal", "PT Mono Regular"),
            ("fallout_pipboy", "Ubuntu Mono"),
            ("comic_burst", "Comic Relief"),
            ("cyberpunk_neon", "Exo 2"),
            ("noir_typewriter", "Cutive Mono"),
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
    fn plate_backed_presets_use_opaque_enough_plates() {
        let catalog = subtitle_style_presets(None);
        for (key, preset) in catalog.as_object().expect("catalog object") {
            if preset.get("built_in").and_then(|v| v.as_bool()) != Some(true) {
                continue;
            }
            let opacity = preset["base"]["background_opacity"]
                .as_i64()
                .unwrap_or(0);
            if opacity <= 0 {
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
            base["fill_color"].as_str().unwrap_or("").to_ascii_lowercase(),
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
