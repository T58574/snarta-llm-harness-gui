use image::imageops::FilterType;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const DEFAULT_PERSONA_NAME: &str = "default";
pub const DEFAULT_PERSONA_PROMPT: &str = "\
Ты - Snarta, спокойный, точный и прямой локальный собеседник и партнёр по размышлениям.
Отвечай по делу, без воды, канцелярита и извинений. Если нужно запомнить важный факт о собеседнике или его планах, используй инструмент remember.
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Persona {
    pub name: String,
    pub display_name: String,
    pub path: PathBuf,
    pub system_prompt: String,
    pub description: Option<String>,
    pub age: Option<String>,
    pub character: Option<String>,
    pub modifiers: Option<String>,
    pub avatar_path: Option<PathBuf>,
    pub avatar_bg_path: Option<PathBuf>,
}

pub fn ensure_default_persona(personas_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(personas_dir)?;
    let default_path = personas_dir.join(format!("{DEFAULT_PERSONA_NAME}.md"));
    if !default_path.exists() {
        fs::write(&default_path, DEFAULT_PERSONA_PROMPT)?;
    }
    Ok(())
}

pub fn list_personas(personas_dir: &Path) -> io::Result<Vec<Persona>> {
    ensure_default_persona(personas_dir)?;
    let mut out = Vec::new();
    for entry in fs::read_dir(personas_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_md = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("md"));
        if !is_md {
            continue;
        }
        if let Ok(p) = load_persona_file(&path) {
            out.push(p);
        }
    }
    out.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(out)
}

pub fn load_persona(personas_dir: &Path, name: &str) -> io::Result<Persona> {
    ensure_default_persona(personas_dir)?;
    let clean = name.trim().trim_end_matches(".md");
    if clean.is_empty()
        || clean.contains('/')
        || clean.contains('\\')
        || clean.contains(':')
        || clean.contains("..")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid persona name '{clean}'"),
        ));
    }
    let target = personas_dir.join(format!("{clean}.md"));
    if target.is_file() {
        return load_persona_file(&target);
    }
    let prefixed = personas_dir.join(format!("persona_{clean}.md"));
    if prefixed.is_file() {
        return load_persona_file(&prefixed);
    }
    let all = list_personas(personas_dir)?;
    if let Some(found) = all.into_iter().find(|p| {
        p.name.eq_ignore_ascii_case(clean) || p.display_name.eq_ignore_ascii_case(clean)
    }) {
        return Ok(found);
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("persona '{clean}' not found in {}", personas_dir.display()),
    ))
}

pub fn load_persona_file(path: &Path) -> io::Result<Persona> {
    let raw = fs::read_to_string(path)?;
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(DEFAULT_PERSONA_NAME)
        .to_string();
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let (meta, body) = split_optional_toml_frontmatter(&raw);

    let fallback_display = file_stem
        .strip_prefix("persona_")
        .filter(|s| !s.is_empty())
        .unwrap_or(&file_stem)
        .to_string();
    let display_name = meta.name.unwrap_or(fallback_display);

    let resolve_rel = |rel: Option<String>| -> Option<PathBuf> {
        let r = rel?;
        let trimmed = r.trim();
        if trimmed.is_empty() {
            return None;
        }
        let candidate = PathBuf::from(trimmed);
        let full = if candidate.is_absolute() {
            candidate
        } else {
            base_dir.join(candidate)
        };
        full.is_file().then_some(full)
    };

    Ok(Persona {
        name: file_stem,
        display_name,
        path: path.to_path_buf(),
        system_prompt: body.trim().to_string(),
        description: meta.description,
        age: meta.age,
        character: meta.character,
        modifiers: meta.modifiers,
        avatar_path: resolve_rel(meta.avatar),
        avatar_bg_path: resolve_rel(meta.avatar_bg),
    })
}

#[derive(Default)]
struct FrontmatterMeta {
    name: Option<String>,
    description: Option<String>,
    age: Option<String>,
    character: Option<String>,
    modifiers: Option<String>,
    avatar: Option<String>,
    avatar_bg: Option<String>,
}

fn split_optional_toml_frontmatter(raw: &str) -> (FrontmatterMeta, &str) {
    let trimmed = raw.trim_start_matches('\u{feff}');
    for delim in ["+++", "---"] {
        if let Some(rest) = trimmed.strip_prefix(delim)
            && let Some(after_first_nl) = rest
                .strip_prefix('\n')
                .or_else(|| rest.strip_prefix("\r\n"))
            && let Some(end_idx) = after_first_nl.find(&format!("\n{delim}"))
        {
            let header = &after_first_nl[..end_idx];
            let tail = &after_first_nl[end_idx + 1 + delim.len()..];
            let body = tail
                .strip_prefix('\n')
                .or_else(|| tail.strip_prefix("\r\n"))
                .unwrap_or(tail);
            let parsed = toml::from_str::<toml::Value>(header).ok();
            let get_str = |key: &str| -> Option<String> {
                parsed
                    .as_ref()
                    .and_then(|v| v.get(key)?.as_str().map(str::trim))
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            };
            let meta = FrontmatterMeta {
                name: get_str("name"),
                description: get_str("description"),
                age: get_str("age"),
                character: get_str("character"),
                modifiers: get_str("modifiers"),
                avatar: get_str("avatar"),
                avatar_bg: get_str("avatar_bg"),
            };
            return (meta, body);
        }
    }
    (FrontmatterMeta::default(), trimmed)
}

