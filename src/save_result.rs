//! Port of `src/lib/saveResult.ts`.

/// `isFailedSaveResult` — `{ok: false}` shapes only.
pub fn is_failed_save_result(result: &serde_json::Value) -> bool {
    result.get("ok") == Some(&serde_json::Value::Bool(false))
}
