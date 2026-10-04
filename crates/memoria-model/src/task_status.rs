//! Linear-style task statuses — port of `src/lib/taskStatus.ts`.
//!
//! Lifecycle: `triage → backlog/todo → done/canceled`, stored in
//! `task_obj.propsJson.status`. Delphi reads the derived `is_completed` /
//! `is_cancelled` flags; they are written in sync with `status` (see
//! `task_sync::patch_task`).

use serde_json::Value;

/// `TASK_STATUSES`.
pub const TASK_STATUSES: [&str; 5] = ["triage", "backlog", "todo", "done", "canceled"];

/// `TaskStatus` union — `&'static str` keeps the wire spelling.
pub type TaskStatus = &'static str;

/// `TASK_STATUS_DEFAULT` — a new task needs triage, not auto-todo.
pub const TASK_STATUS_DEFAULT: TaskStatus = "triage";

/// `normalizeStatus` — valid strings pass through; legacy values migrate
/// (`duplicate` → `canceled`, `in_progress` → `todo`); otherwise derive from
/// the Delphi flags.
pub fn normalize_status(
    status: Option<&Value>,
    is_completed: bool,
    is_cancelled: bool,
) -> TaskStatus {
    if let Some(value) = status.and_then(Value::as_str) {
        match value {
            "triage" => return "triage",
            "backlog" => return "backlog",
            "todo" => return "todo",
            "done" => return "done",
            "canceled" => return "canceled",
            "duplicate" => return "canceled",
            "in_progress" => return "todo",
            _ => {}
        }
    }
    if is_completed {
        return "done";
    }
    if is_cancelled {
        return "canceled";
    }
    TASK_STATUS_DEFAULT
}

/// `deriveCompletedFlag`.
pub fn derive_completed_flag(status: TaskStatus) -> bool {
    status == "done"
}

/// `deriveCancelledFlag`.
pub fn derive_cancelled_flag(status: TaskStatus) -> bool {
    status == "canceled"
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn normalize_status_passthrough_and_legacy() {
        for status in TASK_STATUSES {
            assert_eq!(normalize_status(Some(&json!(status)), false, false), status);
        }
        assert_eq!(
            normalize_status(Some(&json!("duplicate")), false, false),
            "canceled"
        );
        assert_eq!(
            normalize_status(Some(&json!("in_progress")), false, false),
            "todo"
        );
        assert_eq!(
            normalize_status(Some(&json!("bogus")), false, false),
            "triage"
        );
        assert_eq!(normalize_status(Some(&json!("bogus")), true, false), "done");
        assert_eq!(
            normalize_status(Some(&json!("bogus")), false, true),
            "canceled"
        );
        assert_eq!(normalize_status(None, true, false), "done");
        assert_eq!(normalize_status(None, false, false), "triage");
    }
}
