use gpui::{Image as GpuiImage, ImageFormat as GpuiImageFormat};
use image::DynamicImage;
use image::imageops::FilterType;
use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub avatar_image: Option<Arc<GpuiImage>>,
    pub avatar_bg_image: Option<Arc<GpuiImage>>,
}

/// Loads raw PNG bytes from disk into an in-memory `Arc<gpui::Image>` (content-hashed by GPUI,
/// bypassing path-based caching).
pub fn load_gpui_png(path: &Path) -> Option<Arc<GpuiImage>> {
    let bytes = fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some(Arc::new(GpuiImage::from_bytes(GpuiImageFormat::Png, bytes)))
}

/// Decodes an image file by inspecting its magic bytes (`with_guessed_format`) rather than
/// trusting its file extension, so `.jpg` files containing WebP/PNG bytes (or vice versa) load cleanly.
pub fn decode_image_any(path: &Path) -> io::Result<DynamicImage> {
    let bytes = fs::read(path).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("Не удалось прочитать файл {}: {e}", path.display()),
        )
    })?;
    if bytes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Файл изображения пуст (0 байт)",
        ));
    }

    if let Ok(reader) = image::ImageReader::new(Cursor::new(&bytes)).with_guessed_format()
        && let Ok(img) = reader.decode()
    {
        return Ok(img);
    }

    image::load_from_memory(&bytes)
        .or_else(|_| image::open(path))
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Не удалось декодировать изображение: {e}"),
            )
        })
}