pub fn sanitize_persona_slug(display_name: &str) -> String {
    let mut slug = String::new();
    for ch in display_name.trim().chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            slug.push(ch);
        } else if ch.is_whitespace() && !slug.ends_with('_') {
            slug.push('_');
        }
    }
    let slug = slug.trim_matches('_').to_string();
    if slug.is_empty() {
        "custom".to_string()
    } else {
        slug
    }
}

pub fn format_persona_markdown(
    display_name: &str,
    age: &str,
    character: &str,
    modifiers: &str,
) -> String {
    let mut out = format!("# Персона: {}\n", display_name.trim());
    out.push_str(&format!("- Имя: {}\n", display_name.trim()));
    if !age.trim().is_empty() {
        out.push_str(&format!("- Возраст: {}\n", age.trim()));
    }
    if !character.trim().is_empty() {
        out.push_str(&format!("- Характер: {}\n", character.trim()));
    }
    if !modifiers.trim().is_empty() {
        out.push_str("\n## Модификаторы и стиль общения\n");
        out.push_str(modifiers.trim());
        out.push('\n');
    }
    out
}

#[derive(Debug, Clone)]
pub struct PersonaSaveInput {
    pub existing_file_stem: Option<String>,
    pub display_name: String,
    pub age: String,
    pub character: String,
    pub modifiers: String,
    pub avatar_rel: Option<String>,
    pub avatar_bg_rel: Option<String>,
}

/// Saves a structured persona in unified `persona_<Name>.md` format (or updates an existing file).
pub fn save_structured_persona(personas_dir: &Path, input: &PersonaSaveInput) -> io::Result<Persona> {
    fs::create_dir_all(personas_dir)?;
    let display = input.display_name.trim();
    if display.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Имя персоны не может быть пустым",
        ));
    }

    let slug = sanitize_persona_slug(display);
    let target_stem = match &input.existing_file_stem {
        Some(stem) if stem == DEFAULT_PERSONA_NAME => DEFAULT_PERSONA_NAME.to_string(),
        Some(stem) if !stem.is_empty() => stem.clone(),
        _ => format!("persona_{slug}"),
    };
    let target_path = personas_dir.join(format!("{target_stem}.md"));

    let mut table = toml::Table::new();
    table.insert("name".into(), toml::Value::String(display.to_string()));
    if !input.age.trim().is_empty() {
        table.insert(
            "age".into(),
            toml::Value::String(input.age.trim().to_string()),
        );
    }
    if !input.character.trim().is_empty() {
        table.insert(
            "character".into(),
            toml::Value::String(input.character.trim().to_string()),
        );
        table.insert(
            "description".into(),
            toml::Value::String(input.character.trim().to_string()),
        );
    }
    if !input.modifiers.trim().is_empty() {
        table.insert(
            "modifiers".into(),
            toml::Value::String(input.modifiers.trim().to_string()),
        );
    }
    if let Some(av) = input.avatar_rel.as_deref().filter(|s| !s.trim().is_empty()) {
        table.insert("avatar".into(), toml::Value::String(av.to_string()));
    }
    if let Some(bg) = input.avatar_bg_rel.as_deref().filter(|s| !s.trim().is_empty()) {
        table.insert("avatar_bg".into(), toml::Value::String(bg.to_string()));
    }

    let frontmatter = toml::to_string(&table).map_err(io::Error::other)?;
    let body = format_persona_markdown(display, &input.age, &input.character, &input.modifiers);
    let full = format!("+++\n{frontmatter}+++\n{body}");
    fs::write(&target_path, full)?;
    load_persona_file(&target_path)
}

