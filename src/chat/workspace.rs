use crate::chat::prompt::{build_context_messages, build_system_prompt, pairs_to_json_messages};
use crate::chat::session::{Session, SessionSummary};
use crate::config::{Config, ModelSettings};
use crate::gguf::{self, GgufMeta};
use crate::paths::AppPaths;
use crate::persona::{self, Persona};
use std::io;
use std::path::Path;

/// Encapsulates the active working context: sessions, personas, and model catalog.
///
/// `Workspace` owns all conversational session IO, persona selection, and GGUF model
/// metadata scanning, keeping these concerns independent of the GUI presentation layer.
pub struct Workspace {
    pub session: Session,
    pub sessions_list: Vec<SessionSummary>,
    pub persona: Persona,
    pub available_personas: Vec<Persona>,
    pub active_model_meta: Option<GgufMeta>,
    pub available_models: Vec<GgufMeta>,
    pub used_ctx: u32,
}

impl Workspace {
    /// Initializes a new workspace from disk, ensuring default personas and selecting
    /// active models and sessions according to configuration.
    pub fn new(paths: &AppPaths, cfg: &mut Config) -> Self {
        let available_personas = persona::list_personas(&paths.personas_dir()).unwrap_or_default();
        let _ = persona::ensure_default_persona(&paths.personas_dir());
        let persona = persona::load_persona(&paths.personas_dir(), &cfg.default_persona)
            .or_else(|_| persona::load_persona(&paths.personas_dir(), "default"))
            .unwrap_or_else(|_| Persona {
                name: persona::DEFAULT_PERSONA_NAME.to_string(),
                path: paths.personas_dir().join("default.md"),
                system_prompt: persona::DEFAULT_PERSONA_PROMPT.to_string(),
                description: None,
            });

        let available_models = gguf::scan_models_dir(&paths.models_dir()).unwrap_or_default();
        let active_model_meta = available_models
            .iter()
            .find(|m| cfg.active_model.as_deref() == Some(&m.file_name))
            .cloned()
            .or_else(|| available_models.first().cloned());

        // Persist the auto-corrected active model so subsequent launches do not
        // re-enter the same fallback path.
        if let Some(ref m) = active_model_meta {
            let new_model = m.file_name.clone();
            if cfg.active_model.as_deref() != Some(&new_model) {
                cfg.active_model = Some(new_model);
                let _ = cfg.save(&paths.config_file());
            }
        }

        let fallback_session = || Session {
            id: "s_fallback".to_string(),
            path: paths.sessions_dir().join("s_fallback.jsonl"),
            title: "Сессия".to_string(),
            persona: persona.name.clone(),
            model: cfg.active_model.clone(),
            updated_ts: 0,
            turns: Vec::new(),
            last_compact: None,
            mem_notices: Vec::new(),
        };

        let sessions_list =
            crate::chat::session::list_sessions(&paths.sessions_dir()).unwrap_or_default();
        let session = if let Some(first) = sessions_list.first() {
            Session::load(&first.path).unwrap_or_else(|_| {
                Session::create_new(
                    &paths.sessions_dir(),
                    &persona.name,
                    cfg.active_model.as_deref(),
                )
                .unwrap_or_else(|_| fallback_session())
            })
        } else {
            Session::create_new(
                &paths.sessions_dir(),
                &persona.name,
                cfg.active_model.as_deref(),
            )
            .unwrap_or_else(|_| fallback_session())
        };

        let mut ws = Self {
            session,
            sessions_list,
            persona,
            available_personas,
            active_model_meta,
            available_models,
            used_ctx: 0,
        };
        ws.recalculate_used_ctx(paths, cfg, None);
        ws
    }

    /// Returns the effective `ModelSettings` for a given model file.
    pub fn settings_for(&self, cfg: &Config, model_file: &str) -> ModelSettings {
        cfg.model_settings(model_file)
    }

    /// Returns the effective `ModelSettings` for the currently active model.
    pub fn settings_for_active(&self, cfg: &Config) -> ModelSettings {
        match self.active_model_meta.as_ref() {
            Some(meta) => self.settings_for(cfg, &meta.file_name),
            None => cfg.model_defaults.clone(),
        }
    }

    /// Reloads the list of persisted sessions from disk.
    pub fn reload_sessions(&mut self, paths: &AppPaths) {
        self.sessions_list =
            crate::chat::session::list_sessions(&paths.sessions_dir()).unwrap_or_default();
    }

    /// Creates a new chat session and activates it.
    pub fn create_new_session(
        &mut self,
        paths: &AppPaths,
        cfg: &Config,
        active_server_ctx: Option<u32>,
    ) -> io::Result<()> {
        let new_sess = Session::create_new(
            &paths.sessions_dir(),
            &self.persona.name,
            cfg.active_model.as_deref(),
        )?;
        self.session = new_sess;
        self.reload_sessions(paths);
        self.recalculate_used_ctx(paths, cfg, active_server_ctx);
        Ok(())
    }

    /// Switches to an existing session file.
    pub fn switch_session(
        &mut self,
        paths: &AppPaths,
        cfg: &Config,
        path: &Path,
        active_server_ctx: Option<u32>,
    ) -> io::Result<()> {
        let loaded = Session::load(path)?;
        self.session = loaded;
        self.recalculate_used_ctx(paths, cfg, active_server_ctx);
        Ok(())
    }

