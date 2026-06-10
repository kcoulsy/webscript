use crate::diagnostic::{Diagnostic, FileDiagnostic, Span};
use crate::parser::Value;
use rusqlite::{params, Connection};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_SNAPSHOT_PATH: &str = "db/schema.sql";
const MIGRATIONS_PATH: &str = "db/migrations";
pub const SQLITE_PATH: &str = ".web/data.sqlite";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDecl {
    pub name: String,
    pub fields: Vec<ModelField>,
    pub indexes: Vec<ModelIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelField {
    pub name: String,
    pub type_name: String,
    pub primary: bool,
    pub auto: bool,
    pub unique: bool,
    pub nullable: bool,
    pub default: Option<String>,
    pub reference: Option<ModelReference>,
    pub relation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelReference {
    pub model: String,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelIndex {
    pub fields: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateOutcome {
    pub migration: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrateOutcome {
    pub applied: Vec<String>,
}

pub fn check_models(root: &Path) -> Result<Vec<FileDiagnostic>, String> {
    let mut diagnostics = Vec::new();
    match discover_models(root) {
        Ok(_) => {}
        Err(ModelLoadError::Diagnostic(diagnostic)) => diagnostics.push(diagnostic),
        Err(ModelLoadError::Io(error)) => return Err(error),
    }
    Ok(diagnostics)
}

pub fn generate(root: &Path, name: Option<&str>) -> Result<GenerateOutcome, ModelLoadError> {
    let models = discover_models(root)?;
    let schema = generate_sql(&models);
    let snapshot_path = root.join(SCHEMA_SNAPSHOT_PATH);

    if let Ok(existing) = fs::read_to_string(&snapshot_path) {
        if normalize_newlines(&existing) == schema {
            return Ok(GenerateOutcome { migration: None });
        }
    }

    let migrations_dir = root.join(MIGRATIONS_PATH);
    fs::create_dir_all(&migrations_dir).map_err(io_error)?;
    if let Some(parent) = snapshot_path.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }

    let migration_name = sanitize_migration_name(name.unwrap_or("schema"));
    let migration_file = migrations_dir.join(format!("{}_{}.sql", timestamp(), migration_name));
    fs::write(&migration_file, &schema).map_err(io_error)?;
    fs::write(snapshot_path, schema).map_err(io_error)?;

    Ok(GenerateOutcome {
        migration: Some(migration_file),
    })
}

pub fn migrate(root: &Path) -> Result<MigrateOutcome, String> {
    let web_dir = root.join(".web");
    fs::create_dir_all(&web_dir).map_err(|error| error.to_string())?;

    let migrations = migration_files(root)?;
    let database_path = root.join(SQLITE_PATH);
    let mut connection = Connection::open(&database_path).map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS _webscript_migrations (
  name TEXT PRIMARY KEY NOT NULL,
  checksum TEXT NOT NULL,
  applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);",
        )
        .map_err(|error| error.to_string())?;

    let applied_checksums = applied_migrations(&connection)?;
    let mut applied = Vec::new();

    for path in migrations {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("invalid migration filename `{}`", path.display()))?
            .to_string();
        let sql = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let checksum = checksum(&sql);

        if let Some(applied_checksum) = applied_checksums.get(&name) {
            if applied_checksum != &checksum {
                return Err(format!(
                    "applied migration `{name}` has changed checksum: expected {applied_checksum}, found {checksum}"
                ));
            }
            continue;
        }

        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute_batch(&sql)
            .map_err(|error| format!("failed to apply `{name}`: {error}"))?;
        transaction
            .execute(
                "INSERT INTO _webscript_migrations (name, checksum) VALUES (?1, ?2)",
                params![name, checksum],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())?;
        applied.push(path.file_name().unwrap().to_string_lossy().to_string());
    }

    Ok(MigrateOutcome { applied })
}

pub fn discover_models(root: &Path) -> Result<Vec<ModelDecl>, ModelLoadError> {
    let mut files = Vec::new();
    collect_model_files(&root.join("app").join("models"), &mut files).map_err(io_error)?;
    files.sort();

    let mut models = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).map_err(io_error)?;
        match parse_models(&source) {
            Ok(mut parsed) => models.append(&mut parsed),
            Err(diagnostic) => {
                return Err(ModelLoadError::Diagnostic(FileDiagnostic::new(
                    file, source, diagnostic,
                )))
            }
        }
    }

    validate_models(&models).map_err(|diagnostic| {
        let file = root.join("app").join("models");
        ModelLoadError::Diagnostic(FileDiagnostic::new(file, String::new(), diagnostic))
    })?;
    models.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(models)
}

