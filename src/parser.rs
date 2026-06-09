use crate::diagnostic::{Diagnostic, Span};
use std::collections::BTreeMap;

fn parse_diagnostic(
    line: usize,
    start_col: usize,
    end_col: usize,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(Span::new(line, start_col, end_col), message, None)
}

fn parse_diagnostic_line(line: usize, message: impl Into<String>) -> Diagnostic {
    parse_diagnostic(line, 1, 1, message)
}

#[derive(Debug, Clone)]
pub struct WebFile {
    pub route: Option<RoutePattern>,
    pub component: Option<ComponentDecl>,
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

#[derive(Debug, Clone)]
pub struct ComponentDecl {
    pub name: String,
    pub props: Vec<PropDecl>,
}

#[derive(Debug, Clone)]
pub struct PropDecl {
    pub name: String,
    pub type_name: String,
    pub default: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateNode {
    Text(String),
    Expr(SourceExpr),
    Component(ComponentCall),
    If {
        condition: SourceExpr,
        then_nodes: Vec<TemplateNode>,
        else_nodes: Vec<TemplateNode>,
    },
    For {
        item_name: String,
        source: SourceExpr,
        body: Vec<TemplateNode>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceExpr {
    pub name: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentCall {
    pub name: String,
    pub props: Vec<ComponentProp>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentProp {
    pub name: String,
    pub value: PropValue,
    pub line: usize,
    pub column: usize,
    pub value_start_col: usize,
    pub value_end_col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropValue {
    Expr(SourceExpr),
    Literal(Value),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    String(String),
    Int(i64),
    Bool(bool),
    Array {
        element_type: String,
        values: Vec<Value>,
    },
}

impl Value {
    pub fn render(&self) -> String {
        match self {
            Value::String(value) => value.clone(),
            Value::Int(value) => value.to_string(),
            Value::Bool(value) => value.to_string(),
            Value::Array { .. } => "[array]".to_string(),
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array { values, .. } => Some(values),
            _ => None,
        }
    }

    pub fn array_sample(&self) -> Option<Value> {
        match self {
            Value::Array {
                element_type,
                values,
            } => values
                .first()
                .cloned()
                .or_else(|| sample_value(element_type)),
            _ => None,
        }
    }

    pub fn type_name(&self) -> String {
        match self {
            Value::String(_) => "string".to_string(),
            Value::Int(_) => "int".to_string(),
            Value::Bool(_) => "bool".to_string(),
            Value::Array { element_type, .. } => format!("{element_type}[]"),
        }
    }
}

fn sample_value(type_name: &str) -> Option<Value> {
    match type_name {
        "string" => Some(Value::String(String::new())),
        "int" => Some(Value::Int(0)),
        "bool" => Some(Value::Bool(false)),
        _ => None,
    }
}

pub fn parse(source: &str) -> Result<WebFile, Diagnostic> {
    let mut route = None;
    let mut component = None;
    let mut lets = BTreeMap::new();
    let mut template_start = None;
    let lines: Vec<&str> = source.lines().collect();
    let mut line_index = 0;

    while line_index < lines.len() {
        let raw_line = lines[line_index];
        let line_number = line_index + 1;
        let trimmed = raw_line.trim();

        if trimmed.is_empty() {
            line_index += 1;
            continue;
        }

        if trimmed.starts_with("@page") {
            if component.is_some() {
                return Err(parse_diagnostic_line(
                    line_number,
                    "@page cannot be combined with @component",
                ));
            }
            route = Some(parse_page(trimmed, line_number)?);
            line_index += 1;
            continue;
        }

        if trimmed.starts_with("@component") {
            if route.is_some() {
                return Err(parse_diagnostic_line(
                    line_number,
                    "@component cannot be combined with @page",
                ));
            }
            component = Some(parse_component(&lines, &mut line_index)?);
            continue;
        }

        if trimmed.starts_with("@let") {
            let (name, value) = parse_let(trimmed, line_number)?;
            lets.insert(name, value);
            line_index += 1;
            continue;
        }

        template_start = Some(line_index);
        break;
    }

    if route.is_none() && component.is_none() {
        return Err(parse_diagnostic_line(
            1,
            "missing @page or @component directive",
        ));
    }

    let template = match template_start {
        Some(start) => {
            let mut cursor = start;
            parse_nodes(&lines, &mut cursor, false, &mut lets)?
        }
        None => Vec::new(),
    };

    Ok(WebFile {
        route,
        component,
        lets,
        template,
    })
}

fn parse_nodes(
    lines: &[&str],
    cursor: &mut usize,
    stop_on_close: bool,
    lets: &mut BTreeMap<String, Value>,
) -> Result<Vec<TemplateNode>, Diagnostic> {
    let mut nodes = Vec::new();

    while *cursor < lines.len() {
        let raw_line = lines[*cursor];
        let line_number = *cursor + 1;
        let trimmed = raw_line.trim();

        if stop_on_close && (trimmed == "}" || trimmed.starts_with("} @else")) {
            break;
        }

        if trimmed.starts_with("@if ") {
            nodes.push(parse_if(lines, cursor, lets)?);
            if *cursor < lines.len() && !is_block_close(lines[*cursor].trim()) {
                nodes.push(TemplateNode::Text("\n".to_string()));
            }
            continue;
        }

        if trimmed.starts_with("@for ") {
            nodes.push(parse_for(lines, cursor, lets)?);
            if *cursor < lines.len() && !is_block_close(lines[*cursor].trim()) {
                nodes.push(TemplateNode::Text("\n".to_string()));
            }
            continue;
        }

        if trimmed.starts_with("@let") {
            let (name, value) = parse_let(trimmed, line_number)?;
            lets.insert(name, value);
            *cursor += 1;
            continue;
        }

        if trimmed.starts_with('@') {
            let col = raw_line.find('@').map(|index| index + 1).unwrap_or(1);
            return Err(parse_diagnostic(
                line_number,
                col,
                col + trimmed.len(),
                format!("unexpected directive `{trimmed}`"),
            ));
        }

        if let Some(component) = parse_component_call_line(raw_line, line_number)? {
            nodes.push(TemplateNode::Component(component));
            if *cursor + 1 < lines.len() && !is_block_close(lines[*cursor + 1].trim()) {
                nodes.push(TemplateNode::Text("\n".to_string()));
            }
            *cursor += 1;
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

fn parse_component(lines: &[&str], cursor: &mut usize) -> Result<ComponentDecl, Diagnostic> {
    let line_number = *cursor + 1;
    let trimmed = lines[*cursor].trim();
    let header = trimmed
        .strip_prefix("@component")
        .expect("@component prefix already checked")
        .trim();
    let name = header.strip_suffix('{').ok_or_else(|| {
        parse_diagnostic_line(line_number, "@component expects `@component Name {`")
    })?;
    let name = name.trim();

    if !is_component_name(name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid component name `{name}`"),
        ));
    }

    *cursor += 1;
    let mut props = Vec::new();

    while *cursor < lines.len() {
        let prop_line_number = *cursor + 1;
        let trimmed = lines[*cursor].trim();
        if trimmed == "}" {
            *cursor += 1;
            return Ok(ComponentDecl {
                name: name.to_string(),
                props,
            });
        }
        if trimmed.is_empty() {
            *cursor += 1;
            continue;
        }

        props.push(parse_prop_decl(trimmed, prop_line_number)?);
        *cursor += 1;
    }

    Err(parse_diagnostic_line(
        line_number,
        "unclosed @component block",
    ))
}

fn parse_prop_decl(line: &str, line_number: usize) -> Result<PropDecl, Diagnostic> {
    let (left, default_value) = match line.split_once('=') {
        Some((left, right)) => (left.trim(), Some(right.trim())),
        None => (line, None),
    };
    let (name, type_name) = left
        .split_once(':')
        .ok_or_else(|| parse_diagnostic_line(line_number, "component props use `name: type`"))?;
    let name = name.trim();
    let type_name = type_name.trim();

    if !is_identifier(name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid prop name `{name}`"),
        ));
    }

    let default = default_value
        .map(|value| {
            let value_col = line.find(value).map(|index| index + 1).unwrap_or(1);
            parse_value(
                type_name,
                value,
                line_number,
                value_col,
                value_col + value.len(),
            )
        })
        .transpose()?;

    Ok(PropDecl {
        name: name.to_string(),
        type_name: type_name.to_string(),
        default,
    })
}

fn parse_component_call_line(
    raw_line: &str,
    line_number: usize,
) -> Result<Option<ComponentCall>, Diagnostic> {
    let trimmed = raw_line.trim();
    if !trimmed.starts_with('<') || !trimmed.ends_with("/>") {
        return Ok(None);
    }

    let inner = trimmed[1..trimmed.len() - 2].trim();
    let Some((name, rest)) = split_tag_name(inner) else {
        return Ok(None);
    };
    if !is_component_name(name) {
        return Ok(None);
    }

    let column = raw_line.find('<').map(|index| index + 1).unwrap_or(1);
    let rest_column = if rest.is_empty() {
        column + 1 + name.len()
    } else {
        raw_line.find(rest).map(|index| index + 1).unwrap_or(column)
    };
    let props = parse_component_props(rest, line_number, rest_column)?;
    Ok(Some(ComponentCall {
        name: name.to_string(),
        props,
        line: line_number,
        column,
    }))
}

fn split_tag_name(value: &str) -> Option<(&str, &str)> {
    let split_at = value
        .find(|char: char| char.is_whitespace())
        .unwrap_or(value.len());
    let name = &value[..split_at];
    if name.is_empty() {
        None
    } else {
        Some((name, value[split_at..].trim()))
    }
}

fn parse_component_props(
    value: &str,
    line_number: usize,
    start_column: usize,
) -> Result<Vec<ComponentProp>, Diagnostic> {
    let mut props = Vec::new();
    let mut rest = value.trim();
    let mut rest_column = start_column + (value.len() - rest.len());

    while !rest.is_empty() {
        let eq_index = rest.find('=').ok_or_else(|| {
            parse_diagnostic_line(line_number, "component props use `name=value`")
        })?;
        let name = rest[..eq_index].trim();
        let name_column = rest_column + rest[..eq_index].find(name).unwrap_or(0);
        if !is_identifier(name) {
            return Err(parse_diagnostic(
                line_number,
                name_column,
                name_column + name.len(),
                format!("invalid prop name `{name}`"),
            ));
        }

        let value_start = eq_index + 1;
        let after_eq = rest[value_start..].trim_start();
        let leading_trim = rest[value_start..].len() - after_eq.len();
        let value_start_col = rest_column + eq_index + 1 + leading_trim;
        let (value, consumed) = parse_prop_value(after_eq, line_number, value_start_col)?;
        props.push(ComponentProp {
            name: name.to_string(),
            value,
            line: line_number,
            column: name_column,
            value_start_col,
            value_end_col: value_start_col + consumed,
        });
        let next = &after_eq[consumed..];
        let skipped = next.len() - next.trim_start().len();
        rest_column +=
            value_start + (rest[value_start..].len() - after_eq.len()) + consumed + skipped;
        rest = next.trim_start();
    }

    Ok(props)
}

fn parse_prop_value(
    value: &str,
    line_number: usize,
    value_start_col: usize,
) -> Result<(PropValue, usize), Diagnostic> {
    if let Some(stripped) = value.strip_prefix('{') {
        let end = stripped.find('}').ok_or_else(|| {
            parse_diagnostic(
                line_number,
                value_start_col,
                value_start_col + value.len(),
                "unclosed component prop expression",
            )
        })?;
        let inner = stripped[..end].trim();
        if let Some(literal) = parse_prop_literal(inner) {
            return Ok((PropValue::Literal(literal), end + 2));
        }
        let name = inner;
        if !is_identifier(name) {
            return Err(parse_diagnostic(
                line_number,
                value_start_col,
                value_start_col + value.len().min(end + 2),
                "component prop expressions must be simple identifiers",
            ));
        }
        let expr_col = value_start_col
            + value[..end + 1]
                .find(name)
                .map(|index| index + 1)
                .unwrap_or(1);
        return Ok((
            PropValue::Expr(SourceExpr {
                name: name.to_string(),
                line: line_number,
                column: expr_col,
            }),
            end + 2,
        ));
    }

    if value.starts_with('"') {
        let mut escaped = false;
        for (index, char) in value[1..].char_indices() {
            if char == '"' && !escaped {
                let end = index + 1;
                let literal = &value[..=end];
                return Ok((
                    PropValue::Literal(Value::String(
                        parse_quoted(literal).expect("literal bounds checked"),
                    )),
                    end + 1,
                ));
            }
            escaped = char == '\\' && !escaped;
        }
        return Err(parse_diagnostic(
            line_number,
            value_start_col,
            value_start_col + value.len(),
            "unterminated string component prop",
        ));
    }

    let end = value.find(char::is_whitespace).unwrap_or(value.len());
    let literal = &value[..end];
    let parsed = parse_prop_literal(literal).ok_or_else(|| {
        parse_diagnostic(
            line_number,
            value_start_col,
            value_start_col + end,
            format!("unsupported component prop literal `{literal}`"),
        )
    })?;
    Ok((PropValue::Literal(parsed), end))
}

fn parse_prop_literal(value: &str) -> Option<Value> {
    match value {
        "true" => Some(Value::Bool(true)),
        "false" => Some(Value::Bool(false)),
        _ => value.parse::<i64>().map(Value::Int).ok(),
    }
}

fn is_block_close(trimmed: &str) -> bool {
    trimmed == "}" || trimmed.starts_with("} @else")
}

fn parse_for(
    lines: &[&str],
    cursor: &mut usize,
    lets: &mut BTreeMap<String, Value>,
) -> Result<TemplateNode, Diagnostic> {
    let raw_line = lines[*cursor];
    let line_number = *cursor + 1;
    let trimmed = raw_line.trim();

    let header = trimmed
        .strip_prefix("@for")
        .expect("@for prefix already checked")
        .trim()
        .strip_suffix('{')
        .ok_or_else(|| parse_diagnostic_line(line_number, "@for expects `@for item in items {`"))?
        .trim();
    let (item_name, source_name) = header
        .split_once(" in ")
        .ok_or_else(|| parse_diagnostic_line(line_number, "@for expects `@for item in items {`"))?;
    let item_name = item_name.trim();
    let source_name = source_name.trim();

    if !is_identifier(item_name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid loop variable `{item_name}`"),
        ));
    }
    if !is_identifier(source_name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid loop source `{source_name}`"),
        ));
    }

    let source_column = raw_line
        .find(source_name)
        .map(|index| index + 1)
        .unwrap_or(1);

    *cursor += 1;
    let body = parse_nodes(lines, cursor, true, lets)?;

    if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
        return Err(parse_diagnostic_line(line_number, "unclosed @for block"));
    }
    *cursor += 1;