/// Encodes a `DynamicImage` into an in-memory PNG `Arc<gpui::Image>` for live preview without disk cache issues.
pub fn dynamic_image_to_gpui_png(img: &DynamicImage) -> io::Result<Arc<GpuiImage>> {
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
        .map_err(|e| io::Error::other(format!("Ошибка кодирования PNG: {e}")))?;
    Ok(Arc::new(GpuiImage::from_bytes(GpuiImageFormat::Png, buf)))
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

    // Strip optional legacy TOML frontmatter if present, then also parse unified Markdown fields.
    let (mut meta, body_after_fm) = split_optional_toml_frontmatter(&raw);
    let cleaned_body = parse_markdown_persona_fields(body_after_fm, &mut meta);

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

    let avatar_path = resolve_rel(meta.avatar);
    let avatar_bg_path = resolve_rel(meta.avatar_bg);
    let avatar_image = avatar_path.as_deref().and_then(load_gpui_png);
    let avatar_bg_image = avatar_bg_path.as_deref().and_then(load_gpui_png);
    let description = meta.character.clone().or(meta.description);

    Ok(Persona {
        name: file_stem,
        display_name,
        path: path.to_path_buf(),
        system_prompt: cleaned_body,
        description,
        age: meta.age,
        character: meta.character,
        modifiers: meta.modifiers,
        avatar_path,
        avatar_bg_path,
        avatar_image,
        avatar_bg_image,
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

/// Parses structured Markdown persona fields (`# Персона: ...`, `- Имя: ...`, `- Возраст: ...`,
/// `- Характер: ...`, `- Аватар: ...`, `- Фон: ...`, `## Модификаторы...`) and returns a clean
/// system prompt for the LLM without technical avatar path lines.
fn parse_markdown_persona_fields(body: &str, meta: &mut FrontmatterMeta) -> String {
    let mut prompt_lines = Vec::new();
    let mut in_modifiers_section = false;
    let mut modifiers_lines = Vec::new();
    let mut has_structured_header = false;

    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(h) = trimmed.strip_prefix("# Персона:") {
            has_structured_header = true;
            let val = h.trim();
            if !val.is_empty() && meta.name.is_none() {
                meta.name = Some(val.to_string());
            }
            prompt_lines.push(line);
            continue;
        }
        if trimmed.starts_with("## Модификаторы") {
            has_structured_header = true;
            in_modifiers_section = true;
            prompt_lines.push(line);
            continue;
        }
        if in_modifiers_section {
            modifiers_lines.push(line);
            prompt_lines.push(line);
            continue;
        }
        if let Some(v) = trimmed.strip_prefix("- Имя:") {
            has_structured_header = true;
            let val = v.trim();
            if !val.is_empty() {
                meta.name = Some(val.to_string());
            }
            prompt_lines.push(line);
            continue;
        }
        if let Some(v) = trimmed.strip_prefix("- Возраст:") {
            has_structured_header = true;
            let val = v.trim();
            if !val.is_empty() {
                meta.age = Some(val.to_string());
            }
            prompt_lines.push(line);
            continue;
        }
        if let Some(v) = trimmed.strip_prefix("- Характер:") {
            has_structured_header = true;
            let val = v.trim();
            if !val.is_empty() {
                meta.character = Some(val.to_string());
            }
            prompt_lines.push(line);
            continue;
        }
        if let Some(v) = trimmed.strip_prefix("- Аватар:") {
            has_structured_header = true;
            let val = v.trim();
            if !val.is_empty() {
                meta.avatar = Some(val.to_string());
            }
            // Do not include technical file paths in the LLM system prompt
            continue;
        }
        if let Some(v) = trimmed.strip_prefix("- Фон:") {
            has_structured_header = true;
            let val = v.trim();
            if !val.is_empty() {
                meta.avatar_bg = Some(val.to_string());
            }
            continue;
        }
        prompt_lines.push(line);
    }

    if !modifiers_lines.is_empty() {
        let joined = modifiers_lines.join("\n").trim().to_string();
        if !joined.is_empty() {
            meta.modifiers = Some(joined);
        }
    } else if !has_structured_header && meta.modifiers.is_none() {
        let plain = body.trim();
        if !plain.is_empty() {
            meta.modifiers = Some(plain.to_string());
        }
    }

    prompt_lines.join("\n").trim().to_string()
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

/// Formats the persona into a single unified Markdown file (no duplicate TOML frontmatter).
pub fn format_persona_markdown(
    display_name: &str,
    age: &str,
    character: &str,
    modifiers: &str,
    avatar_rel: Option<&str>,
    avatar_bg_rel: Option<&str>,
) -> String {
    let mut out = format!("# Персона: {}\n", display_name.trim());
    out.push_str(&format!("- Имя: {}\n", display_name.trim()));
    if !age.trim().is_empty() {
        out.push_str(&format!("- Возраст: {}\n", age.trim()));
    }
    if !character.trim().is_empty() {
        out.push_str(&format!("- Характер: {}\n", character.trim()));
    }
    if let Some(av) = avatar_rel.map(str::trim).filter(|s| !s.is_empty()) {
        out.push_str(&format!("- Аватар: {av}\n"));
    }
    if let Some(bg) = avatar_bg_rel.map(str::trim).filter(|s| !s.is_empty()) {
        out.push_str(&format!("- Фон: {bg}\n"));
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

/// Saves a structured persona in unified `persona_<Name>.md` Markdown format without duplicate frontmatter.
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

    // If user removed the avatar, clean up old avatar files for this slug.
    if input.avatar_rel.is_none() {
        cleanup_old_avatars(&personas_dir.join("avatars"), &slug, None, None);
    }

    let full_markdown = format_persona_markdown(
        display,
        &input.age,
        &input.character,
        &input.modifiers,
        input.avatar_rel.as_deref(),
        input.avatar_bg_rel.as_deref(),
    );
    fs::write(&target_path, full_markdown)?;
    load_persona_file(&target_path)
}

fn cleanup_old_avatars(
    avatars_dir: &Path,
    safe_slug: &str,
    keep_av: Option<&str>,
    keep_bg: Option<&str>,
) {
    let prefix = format!("persona_{safe_slug}");
    if let Ok(entries) = fs::read_dir(avatars_dir) {
        for entry in entries.flatten() {
            if let Some(fname) = entry.file_name().to_str()
                && fname.starts_with(&prefix)
                && Some(fname) != keep_av
                && Some(fname) != keep_bg
            {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// Crops a square avatar from `src_path` with `zoom` (1.0..=4.0) and normalized pan (`-1.0..=1.0`),
/// saving a 256x256 avatar PNG and a softly blurred 320x320 atmospheric backdrop PNG with a unique
/// timestamped filename under `personas_dir/avatars/` so GPUI never serves stale cached paths.
pub fn crop_and_save_avatar(
    src_path: &Path,
    personas_dir: &Path,
    slug: &str,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
) -> io::Result<(String, String)> {
    let img = decode_image_any(src_path)?;
    crop_and_save_loaded_image(&img, personas_dir, slug, zoom, pan_x, pan_y)
}

pub fn crop_and_save_loaded_image(
    img: &DynamicImage,
    personas_dir: &Path,
    slug: &str,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
) -> io::Result<(String, String)> {
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

    let cropped = image::imageops::crop_imm(img, x, y, crop_side, crop_side).to_image();
    let avatar_img = image::imageops::resize(&cropped, 256, 256, FilterType::Lanczos3);
    let bg_small = image::imageops::resize(&cropped, 320, 320, FilterType::Triangle);
    let bg_blurred = image::imageops::blur(&bg_small, 18.0);

    let avatars_dir = personas_dir.join("avatars");
    fs::create_dir_all(&avatars_dir)?;

    let safe_slug = sanitize_persona_slug(slug);
    let uniq_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let av_fname = format!("persona_{safe_slug}_{uniq_ms}.png");
    let bg_fname = format!("persona_{safe_slug}_{uniq_ms}_bg.png");
    let avatar_rel = format!("avatars/{av_fname}");
    let bg_rel = format!("avatars/{bg_fname}");

    avatar_img
        .save(personas_dir.join(&avatar_rel))
        .map_err(|e| io::Error::other(format!("Не удалось сохранить аватар: {e}")))?;
    bg_blurred
        .save(personas_dir.join(&bg_rel))
        .map_err(|e| io::Error::other(format!("Не удалось сохранить фон: {e}")))?;

    cleanup_old_avatars(&avatars_dir, &safe_slug, Some(&av_fname), Some(&bg_fname));

    Ok((avatar_rel, bg_rel))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(saved.modifiers.as_deref(), Some("Отвечай кратко"));

        // Verify the file on disk has NO duplicate +++ TOML block
        let raw_on_disk = fs::read_to_string(&saved.path).unwrap();
        assert!(!raw_on_disk.contains("+++"));
        assert_eq!(raw_on_disk.matches("Спокойная и ироничная").count(), 1);

        // Saving a second time after editing must not duplicate headers
        let resaved = save_structured_persona(
            &dir,
            &PersonaSaveInput {
                existing_file_stem: Some(saved.name.clone()),
                display_name: saved.display_name.clone(),
                age: saved.age.clone().unwrap_or_default(),
                character: saved.character.clone().unwrap_or_default(),
                modifiers: saved.modifiers.clone().unwrap_or_default(),
                avatar_rel: None,
                avatar_bg_rel: None,
            },
        )
        .unwrap();
        let raw_resaved = fs::read_to_string(&resaved.path).unwrap();
        assert_eq!(raw_resaved.matches("# Персона:").count(), 1);
        assert_eq!(resaved.modifiers.as_deref(), Some("Отвечай кратко"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn crops_and_saves_avatar_even_with_mismatched_extension() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("snarta_crop_{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        // Save PNG bytes into a file named `.jpg` to simulate web/Telegram extension mismatch
        let src = dir.join("mismatched.jpg");
        let img = image::RgbaImage::from_pixel(400, 300, image::Rgba([200, 120, 80, 255]));
        let mut png_bytes = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .unwrap();
        fs::write(&src, &png_bytes).unwrap();

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
