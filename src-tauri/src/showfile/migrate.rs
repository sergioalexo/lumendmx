//! Upgrades an on-disk `.lumen` JSON document (any past `schema_version`) to
//! the current `ShowFile` shape.
//!
//! To add a new version: bump `CURRENT_SCHEMA_VERSION` in `schema.rs`, add an
//! `upgrade_N_to_N+1` step below, and call it from the match arm for version
//! `N`. Each step takes and returns a `serde_json::Value` so it can add/rename/
//! drop fields without needing the old Rust struct to still exist.

use serde_json::Value;

use super::schema::{ShowFile, CURRENT_SCHEMA_VERSION};

pub fn migrate(mut raw: Value) -> Result<ShowFile, String> {
    let obj = raw
        .as_object_mut()
        .ok_or_else(|| "Show file is not a JSON object".to_string())?;

    let version = obj
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;

    if version > CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "This show file is from a newer version of LumenDMX (schema {}, this app supports up to {}). Update the app first.",
            version, CURRENT_SCHEMA_VERSION
        ));
    }

    let mut current_version = version;
    let mut value = raw;

    if current_version == 0 {
        value = upgrade_0_to_1(value);
        current_version = 1;
    }

    debug_assert_eq!(current_version, CURRENT_SCHEMA_VERSION);

    serde_json::from_value(value).map_err(|e| format!("Failed to parse show file: {e}"))
}

/// Version 0 is "no `schema_version` field at all" — either a hand-written
/// test fixture or, in the future, genuinely pre-showfile data. Fills in every
/// field `ShowFile` requires with sensible defaults, keeping whatever the
/// input already had.
fn upgrade_0_to_1(value: Value) -> Value {
    let mut obj = match value {
        Value::Object(o) => o,
        _ => serde_json::Map::new(),
    };

    let default = ShowFile::new_default("Untitled Show");
    let default_value = serde_json::to_value(&default).expect("ShowFile always serializes");
    let default_obj = default_value.as_object().expect("object").clone();

    for (key, default_val) in default_obj {
        obj.entry(key).or_insert(default_val);
    }
    obj.insert("schemaVersion".to_string(), Value::from(1));

    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_round_trips_exactly() {
        let show = ShowFile::new_default("My Show");
        let json = serde_json::to_value(&show).unwrap();
        let migrated = migrate(json).unwrap();
        assert_eq!(migrated, show);
    }

    #[test]
    fn missing_schema_version_fills_in_defaults() {
        let raw = serde_json::json!({ "name": "Old Show" });
        let migrated = migrate(raw).unwrap();
        assert_eq!(migrated.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(migrated.name, "Old Show");
        assert_eq!(migrated.universes.len(), 1);
        assert_eq!(migrated.workspaces.len(), 4);
    }

    #[test]
    fn preserves_existing_patch_and_settings_while_filling_gaps() {
        let raw = serde_json::json!({
            "name": "Partial",
            "patch": [
                { "id": "f1", "fixtureId": "generic-6color-par", "modeIndex": 0, "address": 1, "name": "Par 1" }
            ],
        });
        let migrated = migrate(raw).unwrap();
        assert_eq!(migrated.patch.len(), 1);
        assert_eq!(migrated.patch[0].address, 1);
        assert_eq!(migrated.settings.output_rate_hz, 30);
    }

    #[test]
    fn future_schema_version_is_rejected() {
        let raw = serde_json::json!({ "schemaVersion": CURRENT_SCHEMA_VERSION + 1 });
        let err = migrate(raw).unwrap_err();
        assert!(err.contains("newer version"));
    }

    #[test]
    fn non_object_input_is_rejected() {
        let err = migrate(serde_json::json!([1, 2, 3])).unwrap_err();
        assert!(err.contains("not a JSON object"));
    }
}