    Ok(TemplateNode::For {
        item_name: item_name.to_string(),
        source: SourceExpr {
            name: source_name.to_string(),
            line: line_number,
            column: source_column,
        },
        body,
    })
}

fn parse_if(
    lines: &[&str],
    cursor: &mut usize,
    lets: &mut BTreeMap<String, Value>,
) -> Result<TemplateNode, Diagnostic> {
    let raw_line = lines[*cursor];
    let line_number = *cursor + 1;
    let trimmed = raw_line.trim();

    let condition_name = trimmed
        .strip_prefix("@if")
        .expect("@if prefix already checked")
        .trim()
        .strip_suffix('{')
        .ok_or_else(|| parse_diagnostic_line(line_number, "@if expects `@if condition {`"))?
        .trim();

    if !is_identifier(condition_name) {
        return Err(parse_diagnostic_line(
            line_number,
            "@if condition must be a simple identifier in the MVP",
        ));
    }

    let column = raw_line
        .find(condition_name)
        .map(|index| index + 1)
        .unwrap_or(1);

    *cursor += 1;
    let then_nodes = parse_nodes(lines, cursor, true, lets)?;

    if *cursor >= lines.len() {
        return Err(parse_diagnostic_line(line_number, "unclosed @if block"));
    }

    let close_line = lines[*cursor].trim();
    let mut else_nodes = Vec::new();

    if close_line == "}" {
        *cursor += 1;
        if *cursor < lines.len() && lines[*cursor].trim() == "@else {" {
            *cursor += 1;
            else_nodes = parse_nodes(lines, cursor, true, lets)?;
            if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
                return Err(parse_diagnostic_line(*cursor + 1, "unclosed @else block"));
            }
            *cursor += 1;
        }
    } else if close_line == "} @else {" {
        *cursor += 1;
        else_nodes = parse_nodes(lines, cursor, true, lets)?;
        if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
            return Err(parse_diagnostic_line(*cursor + 1, "unclosed @else block"));
        }
        *cursor += 1;
    } else {
        return Err(parse_diagnostic_line(*cursor + 1, "expected `}`"));
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

fn parse_page(line: &str, line_number: usize) -> Result<RoutePattern, Diagnostic> {
    let rest = line
        .strip_prefix("@page")
        .expect("@page prefix already checked")
        .trim();

    let raw = parse_quoted(rest).ok_or_else(|| {
        parse_diagnostic_line(
            line_number,
            "@page expects a quoted route, for example @page \"/\"",
        )
    })?;
    let params = parse_route_params(&raw, line_number)?;

    Ok(RoutePattern { raw, params })
}

fn parse_route_params(raw: &str, line_number: usize) -> Result<Vec<RouteParam>, Diagnostic> {
    let mut params = Vec::new();
    let mut rest = raw;

    while let Some(start) = rest.find('{') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('}') else {
            return Err(parse_diagnostic_line(
                line_number,
                "unclosed route parameter",
            ));
        };
        let param = &rest[..end];
        let (name, type_name) = param
            .split_once(':')
            .ok_or_else(|| parse_diagnostic_line(line_number, "route params use {name:type}"))?;
        if !is_identifier(name) {
            return Err(parse_diagnostic_line(
                line_number,
                format!("invalid route param `{name}`"),
            ));
        }
        params.push(RouteParam {
            name: name.to_string(),
            type_name: type_name.to_string(),
        });
        rest = &rest[end + 1..];
    }

    Ok(params)
}

