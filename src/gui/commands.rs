#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandDef {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
}

macro_rules! cmd {
    ($n:literal, [$($a:literal),*], $d:literal) => {
        CommandDef {
            name: $n,
            aliases: &[$($a),*],
            description: $d,
        }
    };
}

pub const COMMANDS: &[CommandDef] = &[
    cmd!("/new", ["/clear"], "Новый чат"),
    cmd!("/prepare", [], "Освободить VRAM"),
    cmd!("/model", ["/models"], "Выбрать или переключить модель GGUF"),
    cmd!("/persona", ["/system-prompt"], "Выбрать или переключить персону"),
    cmd!("/settings", ["/config"], "Открыть настройки приложения"),
    cmd!("/compact", [], "Сжать историю диалога в резюме"),
    cmd!("/status", [], "Сводка: модель, контекст, VRAM"),
];
