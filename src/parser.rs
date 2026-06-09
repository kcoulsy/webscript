use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct WebFile {
    pub route: RoutePattern,
    pub lets: BTreeMap<String, Value>,
    pub template: Vec<TemplateNode>,
}

#[derive(Debug, Clone)]
pub struct RoutePattern {
    pub raw: String,
    pub params: Vec<RouteParam>,
}

#[derive(Debug, Clone)]
pub struct RouteParam {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateNode {
    Text(String),
    Expr(SourceExpr),
    If {
        condition: SourceExpr,
        then_nodes: Vec<TemplateNode>,
        else_nodes: Vec<TemplateNode>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceExpr {
    pub name: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(*value),
            _ => None,
        }
    }
}

pub fn parse(source: &str) -> Result<WebFile, String> {
    let mut route = None;
    let mut lets = BTreeMap::new();
    let mut template_start = None;
    let lines: Vec<&str> = source.lines().collect();

    for (line_index, raw_line) in lines.iter().enumerate() {
        let line_number = line_index + 1;
        let trimmed = raw_line.trim();

        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("@page") {
            route = Some(parse_page(trimmed, line_number)?);
            continue;
        }

        if trimmed.starts_with("@let") {
            let (name, value) = parse_let(trimmed, line_number)?;
            lets.insert(name, value);
            continue;
        }

        template_start = Some(line_index);
        break;
    }

    let route = route.ok_or_else(|| "line 1: missing @page directive".to_string())?;
    let template = match template_start {
        Some(start) => {
            let mut cursor = start;
            parse_nodes(&lines, &mut cursor, false)?
        }
        None => Vec::new(),
    };

    Ok(WebFile {
        route,
        lets,
        template,
    })
}

fn parse_nodes(
    lines: &[&str],
    cursor: &mut usize,
    stop_on_close: bool,
) -> Result<Vec<TemplateNode>, String> {
    let mut nodes = Vec::new();

    while *cursor < lines.len() {
        let raw_line = lines[*cursor];
        let line_number = *cursor + 1;
        let trimmed = raw_line.trim();

        if stop_on_close && (trimmed == "}" || trimmed.starts_with("} @else")) {
            break;
        }

        if trimmed.starts_with("@if ") {
            nodes.push(parse_if(lines, cursor)?);
            if *cursor < lines.len() && !is_block_close(lines[*cursor].trim()) {
                nodes.push(TemplateNode::Text("\n".to_string()));
            }
            continue;
        }

        nodes.extend(parse_text_line(raw_line, line_number));
        if *cursor + 1 < lines.len() && !is_block_close(lines[*cursor + 1].trim()) {
            nodes.push(TemplateNode::Text("\n".to_string()));
        }
        *cursor += 1;
    }

    Ok(nodes)
}

fn is_block_close(trimmed: &str) -> bool {
    trimmed == "}" || trimmed.starts_with("} @else")
}

fn parse_if(lines: &[&str], cursor: &mut usize) -> Result<TemplateNode, String> {
    let raw_line = lines[*cursor];
    let line_number = *cursor + 1;
    let trimmed = raw_line.trim();

    let condition_name = trimmed
        .strip_prefix("@if")
        .expect("@if prefix already checked")
        .trim()
        .strip_suffix('{')
        .ok_or_else(|| format!("line {line_number}: @if expects `@if condition {{`"))?
        .trim();

    if !is_identifier(condition_name) {
        return Err(format!(
            "line {line_number}: @if condition must be a simple identifier in the MVP"
        ));
    }

    let column = raw_line
        .find(condition_name)
        .map(|index| index + 1)
        .unwrap_or(1);

    *cursor += 1;
    let then_nodes = parse_nodes(lines, cursor, true)?;

    if *cursor >= lines.len() {
        return Err(format!("line {line_number}: unclosed @if block"));
    }

    let close_line = lines[*cursor].trim();
    let mut else_nodes = Vec::new();

    if close_line == "}" {
        *cursor += 1;
        if *cursor < lines.len() && lines[*cursor].trim() == "@else {" {
            *cursor += 1;
            else_nodes = parse_nodes(lines, cursor, true)?;
            if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
                return Err(format!("line {}: unclosed @else block", *cursor + 1));
            }
            *cursor += 1;
        }
    } else if close_line == "} @else {" {
        *cursor += 1;
        else_nodes = parse_nodes(lines, cursor, true)?;
        if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
            return Err(format!("line {}: unclosed @else block", *cursor + 1));
        }
        *cursor += 1;
    } else {
        return Err(format!("line {}: expected `}}`", *cursor + 1));
    }

    Ok(TemplateNode::If {
        condition: SourceExpr {
            name: condition_name.to_string(),
            line: line_number,
            column,
        },
        then_nodes,
        else_nodes,
    })
}

