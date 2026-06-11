use crate::parser::Value;
use crate::schema::{discover_schemas, validate_value, SchemaDecl};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct SchemaRuntime {
    schemas: BTreeMap<String, SchemaDecl>,
}

impl SchemaRuntime {
    pub fn new(root: PathBuf) -> Result<Arc<Self>, String> {
        let schemas = discover_schemas(&root)
            .map_err(|error| match error {
                crate::schema::SchemaLoadError::Diagnostic(diagnostic) => {
                    diagnostic.diagnostic.message
                }
                crate::schema::SchemaLoadError::Io(error) => error,
            })?
            .into_iter()
            .map(|schema| (schema.name.clone(), schema))
            .collect();

        Ok(Arc::new(Self { schemas }))
    }

    pub fn get(&self, name: &str) -> Option<&SchemaDecl> {
        self.schemas.get(name)
    }

    pub fn validate(&self, name: &str, value: Value) -> Result<Value, String> {
        let schema = self
            .schemas
            .get(name)
            .ok_or_else(|| format!("unknown schema `{name}`"))?;
        validate_value(schema, &value)
    }

    pub fn validate_rows(&self, name: &str, rows: Vec<Value>) -> Result<Value, String> {
        let schema = self
            .schemas
            .get(name)
            .ok_or_else(|| format!("unknown schema `{name}`"))?;
        let mut values = Vec::with_capacity(rows.len());
        for row in rows {
            values.push(validate_value(schema, &row)?);
        }
        Ok(Value::Array {
            element_type: name.to_string(),
            values,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("webscript-schema-runtime-{nanos}"))
    }

    #[test]
    fn validates_rows_with_element_type() {
        let root = temp_root();
        fs::create_dir_all(root.join("app/schemas")).expect("schemas dir");
        fs::write(
            root.join("app/schemas/Row.web"),
            "@schema Row {\n  title: string\n  done: bool\n}\n",
        )
        .expect("write schema");

        let runtime = SchemaRuntime::new(root.clone()).expect("runtime");
        let rows = runtime
            .validate_rows(
                "Row",
                vec![Value::Object(BTreeMap::from([
                    ("title".to_string(), Value::String("Ship".to_string())),
                    ("done".to_string(), Value::Int(0)),
                ]))],
            )
            .expect("validate");
        let Value::Array {
            element_type,
            values,
        } = rows
        else {
            panic!("expected array");
        };
        assert_eq!(element_type, "Row");
        assert_eq!(values.len(), 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unknown_schema_errors() {
        let root = temp_root();
        fs::create_dir_all(root.join("app/schemas")).expect("schemas dir");
        let runtime = SchemaRuntime::new(root.clone()).expect("runtime");
        let error = runtime
            .validate("Missing", Value::Object(BTreeMap::new()))
            .expect_err("missing");
        assert!(error.contains("unknown schema"));

        let _ = fs::remove_dir_all(root);
    }
}