/// Crops a square avatar from `src_path` with `zoom` (1.0..=4.0) and normalized pan (`-1.0..=1.0`),
/// saving a 256x256 avatar PNG and a softly blurred 320x320 atmospheric backdrop PNG under `personas_dir/avatars/`.
pub fn crop_and_save_avatar(
    src_path: &Path,
    personas_dir: &Path,
    slug: &str,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
) -> io::Result<(String, String)> {
    let img = image::open(src_path).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Не удалось открыть изображение: {e}"),
        )
    })?;
    let width = img.width().max(1);
    let height = img.height().max(1);
    let base_side = width.min(height) as f32;
    let z = zoom.clamp(1.0, 4.0);
    let crop_side = (base_side / z).round().clamp(16.0, base_side) as u32;

    let max_dx = (width.saturating_sub(crop_side)) as f32 / 2.0;
    let max_dy = (height.saturating_sub(crop_side)) as f32 / 2.0;
    let cx = width as f32 / 2.0 + pan_x.clamp(-1.0, 1.0) * max_dx;
    let cy = height as f32 / 2.0 + pan_y.clamp(-1.0, 1.0) * max_dy;
    let x = (cx - crop_side as f32 / 2.0)
        .round()
        .clamp(0.0, width.saturating_sub(crop_side) as f32) as u32;
    let y = (cy - crop_side as f32 / 2.0)
        .round()
        .clamp(0.0, height.saturating_sub(crop_side) as f32) as u32;

    let cropped = image::imageops::crop_imm(&img, x, y, crop_side, crop_side).to_image();
    let avatar_img = image::imageops::resize(&cropped, 256, 256, FilterType::Lanczos3);
    let bg_small = image::imageops::resize(&cropped, 320, 320, FilterType::Triangle);
    let bg_blurred = image::imageops::blur(&bg_small, 18.0);

    let avatars_dir = personas_dir.join("avatars");
    fs::create_dir_all(&avatars_dir)?;

    let safe_slug = sanitize_persona_slug(slug);
    let avatar_rel = format!("avatars/persona_{safe_slug}.png");
    let bg_rel = format!("avatars/persona_{safe_slug}_bg.png");

    avatar_img
        .save(personas_dir.join(&avatar_rel))
        .map_err(|e| io::Error::other(format!("Не удалось сохранить аватар: {e}")))?;
    bg_blurred
        .save(personas_dir.join(&bg_rel))
        .map_err(|e| io::Error::other(format!("Не удалось сохранить фон: {e}")))?;

    Ok((avatar_rel, bg_rel))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn creates_and_loads_default_persona_and_frontmatter() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("snarta_persona_{nanos}"));
        ensure_default_persona(&dir).unwrap();

        let p = load_persona(&dir, "default").unwrap();
        assert_eq!(p.name, "default");
        assert!(p.system_prompt.contains("Snarta"));

        let saved = save_structured_persona(
            &dir,
            &PersonaSaveInput {
                existing_file_stem: None,
                display_name: "Akane".into(),
                age: "21".into(),
                character: "Спокойная и ироничная".into(),
                modifiers: "Отвечай кратко".into(),
                avatar_rel: None,
                avatar_bg_rel: None,
            },
        )
        .unwrap();
        assert_eq!(saved.name, "persona_Akane");
        assert_eq!(saved.display_name, "Akane");
        assert_eq!(saved.age.as_deref(), Some("21"));
        assert_eq!(saved.character.as_deref(), Some("Спокойная и ироничная"));
        assert!(saved.system_prompt.contains("Возраст: 21"));

        let loaded = load_persona(&dir, "Akane").unwrap();
        assert_eq!(loaded.display_name, "Akane");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn crops_and_saves_avatar_and_blurred_backdrop() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("snarta_crop_{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.png");
        let img = image::RgbaImage::from_pixel(400, 300, image::Rgba([200, 120, 80, 255]));
        img.save(&src).unwrap();

        let (av_rel, bg_rel) = crop_and_save_avatar(&src, &dir, "Akane", 1.5, 0.25, -0.25).unwrap();
        assert!(dir.join(&av_rel).is_file());
        assert!(dir.join(&bg_rel).is_file());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sliding_window_never_cuts_system_prompt_and_preserves_jsonl_on_compact() {
        use crate::chat::prompt::build_context_messages;
        use crate::chat::session::Session;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("snarta_sess_test_{nanos}"));
        let mut s = Session::create_new(&dir, "default", Some("qwen3.gguf")).unwrap();
        s.append_turn("user", "Привет", None, None, None, None)
            .unwrap();
        s.append_turn(
            "assistant",
            "Принято",
            Some("думаю".into()),
            Some(28.5),
            Some(1.2),
            None,
        )
        .unwrap();
        s.append_turn("user", "Обсудим Snarta", None, None, None, None)
            .unwrap();
        s.append_turn("assistant", "Готов", None, Some(29.1), Some(0.8), None)
            .unwrap();

        let sys = "SYSTEM_PROMPT_NEVER_CUT";
        let (msgs_small, _) = build_context_messages(&s, sys, 60, 10, false);
        assert_eq!(msgs_small[0].0, "system");
        assert!(msgs_small[0].1.contains(sys));

        s.append_compact(2, "Собеседник Алекс".into()).unwrap();
        let reloaded = Session::load(&s.path).unwrap();
        assert_eq!(reloaded.turns.len(), 4);
        assert_eq!(reloaded.last_compact, Some((2, "Собеседник Алекс".into())));
        let (msgs_after, _) = build_context_messages(&reloaded, sys, 4096, 512, false);
        assert!(msgs_after[0].1.contains("Собеседник Алекс"));
        assert_eq!(msgs_after.len(), 3);
        let _ = fs::remove_dir_all(&dir);
    }
}
