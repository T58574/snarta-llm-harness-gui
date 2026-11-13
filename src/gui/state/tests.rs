use super::*;
use crate::chat::engine::{ActiveAgentStream, AgentEvent};
use crate::chat::session::{ChatTurn, ToolTrace};

fn make_test_temp_dir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("snarta_gui_test_{tag}_{nanos}"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[core::prelude::v1::test]
fn app_state_initializes_and_switches_persona() {
    let dir = make_test_temp_dir("app_state");
    let paths = AppPaths::resolve_from(&dir, None);
    paths.ensure_dirs().unwrap();

    let cfg = Config::default();
    let logger = Logger::new(paths.snarta_log_file(), 1024 * 1024);

    let mut state = AppState::new(paths, cfg, logger);
    assert_eq!(state.persona.name, "default");
    assert!(state.session.turns.is_empty());
    assert_eq!(state.server_status, ServerStatus::Stopped);

    state.create_new_session();
    assert!(state.session.id.starts_with("s_"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn settings_modal_state_activation() {
    let dir = make_test_temp_dir("settings_modal");
    let paths = AppPaths::resolve_from(&dir, None);
    paths.ensure_dirs().unwrap();

    let cfg = Config::default();
    let logger = Logger::new(paths.snarta_log_file(), 1024 * 1024);

    let mut state = AppState::new(paths, cfg, logger);
    assert_eq!(state.active_modal, None);
    assert!(!state.settings_open);

    state.settings_open = true;
    assert!(state.settings_open);

    state.settings_open = false;
    assert!(!state.settings_open);

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn app_state_server_lifecycle_and_send_message_notice() {
    let dir = make_test_temp_dir("app_state_server");
    let paths = AppPaths::resolve_from(&dir, None);
    paths.ensure_dirs().unwrap();

    let cfg = Config::default();
    let logger = Logger::new(paths.snarta_log_file(), 1024 * 1024);

    let mut state = AppState::new(paths, cfg, logger);
    assert_eq!(state.server_status, ServerStatus::Stopped);

    // When no active model is selected and send_message is invoked
    state.active_model_meta = None;
    state.send_message("Привет".to_string());
    assert_eq!(state.session.turns.len(), 1);
    assert!(state.session.turns[0]
        .content
        .contains("Сервер llama.cpp выключен"));

    // Test stop_server transitions status to Stopped
    state.server_status = ServerStatus::Error("Test error".to_string());
    state.stop_server();
    assert_eq!(state.server_status, ServerStatus::Stopped);

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn app_state_cancel_generation_and_scroll_flag() {
    let dir = make_test_temp_dir("app_state_tool");
    let paths = AppPaths::resolve_from(&dir, None);
    paths.ensure_dirs().unwrap();

    let cfg = Config::default();
    let logger = Logger::new(paths.snarta_log_file(), 1024 * 1024);

    let mut state = AppState::new(paths, cfg, logger);
    assert!(state.needs_scroll_to_bottom);

    state.needs_scroll_to_bottom = false;
    state.create_new_session();
    assert!(state.needs_scroll_to_bottom);

    state.is_generating = true;
    state.streaming_content = "Draft response".to_string();
    state.streaming_reasoning = "Thinking about test".to_string();
    state.streaming_tools.push(ToolTrace {
        name: "list_files".to_string(),
        arguments: "{}".to_string(),
        output: "test output".to_string(),
    });

    state.cancel_generation();
    assert!(!state.is_generating);
    assert!(state.streaming_content.is_empty());
    assert!(state.streaming_reasoning.is_empty());
    assert!(state.streaming_tools.is_empty());
    assert_eq!(state.session.turns.len(), 1);
    assert_eq!(state.session.turns[0].content, "Draft response");
    assert_eq!(state.session.turns[0].tool_traces.len(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn app_state_poll_agent_events_drains_and_commits() {
    let dir = make_test_temp_dir("app_state_events");
    let paths = AppPaths::resolve_from(&dir, None);
    paths.ensure_dirs().unwrap();

    let cfg = Config::default();
    let logger = Logger::new(paths.snarta_log_file(), 1024 * 1024);

    let mut state = AppState::new(paths, cfg, logger);
    let (tx, rx) = std::sync::mpsc::channel();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    state.is_generating = true;
    state.active_stream = Some(ActiveAgentStream { rx, cancel });

    tx.send(AgentEvent::StepStarted { step_idx: 0 }).unwrap();
    tx.send(AgentEvent::ContentDelta {
        step_idx: 0,
        delta: "Chunk 1".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::ToolStarted {
        step_idx: 0,
        call_idx: 0,
        name: "list_files".to_string(),
        arguments: "{}".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::ToolFinished {
        step_idx: 0,
        call_idx: 0,
        name: "list_files".to_string(),
        arguments: "{}".to_string(),
        output: "file1.txt".to_string(),
    })
    .unwrap();

    let updated = state.poll_agent_events(true);
    assert!(updated);
    assert_eq!(state.streaming_content, "Chunk 1");
    assert_eq!(state.streaming_tools.len(), 1);
    assert_eq!(state.streaming_tools[0].output, "file1.txt");

    // Complete the turn
    tx.send(AgentEvent::TurnComplete {
        turn: ChatTurn {
            role: "assistant".to_string(),
            content: "Final answer".to_string(),
            reasoning: None,
            gen_tps: Some(30.0),
            duration_secs: Some(1.0),
            tokens: Some(40),
            image: None,
            images: Vec::new(),
            tool_traces: Vec::new(),
            steps: Vec::new(),
        },
        gen_tps: 30.0,
        prompt_tokens: Some(10),
        completion_tokens: Some(40),
    })
    .unwrap();

    let finished = state.poll_agent_events(true);
    assert!(finished);
    assert!(!state.is_generating);
    assert!(state.active_stream.is_none());
    assert_eq!(state.session.turns.len(), 1);
    assert_eq!(state.session.turns[0].content, "Final answer");

    let _ = std::fs::remove_dir_all(&dir);
}
