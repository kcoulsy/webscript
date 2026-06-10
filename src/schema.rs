use crate::db::coerce_bool;
use crate::db::coerce_float;
use crate::db::coerce_int;
use crate::diagnostic::{Diagnostic, FileDiagnostic, Span};
use crate::parser::Value;
use serde_json::Number;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDecl {
    pub name: String,
    pub fields: Vec<SchemaField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaField {
    pub name: String,
    pub type_name: String,
    pub optional: bool,
    pub rules: Vec<SchemaRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaRule {
    Min(i64),
    Max(i64),
    Email,
}

pub fn check_schemas(root: &Path) -> Result<Vec<FileDiagnostic>, String> {
    let mut diagnostics = Vec::new();
    match discover_schemas(root) {
        Ok(_) => {}
        Err(SchemaLoadError::Diagnostic(diagnostic)) => diagnostics.push(diagnostic),
        Err(SchemaLoadError::Io(error)) => return Err(error),
    }
    Ok(diagnostics)
}

pub fn discover_schemas(root: &Path) -> Result<Vec<SchemaDecl>, SchemaLoadError> {
    let mut files = Vec::new();
    collect_schema_files(&root.join("app").join("schemas"), &mut files).map_err(io_error)?;
    files.sort();

    let mut schemas = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).map_err(io_error)?;
        match parse_schemas(&source) {
            Ok(mut parsed) => schemas.append(&mut parsed),
            Err(diagnostic) => {
                return Err(SchemaLoadError::Diagnostic(FileDiagnostic::new(
                    file, source, diagnostic,
                )))
            }
        }
    }

    validate_schemas(&schemas).map_err(|diagnostic| {
        let file = root.join("app").join("schemas");
        SchemaLoadError::Diagnostic(FileDiagnostic::new(file, String::new(), diagnostic))
    })?;

    if let Ok(models) = crate::db::discover_models(root) {
        validate_no_name_collisions(&schemas, &models).map_err(|diagnostic| {
            SchemaLoadError::Diagnostic(FileDiagnostic::new(
                root.join("app"),
                String::new(),
                diagnostic,
            ))
        })?;
    }

    schemas.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(schemas)
}

pub fn validate_no_name_collisions(
    schemas: &[SchemaDecl],
    models: &[crate::db::ModelDecl],
) -> Result<(), Diagnostic> {
    let model_names: BTreeSet<&str> = models.iter().map(|model| model.name.as_str()).collect();
    for schema in schemas {
        if model_names.contains(schema.name.as_str()) {
            return Err(parse_error(
                1,
                1,
                schema.name.len().max(1),
                format!(
                    "schema `{}` conflicts with an existing model of the same name",
                    schema.name
                ),
            ));
        }
    }
    Ok(())
}

pub fn parse_schemas(source: &str) -> Result<Vec<SchemaDecl>, Diagnostic> {
    let lines: Vec<&str> = source.lines().collect();
    let mut cursor = 0;
    let mut schemas = Vec::new();

    while cursor < lines.len() {
        let trimmed = lines[cursor].trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            cursor += 1;
            continue;
        }
        if !trimmed.starts_with("@schema") {
            let line_number = cursor + 1;
            return Err(parse_error(
                line_number,
                1,
                trimmed.len().max(1),
                format!("unexpected directive `{trimmed}`"),
            ));
        }

        schemas.push(parse_schema(&lines, &mut cursor)?);
    }

    if schemas.is_empty() {
        return Err(parse_error(1, 1, 1, "missing @schema directive"));
    }
    Ok(schemas)
}

pub fn validate_value(schema: &SchemaDecl, value: &Value) -> Result<Value, String> {
    let Value::Object(fields) = value else {
        return Err(format!(
            "expected object for schema `{}`, found `{}`",
            schema.name,
            value.type_name()
        ));
    };

    let mut output = BTreeMap::new();
    for field in &schema.fields {
        let raw = fields.get(&field.name);
        if raw.is_none() {
            if field.optional {
                continue;
            }
            return Err(format!("{}.{}: required field is missing", schema.name, field.name));
        }
        let raw = raw.unwrap();
        if is_nullish(raw) {
            if field.optional {
                continue;
            }
            return Err(format!("{}.{}: required field is null", schema.name, field.name));
        }
        let coerced = coerce_field(field, raw)?;
        apply_rules(field, &coerced)?;
        output.insert(field.name.clone(), coerced);
    }

    Ok(Value::Object(output))
}

