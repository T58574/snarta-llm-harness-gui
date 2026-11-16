use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const DEFAULT_PERSONA_NAME: &str = "default";
pub const DEFAULT_PERSONA_PROMPT: &str = "\
Ты - Snarta, спокойный, точный и прямой локальный собеседник и партнёр по размышлениям.
Отвечай по делу, без воды, канцелярита и извинений. Если нужно запомнить важный факт о собеседнике или его планах, используй инструмент remember.
";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PersonaGender {
    #[default]
    Female,
    Male,
}

impl PersonaGender {
    pub fn from_str_or_name(raw: Option<&str>, name: &str) -> Self {
        if let Some(s) = raw {
            match s.trim().to_ascii_lowercase().as_str() {
                "male" | "m" | "man" | "boy" | "м" | "муж" | "мужской" => {
                    return Self::Male;
                }
                "female" | "f" | "woman" | "girl" | "ж" | "жен" | "женский" => {
                    return Self::Female;
                }
                _ => {}
            }
        }
        let low = name.trim().to_ascii_lowercase();
        if low == "agent" || low.contains("alex") || low.contains("coach") || low.contains("male") {
            Self::Male
        } else {
            Self::Female
        }
    }

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Female => "Женский",
            Self::Male => "Мужской",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Female => "female",
            Self::Male => "male",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            Self::Female => Self::Male,
            Self::Male => Self::Female,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Persona {
    pub name: String,
    pub path: PathBuf,
    pub system_prompt: String,
    pub description: Option<String>,
    pub gender: PersonaGender,
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
    out.sort_by(|a, b| a.name.cmp(&b.name));
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
    let all = list_personas(personas_dir)?;
    if let Some(found) = all.into_iter().find(|p| p.name.eq_ignore_ascii_case(clean)) {
        return Ok(found);
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("persona '{clean}' not found in {}", personas_dir.display()),
    ))
}

pub fn load_persona_file(path: &Path) -> io::Result<Persona> {
    let raw = fs::read_to_string(path)?;
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(DEFAULT_PERSONA_NAME)
        .to_string();
    let (description, gender_raw, body) = split_optional_toml_frontmatter(&raw);
    let gender = PersonaGender::from_str_or_name(gender_raw.as_deref(), &name);
    Ok(Persona {
        name,
        path: path.to_path_buf(),
        system_prompt: body.trim().to_string(),
        description,
        gender,
    })
}

fn split_optional_toml_frontmatter(raw: &str) -> (Option<String>, Option<String>, &str) {
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
            let desc = parsed
                .as_ref()
                .and_then(|v| v.get("description")?.as_str().map(str::to_string));
            let gender = parsed
                .as_ref()
                .and_then(|v| v.get("gender")?.as_str().map(str::to_string));
            return (desc, gender, body);
        }
    }
    (None, None, trimmed)
}

/// Persists `gender` into the TOML frontmatter (`+++`) of the named persona file,
/// preserving existing metadata and prompt body, and returns the reloaded `Persona`.
pub fn set_persona_gender(
    personas_dir: &Path,
    name: &str,
    gender: PersonaGender,
) -> io::Result<Persona> {
    let existing = load_persona(personas_dir, name)?;
    let raw = fs::read_to_string(&existing.path)?;
    let trimmed = raw.trim_start_matches('\u{feff}');

    let mut updated_content = None;
    for delim in ["+++", "---"] {
        if let Some(rest) = trimmed.strip_prefix(delim)
            && let Some(after_first_nl) = rest
                .strip_prefix('\n')
                .or_else(|| rest.strip_prefix("\r\n"))
            && let Some(end_idx) = after_first_nl.find(&format!("\n{delim}"))
        {
            let header = after_first_nl[..end_idx].trim_end_matches('\r');
            let tail = &after_first_nl[end_idx + 1 + delim.len()..];
            let body = tail
                .strip_prefix('\n')
                .or_else(|| tail.strip_prefix("\r\n"))
                .unwrap_or(tail);

            let new_header = if let Ok(mut table) = toml::from_str::<toml::Table>(header) {
                table.insert(
                    "gender".to_string(),
                    toml::Value::String(gender.as_str().to_string()),
                );
                toml::to_string(&table).unwrap_or_else(|_| format!("gender = \"{}\"\n", gender.as_str()))
            } else {
                let mut lines: Vec<String> = header
                    .lines()
                    .filter(|l| {
                        let k = l.split(['=', ':']).next().map(str::trim).unwrap_or("");
                        !k.eq_ignore_ascii_case("gender")
                    })
                    .map(str::to_string)
                    .collect();
                lines.push(format!("gender = \"{}\"", gender.as_str()));
                format!("{}\n", lines.join("\n"))
            };

            let mut out = format!("+++\n{new_header}+++\n");
            out.push_str(body);
            updated_content = Some(out);
            break;
        }
    }

    let final_text = updated_content.unwrap_or_else(|| {
        let body = trimmed.trim_start_matches(['\r', '\n']);
        format!("+++\ngender = \"{}\"\n+++\n{body}", gender.as_str())
    });

    fs::write(&existing.path, final_text)?;
    load_persona_file(&existing.path)
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

        let custom = dir.join("coach.md");
        fs::write(
            &custom,
            "+++\ndescription = \"Strict coach\"\ngender = \"female\"\n+++\nKeep answers under 3 sentences.\n",
        )
        .unwrap();
        let c = load_persona(&dir, "coach").unwrap();
        assert_eq!(c.description.as_deref(), Some("Strict coach"));
        assert_eq!(c.gender, PersonaGender::Female);
        assert_eq!(c.system_prompt, "Keep answers under 3 sentences.");

        let agent = dir.join("agent.md");
        fs::write(&agent, "Work fast.\n").unwrap();
        let a = load_persona(&dir, "agent").unwrap();
        assert_eq!(a.gender, PersonaGender::Male);

        let updated_coach = set_persona_gender(&dir, "coach", PersonaGender::Male).unwrap();
        assert_eq!(updated_coach.gender, PersonaGender::Male);
        assert_eq!(updated_coach.description.as_deref(), Some("Strict coach"));
        assert_eq!(updated_coach.system_prompt, "Keep answers under 3 sentences.");

        let plain = dir.join("roksana_main.md");
        fs::write(&plain, "Привет, я Роксана.\n").unwrap();
        let updated_plain = set_persona_gender(&dir, "roksana_main", PersonaGender::Male).unwrap();
        assert_eq!(updated_plain.gender, PersonaGender::Male);
        assert_eq!(updated_plain.system_prompt, "Привет, я Роксана.");
        let back_female = set_persona_gender(&dir, "roksana_main", PersonaGender::Female).unwrap();
        assert_eq!(back_female.gender, PersonaGender::Female);
        assert_eq!(back_female.system_prompt, "Привет, я Роксана.");

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
