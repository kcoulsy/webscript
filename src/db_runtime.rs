use crate::db::{open_database, value_to_sql};
use crate::parser::Value;
use crate::schema_runtime::SchemaRuntime;
use rusqlite::{params_from_iter, Row};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct DbRuntime {
    root: PathBuf,
    schemas: Arc<SchemaRuntime>,
}

impl DbRuntime {
    pub fn new(root: PathBuf, schemas: Arc<SchemaRuntime>) -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self { root, schemas }))
    }

    pub fn call(&self, method: &str, args: &[Value]) -> Result<Value, String> {
        match method {
            "execute" => self.execute(args),
            "query" => Err("db.query requires a schema; use db.query(sql, Schema) or db.query(sql, params, Schema)".to_string()),
            other => Err(format!("unknown method `db.{other}`")),
        }
    }

    pub fn call_query(&self, args: &[Value], schema_name: &str) -> Result<Value, String> {
        let connection = open_database(&self.root)?;
        let sql = expect_string(args, 0, "db.query")?;
        let params = expect_query_params(args, "query")?;
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params_from_iter(params.iter()), row_to_object)
            .map_err(|error| error.to_string())?;
        let mut values = Vec::new();
        for row in rows {
            values.push(row.map_err(|error| error.to_string())?);
        }
        self.schemas.validate_rows(schema_name, values)
    }

    fn execute(&self, args: &[Value]) -> Result<Value, String> {
        let connection = open_database(&self.root)?;
        let sql = expect_string(args, 0, "db.execute")?;
        let params = expect_execute_params(args, "execute")?;
        connection
            .execute(&sql, params_from_iter(params.iter()))
            .map_err(|error| error.to_string())?;
        let mut fields = BTreeMap::new();
        fields.insert(
            "changes".to_string(),
            Value::Int(connection.changes() as i64),
        );
        fields.insert(
            "lastInsertId".to_string(),
            Value::Int(connection.last_insert_rowid()),
        );
        Ok(Value::Object(fields))
    }
}

fn row_to_object(row: &Row<'_>) -> rusqlite::Result<Value> {
    let mut fields = BTreeMap::new();
    let column_count = row.as_ref().column_count();
    for index in 0..column_count {
        let value = row.get_ref(index)?;
        if value.data_type() == rusqlite::types::Type::Null {
            continue;
        }
        let name = row.as_ref().column_name(index)?.to_string();
        fields.insert(name, sqlite_value_to_parser(value)?);
    }
    Ok(Value::Object(fields))
}

fn sqlite_value_to_parser(value: rusqlite::types::ValueRef<'_>) -> rusqlite::Result<Value> {
    Ok(match value {
        rusqlite::types::ValueRef::Null => Value::String(String::new()),
        rusqlite::types::ValueRef::Integer(value) => Value::Int(value),
        rusqlite::types::ValueRef::Real(value) => Value::Int(value.round() as i64),
        rusqlite::types::ValueRef::Text(text) => {
            Value::String(String::from_utf8_lossy(text).into_owned())
        }
        rusqlite::types::ValueRef::Blob(bytes) => {
            Value::String(String::from_utf8_lossy(bytes).into_owned())
        }
    })
}

fn expect_string(args: &[Value], index: usize, name: &str) -> Result<String, String> {
    let value = args
        .get(index)
        .ok_or_else(|| format!("{name} expects a SQL string"))?;
    match value {
        Value::String(text) => Ok(text.clone()),
        other => Err(format!(
            "{name} expects string, found `{}`",
            other.type_name()
        )),
    }
}

fn expect_query_params(
    args: &[Value],
    method: &str,
) -> Result<Vec<rusqlite::types::Value>, String> {
    if args.len() <= 1 {
        return Ok(Vec::new());
    }
    if args.len() > 2 {
        return Err(format!(
            "db.{method} expects (sql, Schema) or (sql, params, Schema)"
        ));
    }
    match &args[1] {
        Value::Array { values, .. } => values.iter().map(value_to_sql).collect(),
        other => Err(format!(
            "db.{method} params must be an array, found `{}`",
            other.type_name()
        )),
    }
}

