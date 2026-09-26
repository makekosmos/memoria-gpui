//! `ArkRequest` seam from `kepler-entry-api.ts`: every API layer speaks raw
//! `(operation, params) -> Value` RPC, so tests can swap the HTTP Engine for
//! an in-memory fake without changing call sites.

use super::*;

/// The `ArkRequest` seam from `kepler-entry-api.ts`: every API layer speaks
/// raw `(operation, params) -> Value` RPC, so tests can swap the HTTP Engine
/// for an in-memory fake without changing call sites.
pub trait ArkBridge: Clone {
    fn rpc(&self, operation: &str, params: Value) -> Result<Value, EngineError>;

    /// `ensureList` — bare arrays or `{items|objects|links|types}` wrappers.
    fn rpc_list(&self, operation: &str, params: Value) -> Result<Vec<Value>, EngineError> {
        self.rpc(operation, params).map(|v| ensure_list(&v))
    }

    // Typed ops (manifest permissions + the summary/type reads the Vue entry
    // API uses).
    fn list_objects_by_type(&self, type_id: &str) -> Result<Vec<Value>, EngineError> {
        self.rpc_list("list_objects_by_type", json!({ "type_id": type_id }))
    }

    fn list_objects(&self, type_id: &str) -> Result<Vec<Value>, EngineError> {
        self.rpc_list("list_objects", json!({ "type_id": type_id }))
    }

    fn get_object(&self, id: &str) -> Result<Value, EngineError> {
        self.rpc("get_object", json!({ "id": id }))
    }

    fn search_objects(&self, query: &str) -> Result<Value, EngineError> {
        self.rpc("search_objects", json!({ "query": query }))
    }

    fn list_object_types(&self) -> Result<Vec<Value>, EngineError> {
        self.rpc_list("list_object_types", json!({}))
    }

    fn get_object_type(&self, type_id: &str) -> Result<Value, EngineError> {
        self.rpc("get_object_type", json!({ "type_id": type_id }))
    }

    fn list_object_summaries(&self, type_id: &str) -> Result<Vec<Value>, EngineError> {
        self.rpc_list("list_object_summaries", json!({ "type_id": type_id }))
    }

    fn list_object_summaries_by_type(&self, type_id: &str) -> Result<Vec<Value>, EngineError> {
        self.rpc_list(
            "list_object_summaries_by_type",
            json!({ "type_id": type_id }),
        )
    }

    fn list_object_links(&self) -> Result<Vec<Value>, EngineError> {
        self.rpc_list("list_object_links", json!({}))
    }

    fn upsert_object(&self, object: Value) -> Result<Value, EngineError> {
        self.rpc("upsert_object", json!({ "object": object }))
    }

    fn upsert_object_type(&self, object_type: Value) -> Result<Value, EngineError> {
        self.rpc("upsert_object_type", json!({ "object_type": object_type }))
    }

    fn delete_object(&self, id: &str) -> Result<Value, EngineError> {
        self.rpc("delete_object", json!({ "id": id }))
    }

    fn upsert_object_link(&self, object_link: Value) -> Result<Value, EngineError> {
        self.rpc("upsert_object_link", json!({ "object_link": object_link }))
    }

    fn delete_object_link(&self, id: &str) -> Result<Value, EngineError> {
        self.rpc("delete_object_link", json!({ "id": id }))
    }
}

impl ArkBridge for Engine {
    fn rpc(&self, operation: &str, params: Value) -> Result<Value, EngineError> {
        Engine::rpc(self, operation, params)
    }
}
