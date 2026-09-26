//! Live Preview projection: source markdown → visible text + styled spans
//! + bidirectional source↔visible map.

mod autolink;
mod types;
mod walk;

pub use types::{BlockTag, Chunk, Marks, Payload, Projection, RangeB, Span};
pub use walk::project;
