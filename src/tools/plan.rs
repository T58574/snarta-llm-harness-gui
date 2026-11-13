use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Blocked => "blocked",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "pending" | "todo" => Some(Self::Pending),
            "in_progress" | "progress" | "doing" => Some(Self::InProgress),
            "completed" | "done" => Some(Self::Completed),
            "blocked" => Some(Self::Blocked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: usize,
    pub subject: String,
    pub description: String,
    pub status: TaskStatus,
}

pub struct TaskManager {
    tasks: Mutex<Vec<TaskItem>>,
    next_id: Mutex<usize>,
}

impl Default for TaskManager {
    fn default() -> Self {
        Self {
            tasks: Mutex::new(Vec::new()),
            next_id: Mutex::new(1),
        }
    }
}

impl TaskManager {
    pub fn global() -> &'static TaskManager {
        static MANAGER: OnceLock<TaskManager> = OnceLock::new();
        MANAGER.get_or_init(TaskManager::default)
    }

    pub fn ensure_loaded(&self, life_dir: &Path) {
        static LOADED: AtomicBool = AtomicBool::new(false);
        if !LOADED.swap(true, Ordering::SeqCst) {
            self.load_from_life(life_dir);
        }
    }

    pub fn load_from_life(&self, life_dir: &Path) {
        let path = life_dir.join("tasks.json");
        if let Ok(data) = fs::read_to_string(&path)
            && let Ok(loaded) = serde_json::from_str::<Vec<TaskItem>>(&data)
        {
            let max_id = loaded.iter().map(|t| t.id).max().unwrap_or(0);
            if let Ok(mut t_guard) = self.tasks.lock() {
                *t_guard = loaded;
            }
            if let Ok(mut id_guard) = self.next_id.lock() {
                *id_guard = max_id + 1;
            }
        }
    }

    pub fn save_to_life(&self, life_dir: &Path) -> io::Result<()> {
        let path = life_dir.join("tasks.json");
        if let Ok(tasks) = self.tasks.lock()
            && let Ok(json) = serde_json::to_string_pretty(&*tasks)
        {
            fs::write(path, json)?;
        }
        Ok(())
    }

    pub fn create_task(
        &self,
        life_dir: &Path,
        subject: &str,
        description: &str,
    ) -> io::Result<TaskItem> {
        self.ensure_loaded(life_dir);
        let mut id_guard = self.next_id.lock().unwrap_or_else(|e| e.into_inner());
        let id = *id_guard;
        *id_guard += 1;

        let item = TaskItem {
            id,
            subject: subject.trim().to_string(),
            description: description.trim().to_string(),
            status: TaskStatus::Pending,
        };

        {
            let mut list = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
            list.push(item.clone());
        }

        let _ = self.save_to_life(life_dir);
        Ok(item)
    }

    pub fn list_tasks(&self) -> Vec<TaskItem> {
        self.tasks.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn update_task(
        &self,
        life_dir: &Path,
        id: usize,
        new_status: TaskStatus,
    ) -> io::Result<TaskItem> {
        self.ensure_loaded(life_dir);
        let mut list = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(item) = list.iter_mut().find(|t| t.id == id) {
            item.status = new_status;
            let cloned = item.clone();
            drop(list);
            let _ = self.save_to_life(life_dir);
            Ok(cloned)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Task #{id} not found"),
            ))
        }
    }

    pub fn format_list(&self) -> String {
        let list = self.list_tasks();
        if list.is_empty() {
            return "Task list is empty.".to_string();
        }

        let mut out = String::from("### Current tasks:\n");
        for t in list {
            let badge = match t.status {
                TaskStatus::Pending => "[ ]",
                TaskStatus::InProgress => "[>]",
                TaskStatus::Completed => "[x]",
                TaskStatus::Blocked => "[!]",
            };
            out.push_str(&format!(
                "{} #{}: {} ({})\n",
                badge,
                t.id,
                t.subject,
                t.status.as_str()
            ));
            if !t.description.is_empty() {
                out.push_str(&format!("    {}\n", t.description));
            }
        }
        out
    }
}

pub static PLAN_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn is_plan_mode() -> bool {
    PLAN_MODE_ACTIVE.load(Ordering::SeqCst)
}

pub fn set_plan_mode(active: bool) {
    PLAN_MODE_ACTIVE.store(active, Ordering::SeqCst);
}

pub fn enter_plan_mode() -> String {
    set_plan_mode(true);
    "Plan mode activated. File modifications are blocked until plan approval.".to_string()
}

pub fn exit_plan_mode(summary: &str) -> String {
    set_plan_mode(false);
    format!("Plan mode finished. Plan approved for execution:\n{summary}")
}

pub fn format_ask_user(question: &str, options: &[String], allow_free_text: bool) -> String {
    let mut out = format!("Question for user: {question}\n");
    if !options.is_empty() {
        out.push_str("Options:\n");
        for (idx, opt) in options.iter().enumerate() {
            out.push_str(&format!("  {}. {}\n", idx + 1, opt));
        }
    }
    if allow_free_text {
        out.push_str("  (Free text response is also accepted)\n");
    }
    out
}
