#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandDef {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub arg_hint: &'static str,
    pub description: &'static str,
}

impl CommandDef {
    #[allow(dead_code)]
    pub fn label(&self) -> String {
        if self.arg_hint.is_empty() {
            self.name.to_string()
        } else {
            format!("{} {}", self.name, self.arg_hint)
        }
    }
}

macro_rules! cmd {
    ($n:literal, [$($a:literal),*], $h:literal, $d:literal) => {
        CommandDef {
            name: $n,
            aliases: &[$($a),*],
            arg_hint: $h,
            description: $d,
        }
    };
}

pub const COMMANDS: &[CommandDef] = &[
    cmd!("/new", ["/clear"], "", "Новый чат"),
    cmd!("/prepare", [], "", "Освободить VRAM"),
    cmd!("/model", ["/models"], "", "Выбрать или переключить модель GGUF"),
    cmd!("/persona", ["/system-prompt"], "", "Выбрать или переключить персону"),
    cmd!("/settings", ["/config"], "", "Открыть настройки приложения"),
    cmd!("/compact", [], "", "Сжать историю диалога в резюме"),
    cmd!("/status", [], "", "Сводка: модель, контекст, VRAM"),
];

#[allow(dead_code)]
pub fn filter_commands(input: &str) -> Vec<&'static CommandDef> {
    let trimmed = input.trim_start();
    if !trimmed.starts_with('/') || trimmed.contains('\n') || trimmed.contains(' ') {
        return Vec::new();
    }
    let q = trimmed.to_ascii_lowercase();
    COMMANDS
        .iter()
        .filter(|d| d.name.starts_with(&q) || d.aliases.iter().any(|a| a.starts_with(&q)))
        .collect()
}
