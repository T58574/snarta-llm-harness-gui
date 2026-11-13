use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SESSION_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolTrace {
    pub name: String,
    pub arguments: String,
    pub output: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TurnStep {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolTrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionEvent {
    Meta {
        title: String,
        persona: String,
        model: Option<String>,
        updated_ts: u64,
    },
    Message {
        role: String,
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gen_tps: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_secs: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tokens: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_traces: Vec<ToolTrace>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        steps: Vec<TurnStep>,
        ts: u64,
    },
    Compact {
        covered_messages: usize,
        summary: String,
        ts: u64,
    },
    MemNotice {
        fact: String,
        ts: u64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
    pub reasoning: Option<String>,
    pub gen_tps: Option<f64>,
    pub duration_secs: Option<f64>,
    pub tokens: Option<u32>,
    pub image: Option<String>,
    pub images: Vec<String>,
    pub tool_traces: Vec<ToolTrace>,
    pub steps: Vec<TurnStep>,
}

impl ChatTurn {
    pub fn effective_images(&self) -> Vec<String> {
        if !self.images.is_empty() {
            return self.images.clone();
        }
        self.image.iter().cloned().collect()
    }

    pub fn effective_steps(&self) -> Vec<TurnStep> {
        if !self.steps.is_empty() {
            return self.steps.clone();
        }
        if self.reasoning.is_none() && self.content.is_empty() && self.tool_traces.is_empty() {
            return Vec::new();
        }
        vec![TurnStep {
            reasoning: self.reasoning.clone(),
            content: self.content.clone(),
            tool_calls: self.tool_traces.clone(),
        }]
    }
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub path: PathBuf,
    pub title: String,
    #[allow(dead_code)]
    pub persona: String,
    pub message_count: usize,
    pub updated_ts: u64,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub path: PathBuf,
    pub title: String,
    pub persona: String,
    pub model: Option<String>,
    pub updated_ts: u64,
    pub turns: Vec<ChatTurn>,
    pub last_compact: Option<(usize, String)>,
    pub mem_notices: Vec<(usize, String)>,
}

impl Session {
    pub fn create_new(sessions_dir: &Path, persona: &str, model: Option<&str>) -> io::Result<Self> {
        fs::create_dir_all(sessions_dir)?;
        let ts = now_secs();
        let seq = SESSION_SEQ.fetch_add(1, Ordering::Relaxed);
        let mut id = if seq == 0 {
            format!("s_{ts}")
        } else {
            format!("s_{ts}_{seq}")
        };
        let mut path = sessions_dir.join(format!("{id}.jsonl"));
        let mut extra = 1u32;
        while path.exists() {
            id = format!("s_{ts}_{seq}_{extra}");
            path = sessions_dir.join(format!("{id}.jsonl"));
            extra += 1;
        }
        Ok(Self {
            id: id.clone(),
            path,
            title: format!("Сессия {id}"),
            persona: persona.to_string(),
            model: model.map(str::to_string),
            updated_ts: ts,
            turns: Vec::new(),
            last_compact: None,
            mem_notices: Vec::new(),
        })
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("session")
            .to_string();
        let f = fs::File::open(path)?;
        let mut s = Self {
            id: id.clone(),
            path: path.to_path_buf(),
            title: id,
            persona: "default".to_string(),
            model: None,
            updated_ts: 0,
            turns: Vec::new(),
            last_compact: None,
            mem_notices: Vec::new(),
        };

        for line in BufReader::new(f).lines().map_while(Result::ok) {
            let Ok(ev) = serde_json::from_str::<SessionEvent>(line.trim()) else {
                continue;
            };
            match ev {
                SessionEvent::Meta {
                    title,
                    persona,
                    model,
                    updated_ts,
                } => {
                    s.title = title;
                    s.persona = persona;
                    s.model = model;
                    s.updated_ts = updated_ts;
                }
                SessionEvent::Message {
                    role,
                    content,
                    reasoning,
                    gen_tps,
                    duration_secs,
                    tokens,
                    image,
                    images,
                    tool_traces,
                    steps,
                    ts,
                } => {
                    s.updated_ts = s.updated_ts.max(ts);
                    if s.turns.is_empty() && role == "user" && s.title.starts_with("Сессия ")
                    {
                        s.title = auto_title(&content);
                    }
                    s.turns.push(ChatTurn {
                        role,
                        content,
                        reasoning,
                        gen_tps,
                        duration_secs,
                        tokens,
                        image,
                        images,
                        tool_traces,
                        steps,
                    });
                }
                SessionEvent::Compact {
                    covered_messages,
                    summary,
                    ts,
                } => {
                    s.updated_ts = s.updated_ts.max(ts);
                    s.last_compact = Some((covered_messages, summary));
                }
                SessionEvent::MemNotice { fact, ts } => {
                    s.updated_ts = s.updated_ts.max(ts);
                    s.mem_notices.push((s.turns.len(), fact));
                }
            }
        }
        Ok(s)
    }

    #[allow(dead_code)]
    pub fn append_turn(
        &mut self,
        role: &str,
        content: &str,
        reasoning: Option<String>,
        gen_tps: Option<f64>,
        duration_secs: Option<f64>,
        image: Option<String>,
    ) -> io::Result<()> {
        self.append_turn_with_tools(ChatTurn {
            role: role.to_string(),
            content: content.to_string(),
            reasoning,
            gen_tps,
            duration_secs,
            tokens: None,
            image,
            images: Vec::new(),
            tool_traces: Vec::new(),
            steps: Vec::new(),
        })
    }

    pub fn append_turn_with_tools(&mut self, turn: ChatTurn) -> io::Result<()> {
        let ts = now_secs();
        let is_first = self.turns.is_empty();
        let auto_ren = is_first && turn.role == "user" && self.title.starts_with("Сессия ");
        if auto_ren {
            self.title = auto_title(&turn.content);
        }
        self.updated_ts = ts;
        if is_first && !self.path.exists() {
            self.append_meta()?;
        }
        self.append_event(&SessionEvent::Message {
            role: turn.role.clone(),
            content: turn.content.clone(),
            reasoning: turn.reasoning.clone(),
            gen_tps: turn.gen_tps,
            duration_secs: turn.duration_secs,
            tokens: turn.tokens,
            image: turn.image.clone(),
            images: turn.images.clone(),
            tool_traces: turn.tool_traces.clone(),
            steps: turn.steps.clone(),
            ts,
        })?;
        self.turns.push(turn);
        if auto_ren {
            self.append_meta()?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn append_compact(&mut self, covered_messages: usize, summary: String) -> io::Result<()> {
        let ts = now_secs();
        self.updated_ts = ts;
        self.last_compact = Some((covered_messages, summary.clone()));
        self.append_event(&SessionEvent::Compact {
            covered_messages,
            summary,
            ts,
        })
    }

    #[allow(dead_code)]
    pub fn append_mem_notice(&mut self, fact: &str) -> io::Result<()> {
        let ts = now_secs();
        self.updated_ts = ts;
        self.mem_notices.push((self.turns.len(), fact.to_string()));
        self.append_event(&SessionEvent::MemNotice {
            fact: fact.to_string(),
            ts,
        })
    }

    #[allow(dead_code)]
    pub fn rename(&mut self, new_title: &str) -> io::Result<()> {
        self.title = new_title.trim().to_string();
        self.updated_ts = now_secs();
        self.append_meta()
    }

    pub fn set_persona(&mut self, persona: &str) -> io::Result<()> {
        self.persona = persona.to_string();
        self.updated_ts = now_secs();
        if self.path.exists() {
            self.append_meta()?;
        }
        Ok(())
    }

    fn append_meta(&self) -> io::Result<()> {
        self.append_event(&SessionEvent::Meta {
            title: self.title.clone(),
            persona: self.persona.clone(),
            model: self.model.clone(),
            updated_ts: self.updated_ts,
        })
    }

    fn append_event(&self, ev: &SessionEvent) -> io::Result<()> {
        if let Some(p) = self.path.parent() {
            fs::create_dir_all(p)?;
        }
        let line = serde_json::to_string(ev).map_err(|e| io::Error::other(e.to_string()))?;
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(f, "{line}")
    }
}

pub fn list_sessions(sessions_dir: &Path) -> io::Result<Vec<SessionSummary>> {
    if !sessions_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(sessions_dir)?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl")
            && let Ok(s) = Session::load(&path)
        {
            out.push(SessionSummary {
                id: s.id,
                path: s.path,
                title: s.title,
                persona: s.persona,
                message_count: s.turns.len(),
                updated_ts: s.updated_ts,
            });
        }
    }
    out.sort_by_key(|a| Reverse(a.updated_ts));
    Ok(out)
}

fn auto_title(first_msg: &str) -> String {
    let single = first_msg.lines().next().unwrap_or("").trim();
    let chars: Vec<char> = single.chars().collect();
    if chars.len() > 42 {
        format!("{}...", chars[..42].iter().collect::<String>())
    } else if single.is_empty() {
        "Новая сессия".to_string()
    } else {
        single.to_string()
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[allow(dead_code)]
pub fn format_duration(secs: f64) -> String {
    if secs < 60.0 {
        format!("{secs:.1}с")
    } else {
        format!("{}м {:02}с", (secs as u64) / 60, (secs as u64) % 60)
    }
}
