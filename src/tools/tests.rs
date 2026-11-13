use super::*;
use crate::chat::client::ToolCallItem;
use std::time::{SystemTime, UNIX_EPOCH};

fn read_memory_lines(path: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

#[test]
fn blocks_escape_from_life_and_dangerous_commands() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("snarta_tools_test_{nanos}"));
    let life = base.join("life");
    let work = base.join("workspace");
    let outside = base.join("secret.txt");
    std::fs::create_dir_all(&life).unwrap();
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(&outside, "top secret").unwrap();

    assert!(resolve_inside_life(&life, "../secret.txt").is_err());
    assert!(resolve_inside_life(&life, "..\\secret.txt").is_err());
    assert!(resolve_inside_life(&life, "/etc/passwd").is_err());
    assert!(resolve_inside_life(&life, "C:\\Windows\\win.ini").is_err());

    assert!(is_dangerous_command("rm -rf /").is_some());
    assert!(is_dangerous_command("format C: /q").is_some());
    assert!(is_dangerous_command("Remove-Item -Recurse C:\\").is_some());
    assert!(is_dangerous_command("shutdown /s /t 0").is_some());
    assert!(is_dangerous_command("git status").is_none());

    let rem = execute_tool_call(
        &work,
        &life,
        &ToolCallItem {
            id: "1".into(),
            name: "remember".into(),
            arguments: r#"{"text":"Любит RX 7800 XT"}"#.into(),
        },
        4096,
    );
    assert!(rem.mem_added.is_some());
    let lines = read_memory_lines(&life.join("memory.md"));
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("Любит RX 7800 XT"));

    let wf = execute_tool_call(
        &work,
        &life,
        &ToolCallItem {
            id: "2".into(),
            name: "write_file".into(),
            arguments: r#"{"path":"hello.txt","content":"line 1\nline 2\nline 3"}"#.into(),
        },
        4096,
    );
    assert!(wf.output.contains("File saved"));

    // Harness pagination read
    let rf = execute_tool_call(
        &work,
        &life,
        &ToolCallItem {
            id: "3".into(),
            name: "read_file".into(),
            arguments: r#"{"path":"hello.txt","start_line":2,"end_line":2}"#.into(),
        },
        4096,
    );
    assert!(rf.output.contains("line 2"));
    assert!(!rf.output.contains("line 1"));

    // Harness edit with uniqueness
    let ed = execute_tool_call(
        &work,
        &life,
        &ToolCallItem {
            id: "4".into(),
            name: "edit_file".into(),
            arguments: r#"{"path":"hello.txt","old_text":"line 2","new_text":"line TWO"}"#.into(),
        },
        4096,
    );
    assert!(ed.output.contains("File edited successfully"));
    let modified = std::fs::read_to_string(work.join("hello.txt")).unwrap();
    assert!(modified.contains("line TWO"));

    // Non-unique edit rejection
    std::fs::write(work.join("dup.txt"), "same\nsame\n").unwrap();
    let ed_dup = execute_tool_call(
        &work,
        &life,
        &ToolCallItem {
            id: "5".into(),
            name: "edit_file".into(),
            arguments: r#"{"path":"dup.txt","old_text":"same","new_text":"different"}"#.into(),
        },
        4096,
    );
    assert!(ed_dup.output.contains("OLD_STRING_NOT_UNIQUE"));

    // Plan mode guard
    set_plan_mode(true);
    let blocked_in_plan = execute_tool_call(
        &work,
        &life,
        &ToolCallItem {
            id: "6".into(),
            name: "write_file".into(),
            arguments: r#"{"path":"plan_fail.txt","content":"fail"}"#.into(),
        },
        4096,
    );
    assert!(blocked_in_plan.output.contains("PLAN_MODE_ACTIVE"));
    set_plan_mode(false);

    // Task create and list
    let tc = execute_tool_call(
        &work,
        &life,
        &ToolCallItem {
            id: "7".into(),
            name: "task_create".into(),
            arguments: r#"{"subject":"Build agent harness","description":"Phases 1-6"}"#.into(),
        },
        4096,
    );
    assert!(tc.output.contains("Task created"));
    let tl = execute_tool_call(
        &work,
        &life,
        &ToolCallItem {
            id: "8".into(),
            name: "task_list".into(),
            arguments: "{}".into(),
        },
        4096,
    );
    assert!(tl.output.contains("Build agent harness"));

    // Glob and Grep
    let gl = execute_tool_call(
        &work,
        &life,
        &ToolCallItem {
            id: "9".into(),
            name: "glob_files".into(),
            arguments: r#"{"pattern":"*.txt"}"#.into(),
        },
        4096,
    );
    assert!(gl.output.contains("hello.txt"));
    assert!(gl.output.contains("lines"));

    let gr = execute_tool_call(
        &work,
        &life,
        &ToolCallItem {
            id: "10".into(),
            name: "grep_files".into(),
            arguments: r#"{"query":"line TWO"}"#.into(),
        },
        4096,
    );
    assert!(gr.output.contains("line TWO"));

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn sanitizes_powershell_nested_commands() {
    let unwrapped = sanitize_shell_command(r#"powershell -NoProfile -Command "Get-ChildItem -Recurse""#);
    assert_eq!(unwrapped, "Get-ChildItem -Recurse");

    let unwrapped_short = sanitize_shell_command(r#"powershell.exe -c "cargo check""#);
    assert_eq!(unwrapped_short, "cargo check");
}

#[test]
fn glob_patterns_follow_gitignore_semantics() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("snarta_glob_test_{nanos}"));
    let life = base.join("life");
    let work = base.join("workspace");

    std::fs::create_dir_all(&life).unwrap();
    std::fs::create_dir_all(work.join("src").join("nested")).unwrap();
    std::fs::write(work.join("main.rs"), "fn main() {}\n").unwrap();
    std::fs::write(work.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    std::fs::write(work.join("src").join("config.rs"), "pub const X: u8 = 1;\n").unwrap();
    std::fs::write(
        work.join("src").join("nested").join("deep.rs"),
        "pub fn deep() {}\n",
    )
    .unwrap();
    std::fs::write(work.join("README.md"), "# test\n").unwrap();

    let run_glob = |pattern: &str| -> String {
        execute_tool_call(
            &work,
            &life,
            &ToolCallItem {
                id: "glob".into(),
                name: "glob_files".into(),
                arguments: format!(r#"{{"pattern":"{pattern}"}}"#),
            },
            4096,
        )
        .output
    };

    // `**/*.rs` — все .rs на любой глубине, включая корень.
    let out = run_glob("**/*.rs");
    assert!(out.contains("main.rs"), "**/*.rs: {out}");
    assert!(out.contains("src/main.rs"), "**/*.rs: {out}");
    assert!(out.contains("src/config.rs"), "**/*.rs: {out}");
    assert!(out.contains("src/nested/deep.rs"), "**/*.rs: {out}");
    assert!(!out.contains("README.md"), "**/*.rs: {out}");

    // `src/*.rs` — только один уровень под src, `*` не пересекает `/`.
    let out = run_glob("src/*.rs");
    assert!(out.contains("src/main.rs"), "src/*.rs: {out}");
    assert!(out.contains("src/config.rs"), "src/*.rs: {out}");
    assert!(!out.contains("src/nested/deep.rs"), "src/*.rs: {out}");

    // `*.rs` без `/` — basename на любой глубине (gitignore-style).
    let out = run_glob("*.rs");
    assert!(out.contains("src/main.rs"), "*.rs: {out}");
    assert!(out.contains("src/nested/deep.rs"), "*.rs: {out}");

    // `?` тоже должен работать (старый матчер его не поддерживал).
    let out = run_glob("src/??????.rs"); // config.rs / main.rs — 6 и 4, см. ниже
    assert!(!out.is_empty(), "??????.rs: {out}");

    // Точное имя без wildcard — на любой глубине.
    let out = run_glob("config.rs");
    assert!(out.contains("src/config.rs"), "config.rs: {out}");

    let _ = std::fs::remove_dir_all(&base);
}
