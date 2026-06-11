use crate::db::{
    coerce_bool, coerce_float, coerce_int, discover_models, open_database, quote_ident, ModelDecl,
    ModelField,
};
use crate::debugbar::{QuerySource, TaskTrace};
use crate::parser::Value;
use rusqlite::{params_from_iter, Connection, Row};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
        self.call_with_trace(model_name, method, args, None)
    }

    pub fn call_with_trace(
        &self,
        model_name: &str,
        method: &str,
        args: &[Value],
        trace: Option<&Arc<Mutex<TaskTrace>>>,
    ) -> Result<Value, String> {
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
                let query =
                    begin_model_query(trace, format!("{model_name}.all"), sql.clone(), Vec::new());
                let result = (|| {
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
                })();
                finish_model_query(trace, query, &result);
                result
            }
            "find" => {
                let id = expect_int(args, 0, &format!("{model_name}.find"))?;
                let sql = format!(
                    "SELECT * FROM {} WHERE {} = ?1",
                    quote_ident(&model.name),
                    quote_ident("id")
                );
                find_by_sql(
                    &connection,
                    model,
                    &sql,
                    [id],
                    trace,
                    format!("{model_name}.find"),
                    vec![id.to_string()],
                )
            }
            "create" => {
                let fields = expect_object(args, 0, &format!("{model_name}.create"))?;
                insert_row(&connection, model, &fields, trace)
            }
            "update" => {
                let id = expect_int(args, 0, &format!("{model_name}.update"))?;
                let fields = expect_object(args, 1, &format!("{model_name}.update"))?;
                update_row(&connection, model, id, &fields, trace)
            }
            "deleteAll" => {
                if !args.is_empty() {
                    return Err(format!("{model_name}.deleteAll expects 0 arguments"));
                }
                let sql = format!("DELETE FROM {}", quote_ident(&model.name));
                let query = begin_model_query(
                    trace,
                    format!("{model_name}.deleteAll"),
                    sql.clone(),
                    Vec::new(),
                );
                let result = connection
                    .execute(&sql, [])
                    .map(|_| empty_object())
                    .map_err(|error| error.to_string());
                finish_model_query(trace, query, &result);
                result
            }
            other => Err(format!("unknown method `{model_name}.{other}`")),
        }
    }
}

fn insert_row(
    connection: &Connection,
    model: &ModelDecl,
    fields: &BTreeMap<String, Value>,
    trace: Option<&Arc<Mutex<TaskTrace>>>,
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
    let params = values.iter().map(sql_param_string).collect();
    let query = begin_model_query(trace, format!("{}.create", model.name), sql.clone(), params);
    let result = connection
        .execute(&sql, params_from_iter(values.iter()))
        .map_err(|error| error.to_string());
    finish_model_query(trace, query, &result);
    result?;
    let id = connection.last_insert_rowid();
    find_by_id(connection, model, id, trace)
}

fn update_row(
    connection: &Connection,
    model: &ModelDecl,
    id: i64,
    fields: &BTreeMap<String, Value>,
    trace: Option<&Arc<Mutex<TaskTrace>>>,
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
        return find_by_id(connection, model, id, trace);
    }

    values.push(rusqlite::types::Value::Integer(id));
    let sql = format!(
        "UPDATE {} SET {} WHERE {} = ?",
        quote_ident(&model.name),
        assignments.join(", "),
        quote_ident("id")
    );
    let params = values.iter().map(sql_param_string).collect();
    let query = begin_model_query(trace, format!("{}.update", model.name), sql.clone(), params);
    let update_result = connection
        .execute(&sql, params_from_iter(values.iter()))
        .map_err(|error| error.to_string());
    finish_model_query(trace, query, &update_result);
    let updated = update_result?;
    if updated == 0 {
        return Err(format!("{} {id} not found", model.name));
    }
    find_by_id(connection, model, id, trace)
}

fn find_by_id(
    connection: &Connection,
    model: &ModelDecl,
    id: i64,
    trace: Option<&Arc<Mutex<TaskTrace>>>,
) -> Result<Value, String> {
    let sql = format!(
        "SELECT * FROM {} WHERE {} = ?1",
        quote_ident(&model.name),
        quote_ident("id")
    );
    find_by_sql(
        connection,
        model,
        &sql,
        [id],
        trace,
        format!("{}.find", model.name),
        vec![id.to_string()],
    )
}

fn find_by_sql<const N: usize>(
    connection: &Connection,
    model: &ModelDecl,
    sql: &str,
    params: [i64; N],
    trace: Option<&Arc<Mutex<TaskTrace>>>,
    label: String,
    param_labels: Vec<String>,
) -> Result<Value, String> {
    let query = begin_model_query(trace, label, sql.to_string(), param_labels);
    let result = (|| {
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| error.to_string())?;
        let mut rows = statement
            .query_map(params_from_iter(params.iter()), |row| {
                row_to_value(row, model)
            })
            .map_err(|error| error.to_string())?;
        match rows.next() {
            Some(Ok(value)) => Ok(value),
            Some(Err(error)) => Err(error.to_string()),
            None => Ok(empty_object()),
        }
    })();
    finish_model_query(trace, query, &result);
    result
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
        "float" => Ok(Value::Int(row.get::<_, f64>(index)?.round() as i64)),
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

fn sql_param_string(value: &rusqlite::types::Value) -> String {
    match value {
        rusqlite::types::Value::Null => "null".to_string(),
        rusqlite::types::Value::Integer(value) => value.to_string(),
        rusqlite::types::Value::Real(value) => value.to_string(),
        rusqlite::types::Value::Text(value) => value.clone(),
        rusqlite::types::Value::Blob(value) => format!("<{} bytes>", value.len()),
    }
}

fn begin_model_query(
    trace: Option<&Arc<Mutex<TaskTrace>>>,
    label: String,
    sql: String,
    params: Vec<String>,
) -> Option<usize> {
    trace.and_then(|trace| {
        trace
            .lock()
            .ok()
            .map(|mut guard| guard.begin_query(label, QuerySource::Model, sql, params))
    })
}

fn finish_model_query<T>(
    trace: Option<&Arc<Mutex<TaskTrace>>>,
    query_index: Option<usize>,
    result: &Result<T, String>,
) {
    if let (Some(trace), Some(query_index)) = (trace, query_index) {
        if let Ok(mut guard) = trace.lock() {
            guard.finish_query(query_index, result.is_ok(), result.as_ref().err().cloned());
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use std::fs;
    use std::sync::{Arc, Mutex};
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

    #[test]
    fn records_model_queries_with_generated_sql() {
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
        let trace = Arc::new(Mutex::new(TaskTrace::new()));
        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), Value::String("Ship it".to_string()));
        runtime
            .call_with_trace("Todo", "create", &[Value::Object(fields)], Some(&trace))
            .expect("create");
        runtime
            .call_with_trace("Todo", "all", &[], Some(&trace))
            .expect("all");

        let trace = trace.lock().expect("trace");
        let queries = trace.queries();
        assert!(queries
            .iter()
            .any(|query| query.sql.starts_with("INSERT INTO")));
        assert!(queries
            .iter()
            .any(|query| query.sql.contains("WHERE") && query.sql.contains("?1")));
        assert!(queries
            .iter()
            .any(|query| query.sql.contains("ORDER BY")));
        assert!(queries.iter().all(|query| query.source == QuerySource::Model));

        let _ = fs::remove_dir_all(root);
    }
}
