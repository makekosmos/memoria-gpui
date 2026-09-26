//! `fakeArk` port — map-backed `ArkBridge` mirroring the Vue test fake
//! (objects/links maps, `reorderJsonOnRead` for the key-order-insensitive
//! read-back case). Shared by `bubble_ark_api`/`bubble_ark_migrate`.

use std::sync::{Arc, Mutex};

use memoria_gpui::store::bubble_api::BubbleApi;
use memoria_gpui::store::transport::{ArkBridge, EngineError};
use serde_json::{Map, Value};

type Objects = Arc<Mutex<Map<String, Value>>>;
type Links = Arc<Mutex<Map<String, Value>>>;

/// `fakeArk` — every op the bubble API touches, stored in plain maps.
#[derive(Clone)]
pub struct FakeArk {
    pub objects: Objects,
    pub links: Links,
    pub calls: Arc<Mutex<Vec<String>>>,
    reorder_json_on_read: bool,
}

pub(crate) fn reverse_json_keys(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(reverse_json_keys).collect()),
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map.iter().rev() {
                out.insert(k.clone(), reverse_json_keys(v));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

impl FakeArk {
    pub fn new(reorder_json_on_read: bool) -> Self {
        Self {
            objects: Default::default(),
            links: Default::default(),
            calls: Default::default(),
            reorder_json_on_read,
        }
    }
}

impl ArkBridge for FakeArk {
    fn rpc(&self, operation: &str, params: Value) -> Result<Value, EngineError> {
        self.calls.lock().unwrap().push(operation.to_string());
        match operation {
            "list_objects_by_type" => {
                let type_id = params["type_id"].as_str().unwrap_or_default();
                Ok(Value::Array(
                    self.objects
                        .lock()
                        .unwrap()
                        .values()
                        .filter(|o| o["typeId"] == type_id)
                        .cloned()
                        .collect(),
                ))
            }
            "list_object_links" => Ok(Value::Array(
                self.links.lock().unwrap().values().cloned().collect(),
            )),
            "get_object" => {
                let object = self
                    .objects
                    .lock()
                    .unwrap()
                    .get(params["id"].as_str().unwrap_or_default())
                    .cloned()
                    .unwrap_or(Value::Null);
                Ok(if self.reorder_json_on_read && !object.is_null() {
                    let mut object = object;
                    if let Some(content) = object.get_mut("contentJson") {
                        *content = reverse_json_keys(content);
                    }
                    object
                } else {
                    object
                })
            }
            "upsert_object" => {
                let object = params["object"].clone();
                self.objects.lock().unwrap().insert(
                    object["id"].as_str().unwrap_or_default().to_string(),
                    object,
                );
                Ok(Value::from(true))
            }
            "delete_object" => {
                let mut objects = self.objects.lock().unwrap();
                if let Some(object) = objects.get_mut(params["id"].as_str().unwrap_or_default()) {
                    object["deletedAt"] = Value::from(memoria_gpui::time::millis_to_iso(
                        memoria_gpui::time::now_millis(),
                    ));
                }
                Ok(Value::from(true))
            }
            "upsert_object_link" => {
                let link = params["object_link"].clone();
                self.links
                    .lock()
                    .unwrap()
                    .insert(link["id"].as_str().unwrap_or_default().to_string(), link);
                Ok(Value::from(true))
            }
            "delete_object_link" => {
                self.links
                    .lock()
                    .unwrap()
                    .shift_remove(params["id"].as_str().unwrap_or_default());
                Ok(Value::from(true))
            }
            _ => panic!("unexpected {operation}"),
        }
    }
}

pub fn api_for(ark: &FakeArk) -> BubbleApi<FakeArk> {
    BubbleApi::new(ark.clone())
}
