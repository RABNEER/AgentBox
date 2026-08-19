use chrono::Utc;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

static TASK_SUBJECT_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\[TASK(?::([a-z0-9_-]+))?\]|\[BUG\]|\[FEATURE\]|\[REVIEW\]|^TASK:|^BUG:|^ISSUE:",
    )
    .unwrap()
});

static REPO_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:Repository|Repo):\s*([a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+)|github\.com/([a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+)").unwrap()
});

static BRANCH_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)Branch:\s*([a-zA-Z0-9_.-]+)").unwrap());

static PRIORITY_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)Priority:\s*(low|normal|high|urgent|critical)").unwrap());

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

impl std::str::FromStr for TaskStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "received" => Ok(TaskStatus::Received),
            "claimed" => Ok(TaskStatus::Claimed),
            "running" => Ok(TaskStatus::Running),
            "testing" => Ok(TaskStatus::Testing),
            "pr_opened" | "propened" => Ok(TaskStatus::PrOpened),
            "completed" => Ok(TaskStatus::Completed),
            "failed" => Ok(TaskStatus::Failed),
            _ => Err(()),
        }
    }
}

#[allow(dead_code)]
impl TaskStatus {
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

impl std::str::FromStr for TaskPriority {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" => Ok(TaskPriority::Low),
            "high" => Ok(TaskPriority::High),
            "urgent" | "critical" => Ok(TaskPriority::Urgent),
            _ => Ok(TaskPriority::Normal),
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
    #[allow(clippy::too_many_arguments)]
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

/// Automatically detects and parses incoming emails into first-class AgentTasks
pub struct TaskDetector;

impl TaskDetector {
    pub fn detect_and_parse(
        subject: Option<&str>,
        body_text: Option<&str>,
        from_address: &str,
        to_address: &str,
    ) -> Option<AgentTask> {
        let subject_str = subject.unwrap_or("").trim();
        let body_str = body_text.unwrap_or("").trim();

        // 1. JSON Task Protocol Payload in body
        if body_str.starts_with('{') && body_str.ends_with('}') {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body_str) {
                if val.get("type").and_then(|t| t.as_str()) == Some("task") {
                    let action = val
                        .get("action")
                        .and_then(|a| a.as_str())
                        .unwrap_or("general_task");
                    let repo = val.get("repository").and_then(|r| r.as_str());
                    let branch = val.get("branch").and_then(|b| b.as_str());
                    let priority = TaskPriority::from_str(
                        val.get("priority")
                            .and_then(|p| p.as_str())
                            .unwrap_or("normal"),
                    );
                    let desc = val
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or(subject_str);

                    let evidence = val.get("evidence").and_then(|e| e.as_array()).map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    });

                    let criteria = val
                        .get("acceptance_criteria")
                        .and_then(|a| a.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        });

                    let target = val
                        .get("to")
                        .and_then(|t| t.as_str())
                        .or_else(|| to_address.split('@').next());

                    return Some(AgentTask::new(
                        from_address,
                        target,
                        action,
                        repo,
                        branch,
                        priority,
                        desc,
                        evidence,
                        criteria,
                    ));
                }
            }
        }

        // 2. Structured Email Task Detection ([TASK:...], [BUG], [FEATURE], TASK:, BUG:)
        if TASK_SUBJECT_PATTERN.is_match(subject_str) {
            let action = if subject_str.to_lowercase().contains("bug") {
                "fix_bug"
            } else if subject_str.to_lowercase().contains("review") {
                "code_review"
            } else if subject_str.to_lowercase().contains("feature") {
                "implement_feature"
            } else if subject_str.to_lowercase().contains("test") {
                "e2e_test"
            } else {
                "general_task"
            };

            // Extract Repository from body or subject
            let repo = REPO_PATTERN
                .captures(body_str)
                .or_else(|| REPO_PATTERN.captures(subject_str))
                .and_then(|c| c.get(1).or_else(|| c.get(2)))
                .map(|m| m.as_str());

            // Extract Branch
            let branch = BRANCH_PATTERN
                .captures(body_str)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str());

            // Extract Priority
            let priority =
                if let Some(cap) = PRIORITY_PATTERN.captures(body_str).and_then(|c| c.get(1)) {
                    TaskPriority::from_str(cap.as_str())
                } else if subject_str.to_uppercase().contains("URGENT")
                    || subject_str.to_uppercase().contains("HIGH")
                {
                    TaskPriority::High
                } else {
                    TaskPriority::Normal
                };

            // Extract Evidence lines
            let mut evidence = Vec::new();
            let mut criteria = Vec::new();

            for line in body_str.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("Evidence:")
                    || trimmed.starts_with("File:")
                    || trimmed.starts_with("Trace:")
                {
                    if let Some((_, val)) = trimmed.split_once(':') {
                        evidence.push(val.trim().to_string());
                    }
                } else if trimmed.starts_with("Expected:")
                    || trimmed.starts_with("Acceptance Criteria:")
                    || trimmed.starts_with("- [ ]")
                {
                    criteria.push(trimmed.trim_start_matches("- [ ]").trim().to_string());
                }
            }

            let evidence_opt = if evidence.is_empty() {
                None
            } else {
                Some(evidence)
            };
            let criteria_opt = if criteria.is_empty() {
                None
            } else {
                Some(criteria)
            };

            let target = to_address.split('@').next();

            return Some(AgentTask::new(
                from_address,
                target,
                action,
                repo,
                branch,
                priority,
                if body_str.is_empty() {
                    subject_str
                } else {
                    body_str
                },
                evidence_opt,
                criteria_opt,
            ));
        }

        None
    }
}