pub fn parse_models(source: &str) -> Result<Vec<ModelDecl>, Diagnostic> {
    let lines: Vec<&str> = source.lines().collect();
    let mut cursor = 0;
    let mut models = Vec::new();

    while cursor < lines.len() {
        let line_number = cursor + 1;
        let trimmed = lines[cursor].trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            cursor += 1;
            continue;
        }
        if !trimmed.starts_with("@model") {
            return Err(parse_error(
                line_number,
                1,
                trimmed.len().max(1),
                format!("unexpected directive `{trimmed}`"),
            ));
        }

        models.push(parse_model(&lines, &mut cursor)?);
    }

    if models.is_empty() {
        return Err(parse_error(1, 1, 1, "missing @model directive"));
    }
    Ok(models)
}

pub fn generate_sql(models: &[ModelDecl]) -> String {
    let mut output = String::new();
    output.push_str("-- @generated by web db:generate\n");
    output.push_str("PRAGMA foreign_keys = ON;\n\n");

    for model in models {
        output.push_str(&format!("CREATE TABLE {} (\n", quote_ident(&model.name)));
        let mut columns = Vec::new();
        for field in &model.fields {
            columns.push(format!("  {}", column_sql(field)));
        }
        output.push_str(&columns.join(",\n"));
        output.push_str("\n);\n\n");
    }

    for model in models {
        for index in &model.indexes {
            let unique = if index.unique { "UNIQUE " } else { "" };
            let prefix = if index.unique { "uniq" } else { "idx" };
            let index_name = format!("{}_{}_{}", prefix, model.name, index.fields.join("_"));
            let fields = index
                .fields
                .iter()
                .map(|field| quote_ident(field))
                .collect::<Vec<_>>()
                .join(", ");
            output.push_str(&format!(
                "CREATE {unique}INDEX IF NOT EXISTS {} ON {} ({fields});\n",
                quote_ident(&index_name),
                quote_ident(&model.name)
            ));
        }
    }

    output
}

fn parse_model(lines: &[&str], cursor: &mut usize) -> Result<ModelDecl, Diagnostic> {
    let line_number = *cursor + 1;
    let trimmed = lines[*cursor].trim();
    let header = trimmed
        .strip_prefix("@model")
        .expect("@model prefix already checked")
        .trim();
    let name = header
        .strip_suffix('{')
        .ok_or_else(|| {
            parse_error(
                line_number,
                1,
                trimmed.len(),
                "@model expects `@model Name {`",
            )
        })?
        .trim();

    if !is_model_name(name) {
        return Err(parse_error(
            line_number,
            1,
            trimmed.len(),
            format!("invalid model name `{name}`"),
        ));
    }

    *cursor += 1;
    let mut fields = Vec::new();
    let mut indexes = Vec::new();
    let mut field_names = BTreeSet::new();

    while *cursor < lines.len() {
        let line_number = *cursor + 1;
        let trimmed = lines[*cursor].trim();
        if trimmed == "}" {
            *cursor += 1;
            return Ok(ModelDecl {
                name: name.to_string(),
                fields,
                indexes,
            });
        }
        if trimmed.is_empty() || trimmed.starts_with("//") {
            *cursor += 1;
            continue;
        }
        if trimmed.starts_with("@index") || trimmed.starts_with("@uniqueIndex") {
            indexes.push(parse_index(trimmed, line_number)?);
            *cursor += 1;
            continue;
        }

        let field = parse_field(trimmed, line_number)?;
        if !field_names.insert(field.name.clone()) {
            return Err(parse_error(
                line_number,
                1,
                field.name.len(),
                format!("duplicate field `{}`", field.name),
            ));
        }
        fields.push(field);
        *cursor += 1;
    }

    Err(parse_error(
        line_number,
        1,
        trimmed.len(),
        "unclosed @model block",
    ))
}