fn expect_execute_params(
    args: &[Value],
    method: &str,
) -> Result<Vec<rusqlite::types::Value>, String> {
    if args.len() <= 1 {
        return Ok(Vec::new());
    }
    if args.len() > 2 {
        return Err(format!("db.{method} expects at most 2 arguments"));
    }
    match &args[1] {
        Value::Array { values, .. } => values.iter().map(value_to_sql).collect(),
        other => Err(format!(
            "db.{method} params must be an array, found `{}`",
            other.type_name()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::schema_runtime::SchemaRuntime;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("webscript-db-runtime-{nanos}"))
    }

    fn setup_todo_db(root: &PathBuf) {
        fs::create_dir_all(root.join("app/models")).expect("models dir");
        fs::create_dir_all(root.join("app/schemas")).expect("schemas dir");
        fs::write(
            root.join("app/models/Todo.web"),
            "@model Todo {\n  id: int @primary @auto\n  title: string\n  done: bool @default(false)\n  createdAt: datetime @default(now)\n}\n",
        )
        .expect("write model");
        fs::write(
            root.join("app/schemas/TodoRow.web"),
            "@schema TodoRow {\n  title: string\n  done: bool\n}\n",
        )
        .expect("write schema");
        db::generate(root, Some("schema")).expect("generate");
        db::migrate(root).expect("migrate");
    }

    fn runtime_for(root: PathBuf) -> Arc<DbRuntime> {
        let schemas = SchemaRuntime::new(root.clone()).expect("schemas");
        DbRuntime::new(root, schemas).expect("runtime")
    }

    #[test]
    fn query_returns_rows() {
        let root = temp_root();
        setup_todo_db(&root);

        let runtime = runtime_for(root.clone());
        runtime
            .call(
                "execute",
                &[
                    Value::String("INSERT INTO Todo (title, done) VALUES (?1, ?2)".to_string()),
                    Value::Array {
                        element_type: "object".to_string(),
                        values: vec![Value::String("Ship it".to_string()), Value::Bool(false)],
                    },
                ],
            )
            .expect("insert");

        let rows = runtime
            .call_query(
                &[Value::String("SELECT title, done FROM Todo".to_string())],
                "TodoRow",
            )
            .expect("query");
        let Value::Array {
            values,
            element_type,
        } = rows
        else {
            panic!("expected array");
        };
        assert_eq!(element_type, "TodoRow");
        assert_eq!(values.len(), 1);
        let Value::Object(fields) = &values[0] else {
            panic!("expected object row");
        };
        assert_eq!(
            fields.get("title"),
            Some(&Value::String("Ship it".to_string()))
        );
        assert_eq!(fields.get("done"), Some(&Value::Bool(false)));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn execute_reports_changes() {
        let root = temp_root();
        setup_todo_db(&root);

        let runtime = runtime_for(root.clone());
        runtime
            .call(
                "execute",
                &[
                    Value::String("INSERT INTO Todo (title) VALUES (?1)".to_string()),
                    Value::Array {
                        element_type: "object".to_string(),
                        values: vec![Value::String("One".to_string())],
                    },
                ],
            )
            .expect("insert");

        let result = runtime
            .call("execute", &[Value::String("DELETE FROM Todo".to_string())])
            .expect("delete");
        let Value::Object(fields) = result else {
            panic!("expected object");
        };
        assert_eq!(fields.get("changes"), Some(&Value::Int(1)));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parameterized_binding_works() {
        let root = temp_root();
        setup_todo_db(&root);

        let runtime = runtime_for(root.clone());
        runtime
            .call(
                "execute",
                &[
                    Value::String("INSERT INTO Todo (title, done) VALUES (?1, ?2)".to_string()),
                    Value::Array {
                        element_type: "object".to_string(),
                        values: vec![Value::String("Active".to_string()), Value::Bool(false)],
                    },
                ],
            )
            .expect("insert active");
        runtime
            .call(
                "execute",
                &[
                    Value::String("INSERT INTO Todo (title, done) VALUES (?1, ?2)".to_string()),
                    Value::Array {
                        element_type: "object".to_string(),
                        values: vec![Value::String("Done".to_string()), Value::Bool(true)],
                    },
                ],
            )
            .expect("insert done");

        let rows = runtime
            .call_query(
                &[
                    Value::String("SELECT title, done FROM Todo WHERE done = ?1".to_string()),
                    Value::Array {
                        element_type: "object".to_string(),
                        values: vec![Value::Bool(true)],
                    },
                ],
                "TodoRow",
            )
            .expect("query");
        let Value::Array { values, .. } = rows else {
            panic!("expected array");
        };
        assert_eq!(values.len(), 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_requires_schema() {
        let root = temp_root();
        setup_todo_db(&root);

        let runtime = runtime_for(root.clone());
        let error = runtime
            .call("query", &[Value::String("SELECT 1".to_string())])
            .expect_err("missing schema");
        assert!(error.contains("requires a schema"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_database_gives_clear_error() {
        let root = temp_root();
        fs::create_dir_all(root.join("app/schemas")).expect("schemas dir");
        let runtime = runtime_for(root.clone());
        let error = runtime
            .call_query(&[Value::String("SELECT 1".to_string())], "TodoRow")
            .expect_err("missing db");
        assert!(error.contains("database not found"));
        assert!(error.contains("db:migrate"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unknown_method_gives_clear_error() {
        let root = temp_root();
        setup_todo_db(&root);

        let runtime = runtime_for(root.clone());
        let error = runtime
            .call("foo", &[Value::String("SELECT 1".to_string())])
            .expect_err("unknown method");
        assert_eq!(error, "unknown method `db.foo`");

        let _ = fs::remove_dir_all(root);
    }
}