pub fn parser_value_to_json(value: &Value) -> Result<serde_json::Value, String> {
    match value {
        Value::String(text) => Ok(serde_json::Value::String(text.clone())),
        Value::Int(number) => Ok(serde_json::Value::Number(
            serde_json::Number::from(*number),
        )),
        Value::Bool(flag) => Ok(serde_json::Value::Bool(*flag)),
        Value::Object(fields) => {
            let mut output = serde_json::Map::new();
            for (name, field) in fields {
                output.insert(name.clone(), parser_value_to_json(field)?);
            }
            Ok(serde_json::Value::Object(output))
        }
        Value::Array { values, .. } => {
            let mut output = Vec::with_capacity(values.len());
            for item in values {
                output.push(parser_value_to_json(item)?);
            }
            Ok(serde_json::Value::Array(output))
        }
        Value::Duration { .. } => Err("cannot serialize duration to JSON".to_string()),
        Value::Function { .. } => Err("cannot serialize function to JSON".to_string()),
        Value::Promise { .. } => Err("cannot serialize promise to JSON".to_string()),
    }
}

pub fn json_value_to_parser(value: serde_json::Value) -> Result<Value, String> {
    match value {
        serde_json::Value::Null => Ok(Value::String(String::new())),
        serde_json::Value::Bool(value) => Ok(Value::Bool(value)),
        serde_json::Value::Number(number) => json_number_to_parser(&number),
        serde_json::Value::String(text) => Ok(Value::String(text)),
        serde_json::Value::Array(_) => Err("expected JSON object, found array".to_string()),
        serde_json::Value::Object(fields) => {
            let mut output = BTreeMap::new();
            for (name, value) in fields {
                output.insert(name, json_value_to_parser(value)?);
            }
            Ok(Value::Object(output))
        }
    }
}

fn json_number_to_parser(number: &Number) -> Result<Value, String> {
    if let Some(value) = number.as_i64() {
        return Ok(Value::Int(value));
    }
    if let Some(value) = number.as_u64() {
        if value <= i64::MAX as u64 {
            return Ok(Value::Int(value as i64));
        }
    }
    if let Some(value) = number.as_f64() {
        if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
            return Ok(Value::Int(value as i64));
        }
        return Ok(Value::Int(value.round() as i64));
    }
    Err("invalid JSON number".to_string())
}

fn parse_schema(lines: &[&str], cursor: &mut usize) -> Result<SchemaDecl, Diagnostic> {
    let line_number = *cursor + 1;
    let trimmed = lines[*cursor].trim();
    let header = trimmed
        .strip_prefix("@schema")
        .expect("@schema prefix already checked")
        .trim();
    let name = header
        .strip_suffix('{')
        .ok_or_else(|| {
            parse_error(
                line_number,
                1,
                trimmed.len(),
                "@schema expects `@schema Name {`",
            )
        })?
        .trim();

    if !is_schema_name(name) {
        return Err(parse_error(
            line_number,
            1,
            trimmed.len(),
            format!("invalid schema name `{name}`"),
        ));
    }

    *cursor += 1;
    let mut fields = Vec::new();
    let mut field_names = BTreeSet::new();

    while *cursor < lines.len() {
        let line_number = *cursor + 1;
        let trimmed = lines[*cursor].trim();
        if trimmed == "}" {
            *cursor += 1;
            if fields.is_empty() {
                return Err(parse_error(
                    line_number,
                    1,
                    1,
                    format!("schema `{name}` must declare at least one field"),
                ));
            }
            return Ok(SchemaDecl {
                name: name.to_string(),
                fields,
            });
        }
        if trimmed.is_empty() || trimmed.starts_with("//") {
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
        "unclosed @schema block",
    ))
}