fn parse_field(line: &str, line_number: usize) -> Result<ModelField, Diagnostic> {
    let (left, right) = line.split_once(':').ok_or_else(|| {
        parse_error(
            line_number,
            1,
            line.len().max(1),
            "model fields expect `name: type`",
        )
    })?;
    let name = left.trim();
    if !is_identifier(name) {
        return Err(parse_error(
            line_number,
            1,
            name.len().max(1),
            format!("invalid field name `{name}`"),
        ));
    }

    let mut parts = right.split_whitespace();
    let type_name = parts.next().ok_or_else(|| {
        parse_error(
            line_number,
            line.find(':').unwrap_or(0) + 2,
            line.len().max(1),
            "model fields require a type",
        )
    })?;
    if !is_supported_type(type_name) {
        return Err(parse_error(
            line_number,
            line.find(type_name).unwrap_or(0) + 1,
            line.find(type_name).unwrap_or(0) + type_name.len() + 1,
            format!("unsupported model type `{type_name}`"),
        ));
    }

    let type_start = right.find(type_name).unwrap_or(0);
    let decorators = right[type_start + type_name.len()..].trim();
    let mut field = ModelField {
        name: name.to_string(),
        type_name: type_name.to_string(),
        primary: false,
        auto: false,
        unique: false,
        nullable: false,
        default: None,
        reference: None,
        relation: None,
    };

    for decorator in parse_decorators(decorators, line_number)? {
        match decorator.name.as_str() {
            "primary" => require_no_args(&decorator, line_number)?,
            "auto" => require_no_args(&decorator, line_number)?,
            "unique" => require_no_args(&decorator, line_number)?,
            "nullable" => require_no_args(&decorator, line_number)?,
            "default" => {
                field.default = Some(require_args(&decorator, line_number)?.to_string());
                continue;
            }
            "references" => {
                let args = require_args(&decorator, line_number)?;
                let (model, field_name) = args.split_once('.').ok_or_else(|| {
                    parse_error(
                        line_number,
                        1,
                        args.len().max(1),
                        "@references expects `Model.field`",
                    )
                })?;
                if !is_model_name(model) || !is_identifier(field_name) {
                    return Err(parse_error(
                        line_number,
                        1,
                        args.len().max(1),
                        "@references expects `Model.field`",
                    ));
                }
                field.reference = Some(ModelReference {
                    model: model.to_string(),
                    field: field_name.to_string(),
                });
                continue;
            }
            "relation" => {
                let relation = require_args(&decorator, line_number)?;
                if !is_identifier(relation) {
                    return Err(parse_error(
                        line_number,
                        1,
                        relation.len().max(1),
                        format!("invalid relation name `{relation}`"),
                    ));
                }
                field.relation = Some(relation.to_string());
                continue;
            }
            other => {
                return Err(parse_error(
                    line_number,
                    1,
                    other.len() + 1,
                    format!("unknown field decorator `@{other}`"),
                ))
            }
        }

        match decorator.name.as_str() {
            "primary" => field.primary = true,
            "auto" => field.auto = true,
            "unique" => field.unique = true,
            "nullable" => field.nullable = true,
            _ => {}
        }
    }

    if field.auto && (!field.primary || field.type_name != "int") {
        return Err(parse_error(
            line_number,
            1,
            line.len().max(1),
            "@auto requires an int @primary field",
        ));
    }

    Ok(field)
}

fn parse_index(line: &str, line_number: usize) -> Result<ModelIndex, Diagnostic> {
    let (name, unique) = if line.starts_with("@uniqueIndex") {
        ("@uniqueIndex", true)
    } else {
        ("@index", false)
    };
    let args = line
        .strip_prefix(name)
        .and_then(|rest| rest.trim().strip_prefix('('))
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or_else(|| parse_error(line_number, 1, line.len(), format!("{name} expects fields")))?;
    let fields: Vec<String> = args
        .split(',')
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .map(str::to_string)
        .collect();
    if fields.is_empty() || fields.iter().any(|field| !is_identifier(field)) {
        return Err(parse_error(
            line_number,
            1,
            line.len(),
            format!("{name} expects one or more field names"),
        ));
    }
    Ok(ModelIndex { fields, unique })
}

