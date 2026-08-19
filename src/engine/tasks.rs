use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Received,
    Claimed,
    Running,
    Testing,
    PrOpened,
    Completed,
    Failed,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Received => write!(f, "received"),
            TaskStatus::Claimed => write!(f, "claimed"),
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::Testing => write!(f, "testing"),
            TaskStatus::PrOpened => write!(f, "pr_opened"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
        }
    }
}

#[allow(dead_code)]
impl TaskStatus {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "received" => Some(TaskStatus::Received),
            "claimed" => Some(TaskStatus::Claimed),
            "running" => Some(TaskStatus::Running),
            "testing" => Some(TaskStatus::Testing),
            "pr_opened" | "propened" => Some(TaskStatus::PrOpened),
            "completed" => Some(TaskStatus::Completed),
            "failed" => Some(TaskStatus::Failed),
            _ => None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Completed | TaskStatus::Failed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskPriority::Low => write!(f, "low"),
            TaskPriority::Normal => write!(f, "normal"),
            TaskPriority::High => write!(f, "high"),
            TaskPriority::Urgent => write!(f, "urgent"),
        }
    }
}

impl TaskPriority {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "low" => TaskPriority::Low,
            "high" => TaskPriority::High,
            "urgent" | "critical" => TaskPriority::Urgent,
            _ => TaskPriority::Normal,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentTask {
    pub id: String,
    pub source_agent_id: String,
    pub target_agent_id: Option<String>,
    pub action: String,
    pub repository: Option<String>,
    pub branch: String,
    pub priority: String,
    pub status: String,
    pub description: String,
    pub evidence_json: Option<String>,
    pub acceptance_criteria_json: Option<String>,
    pub assigned_agent_id: Option<String>,
    pub commit_sha: Option<String>,
    pub pr_url: Option<String>,
    pub test_results_json: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
}

impl AgentTask {
    pub fn new(
        source_agent_id: &str,
        target_agent_id: Option<&str>,
        action: &str,
        repository: Option<&str>,
        branch: Option<&str>,
        priority: TaskPriority,
        description: &str,
        evidence: Option<Vec<String>>,
        acceptance_criteria: Option<Vec<String>>,
    ) -> Self {
        let rand_slug = Uuid::new_v4().to_string().replace('-', "")[..8].to_string();
        let id = format!("task_{}", rand_slug);
        let now = Utc::now().timestamp();

        Self {
            id,
            source_agent_id: source_agent_id.to_string(),
            target_agent_id: target_agent_id.map(|s| s.to_string()),
            action: action.to_string(),
            repository: repository.map(|s| s.to_string()),
            branch: branch.unwrap_or("main").to_string(),
            priority: priority.to_string(),
            status: TaskStatus::Received.to_string(),
            description: description.to_string(),
            evidence_json: evidence.and_then(|e| serde_json::to_string(&e).ok()),
            acceptance_criteria_json: acceptance_criteria
                .and_then(|a| serde_json::to_string(&a).ok()),
            assigned_agent_id: None,
            commit_sha: None,
            pr_url: None,
            test_results_json: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskAuditLog {
    pub id: String,
    pub task_id: String,
    pub agent_id: String,
    pub event_type: String,
    pub details_json: Option<String>,
    pub created_at: i64,
}

impl TaskAuditLog {
    pub fn new(
        task_id: &str,
        agent_id: &str,
        event_type: &str,
        details: Option<serde_json::Value>,
    ) -> Self {
        Self {
            id: format!(
                "audit_{}",
                &Uuid::new_v4().to_string().replace('-', "")[..10]
            ),
            task_id: task_id.to_string(),
            agent_id: agent_id.to_string(),
            event_type: event_type.to_string(),
            details_json: details.map(|d| d.to_string()),
            created_at: Utc::now().timestamp(),
        }
    }
}
