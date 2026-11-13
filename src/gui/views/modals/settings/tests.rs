use super::*;

#[core::prelude::v1::test]
fn settings_tabs_and_state_defaults() {
    let state = SettingsState {
        current_tab: SettingsTab::General,
        default_persona: "default".to_string(),
        auto_scroll: true,
        stream_reasoning: true,
        save_notice: None,
        selected_model: None,
        kv_type: "q8_0".to_string(),
        flash_attn: true,
        mlock: true,
        mmap: true,
        prompt_cache: false,
        reasoning_effort: "medium".to_string(),
        reasoning_preserve: false,
        search_provider: "cascade".to_string(),
    };
    assert_eq!(state.current_tab, SettingsTab::General);
    assert_eq!(state.default_persona, "default");
    assert!(state.auto_scroll);
    assert!(state.stream_reasoning);
    assert_eq!(state.reasoning_effort, "medium");

    let mut next_state = state;
    next_state.current_tab = SettingsTab::Models;
    assert_eq!(next_state.current_tab, SettingsTab::Models);

    next_state.current_tab = SettingsTab::Server;
    assert_eq!(next_state.current_tab, SettingsTab::Server);

    next_state.current_tab = SettingsTab::SearchAndTools;
    assert_eq!(next_state.current_tab, SettingsTab::SearchAndTools);

    next_state.current_tab = SettingsTab::Vram;
    assert_eq!(next_state.current_tab, SettingsTab::Vram);

    next_state.current_tab = SettingsTab::Runtime;
    assert_eq!(next_state.current_tab, SettingsTab::Runtime);

    next_state.current_tab = SettingsTab::Doctor;
    assert_eq!(next_state.current_tab, SettingsTab::Doctor);
}