fn parse_decorators(source: &str, line_number: usize) -> Result<Vec<Decorator>, Diagnostic> {
    let mut decorators = Vec::new();
    let mut cursor = 0;
    let chars: Vec<char> = source.chars().collect();

    while cursor < chars.len() {
        while cursor < chars.len() && chars[cursor].is_whitespace() {
            cursor += 1;
        }
        if cursor >= chars.len() {
            break;
        }
        if chars[cursor] != '@' {
            return Err(parse_error(
                line_number,
                cursor + 1,
                source.len().max(1),
                "expected field decorator",
            ));
        }
        cursor += 1;
        let start = cursor;
        while cursor < chars.len()
            && (chars[cursor].is_ascii_alphanumeric() || chars[cursor] == '_')
        {
            cursor += 1;
        }
        if start == cursor {
            return Err(parse_error(
                line_number,
                cursor,
                cursor + 1,
                "expected decorator name",
            ));
        }
        let name: String = chars[start..cursor].iter().collect();
        let args = if cursor < chars.len() && chars[cursor] == '(' {
            cursor += 1;
            let args_start = cursor;
            let mut depth = 1;
            while cursor < chars.len() && depth > 0 {
                match chars[cursor] {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                cursor += 1;
            }
            if depth != 0 {
                return Err(parse_error(
                    line_number,
                    args_start,
                    source.len().max(1),
                    format!("unclosed decorator `@{name}`"),
                ));
            }
            Some(chars[args_start..cursor - 1].iter().collect())
        } else {
            None
        };

        decorators.push(Decorator { name, args });
    }

    Ok(decorators)
}

fn validate_models(models: &[ModelDecl]) -> Result<(), Diagnostic> {
    let mut model_names = BTreeSet::new();
    let mut fields_by_model = BTreeMap::new();
    for model in models {
        if !model_names.insert(model.name.clone()) {
            return Err(parse_error(
                1,
                1,
                model.name.len().max(1),
                format!("duplicate model `{}`", model.name),
            ));
        }
        let fields: BTreeSet<String> = model
            .fields
            .iter()
            .map(|field| field.name.clone())
            .collect();
        fields_by_model.insert(model.name.clone(), fields);

        let primary_count = model.fields.iter().filter(|field| field.primary).count();
        if primary_count > 1 {
            return Err(parse_error(
                1,
                1,
                model.name.len().max(1),
                format!("model `{}` has multiple primary fields", model.name),
            ));
        }

        for index in &model.indexes {
            for field in &index.fields {
                if !model
                    .fields
                    .iter()
                    .any(|candidate| candidate.name == *field)
                {
                    return Err(parse_error(
                        1,
                        1,
                        field.len().max(1),
                        format!("index references unknown field `{field}`"),
                    ));
                }
            }
        }
    }

    for model in models {
        for field in &model.fields {
            let Some(reference) = &field.reference else {
                continue;
            };
            let Some(fields) = fields_by_model.get(&reference.model) else {
                return Err(parse_error(
                    1,
                    1,
                    reference.model.len().max(1),
                    format!(
                        "field `{}.{}` references unknown model `{}`",
                        model.name, field.name, reference.model
                    ),
                ));
            };
            if !fields.contains(&reference.field) {
                return Err(parse_error(
                    1,
                    1,
                    reference.field.len().max(1),
                    format!(
                        "field `{}.{}` references unknown field `{}.{}`",
                        model.name, field.name, reference.model, reference.field
                    ),
                ));
            }
        }
    }

    Ok(())
}

fn column_sql(field: &ModelField) -> String {
    let mut parts = vec![
        quote_ident(&field.name),
        sqlite_type(&field.type_name).to_string(),
    ];
    if field.primary {
        parts.push("PRIMARY KEY".to_string());
    }
    if field.auto {
        parts.push("AUTOINCREMENT".to_string());
    }
    if field.unique && !field.primary {
        parts.push("UNIQUE".to_string());
    }
    if !field.nullable && !field.primary {
        parts.push("NOT NULL".to_string());
    }
    if let Some(default) = &field.default {
        parts.push(format!("DEFAULT {}", default_sql(default)));
    }
    if let Some(reference) = &field.reference {
        parts.push(format!(
            "REFERENCES {}({})",
            quote_ident(&reference.model),
            quote_ident(&reference.field)
        ));
    }
    parts.join(" ")
}

fn sqlite_type(type_name: &str) -> &'static str {
    match type_name {
        "string" | "date" | "datetime" => "TEXT",
        "int" | "bool" => "INTEGER",
        "float" => "REAL",
        "bytes" => "BLOB",
        _ => "TEXT",
    }
}