fn parse_text_line(line: &str, line_number: usize) -> Vec<TemplateNode> {
    let mut nodes = Vec::new();
    let mut offset = 0;

    while let Some(start) = line[offset..].find('{') {
        let absolute_start = offset + start;
        if absolute_start > offset {
            nodes.push(TemplateNode::Text(line[offset..absolute_start].to_string()));
        }

        let expr_start = absolute_start + 1;
        let Some(end) = line[expr_start..].find('}') else {
            nodes.push(TemplateNode::Text(line[absolute_start..].to_string()));
            return nodes;
        };

        let expr_end = expr_start + end;
        let name = line[expr_start..expr_end].trim();
        let column = line[expr_start..expr_end]
            .find(name)
            .map(|inner| expr_start + inner + 1)
            .unwrap_or(expr_start + 1);

        if name.is_empty() || !is_identifier(name) {
            nodes.push(TemplateNode::Text(
                line[absolute_start..=expr_end].to_string(),
            ));
        } else {
            nodes.push(TemplateNode::Expr(SourceExpr {
                name: name.to_string(),
                line: line_number,
                column,
            }));
        }

        offset = expr_end + 1;
    }

    if offset < line.len() {
        nodes.push(TemplateNode::Text(line[offset..].to_string()));
    }

    nodes
}

fn parse_page(line: &str, line_number: usize) -> Result<RoutePattern, String> {
    let rest = line
        .strip_prefix("@page")
        .expect("@page prefix already checked")
        .trim();

    let raw = parse_quoted(rest).ok_or_else(|| {
        format!("line {line_number}: @page expects a quoted route, for example @page \"/\"")
    })?;
    let params = parse_route_params(&raw, line_number)?;

    Ok(RoutePattern { raw, params })
}

fn parse_route_params(raw: &str, line_number: usize) -> Result<Vec<RouteParam>, String> {
    let mut params = Vec::new();
    let mut rest = raw;

    while let Some(start) = rest.find('{') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('}') else {
            return Err(format!("line {line_number}: unclosed route parameter"));
        };
        let param = &rest[..end];
        let (name, type_name) = param
            .split_once(':')
            .ok_or_else(|| format!("line {line_number}: route params use {{name:type}}"))?;
        if !is_identifier(name) {
            return Err(format!("line {line_number}: invalid route param `{name}`"));
        }
        params.push(RouteParam {
            name: name.to_string(),
            type_name: type_name.to_string(),
        });
        rest = &rest[end + 1..];
    }

    Ok(params)
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

#[cfg(test)]
mod tests {
    use super::{parse, TemplateNode, Value};

    #[test]
    fn parses_page_lets_and_template_ast() {
        let parsed = parse(
            "@page \"/\"\n\n@let name: string = \"Ada\"\n@let visits: int = 3\n\n<h1>{name}</h1>",
        )
        .expect("valid page");

        assert_eq!(parsed.route.raw, "/");
        assert!(matches!(parsed.lets.get("name"), Some(Value::String(value)) if value == "Ada"));
        assert!(matches!(parsed.lets.get("visits"), Some(Value::Int(3))));
        assert!(matches!(parsed.template[1], TemplateNode::Expr(_)));
    }

    #[test]
    fn parses_route_params() {
        let parsed =
            parse("@page \"/posts/{slug:string}\"\n\n<h1>{slug}</h1>").expect("valid page");

        assert_eq!(parsed.route.params[0].name, "slug");
        assert_eq!(parsed.route.params[0].type_name, "string");
    }

    #[test]
    fn parses_if_else() {
        let parsed = parse(
            "@page \"/\"\n\n@let show: bool = true\n\n@if show {\n<p>yes</p>\n} @else {\n<p>no</p>\n}",
        )
        .expect("valid page");

        assert!(matches!(parsed.template[0], TemplateNode::If { .. }));
    }

    #[test]
    fn rejects_missing_page() {
        let error = parse("<h1>No route</h1>").expect_err("missing page should fail");
        assert_eq!(error, "line 1: missing @page directive");
    }
}