fn parse_field(line: &str, line_number: usize) -> Result<SchemaField, Diagnostic> {
    let (left, right) = line.split_once(':').ok_or_else(|| {
        parse_error(
            line_number,
            1,
            line.len().max(1),
            "schema fields expect `name: type`",
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
            "schema fields require a type",
        )
    })?;
    if !is_supported_schema_type(type_name) {
        return Err(parse_error(
            line_number,
            line.find(type_name).unwrap_or(0) + 1,
            line.find(type_name).unwrap_or(0) + type_name.len() + 1,
            format!("unsupported schema type `{type_name}`"),
        ));
    }

    let type_start = right.find(type_name).unwrap_or(0);
    let decorators = right[type_start + type_name.len()..].trim();
    let mut field = SchemaField {
        name: name.to_string(),
        type_name: type_name.to_string(),
        optional: false,
        rules: Vec::new(),
    };

    for decorator in parse_decorators(decorators, line_number)? {
        match decorator.name.as_str() {
            "optional" => {
                require_no_args(&decorator, line_number)?;
                field.optional = true;
            }
            "min" => {
                let args = require_args(&decorator, line_number)?;
                field.rules.push(SchemaRule::Min(parse_numeric_arg(
                    args, line_number, "min",
                )?));
            }
            "max" => {
                let args = require_args(&decorator, line_number)?;
                field.rules.push(SchemaRule::Max(parse_numeric_arg(
                    args, line_number, "max",
                )?));
            }
            "email" => {
                require_no_args(&decorator, line_number)?;
                field.rules.push(SchemaRule::Email);
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
    }

    Ok(field)
}

fn validate_schemas(schemas: &[SchemaDecl]) -> Result<(), Diagnostic> {
    let mut schema_names = BTreeSet::new();
    for schema in schemas {
        if !schema_names.insert(schema.name.clone()) {
            return Err(parse_error(
                1,
                1,
                schema.name.len().max(1),
                format!("duplicate schema `{}`", schema.name),
            ));
        }
    }
    Ok(())
}

fn coerce_field(field: &SchemaField, value: &Value) -> Result<Value, String> {
    match field.type_name.as_str() {
        "string" => match value {
            Value::String(text) => Ok(Value::String(text.clone())),
            Value::Int(value) => Ok(Value::String(value.to_string())),
            Value::Bool(value) => Ok(Value::String(value.to_string())),
            other => Err(format!(
                "{}.{}: expected string, found `{}`",
                "schema",
                field.name,
                other.type_name()
            )),
        },
        "int" => coerce_int(value).map(Value::Int).map_err(|error| {
            format!("{}.{}: {}", "schema", field.name, error)
        }),
        "float" => coerce_float(value)
            .map(|value| Value::Int(value.round() as i64))
            .map_err(|error| format!("{}.{}: {}", "schema", field.name, error)),
        "bool" => coerce_bool(value).map(Value::Bool).map_err(|error| {
            format!("{}.{}: {}", "schema", field.name, error)
        }),
        other => Err(format!(
            "{}.{}: unsupported schema type `{other}`",
            "schema", field.name
        )),
    }
}

fn apply_rules(field: &SchemaField, value: &Value) -> Result<(), String> {
    for rule in &field.rules {
        match rule {
            SchemaRule::Min(min) => apply_min_rule(field, value, *min)?,
            SchemaRule::Max(max) => apply_max_rule(field, value, *max)?,
            SchemaRule::Email => apply_email_rule(field, value)?,
        }
    }
    Ok(())
}

fn apply_min_rule(field: &SchemaField, value: &Value, min: i64) -> Result<(), String> {
    match field.type_name.as_str() {
        "string" => {
            let Value::String(text) = value else {
                unreachable!();
            };
            if (text.len() as i64) < min {
                return Err(format!(
                    "{}.{}: string length must be at least {min}",
                    "schema", field.name
                ));
            }
        }
        "int" | "float" => {
            let number = numeric_value(value)?;
            if number < min {
                return Err(format!(
                    "{}.{}: value must be at least {min}",
                    "schema", field.name
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn apply_max_rule(field: &SchemaField, value: &Value, max: i64) -> Result<(), String> {
    match field.type_name.as_str() {
        "string" => {
            let Value::String(text) = value else {
                unreachable!();
            };
            if (text.len() as i64) > max {
                return Err(format!(
                    "{}.{}: string length must be at most {max}",
                    "schema", field.name
                ));
            }
        }
        "int" | "float" => {
            let number = numeric_value(value)?;
            if number > max {
                return Err(format!(
                    "{}.{}: value must be at most {max}",
                    "schema", field.name
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn apply_email_rule(field: &SchemaField, value: &Value) -> Result<(), String> {
    let Value::String(text) = value else {
        unreachable!();
    };
    if !is_valid_email(text) {
        return Err(format!("{}.{}: invalid email", "schema", field.name));
    }
    Ok(())
}

fn numeric_value(value: &Value) -> Result<i64, String> {
    match value {
        Value::Int(value) => Ok(*value),
        other => Err(format!("expected numeric value, found `{}`", other.type_name())),
    }
}

fn is_valid_email(text: &str) -> bool {
    let Some((local, domain)) = text.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !local.contains(' ')
        && !domain.contains(' ')
}

fn is_nullish(value: &Value) -> bool {
    matches!(value, Value::String(text) if text.is_empty())
}

fn parse_numeric_arg(args: &str, line_number: usize, name: &str) -> Result<i64, Diagnostic> {
    args.trim()
        .parse::<i64>()
        .map_err(|_| parse_error(line_number, 1, args.len().max(1), format!("@{name} expects a number")))
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

pub fn is_schema_name(name: &str) -> bool {
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

fn is_supported_schema_type(type_name: &str) -> bool {
    matches!(type_name, "string" | "int" | "float" | "bool")
}

fn collect_schema_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_schema_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "web") {
            files.push(path);
        }
    }
    Ok(())
}

fn parse_error(
    line: usize,
    start_col: usize,
    end_col: usize,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(Span::new(line, start_col, end_col), message, None)
}

fn io_error(error: std::io::Error) -> SchemaLoadError {
    SchemaLoadError::Io(error.to_string())
}

#[derive(Debug)]
pub enum SchemaLoadError {
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

    #[test]
    fn parses_schema_fields_and_decorators() {
        let schemas = parse_schemas(
            "@schema ApiResponse {
  message: string @min(5) @max(255)
  email: string @email
  nickname: string @optional
}
",
        )
        .expect("parse");
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0].name, "ApiResponse");
        assert_eq!(schemas[0].fields.len(), 3);
        assert!(schemas[0].fields[2].optional);
        assert_eq!(schemas[0].fields[0].rules.len(), 2);
    }

    #[test]
    fn rejects_duplicate_fields() {
        parse_schemas("@schema User {\n  id: int\n  id: int\n}").expect_err("duplicate");
    }

    #[test]
    fn rejects_unknown_type() {
        parse_schemas("@schema User {\n  id: uuid\n}").expect_err("unknown type");
    }

    #[test]
    fn parser_value_round_trips_through_json() {
        let value = Value::Object(BTreeMap::from([
            ("title".to_string(), Value::String("Ship".to_string())),
            ("done".to_string(), Value::Bool(false)),
        ]));
        let json = parser_value_to_json(&value).expect("json");
        let parsed = json_value_to_parser(json).expect("parser");
        assert_eq!(value, parsed);
    }

    #[test]
    fn validates_email_min_max_and_optional() {
        let schema = parse_schemas(
            "@schema User {
  email: string @email
  age: int @min(18) @max(120)
  nickname: string @optional
}",
        )
        .expect("parse")
        .remove(0);

        let valid = validate_value(
            &schema,
            &Value::Object(BTreeMap::from([
                ("email".to_string(), Value::String("a@b.co".to_string())),
                ("age".to_string(), Value::Int(30)),
            ])),
        )
        .expect("valid");
        let Value::Object(fields) = valid else {
            panic!("expected object");
        };
        assert!(!fields.contains_key("nickname"));

        let error = validate_value(
            &schema,
            &Value::Object(BTreeMap::from([
                ("email".to_string(), Value::String("bad".to_string())),
                ("age".to_string(), Value::Int(30)),
            ])),
        )
        .expect_err("invalid email");
        assert!(error.contains("invalid email"));

        let error = validate_value(
            &schema,
            &Value::Object(BTreeMap::from([(
                "email".to_string(),
                Value::String("a@b.co".to_string()),
            )])),
        )
        .expect_err("missing age");
        assert!(error.contains("required"));
    }
}