fn default_sql(value: &str) -> String {
    match value {
        "now" => "CURRENT_TIMESTAMP".to_string(),
        "true" => "1".to_string(),
        "false" => "0".to_string(),
        value if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 => {
            format!("'{}'", value[1..value.len() - 1].replace('\'', "''"))
        }
        value => value.to_string(),
    }
}

fn collect_model_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_model_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "web") {
            files.push(path);
        }
    }
    Ok(())
}

fn migration_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let migrations_dir = root.join(MIGRATIONS_PATH);
    if !migrations_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(migrations_dir).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().is_some_and(|extension| extension == "sql") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn applied_migrations(connection: &Connection) -> Result<BTreeMap<String, String>, String> {
    let mut statement = connection
        .prepare("SELECT name, checksum FROM _webscript_migrations")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| error.to_string())?;

    let mut migrations = BTreeMap::new();
    for row in rows {
        let (name, checksum) = row.map_err(|error| error.to_string())?;
        migrations.insert(name, checksum);
    }
    Ok(migrations)
}

fn require_no_args(decorator: &Decorator, line_number: usize) -> Result<(), Diagnostic> {
    if decorator.args.is_some() {
        return Err(parse_error(
            line_number,
            1,
            decorator.name.len() + 1,
            format!("@{} does not accept arguments", decorator.name),
        ));
    }
    Ok(())
}

fn require_args<'a>(decorator: &'a Decorator, line_number: usize) -> Result<&'a str, Diagnostic> {
    decorator.args.as_deref().ok_or_else(|| {
        parse_error(
            line_number,
            1,
            decorator.name.len() + 1,
            format!("@{} requires arguments", decorator.name),
        )
    })
}

fn is_model_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|ch| ch.is_ascii_uppercase())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn is_supported_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "string" | "int" | "float" | "bool" | "date" | "datetime" | "bytes"
    )
}

pub fn quote_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn parse_error(
    line: usize,
    start_col: usize,
    end_col: usize,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(Span::new(line, start_col, end_col), message, None)
}

fn normalize_newlines(value: &str) -> String {
    value.replace("\r\n", "\n")
}

fn sanitize_migration_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches('_').to_string();
    if sanitized.is_empty() {
        "schema".to_string()
    } else {
        sanitized
    }
}

fn timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = day_seconds / 3_600;
    let minute = (day_seconds % 3_600) / 60;
    let second = day_seconds % 60;
    format!("{year:04}{month:02}{day:02}{hour:02}{minute:02}{second:02}")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year, m, d)
}