fn parse_let(line: &str, line_number: usize) -> Result<(String, Value), Diagnostic> {
    let rest = line
        .strip_prefix("@let")
        .expect("@let prefix already checked")
        .trim();
    let (left, right) = rest
        .split_once('=')
        .ok_or_else(|| parse_diagnostic_line(line_number, "@let expects `name: type = value`"))?;
    let (name, type_name) = left
        .split_once(':')
        .ok_or_else(|| parse_diagnostic_line(line_number, "@let expects an explicit type"))?;

    let name = name.trim();
    if !is_identifier(name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid identifier `{name}`"),
        ));
    }

    let value_text = right.trim();
    let value_col = line.find(value_text).map(|index| index + 1).unwrap_or(1);
    let value = parse_value(
        type_name.trim(),
        value_text,
        line_number,
        value_col,
        value_col + value_text.len(),
    )?;
    Ok((name.to_string(), value))
}

fn parse_value(
    type_name: &str,
    value: &str,
    line_number: usize,
    start_col: usize,
    end_col: usize,
) -> Result<Value, Diagnostic> {
    if let Some(element_type) = type_name.strip_suffix("[]") {
        return parse_array_value(element_type, value, line_number, start_col, end_col);
    }

    match type_name {
        "string" => parse_quoted(value).map(Value::String).ok_or_else(|| {
            parse_diagnostic(
                line_number,
                start_col,
                end_col,
                "string values must be quoted",
            )
        }),
        "int" => value.parse::<i64>().map(Value::Int).map_err(|_| {
            parse_diagnostic(
                line_number,
                start_col,
                end_col,
                format!("invalid int literal `{value}`"),
            )
        }),
        "bool" => match value {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(parse_diagnostic(
                line_number,
                start_col,
                end_col,
                format!("invalid bool literal `{value}`"),
            )),
        },
        other => Err(parse_diagnostic(
            line_number,
            start_col,
            end_col,
            format!("unsupported MVP type `{other}`"),
        )),
    }
}

