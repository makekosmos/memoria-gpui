//! Engine is the only owner of note persistence. No database or local mirror.
//! M0 ships the `/v1/rpc` transport only; ARK object mapping lands per PARITY.md.
mod transport;

#[allow(unused_imports)]
pub use transport::Engine;

/// Object types Memoria reads through the typed ARK grant (manifest `data.access`).
/// Reserved for the M-task store implementation; unused in the M0 scaffold.
#[allow(dead_code)]
pub const NOTE_TYPE: &str = "com.kosmos.note";
#[allow(dead_code)]
pub const TASK_TYPE: &str = "com.kosmos.task";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_reports_missing_lock() {
        let mut engine = Engine::default();
        engine.data_dir = Some(std::env::temp_dir().join("memoria-gpui-no-such-dir"));
        let error = engine
            .rpc("list_objects", serde_json::json!({}))
            .unwrap_err();
        assert!(error.contains("Engine"), "unexpected error: {error}");
    }
}
