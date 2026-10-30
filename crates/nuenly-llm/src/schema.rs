//! JSON schema untuk output terstruktur.

use std::fmt;

use serde_json::Value;

use crate::LlmError;

/// Schema output dari mode pack: dikirim ke model sebagai `format` dan dipakai memvalidasi
/// jawabannya.
pub struct OutputSchema {
    raw: Value,
    validator: jsonschema::Validator,
}

impl OutputSchema {
    /// # Errors
    ///
    /// [`LlmError::InvalidSchema`] bila `raw` bukan JSON schema yang valid, atau memakai `$ref`
    /// ke luar dokumen.
    pub fn new(raw: Value) -> Result<Self, LlmError> {
        let validator = jsonschema::validator_for(&raw).map_err(|_| LlmError::InvalidSchema)?;
        Ok(Self { raw, validator })
    }

    #[must_use]
    pub fn raw(&self) -> &Value {
        &self.raw
    }

    /// Apakah `instance` lolos schema. Detail kegagalan sengaja dibuang karena memuat isi
    /// jawaban.
    #[must_use]
    pub fn accepts(&self, instance: &Value) -> bool {
        self.validator.is_valid(instance)
    }
}

impl fmt::Debug for OutputSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutputSchema").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::OutputSchema;
    use crate::LlmError;

    fn suggestion() -> OutputSchema {
        OutputSchema::new(json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "maxLength": 80 },
                "points": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["title", "points"],
            "additionalProperties": false
        }))
        .unwrap()
    }

    #[test]
    fn accepts_only_matching_output() {
        let schema = suggestion();
        assert!(schema.accepts(&json!({ "title": "Ringkas", "points": [] })));
        assert!(!schema.accepts(&json!({ "title": "Ringkas" })));
        assert!(
            !schema
                .accepts(&json!({ "title": "x", "points": [], "url": "https://contoh.invalid" }))
        );
    }

    #[test]
    fn rejects_invalid_schemas_and_external_refs() {
        assert_eq!(
            OutputSchema::new(json!({ "type": "bukan-tipe" })).unwrap_err(),
            LlmError::InvalidSchema
        );
        assert_eq!(
            OutputSchema::new(json!({ "$ref": "https://contoh.invalid/schema.json" })).unwrap_err(),
            LlmError::InvalidSchema
        );
    }
}