    /// Deletes a session file, reloading summaries and recreating a fresh session
    /// if the active one was deleted. Returns true if the active session was deleted.
    pub fn delete_session(
        &mut self,
        paths: &AppPaths,
        cfg: &Config,
        path: &Path,
        active_server_ctx: Option<u32>,
    ) -> io::Result<bool> {
        let was_active = self.session.path == path;
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        self.reload_sessions(paths);
        if was_active {
            self.create_new_session(paths, cfg, active_server_ctx)?;
        }
        Ok(was_active)
    }

    /// Changes the active persona and saves the choice to configuration.
    pub fn switch_persona(
        &mut self,
        paths: &AppPaths,
        cfg: &mut Config,
        persona_name: &str,
        active_server_ctx: Option<u32>,
    ) {
        if let Some(p) = self
            .available_personas
            .iter()
            .find(|p| p.name == persona_name)
            .cloned()
        {
            self.persona = p;
            cfg.default_persona = persona_name.to_string();
            let _ = cfg.save(&paths.config_file());
            let _ = self.session.set_persona(persona_name);
            self.recalculate_used_ctx(paths, cfg, active_server_ctx);
        }
    }

    /// Selects a new active model file, persisting it to configuration.
    pub fn switch_model(
        &mut self,
        paths: &AppPaths,
        cfg: &mut Config,
        model_file: &str,
        active_server_ctx: Option<u32>,
    ) {
        if let Some(meta) = self
            .available_models
            .iter()
            .find(|m| m.file_name == model_file)
            .cloned()
        {
            self.active_model_meta = Some(meta);
            cfg.active_model = Some(model_file.to_string());
            let _ = cfg.save(&paths.config_file());
            self.session.model = Some(model_file.to_string());
            self.recalculate_used_ctx(paths, cfg, active_server_ctx);
        }
    }

    /// Rescans the models directory and refreshes the available models catalog.
    pub fn rescan_models(
        &mut self,
        paths: &AppPaths,
        cfg: &mut Config,
        active_server_ctx: Option<u32>,
    ) {
        let list = gguf::scan_models_dir(&paths.models_dir()).unwrap_or_default();
        self.available_models = list;
        if let Some(active_name) = cfg.active_model.as_deref() {
            self.active_model_meta = self
                .available_models
                .iter()
                .find(|m| m.file_name == active_name)
                .cloned();
        }
        if self.active_model_meta.is_none() && !self.available_models.is_empty() {
            self.active_model_meta = self.available_models.first().cloned();
            if let Some(ref m) = self.active_model_meta {
                cfg.active_model = Some(m.file_name.clone());
                let _ = cfg.save(&paths.config_file());
            }
        }
        self.recalculate_used_ctx(paths, cfg, active_server_ctx);
    }

    /// Recalculates prompt token consumption against the active server context ceiling.
    pub fn recalculate_used_ctx(
        &mut self,
        paths: &AppPaths,
        cfg: &Config,
        active_server_ctx: Option<u32>,
    ) {
        let system_prompt = build_system_prompt(
            &self.persona,
            &paths.memory_file(),
            &paths.cwd,
            cfg.limits.memory_token_limit,
        );
        let settings = self.settings_for_active(cfg);

        let effective_ctx = active_server_ctx
            .filter(|c| *c > 0)
            .unwrap_or(if settings.ctx > 0 {
                settings.ctx
            } else {
                cfg.model_defaults.ctx
            });

        let (_, used) = build_context_messages(
            &self.session,
            &system_prompt.full_text,
            effective_ctx,
            settings.max_tokens,
            settings.reasoning_preserve,
        );
        self.used_ctx = used;
    }

    /// Builds context messages ready for LLM consumption along with calculated token usage.
    pub fn build_turn_messages(
        &self,
        paths: &AppPaths,
        cfg: &Config,
        active_ctx: u32,
    ) -> (Vec<serde_json::Value>, u32) {
        let system_prompt = build_system_prompt(
            &self.persona,
            &paths.memory_file(),
            &paths.cwd,
            cfg.limits.memory_token_limit,
        );
        let settings = self.settings_for_active(cfg);

        let (pairs, used_toks) = build_context_messages(
            &self.session,
            &system_prompt.full_text,
            active_ctx,
            settings.max_tokens,
            settings.reasoning_preserve,
        );
        (pairs_to_json_messages(&pairs), used_toks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("snarta_workspace_test_{tag}_{nanos}"));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[core::prelude::v1::test]
    fn workspace_initializes_with_default_persona_and_session() {
        let dir = make_test_temp_dir("init");
        let paths = AppPaths::resolve_from(&dir, None);
        paths.ensure_dirs().unwrap();
        let mut cfg = Config::default();

        let mut ws = Workspace::new(&paths, &mut cfg);
        assert_eq!(ws.persona.name, "default");
        assert!(ws.session.turns.is_empty());
        assert!(ws.sessions_list.is_empty());

        // Persist first turn and switch to a new session
        ws.session
            .append_turn("user", "hello", None, None, None, None)
            .unwrap();
        ws.reload_sessions(&paths);
        assert_eq!(ws.sessions_list.len(), 1);

        ws.create_new_session(&paths, &cfg, None).unwrap();
        assert_eq!(ws.sessions_list.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[core::prelude::v1::test]
    fn workspace_session_delete_lifecycle() {
        let dir = make_test_temp_dir("delete");
        let paths = AppPaths::resolve_from(&dir, None);
        paths.ensure_dirs().unwrap();
        let mut cfg = Config::default();

        let mut ws = Workspace::new(&paths, &mut cfg);
        let session_path = ws.session.path.clone();

        // Deleting the active session automatically recreates a new active one
        let was_active = ws.delete_session(&paths, &cfg, &session_path, None).unwrap();
        assert!(was_active);
        assert_ne!(ws.session.path, session_path);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
