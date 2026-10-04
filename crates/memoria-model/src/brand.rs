//! Product brand constants — the single source for the Mundus name and the
//! legacy Kosmos identifiers kept for the 0.10.0 migration window.

/// User-visible product name (UI text, error messages).
pub const NAME: &str = "Mundus";
/// Env var overriding the Engine data dir.
pub const DATA_DIR_ENV: &str = "MUNDUS_DATA_DIR";
/// Data dir name under the per-OS config dir.
pub const DATA_DIR_NAME: &str = "Mundus";

// MIGRATION(KOS-267): remove the legacy pair after 2026-11-01.
/// Pre-rename env var still honoured while old Engines are in the field.
pub const LEGACY_DATA_DIR_ENV: &str = "KOSMOS_DATA_DIR";
/// Pre-rename data dir name under the per-OS config dir.
pub const LEGACY_DATA_DIR_NAME: &str = "Kosmos";
