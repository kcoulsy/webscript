use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct WebFile {
    pub route: String,
    pub lets: BTreeMap<String, Value>,
    pub template: String,
}

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Int(i64),
    Bool(bool),
}

impl Value {
    pub fn render(&self) -> String {
        match self {
            Value::String(value) => value.clone(),
            Value::Int(value) => value.to_string(),
            Value::Bool(value) => value.to_string(),
        }
    }
}

pub fn parse(source: &str) -> Result<WebFile, String> {
    let mut route = None;
    let mut lets = BTreeMap::new();
    let mut template_lines = Vec::new();
    let mut in_template = false;

    for (line_index, raw_line) in source.lines().enumerate() {
        let line_number = line_index + 1;
        let trimmed = raw_line.trim();

        if trimmed.is_empty() && !in_template {
            continue;
        }

        if !in_template && trimmed.starts_with("@page") {
            route = Some(parse_page(trimmed, line_number)?);
            continue;
        }

        if !in_template && trimmed.starts_with("@let") {
            let (name, value) = parse_let(trimmed, line_number)?;
            lets.insert(name, value);
            continue;
        }

        in_template = true;
        template_lines.push(raw_line);
    }

    let route = route.ok_or_else(|| "missing @page directive".to_string())?;
    let template = template_lines.join("\n").trim().to_string();

    Ok(WebFile {
        route,
        lets,
        template,
    })
}

fn parse_page(line: &str, line_number: usize) -> Result<String, String> {
    let rest = line
        .strip_prefix("@page")
        .expect("@page prefix already checked")
        .trim();

    parse_quoted(rest).ok_or_else(|| {
        format!("line {line_number}: @page expects a quoted route, for example @page \"/\"")
    })
}

fn parse_let(line: &str, line_number: usize) -> Result<(String, Value), String> {
    let rest = line
        .strip_prefix("@let")
        .expect("@let prefix already checked")
        .trim();
    let (left, right) = rest
        .split_once('=')
        .ok_or_else(|| format!("line {line_number}: @let expects `name: type = value`"))?;
    let (name, type_name) = left
        .split_once(':')
        .ok_or_else(|| format!("line {line_number}: @let expects an explicit type"))?;

    let name = name.trim();
    if !is_identifier(name) {
        return Err(format!("line {line_number}: invalid identifier `{name}`"));
    }

    let value = parse_value(type_name.trim(), right.trim(), line_number)?;
    Ok((name.to_string(), value))
}

fn parse_value(type_name: &str, value: &str, line_number: usize) -> Result<Value, String> {
    match type_name {
        "string" => parse_quoted(value)
            .map(Value::String)
            .ok_or_else(|| format!("line {line_number}: string values must be quoted")),
        "int" => value
            .parse::<i64>()
            .map(Value::Int)
            .map_err(|_| format!("line {line_number}: invalid int literal `{value}`")),
        "bool" => match value {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(format!(
                "line {line_number}: invalid bool literal `{value}`"
            )),
        },
        other => Err(format!(
            "line {line_number}: unsupported MVP type `{other}`"
        )),
    }
}

fn parse_quoted(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        Some(value[1..value.len() - 1].to_string())
    } else {
        None
    }
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }

    chars.all(|char| char == '_' || char.is_ascii_alphanumeric())
}

pub fn expressions(template: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = template;

    while let Some(start) = rest.find('{') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('}') else {
            break;
        };
        let expr = rest[..end].trim();
        if !expr.is_empty() {
            found.push(expr.to_string());
        }
        rest = &rest[end + 1..];
    }

    found
}

#[cfg(test)]
mod tests {
    use super::{parse, Value};

    #[test]
    fn parses_page_lets_and_template() {
        let parsed = parse(
            "@page \"/\"\n\n@let name: string = \"Ada\"\n@let visits: int = 3\n\n<h1>{name}</h1>",
        )
        .expect("valid page");

        assert_eq!(parsed.route, "/");
        assert!(matches!(parsed.lets.get("name"), Some(Value::String(value)) if value == "Ada"));
        assert!(matches!(parsed.lets.get("visits"), Some(Value::Int(3))));
        assert_eq!(parsed.template, "<h1>{name}</h1>");
    }

    #[test]
    fn rejects_missing_page() {
        let error = parse("<h1>No route</h1>").expect_err("missing page should fail");
        assert_eq!(error, "missing @page directive");
    }
}
