use crate::db::{discover_models, ModelDecl, ModelField, SQLITE_PATH};
use crate::parser::Value;
use rusqlite::{params_from_iter, Connection, Row};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct ModelRuntime {
    root: PathBuf,
    models: BTreeMap<String, ModelDecl>,
}

impl ModelRuntime {
    pub fn new(root: PathBuf) -> Result<Arc<Self>, String> {
        let models = discover_models(&root)
            .map_err(|error| match error {
                crate::db::ModelLoadError::Diagnostic(diagnostic) => diagnostic.diagnostic.message,
                crate::db::ModelLoadError::Io(error) => error,
            })?
            .into_iter()
            .map(|model| (model.name.clone(), model))
            .collect();

        Ok(Arc::new(Self { root, models }))
    }

    pub fn call(&self, model_name: &str, method: &str, args: &[Value]) -> Result<Value, String> {
        let model = self
            .models
            .get(model_name)
            .ok_or_else(|| format!("unknown model `{model_name}`"))?;
        let connection = open_database(&self.root)?;

        match method {
            "all" => {
                if !args.is_empty() {
                    return Err(format!("{model_name}.all expects 0 arguments"));
                }
                let sql = format!(
                    "SELECT * FROM {} ORDER BY {}",
                    quote_ident(&model.name),
                    quote_ident("createdAt")
                );
                let mut statement = connection
                    .prepare(&sql)
                    .map_err(|error| error.to_string())?;
                let rows = statement
                    .query_map([], |row| row_to_value(row, model))
                    .map_err(|error| error.to_string())?;
                let mut values = Vec::new();
                for row in rows {
                    values.push(row.map_err(|error| error.to_string())?);
                }
                Ok(Value::Array {
                    element_type: model.name.clone(),
                    values,
                })
            }
            "find" => {
                let id = expect_int(args, 0, &format!("{model_name}.find"))?;
                let sql = format!(
                    "SELECT * FROM {} WHERE {} = ?1",
                    quote_ident(&model.name),
                    quote_ident("id")
                );
                let mut statement = connection
                    .prepare(&sql)
                    .map_err(|error| error.to_string())?;
                let mut rows = statement
                    .query_map([id], |row| row_to_value(row, model))
                    .map_err(|error| error.to_string())?;
                match rows.next() {
                    Some(Ok(value)) => Ok(value),
                    Some(Err(error)) => Err(error.to_string()),
                    None => Ok(empty_object()),
                }
            }
            "create" => {
                let fields = expect_object(args, 0, &format!("{model_name}.create"))?;
                insert_row(&connection, model, &fields)
            }
            "update" => {
                let id = expect_int(args, 0, &format!("{model_name}.update"))?;
                let fields = expect_object(args, 1, &format!("{model_name}.update"))?;
                update_row(&connection, model, id, &fields)
            }
            "deleteAll" => {
                if !args.is_empty() {
                    return Err(format!("{model_name}.deleteAll expects 0 arguments"));
                }
                let sql = format!("DELETE FROM {}", quote_ident(&model.name));
                connection
                    .execute(&sql, [])
                    .map_err(|error| error.to_string())?;
                Ok(empty_object())
            }
            other => Err(format!("unknown method `{model_name}.{other}`")),
        }
    }
}

fn open_database(root: &Path) -> Result<Connection, String> {
    let database_path = root.join(SQLITE_PATH);
    if !database_path.exists() {
        return Err(format!(
            "database not found at `{}`; run `web db:migrate` first",
            database_path.display()
        ));
    }
    Connection::open(&database_path).map_err(|error| error.to_string())
}

fn insert_row(
    connection: &Connection,
    model: &ModelDecl,
    fields: &BTreeMap<String, Value>,
) -> Result<Value, String> {
    let mut columns = Vec::new();
    let mut placeholders = Vec::new();
    let mut values = Vec::new();

    for field in &model.fields {
        if field.auto {
            continue;
        }
        let Some(value) = fields.get(&field.name) else {
            if field.default.is_some() {
                continue;
            }
            if !field.nullable && !field.primary {
                return Err(format!("missing field `{}` for {}", field.name, model.name));
            }
            continue;
        };
        columns.push(quote_ident(&field.name));
        placeholders.push("?".to_string());
        values.push(sql_value(value, field)?);
    }

    if columns.is_empty() {
        return Err(format!("no fields provided for {}", model.name));
    }

    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        quote_ident(&model.name),
        columns.join(", "),
        placeholders.join(", ")
    );
    connection
        .execute(&sql, params_from_iter(values.iter()))
        .map_err(|error| error.to_string())?;
    let id = connection.last_insert_rowid();
    find_by_id(connection, model, id)
}

fn update_row(
    connection: &Connection,
    model: &ModelDecl,
    id: i64,
    fields: &BTreeMap<String, Value>,
) -> Result<Value, String> {
    let mut assignments = Vec::new();
    let mut values = Vec::new();

    for field in &model.fields {
        if field.primary || field.auto {
            continue;
        }
        let Some(value) = fields.get(&field.name) else {
            continue;
        };
        assignments.push(format!("{} = ?", quote_ident(&field.name)));
        values.push(sql_value(value, field)?);
    }

    if assignments.is_empty() {
        return find_by_id(connection, model, id);
    }

    values.push(rusqlite::types::Value::Integer(id));
    let sql = format!(
        "UPDATE {} SET {} WHERE {} = ?",
        quote_ident(&model.name),
        assignments.join(", "),
        quote_ident("id")
    );
    let updated = connection
        .execute(&sql, params_from_iter(values.iter()))
        .map_err(|error| error.to_string())?;
    if updated == 0 {
        return Err(format!("{} {id} not found", model.name));
    }
    find_by_id(connection, model, id)
}