fn parse_array_value(
    type_name: &str,
    value: &str,
    line_number: usize,
    start_col: usize,
    end_col: usize,
) -> Result<Value, Diagnostic> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return Err(parse_diagnostic(
            line_number,
            start_col,
            end_col,
            "array values must use `[value, ...]`",
        ));
    }

    let inner = value[1..value.len() - 1].trim();
    if inner.is_empty() {
        return Ok(Value::Array {
            element_type: type_name.to_string(),
            values: Vec::new(),
        });
    }

    let mut values = Vec::new();
    for item in split_array_items(inner, line_number, start_col, end_col)? {
        let item = item.trim();
        let item_col = value
            .find(item)
            .map(|index| start_col + index)
            .unwrap_or(start_col);
        values.push(parse_value(
            type_name,
            item,
            line_number,
            item_col,
            item_col + item.len(),
        )?);
    }

    Ok(Value::Array {
        element_type: type_name.to_string(),
        values,
    })
}

fn split_array_items(
    value: &str,
    line_number: usize,
    start_col: usize,
    end_col: usize,
) -> Result<Vec<&str>, Diagnostic> {
    let mut items = Vec::new();
    let mut start = 0;
    let mut in_string = false;

    for (index, char) in value.char_indices() {
        match char {
            '"' => in_string = !in_string,
            ',' if !in_string => {
                items.push(&value[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }

    if in_string {
        return Err(parse_diagnostic(
            line_number,
            start_col,
            end_col,
            "unterminated string in array literal",
        ));
    }

    items.push(&value[start..]);
    Ok(items)
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

fn is_component_name(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|char| char.is_ascii_uppercase())
        && is_identifier(value)
}

#[cfg(test)]
mod tests {
    use super::{parse, PropValue, TemplateNode, Value};

    #[test]
    fn parses_page_lets_and_template_ast() {
        let parsed = parse(
            "@page \"/\"\n\n@let name: string = \"Ada\"\n@let visits: int = 3\n\n<h1>{name}</h1>",
        )
        .expect("valid page");

        assert_eq!(parsed.route.as_ref().expect("route").raw, "/");
        assert!(matches!(parsed.lets.get("name"), Some(Value::String(value)) if value == "Ada"));
        assert!(matches!(parsed.lets.get("visits"), Some(Value::Int(3))));
        assert!(matches!(parsed.template[1], TemplateNode::Expr(_)));
    }

    #[test]
    fn parses_route_params() {
        let parsed =
            parse("@page \"/posts/{slug:string}\"\n\n<h1>{slug}</h1>").expect("valid page");

        let route = parsed.route.as_ref().expect("route");
        assert_eq!(route.params[0].name, "slug");
        assert_eq!(route.params[0].type_name, "string");
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
    fn parses_arrays_and_for_blocks() {
        let parsed = parse(
            "@page \"/\"\n\n@let posts: string[] = [\"One\", \"Two\", \"Three\"]\n\n@for post in posts {\n<p>{post}</p>\n}",
        )
        .expect("valid page");

        assert!(
            matches!(parsed.lets.get("posts"), Some(Value::Array { values, .. }) if values.len() == 3)
        );
        assert!(
            matches!(&parsed.template[0], TemplateNode::For { item_name, .. } if item_name == "post")
        );
    }

    #[test]
    fn rejects_missing_page() {
        let error = parse("<h1>No route</h1>").expect_err("missing page should fail");
        assert_eq!(error.message, "missing @page or @component directive");
        assert_eq!(error.span.line, 1);
    }

    #[test]
    fn parses_component_declarations_and_calls() {
        let component = parse(
            "@component UserCard {\n  name: string\n  visits: int = 0\n}\n\n<article>{name}</article>",
        )
        .expect("valid component");

        let declaration = component.component.as_ref().expect("component");
        assert_eq!(declaration.name, "UserCard");
        assert_eq!(declaration.props[0].name, "name");
        assert!(matches!(declaration.props[1].default, Some(Value::Int(0))));

        let page = parse(
            "@page \"/\"\n\n@let name: string = \"Ada\"\n\n<UserCard name={name} visits=3 />",
        )
        .expect("valid page");

        assert!(matches!(page.template[0], TemplateNode::Component(_)));
    }

    #[test]
    fn parses_let_after_template_start_without_rendering_directive_text() {
        let parsed = parse("@page \"/\"\n\n@let name: string = \"Ada\"\n<h1>{name}</h1>\n@let greeting: string = \"Hello\"\n<p>{greeting}</p>")
            .expect("valid page");

        assert!(
            matches!(parsed.lets.get("greeting"), Some(Value::String(value)) if value == "Hello")
        );
        assert!(!parsed
            .template
            .iter()
            .any(|node| matches!(node, TemplateNode::Text(value) if value.contains("@let"))));
    }

    #[test]
    fn rejects_unknown_template_directives() {
        let error = parse("@page \"/\"\n\n@wat").expect_err("unknown directive should fail");

        assert_eq!(error.message, "unexpected directive `@wat`");
        assert_eq!(error.span.line, 3);
    }

    #[test]
    fn parses_braced_literal_component_props() {
        let page =
            parse("@page \"/\"\n\n<PostPreview featured={true} rank={1} />").expect("valid page");

        let TemplateNode::Component(call) = &page.template[0] else {
            panic!("expected component call");
        };

        assert!(matches!(
            call.props[0].value,
            PropValue::Literal(Value::Bool(true))
        ));
        assert!(matches!(
            call.props[1].value,
            PropValue::Literal(Value::Int(1))
        ));
    }
}