fn checksum(source: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in source.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub fn open_database(root: &Path) -> Result<Connection, String> {
    let database_path = root.join(SQLITE_PATH);
    if !database_path.exists() {
        return Err(format!(
            "database not found at `{}`; run `web db:migrate` first",
            database_path.display()
        ));
    }
    Connection::open(&database_path).map_err(|error| error.to_string())
}

pub fn coerce_int(value: &Value) -> Result<i64, String> {
    match value {
        Value::Int(value) => Ok(*value),
        Value::String(text) => text
            .parse()
            .map_err(|_| format!("expected int, found string `{text}`")),
        other => Err(format!("expected int, found `{}`", other.type_name())),
    }
}

pub fn coerce_bool(value: &Value) -> Result<bool, String> {
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

pub fn coerce_float(value: &Value) -> Result<f64, String> {
    match value {
        Value::Int(value) => Ok(*value as f64),
        Value::String(text) => text
            .parse()
            .map_err(|_| format!("expected float, found string `{text}`")),
        other => Err(format!("expected float, found `{}`", other.type_name())),
    }
}

pub fn value_to_sql(value: &Value) -> Result<rusqlite::types::Value, String> {
    match value {
        Value::Int(value) => Ok(rusqlite::types::Value::Integer(*value)),
        Value::Bool(value) => Ok(rusqlite::types::Value::Integer(if *value { 1 } else { 0 })),
        Value::String(text) => Ok(rusqlite::types::Value::Text(text.clone())),
        other => Ok(rusqlite::types::Value::Text(other.render())),
    }
}

fn io_error(error: std::io::Error) -> ModelLoadError {
    ModelLoadError::Io(error.to_string())
}

#[derive(Debug)]
pub enum ModelLoadError {
    Diagnostic(FileDiagnostic),
    Io(String),
}

#[derive(Debug)]
struct Decorator {
    name: String,
    args: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_model_fields_indexes_and_references() {
        let models = parse_models(
            "@model User {
  id: int @primary @auto
  email: string @unique
  createdAt: datetime @default(now)
  @index(email)
}

@model Post {
  id: int @primary @auto
  authorId: int @references(User.id) @relation(author)
  published: bool @default(false)
  @index(authorId, published)
}",
        )
        .expect("valid models");

        assert_eq!(models.len(), 2);
        assert!(models[0].fields[0].primary);
        assert!(models[0].fields[0].auto);
        assert_eq!(models[0].indexes[0].fields, vec!["email"]);
        assert_eq!(
            models[1].fields[1].reference,
            Some(ModelReference {
                model: "User".to_string(),
                field: "id".to_string()
            })
        );
    }

    #[test]
    fn rejects_duplicate_fields() {
        let error =
            parse_models("@model User {\n  id: int\n  id: string\n}").expect_err("duplicate field");
        assert_eq!(error.message, "duplicate field `id`");
    }

    #[test]
    fn rejects_unknown_model_types() {
        let error = parse_models("@model User {\n  id: uuid\n}").expect_err("unknown model type");
        assert_eq!(error.message, "unsupported model type `uuid`");
    }

    #[test]
    fn rejects_malformed_indexes() {
        let error =
            parse_models("@model User {\n  id: int\n  @index()\n}").expect_err("malformed index");
        assert_eq!(error.message, "@index expects one or more field names");
    }

    #[test]
    fn validates_bad_references() {
        let models =
            parse_models("@model Post {\n  authorId: int @references(User.id)\n}").expect("parse");
        let error = validate_models(&models).expect_err("bad reference");
        assert_eq!(
            error.message,
            "field `Post.authorId` references unknown model `User`"
        );
    }

    #[test]
    fn generates_sql_for_sqlite_schema() {
        let models = parse_models(
            "@model User {
  id: int @primary @auto
  email: string @unique
}
@model Post {
  id: int @primary @auto
  authorId: int @references(User.id)
  published: bool @default(false)
  @uniqueIndex(authorId, published)
}",
        )
        .expect("parse");
        validate_models(&models).expect("valid references");

        let models = parse_models(
            "@model User {
  id: int @primary @auto
  email: string @unique
}
@model Post {
  id: int @primary @auto
  authorId: int @references(User.id)
  published: bool @default(false)
  @uniqueIndex(authorId, published)
}",
        )
        .expect("parse");
        let sql = generate_sql(&models);

        assert!(sql.contains("\"id\" INTEGER PRIMARY KEY AUTOINCREMENT"));
        assert!(sql.contains("\"email\" TEXT UNIQUE NOT NULL"));
        assert!(sql.contains("REFERENCES \"User\"(\"id\")"));
        assert!(sql.contains("CREATE UNIQUE INDEX IF NOT EXISTS"));
    }

    #[test]
    fn migrate_applies_pending_files_and_detects_checksum_changes() {
        let root = temp_root("webscript-db-migrate");
        let migrations = root.join(MIGRATIONS_PATH);
        fs::create_dir_all(&migrations).expect("create migrations");
        fs::write(
            migrations.join("20260101000000_create_users.sql"),
            "CREATE TABLE users (id INTEGER PRIMARY KEY);\n",
        )
        .expect("write migration");

        let first = migrate(&root).expect("first migration");
        assert_eq!(first.applied, vec!["20260101000000_create_users.sql"]);
        let second = migrate(&root).expect("second migration");
        assert!(second.applied.is_empty());

        fs::write(
            migrations.join("20260101000000_create_users.sql"),
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);\n",
        )
        .expect("rewrite migration");
        let error = migrate(&root).expect_err("checksum mismatch");
        assert!(error.contains("has changed checksum"));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn sanitizes_migration_names_without_overriding_valid_names() {
        assert_eq!(sanitize_migration_name("init"), "init");
        assert_eq!(sanitize_migration_name("Create Posts!"), "create_posts");
        assert_eq!(sanitize_migration_name("!!!"), "schema");
    }

    fn temp_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{}-{nanos}", std::process::id()))
    }
}