fn find_by_id(connection: &Connection, model: &ModelDecl, id: i64) -> Result<Value, String> {
    let sql = format!(
        "SELECT * FROM {} WHERE {} = ?1",
        quote_ident(&model.name),
        quote_ident("id")
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| error.to_string())?;
    let mut rows = statement
        .query_map([id], |row| row_to_value(row, model))
        .map_err(|error| error.to_string())?;
    match rows.next() {
        Some(Ok(value)) => Ok(value),
        Some(Err(error)) => Err(error.to_string()),
        None => Ok(empty_object()),
    }
}

fn row_to_value(row: &Row<'_>, model: &ModelDecl) -> rusqlite::Result<Value> {
    let mut fields = BTreeMap::new();
    for field in &model.fields {
        let value = read_field(row, field)?;
        fields.insert(field.name.clone(), value);
    }
    Ok(Value::Object(fields))
}

fn read_field(row: &Row<'_>, field: &ModelField) -> rusqlite::Result<Value> {
    let index = row
        .as_ref()
        .column_index(&field.name)
        .or_else(|_| row.as_ref().column_index(&quote_ident(&field.name)))?;

    match field.type_name.as_str() {
        "int" => Ok(Value::Int(row.get(index)?)),
        "bool" => Ok(Value::Bool(row.get::<_, i64>(index)? != 0)),
        "float" => Ok(Value::Int(
            row.get::<_, f64>(index)?.round() as i64,
        )),
        "bytes" => {
            let bytes: Vec<u8> = row.get(index)?;
            Ok(Value::String(String::from_utf8_lossy(&bytes).to_string()))
        }
        _ => Ok(Value::String(row.get(index)?)),
    }
}

fn sql_value(value: &Value, field: &ModelField) -> Result<rusqlite::types::Value, String> {
    match field.type_name.as_str() {
        "int" => Ok(rusqlite::types::Value::Integer(coerce_int(value)?)),
        "bool" => Ok(rusqlite::types::Value::Integer(if coerce_bool(value)? {
            1
        } else {
            0
        })),
        "float" => Ok(rusqlite::types::Value::Real(coerce_float(value)?)),
        "bytes" => match value {
            Value::String(text) => Ok(rusqlite::types::Value::Blob(text.as_bytes().to_vec())),
            other => Err(format!(
                "field `{}` expects bytes, found `{}`",
                field.name,
                other.type_name()
            )),
        },
        _ => Ok(rusqlite::types::Value::Text(value.render())),
    }
}

fn coerce_int(value: &Value) -> Result<i64, String> {
    match value {
        Value::Int(value) => Ok(*value),
        Value::String(text) => text
            .parse()
            .map_err(|_| format!("expected int, found string `{text}`")),
        other => Err(format!("expected int, found `{}`", other.type_name())),
    }
}

fn coerce_bool(value: &Value) -> Result<bool, String> {
    match value {
        Value::Bool(value) => Ok(*value),
        Value::Int(value) => Ok(*value != 0),
        Value::String(text) => match text.as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            other => Err(format!("expected bool, found string `{other}`")),
        },
        other => Err(format!("expected bool, found `{}`", other.type_name())),
    }
}

fn coerce_float(value: &Value) -> Result<f64, String> {
    match value {
        Value::Int(value) => Ok(*value as f64),
        Value::String(text) => text
            .parse()
            .map_err(|_| format!("expected float, found string `{text}`")),
        other => Err(format!("expected float, found `{}`", other.type_name())),
    }
}

fn expect_int(args: &[Value], index: usize, name: &str) -> Result<i64, String> {
    let value = args
        .get(index)
        .ok_or_else(|| format!("{name} expects argument {}", index + 1))?;
    coerce_int(value)
}

fn expect_object<'a>(
    args: &'a [Value],
    index: usize,
    name: &str,
) -> Result<&'a BTreeMap<String, Value>, String> {
    let value = args
        .get(index)
        .ok_or_else(|| format!("{name} expects argument {}", index + 1))?;
    match value {
        Value::Object(fields) => Ok(fields),
        other => Err(format!(
            "{name} expects object, found `{}`",
            other.type_name()
        )),
    }
}

fn empty_object() -> Value {
    Value::Object(BTreeMap::new())
}

fn quote_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("webscript-model-runtime-{nanos}"))
    }

    #[test]
    fn creates_and_lists_todos() {
        let root = temp_root();
        fs::create_dir_all(root.join("app/models")).expect("models dir");
        fs::write(
            root.join("app/models/Todo.web"),
            "@model Todo {\n  id: int @primary @auto\n  title: string\n  done: bool @default(false)\n  createdAt: datetime @default(now)\n}\n",
        )
        .expect("write model");
        db::generate(&root, Some("schema")).expect("generate");
        db::migrate(&root).expect("migrate");

        let runtime = ModelRuntime::new(root.clone()).expect("runtime");
        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), Value::String("Ship it".to_string()));
        let created = runtime
            .call("Todo", "create", &[Value::Object(fields)])
            .expect("create");
        let Value::Object(created_fields) = created else {
            panic!("expected object");
        };
        assert_eq!(
            created_fields.get("title"),
            Some(&Value::String("Ship it".to_string()))
        );

        let listed = runtime.call("Todo", "all", &[]).expect("all");
        let Value::Array { values, .. } = listed else {
            panic!("expected array");
        };
        assert_eq!(values.len(), 1);

        let _ = fs::remove_dir_all(root);
    }
}
