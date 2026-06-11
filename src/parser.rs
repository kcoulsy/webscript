use crate::diagnostic::{Diagnostic, Span};
use crate::expr;
use crate::stmt::{self, Statement};
use crate::types;
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
    pub layout: Option<ComponentDecl>,
    pub layout_use: Option<LayoutUse>,
    pub client: Option<ClientBlock>,
    pub load: Option<ServerBlock>,
    pub actions: Vec<ActionDecl>,
    pub lets: Vec<LetDecl>,
    pub template: Vec<TemplateNode>,
    pub styles: Vec<StyleBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleBlock {
    pub global: bool,
    pub css: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutUse {
    None,
    Apply {
        name: String,
        props: Vec<ComponentProp>,
        line: usize,
    },
}

#[derive(Debug, Clone)]
pub struct ClientBlock {
    pub signals: Vec<ClientSignalDecl>,
    pub handlers: Vec<ClientHandlerDecl>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct ClientHandlerDecl {
    pub name: String,
    pub param_name: Option<String>,
    pub body: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct ClientSignalDecl {
    pub name: String,
    pub type_name: String,
    pub initial: ClientInitial,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientInitial {
    Literal(Value),
    PropRef(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventBinding {
    pub event: String,
    pub handler_source: String,
    pub line: usize,
    pub column: usize,
    pub prevent_default: bool,
    pub stop_propagation: bool,
}

#[derive(Debug, Clone)]
pub struct ServerBlock {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct ActionDecl {
    pub name: String,
    pub input_schema: Option<String>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct LetDecl {
    pub name: String,
    pub type_name: Option<String>,
    pub value: LetValue,
    pub line: usize,
    pub value_start_col: usize,
    pub value_end_col: usize,
}

#[derive(Debug, Clone)]
pub enum LetValue {
    Expr(expr::Expr),
    Static(Value),
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
    Switch {
        value: SourceExpr,
        cases: Vec<SwitchCase>,
        default_nodes: Vec<TemplateNode>,
    },
    For {
        item_name: String,
        source: SourceExpr,
        body: Vec<TemplateNode>,
    },
    Do {
        statements: Vec<Statement>,
        line: usize,
    },
    Defer {
        prelude: Vec<Statement>,
        body: Vec<TemplateNode>,
        placeholder: Vec<TemplateNode>,
        error_name: Option<String>,
        error_body: Vec<TemplateNode>,
        line: usize,
    },
    EventBinding(EventBinding),
    Slot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchCase {
    pub value: SourceExpr,
    pub nodes: Vec<TemplateNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceExpr {
    pub source: String,
    pub expr: expr::Expr,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentCall {
    pub name: String,
    pub props: Vec<ComponentProp>,
    pub event_bindings: Vec<EventBinding>,
    pub class_expr: Option<SourceExpr>,
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
    Object(BTreeMap<String, Value>),
    Array {
        element_type: String,
        values: Vec<Value>,
    },
    Duration {
        ms: i64,
    },
    Function {
        name: String,
        params: Vec<stmt::FnParam>,
        return_type: Option<String>,
        body: Vec<Statement>,
    },
    Promise {
        id: u64,
    },
}

impl Value {
    pub fn render(&self) -> String {
        match self {
            Value::String(value) => value.clone(),
            Value::Int(value) => value.to_string(),
            Value::Bool(value) => value.to_string(),
            Value::Object(_) => "[object]".to_string(),
            Value::Array { .. } => "[array]".to_string(),
            Value::Duration { ms } => format!("{ms}ms"),
            Value::Function { name, .. } => format!("<fn {name}>"),
            Value::Promise { .. } => "<promise>".to_string(),
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
            Value::Object(fields) => {
                if fields.is_empty() {
                    return "object".to_string();
                }

                let fields = fields
                    .iter()
                    .map(|(name, value)| format!("{name}: {}", value.type_name()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {fields} }}")
            }
            Value::Array { element_type, .. } => format!("{element_type}[]"),
            Value::Duration { .. } => "duration".to_string(),
            Value::Function { .. } => "function".to_string(),
            Value::Promise { .. } => "promise".to_string(),
        }
    }

    pub fn duration_ms(&self) -> Option<i64> {
        match self {
            Value::Duration { ms } => Some(*ms),
            _ => None,
        }
    }
}

fn sample_value(type_name: &str) -> Option<Value> {
    match type_name {
        "string" => Some(Value::String(String::new())),
        "int" => Some(Value::Int(0)),
        "bool" => Some(Value::Bool(false)),
        "object" => Some(Value::Object(BTreeMap::new())),
        _ => None,
    }
}

pub fn parse(source: &str) -> Result<WebFile, Diagnostic> {
    let mut route = None;
    let mut component = None;
    let mut layout = None;
    let mut layout_use = None;
    let mut client = None;
    let mut load = None;
    let mut actions = Vec::new();
    let mut lets = Vec::new();
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
            if component.is_some() || layout.is_some() {
                return Err(parse_diagnostic_line(
                    line_number,
                    "@page cannot be combined with @component or @layout",
                ));
            }
            route = Some(parse_page(trimmed, line_number)?);
            line_index += 1;
            continue;
        }

        if trimmed.starts_with("@layout") {
            if trimmed == "@layout none" {
                if route.is_none() {
                    return Err(parse_diagnostic_line(
                        line_number,
                        "@layout none can only be used on pages",
                    ));
                }
                layout_use = Some(LayoutUse::None);
                line_index += 1;
                continue;
            }
            if route.is_some() {
                layout_use = Some(parse_layout_use(&lines, &mut line_index)?);
                continue;
            }
            if component.is_some() {
                return Err(parse_diagnostic_line(
                    line_number,
                    "@layout cannot be combined with @component",
                ));
            }
            layout = Some(parse_layout(&lines, &mut line_index)?);
            continue;
        }

        if trimmed.starts_with("@component") {
            if route.is_some() || layout.is_some() {
                return Err(parse_diagnostic_line(
                    line_number,
                    "@component cannot be combined with @page or @layout",
                ));
            }
            component = Some(parse_component(&lines, &mut line_index)?);
            continue;
        }

        if trimmed.starts_with("@load") {
            load = Some(parse_server_directive(&lines, &mut line_index, "@load")?);
            continue;
        }

        if trimmed.starts_with("@client") {
            if client.is_some() {
                return Err(parse_diagnostic_line(
                    line_number,
                    "duplicate @client directive",
                ));
            }
            client = Some(parse_client(&lines, &mut line_index)?);
            continue;
        }

        if trimmed.starts_with("@action") {
            actions.push(parse_action(&lines, &mut line_index)?);
            continue;
        }

        if trimmed.starts_with("@let") {
            let declaration = collect_balanced_line(&lines, &mut line_index, trimmed)?;
            lets.push(parse_let(&declaration, line_number)?);
            continue;
        }

        if trimmed.starts_with("@style") {
            return Err(parse_diagnostic_line(
                line_number,
                "@style must appear after markup",
            ));
        }

        template_start = Some(line_index);
        break;
    }

    if route.is_none() && component.is_none() && layout.is_none() {
        return Err(parse_diagnostic_line(
            1,
            "missing @page, @component, or @layout directive",
        ));
    }

    let mut styles = Vec::new();
    let template = match template_start {
        Some(start) => {
            let mut cursor = start;
            let nodes = parse_nodes(&lines, &mut cursor, false, &mut lets)?;
            while cursor < lines.len() {
                let trimmed = lines[cursor].trim();
                if trimmed.is_empty() {
                    cursor += 1;
                    continue;
                }
                if trimmed.starts_with("@style") {
                    styles.push(parse_style(&lines, &mut cursor)?);
                    continue;
                }
                return Err(parse_diagnostic(
                    cursor + 1,
                    1,
                    lines[cursor].len().max(1),
                    "only @style blocks are allowed after markup",
                ));
            }
            nodes
        }
        None => Vec::new(),
    };

    Ok(WebFile {
        route,
        component,
        layout,
        layout_use,
        client,
        load,
        actions,
        lets,
        template,
        styles,
    })
}

fn parse_nodes(
    lines: &[&str],
    cursor: &mut usize,
    stop_on_close: bool,
    lets: &mut Vec<LetDecl>,
) -> Result<Vec<TemplateNode>, Diagnostic> {
    let mut nodes = Vec::new();

    while *cursor < lines.len() {
        let raw_line = lines[*cursor];
        let line_number = *cursor + 1;
        let trimmed = raw_line.trim();

        if stop_on_close
            && (trimmed == "}"
                || trimmed.starts_with("} @else")
                || trimmed.starts_with("} @case")
                || trimmed == "} @default {"
                || trimmed == "} @placeholder {")
        {
            break;
        }

        if trimmed == "@defer {" {
            nodes.push(parse_defer(lines, cursor, lets)?);
            if *cursor < lines.len() && !is_block_close(lines[*cursor].trim()) {
                nodes.push(TemplateNode::Text("\n".to_string()));
            }
            continue;
        }

        if trimmed.starts_with("@if ") {
            nodes.push(parse_if(lines, cursor, lets)?);
            if *cursor < lines.len() && !is_block_close(lines[*cursor].trim()) {
                nodes.push(TemplateNode::Text("\n".to_string()));
            }
            continue;
        }

        if trimmed.starts_with("@do") {
            nodes.push(parse_do(lines, cursor)?);
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

        if trimmed.starts_with("@switch ") {
            nodes.push(parse_switch(lines, cursor, lets)?);
            if *cursor < lines.len() && !is_block_close(lines[*cursor].trim()) {
                nodes.push(TemplateNode::Text("\n".to_string()));
            }
            continue;
        }

        if trimmed.starts_with("@let") {
            let declaration = collect_balanced_line(lines, cursor, trimmed)?;
            lets.push(parse_let(&declaration, line_number)?);
            continue;
        }

        if trimmed.starts_with("@style") {
            break;
        }

        if trimmed.starts_with('@') && parse_event_directive(trimmed).is_none() {
            let col = raw_line.find('@').map(|index| index + 1).unwrap_or(1);
            return Err(parse_diagnostic(
                line_number,
                col,
                col + trimmed.len(),
                format!("unexpected directive `{trimmed}`"),
            ));
        }

        if is_slot_tag(trimmed) {
            nodes.push(TemplateNode::Slot);
            if *cursor + 1 < lines.len() && !is_block_close(lines[*cursor + 1].trim()) {
                nodes.push(TemplateNode::Text("\n".to_string()));
            }
            *cursor += 1;
            continue;
        }

        let component_line = if starts_component_call(trimmed) && !trimmed.ends_with("/>") {
            Some(collect_component_call_line(lines, cursor)?)
        } else {
            None
        };
        let component_source = component_line.as_deref().unwrap_or(raw_line);

        if let Some(component) = parse_component_call_line(component_source, line_number)? {
            nodes.push(TemplateNode::Component(component));
            if component_line.is_none() {
                *cursor += 1;
            }
            if *cursor < lines.len() && !is_block_close(lines[*cursor].trim()) {
                nodes.push(TemplateNode::Text("\n".to_string()));
            }
            continue;
        }

        nodes.extend(parse_text_line(raw_line, line_number)?);
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
    if let Some(name) = header.strip_suffix("{}") {
        let name = name.trim();
        if !is_component_name(name) {
            return Err(parse_diagnostic_line(
                line_number,
                format!("invalid component name `{name}`"),
            ));
        }
        *cursor += 1;
        return Ok(ComponentDecl {
            name: name.to_string(),
            props: Vec::new(),
        });
    }
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

fn parse_layout(lines: &[&str], cursor: &mut usize) -> Result<ComponentDecl, Diagnostic> {
    let line_number = *cursor + 1;
    let trimmed = lines[*cursor].trim();
    let header = trimmed
        .strip_prefix("@layout")
        .expect("@layout prefix already checked")
        .trim();
    let name = header
        .strip_suffix('{')
        .ok_or_else(|| parse_diagnostic_line(line_number, "@layout expects `@layout Name {`"))?;
    let name = name.trim();

    if !is_component_name(name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid layout name `{name}`"),
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

    Err(parse_diagnostic_line(line_number, "unclosed @layout block"))
}

fn parse_layout_use(lines: &[&str], cursor: &mut usize) -> Result<LayoutUse, Diagnostic> {
    let line_number = *cursor + 1;
    let trimmed = lines[*cursor].trim();
    let header = trimmed
        .strip_prefix("@layout")
        .expect("@layout prefix already checked")
        .trim();
    let name = header
        .strip_suffix('{')
        .ok_or_else(|| parse_diagnostic_line(line_number, "@layout expects `@layout Name {`"))?;
    let name = name.trim();

    if !is_component_name(name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid layout name `{name}`"),
        ));
    }

    *cursor += 1;
    let mut props = Vec::new();

    while *cursor < lines.len() {
        let prop_line_number = *cursor + 1;
        let trimmed = lines[*cursor].trim();
        if trimmed == "}" {
            *cursor += 1;
            return Ok(LayoutUse::Apply {
                name: name.to_string(),
                props,
                line: line_number,
            });
        }
        if trimmed.is_empty() {
            *cursor += 1;
            continue;
        }

        props.push(parse_layout_use_prop(trimmed, prop_line_number)?);
        *cursor += 1;
    }

    Err(parse_diagnostic_line(line_number, "unclosed @layout block"))
}

fn parse_layout_use_prop(line: &str, line_number: usize) -> Result<ComponentProp, Diagnostic> {
    let (name, value_text) = line
        .split_once(':')
        .ok_or_else(|| parse_diagnostic_line(line_number, "layout props use `name: value`"))?;
    let name = name.trim();
    if !is_identifier(name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid layout prop name `{name}`"),
        ));
    }

    let value_text = value_text.trim();
    let value_start_col = line.find(value_text).map(|index| index + 1).unwrap_or(1);
    let (value, consumed) = parse_prop_value(value_text, line_number, value_start_col)?;
    Ok(ComponentProp {
        name: name.to_string(),
        value,
        line: line_number,
        column: 1,
        value_start_col,
        value_end_col: value_start_col + consumed,
    })
}

fn parse_client(lines: &[&str], cursor: &mut usize) -> Result<ClientBlock, Diagnostic> {
    let line_number = *cursor + 1;
    let trimmed = lines[*cursor].trim();
    if trimmed != "@client {" {
        return Err(parse_diagnostic_line(
            line_number,
            "@client expects `@client {`",
        ));
    }
    *cursor += 1;
    let mut signals = Vec::new();
    let mut handlers = Vec::new();

    while *cursor < lines.len() {
        let item_line_number = *cursor + 1;
        let trimmed = lines[*cursor].trim();
        if trimmed == "}" {
            *cursor += 1;
            return Ok(ClientBlock {
                signals,
                handlers,
                line: line_number,
            });
        }
        if trimmed.is_empty() {
            *cursor += 1;
            continue;
        }

        if trimmed.starts_with("fn ") {
            handlers.push(parse_client_handler(lines, cursor)?);
            continue;
        }

        signals.push(parse_client_signal(trimmed, item_line_number)?);
        *cursor += 1;
    }

    Err(parse_diagnostic_line(line_number, "unclosed @client block"))
}

fn parse_client_handler(
    lines: &[&str],
    cursor: &mut usize,
) -> Result<ClientHandlerDecl, Diagnostic> {
    let line_number = *cursor + 1;
    let trimmed = lines[*cursor].trim();
    let rest = trimmed.strip_prefix("fn ").ok_or_else(|| {
        parse_diagnostic_line(line_number, "client handler expects `fn name() {`")
    })?;
    let (name_part, _) = rest.split_once('{').ok_or_else(|| {
        parse_diagnostic_line(line_number, "client handler expects `fn name() {`")
    })?;
    let name_part = name_part.trim();
    let (name, param_name) = if let Some((name, params)) = name_part.split_once('(') {
        let name = name.trim();
        let params = params
            .strip_suffix(')')
            .ok_or_else(|| {
                parse_diagnostic_line(line_number, "client handler expects `fn name() {`")
            })?
            .trim();
        if params.is_empty() {
            (name, None)
        } else if is_identifier(params) {
            (name, Some(params.to_string()))
        } else {
            return Err(parse_diagnostic_line(
                line_number,
                "client handler supports a single parameter",
            ));
        }
    } else {
        return Err(parse_diagnostic_line(
            line_number,
            "client handler expects `fn name() {`",
        ));
    };
    if !is_identifier(name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid client handler name `{name}`"),
        ));
    }

    *cursor += 1;
    let mut body_lines = Vec::new();
    while *cursor < lines.len() {
        let body_trimmed = lines[*cursor].trim();
        if body_trimmed == "}" {
            *cursor += 1;
            return Ok(ClientHandlerDecl {
                name: name.to_string(),
                param_name,
                body: body_lines.join("\n"),
                line: line_number,
            });
        }
        if body_trimmed.is_empty() {
            *cursor += 1;
            continue;
        }
        body_lines.push(body_trimmed.to_string());
        *cursor += 1;
    }

    Err(parse_diagnostic_line(
        line_number,
        "unclosed client handler block",
    ))
}

fn parse_client_signal(line: &str, line_number: usize) -> Result<ClientSignalDecl, Diagnostic> {
    let (left, initial_source) = line.split_once('=').ok_or_else(|| {
        parse_diagnostic_line(line_number, "client signals require an initial value")
    })?;
    let left = left.trim();
    let initial_source = initial_source.trim();

    let (name, type_part) = left.split_once(':').ok_or_else(|| {
        parse_diagnostic_line(
            line_number,
            "client signals use `name: signal<type> = value`",
        )
    })?;
    let name = name.trim();
    let type_part = type_part.trim();

    if !is_identifier(name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid signal name `{name}`"),
        ));
    }

    let Some(inner_type) = type_part
        .strip_prefix("signal<")
        .and_then(|value| value.strip_suffix('>'))
    else {
        return Err(parse_diagnostic_line(
            line_number,
            format!("`{type_part}` is not a supported signal type; use signal<int>, signal<bool>, or signal<string>"),
        ));
    };
    let inner_type = inner_type.trim();
    if inner_type != "int" && inner_type != "bool" && inner_type != "string" {
        return Err(parse_diagnostic_line(
            line_number,
            format!("unsupported signal type `{inner_type}`; use int, bool, or string"),
        ));
    }

    let initial = if initial_source.starts_with('"') || initial_source.starts_with('\'') {
        ClientInitial::Literal(parse_value(
            inner_type,
            initial_source,
            line_number,
            1,
            initial_source.len(),
        )?)
    } else if initial_source == "true" || initial_source == "false" {
        ClientInitial::Literal(parse_value(
            inner_type,
            initial_source,
            line_number,
            1,
            initial_source.len(),
        )?)
    } else if initial_source
        .chars()
        .all(|char| char.is_ascii_digit() || char == '-')
        && inner_type == "int"
    {
        ClientInitial::Literal(parse_value(
            inner_type,
            initial_source,
            line_number,
            1,
            initial_source.len(),
        )?)
    } else if is_identifier(initial_source) {
        ClientInitial::PropRef(initial_source.to_string())
    } else {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid client signal initial value `{initial_source}`"),
        ));
    };

    Ok(ClientSignalDecl {
        name: name.to_string(),
        type_name: inner_type.to_string(),
        initial,
        line: line_number,
    })
}

fn parse_style(lines: &[&str], cursor: &mut usize) -> Result<StyleBlock, Diagnostic> {
    let line_number = *cursor + 1;
    let trimmed = lines[*cursor].trim();
    let rest = trimmed
        .strip_prefix("@style")
        .ok_or_else(|| parse_diagnostic_line(line_number, "expected @style directive"))?
        .trim();

    let global = if rest == "{" {
        false
    } else if rest == "global {" {
        true
    } else if rest == "scoped {" {
        false
    } else {
        return Err(parse_diagnostic_line(
            line_number,
            "@style expects `@style {`, `@style global {`, or `@style scoped {`",
        ));
    };

    let header = lines[*cursor];
    let brace_start = header
        .find('{')
        .ok_or_else(|| parse_diagnostic_line(line_number, "unclosed @style block"))?;

    let mut depth = 0usize;
    let mut css = String::new();
    let mut line_index = *cursor;

    for char in header[brace_start..].chars() {
        if char == '{' {
            depth += 1;
            if depth == 1 {
                continue;
            }
        } else if char == '}' {
            depth -= 1;
            if depth == 0 {
                *cursor = line_index + 1;
                return Ok(StyleBlock {
                    global,
                    css: css.trim().to_string(),
                    line: line_number,
                });
            }
        }
        css.push(char);
    }

    line_index += 1;
    while line_index < lines.len() {
        let line = lines[line_index];
        for char in line.chars() {
            if char == '{' {
                depth += 1;
                css.push(char);
            } else if char == '}' {
                depth -= 1;
                if depth == 0 {
                    *cursor = line_index + 1;
                    return Ok(StyleBlock {
                        global,
                        css: css.trim().to_string(),
                        line: line_number,
                    });
                }
                css.push(char);
            } else {
                css.push(char);
            }
        }
        css.push('\n');
        line_index += 1;
    }

    Err(parse_diagnostic_line(line_number, "unclosed @style block"))
}

fn parse_server_directive(
    lines: &[&str],
    cursor: &mut usize,
    directive: &str,
) -> Result<ServerBlock, Diagnostic> {
    let line_number = *cursor + 1;
    let trimmed = lines[*cursor].trim();
    if trimmed != format!("{directive} {{") {
        return Err(parse_diagnostic_line(
            line_number,
            format!("{directive} expects `{directive} {{`"),
        ));
    }
    *cursor += 1;
    let statements =
        stmt::parse_server_block(lines, cursor, line_number, stmt::BlockMode::AsyncCapable)?;
    if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
        return Err(parse_diagnostic_line(
            line_number,
            format!("unclosed {directive} block"),
        ));
    }
    *cursor += 1;
    Ok(ServerBlock { statements })
}

fn parse_action(lines: &[&str], cursor: &mut usize) -> Result<ActionDecl, Diagnostic> {
    let line_number = *cursor + 1;
    let trimmed = lines[*cursor].trim();
    let header = trimmed
        .strip_prefix("@action")
        .expect("@action prefix already checked")
        .trim();
    let header = header
        .strip_suffix('{')
        .ok_or_else(|| parse_diagnostic_line(line_number, "@action expects `@action name {`"))?
        .trim();
    let (name, input_schema) = parse_action_header(header, line_number)?;

    *cursor += 1;
    let statements =
        stmt::parse_server_block(lines, cursor, line_number, stmt::BlockMode::AsyncCapable)?;
    if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
        return Err(parse_diagnostic_line(line_number, "unclosed @action block"));
    }
    *cursor += 1;

    Ok(ActionDecl {
        name,
        input_schema,
        statements,
    })
}

fn parse_action_header(
    header: &str,
    line_number: usize,
) -> Result<(String, Option<String>), Diagnostic> {
    let Some((name, rest)) = header.split_once('(') else {
        let name = header.trim();
        if !is_identifier(name) {
            return Err(parse_diagnostic_line(
                line_number,
                format!("invalid action name `{name}`"),
            ));
        }
        return Ok((name.to_string(), None));
    };

    let name = name.trim();
    if !is_identifier(name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid action name `{name}`"),
        ));
    }

    let rest = rest
        .strip_suffix(')')
        .ok_or_else(|| {
            parse_diagnostic_line(
                line_number,
                "@action expects `@action name(input: SchemaName) {`",
            )
        })?
        .trim();
    let (param_name, schema_name) = rest.split_once(':').ok_or_else(|| {
        parse_diagnostic_line(
            line_number,
            "@action expects `@action name(input: SchemaName) {`",
        )
    })?;
    if param_name.trim() != "input" {
        return Err(parse_diagnostic_line(
            line_number,
            "@action input parameter must be named `input`",
        ));
    }
    let schema_name = schema_name.trim();
    if !is_identifier(schema_name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid action input schema `{schema_name}`"),
        ));
    }

    Ok((name.to_string(), Some(schema_name.to_string())))
}

fn parse_do(lines: &[&str], cursor: &mut usize) -> Result<TemplateNode, Diagnostic> {
    let line_number = *cursor + 1;
    let trimmed = lines[*cursor].trim();
    if trimmed != "@do {" {
        return Err(parse_diagnostic_line(line_number, "@do expects `@do {`"));
    }
    *cursor += 1;
    let statements =
        stmt::parse_server_block(lines, cursor, line_number, stmt::BlockMode::SyncOnly)?;
    if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
        return Err(parse_diagnostic_line(line_number, "unclosed @do block"));
    }
    *cursor += 1;
    Ok(TemplateNode::Do {
        statements,
        line: line_number,
    })
}

fn parse_prop_decl(line: &str, line_number: usize) -> Result<PropDecl, Diagnostic> {
    let (left, default_value) = match split_once_unquoted(line, '=') {
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

fn collect_balanced_line(
    lines: &[&str],
    cursor: &mut usize,
    first_trimmed: &str,
) -> Result<String, Diagnostic> {
    let start_line = *cursor + 1;
    let mut collected = first_trimmed.to_string();
    *cursor += 1;

    while !delimiters_balanced(&collected)? {
        if *cursor >= lines.len() {
            return Err(parse_diagnostic_line(
                start_line,
                "unclosed literal in @let declaration",
            ));
        }
        collected.push('\n');
        collected.push_str(lines[*cursor].trim());
        *cursor += 1;
    }

    Ok(collected)
}

fn collect_component_call_line(lines: &[&str], cursor: &mut usize) -> Result<String, Diagnostic> {
    let start_line = *cursor + 1;
    let mut collected = lines[*cursor].trim().to_string();
    *cursor += 1;

    while !collected.trim_end().ends_with("/>") {
        if *cursor >= lines.len() {
            return Err(parse_diagnostic_line(start_line, "unclosed component call"));
        }
        collected.push(' ');
        collected.push_str(lines[*cursor].trim());
        *cursor += 1;
    }

    Ok(collected)
}

fn delimiters_balanced(value: &str) -> Result<bool, Diagnostic> {
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;

    for char in value.chars() {
        if in_string {
            if char == '"' && !escaped {
                in_string = false;
            }
            escaped = char == '\\' && !escaped;
            if char != '\\' {
                escaped = false;
            }
            continue;
        }

        match char {
            '"' => in_string = true,
            '{' | '[' | '(' => stack.push(char),
            '}' => {
                if stack.pop() != Some('{') {
                    return Ok(true);
                }
            }
            ']' => {
                if stack.pop() != Some('[') {
                    return Ok(true);
                }
            }
            ')' => {
                if stack.pop() != Some('(') {
                    return Ok(true);
                }
            }
            _ => {}
        }
    }

    Ok(!in_string && stack.is_empty())
}

fn parse_component_call_line(
    raw_line: &str,
    line_number: usize,
) -> Result<Option<ComponentCall>, Diagnostic> {
    let trimmed = raw_line.trim();
    if !trimmed.starts_with('<') || !trimmed.ends_with("/>") {
        return Ok(None);
    }

    let (line_without_events, event_bindings) =
        extract_event_bindings(trimmed, line_number, false)?;

    let inner = line_without_events[1..line_without_events.len() - 2].trim();
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
        line_without_events
            .find(rest)
            .map(|index| index + 1)
            .unwrap_or(column)
    };
    let (props, class_expr) = parse_component_props(rest, line_number, rest_column)?;
    Ok(Some(ComponentCall {
        name: name.to_string(),
        props,
        event_bindings,
        class_expr,
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

fn starts_component_call(trimmed: &str) -> bool {
    if !trimmed.starts_with('<') {
        return false;
    }
    let inner = trimmed[1..].trim_start();
    let Some((name, _)) = split_tag_name(inner) else {
        return false;
    };
    is_component_name(name)
}

fn parse_component_props(
    value: &str,
    line_number: usize,
    start_column: usize,
) -> Result<(Vec<ComponentProp>, Option<SourceExpr>), Diagnostic> {
    let mut props = Vec::new();
    let mut class_expr = None;
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
        if name == "class" {
            class_expr = Some(prop_value_to_source_expr(
                &value,
                line_number,
                value_start_col,
            )?);
        } else {
            props.push(ComponentProp {
                name: name.to_string(),
                value,
                line: line_number,
                column: name_column,
                value_start_col,
                value_end_col: value_start_col + consumed,
            });
        }
        let next = &after_eq[consumed..];
        let skipped = next.len() - next.trim_start().len();
        rest_column +=
            value_start + (rest[value_start..].len() - after_eq.len()) + consumed + skipped;
        rest = next.trim_start();
    }

    Ok((props, class_expr))
}

fn prop_value_to_source_expr(
    value: &PropValue,
    line_number: usize,
    column: usize,
) -> Result<SourceExpr, Diagnostic> {
    match value {
        PropValue::Literal(literal) => Ok(SourceExpr {
            source: literal.render(),
            expr: expr::Expr::Literal(literal.clone()),
            line: line_number,
            column,
        }),
        PropValue::Expr(expr) => Ok(expr.clone()),
    }
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
        let expr_col = value_start_col
            + value[..end + 1]
                .find(inner)
                .map(|index| index + 1)
                .unwrap_or(1);
        let expr = expr::parse(inner, line_number, expr_col)?;
        return Ok((
            PropValue::Expr(SourceExpr {
                source: inner.to_string(),
                expr,
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
    trimmed == "}"
        || trimmed.starts_with("} @else")
        || trimmed == "} @placeholder {"
        || trimmed.starts_with("} @error ")
}

fn is_defer_statement_line(trimmed: &str) -> bool {
    if trimmed.is_empty() || trimmed == "} @placeholder {" {
        return false;
    }
    trimmed.starts_with("fn ")
        || trimmed.starts_with("if ")
        || trimmed.starts_with("while ")
        || trimmed == "try {"
        || trimmed.starts_with("return")
        || trimmed.starts_with("throw(")
        || trimmed.starts_with("throw (")
        || trimmed.starts_with("fail(")
        || trimmed.starts_with("redirect(")
        || trimmed.starts_with("session.")
        || trimmed.contains(":=")
        || (trimmed.contains('=')
            && !trimmed.starts_with('<')
            && !trimmed.starts_with('@')
            && !trimmed.starts_with('{'))
}

fn parse_defer_body(
    lines: &[&str],
    cursor: &mut usize,
    block_line: usize,
    lets: &mut Vec<LetDecl>,
) -> Result<(Vec<Statement>, Vec<TemplateNode>), Diagnostic> {
    let mut prelude = Vec::new();
    let mut body = Vec::new();
    let mut template_mode = false;

    while *cursor < lines.len() {
        let trimmed = lines[*cursor].trim();
        if trimmed == "} @placeholder {" {
            break;
        }

        if !template_mode && is_defer_statement_line(trimmed) {
            prelude.push(stmt::parse_one_statement(
                lines,
                cursor,
                stmt::BlockMode::AsyncCapable,
            )?);
            continue;
        }

        template_mode = true;
        body = parse_nodes(lines, cursor, true, lets)?;
        break;
    }

    if *cursor >= lines.len() || lines[*cursor].trim() != "} @placeholder {" {
        return Err(parse_diagnostic_line(
            block_line,
            "@defer expects `} @placeholder {` after the deferred body",
        ));
    }
    *cursor += 1;
    Ok((prelude, body))
}

fn parse_defer(
    lines: &[&str],
    cursor: &mut usize,
    lets: &mut Vec<LetDecl>,
) -> Result<TemplateNode, Diagnostic> {
    let line_number = *cursor + 1;
    let trimmed = lines[*cursor].trim();
    if trimmed != "@defer {" {
        return Err(parse_diagnostic_line(line_number, "@defer expects `@defer {`"));
    }
    *cursor += 1;

    let (prelude, body) = parse_defer_body(lines, cursor, line_number, lets)?;
    let placeholder = parse_nodes(lines, cursor, true, lets)?;

    if *cursor >= lines.len() {
        return Err(parse_diagnostic_line(line_number, "unclosed @placeholder block"));
    }

    let close_line = lines[*cursor].trim();
    let mut error_name = None;
    let mut error_body = Vec::new();

    if close_line == "}" {
        *cursor += 1;
        if *cursor < lines.len() {
            let next = lines[*cursor].trim();
            if let Some(name) = next.strip_prefix("@error ") {
                let rest = name.trim().strip_suffix('{').ok_or_else(|| {
                    parse_diagnostic_line(*cursor + 1, "@error expects `@error name {`")
                })?;
                let name = rest.trim();
                if !is_identifier(name) {
                    return Err(parse_diagnostic_line(
                        *cursor + 1,
                        format!("invalid @error binding `{name}`"),
                    ));
                }
                error_name = Some(name.to_string());
                *cursor += 1;
                error_body = parse_nodes(lines, cursor, true, lets)?;
                if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
                    return Err(parse_diagnostic_line(*cursor + 1, "unclosed @error block"));
                }
                *cursor += 1;
            }
        }
    } else if let Some(rest) = close_line.strip_prefix("} @error ") {
        let name_part = rest.trim().strip_suffix('{').ok_or_else(|| {
            parse_diagnostic_line(*cursor + 1, "@error expects `} @error name {`")
        })?;
        let name = name_part.trim();
        if !is_identifier(name) {
            return Err(parse_diagnostic_line(
                *cursor + 1,
                format!("invalid @error binding `{name}`"),
            ));
        }
        error_name = Some(name.to_string());
        *cursor += 1;
        error_body = parse_nodes(lines, cursor, true, lets)?;
        if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
            return Err(parse_diagnostic_line(*cursor + 1, "unclosed @error block"));
        }
        *cursor += 1;
    } else {
        return Err(parse_diagnostic_line(*cursor + 1, "expected `}`"));
    }

    if placeholder.is_empty() {
        return Err(parse_diagnostic_line(
            line_number,
            "@defer requires a non-empty @placeholder block",
        ));
    }

    Ok(TemplateNode::Defer {
        prelude,
        body,
        placeholder,
        error_name,
        error_body,
        line: line_number,
    })
}

fn parse_for(
    lines: &[&str],
    cursor: &mut usize,
    lets: &mut Vec<LetDecl>,
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
    let (item_name, source_text) = header
        .split_once(" in ")
        .ok_or_else(|| parse_diagnostic_line(line_number, "@for expects `@for item in items {`"))?;
    let item_name = item_name.trim();
    let source_text = source_text.trim();

    if !is_identifier(item_name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid loop variable `{item_name}`"),
        ));
    }
    let source_column = raw_line
        .find(source_text)
        .map(|index| index + 1)
        .unwrap_or(1);
    let source_expr = expr::parse(source_text, line_number, source_column)?;

    *cursor += 1;
    let body = parse_nodes(lines, cursor, true, lets)?;

    if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
        return Err(parse_diagnostic_line(line_number, "unclosed @for block"));
    }
    *cursor += 1;

    Ok(TemplateNode::For {
        item_name: item_name.to_string(),
        source: SourceExpr {
            source: source_text.to_string(),
            expr: source_expr,
            line: line_number,
            column: source_column,
        },
        body,
    })
}

fn parse_if(
    lines: &[&str],
    cursor: &mut usize,
    lets: &mut Vec<LetDecl>,
) -> Result<TemplateNode, Diagnostic> {
    let raw_line = lines[*cursor];
    let line_number = *cursor + 1;
    let trimmed = raw_line.trim();

    let condition_source = trimmed
        .strip_prefix("@if")
        .expect("@if prefix already checked")
        .trim()
        .strip_suffix('{')
        .ok_or_else(|| parse_diagnostic_line(line_number, "@if expects `@if condition {`"))?
        .trim();

    let column = raw_line
        .find(condition_source)
        .map(|index| index + 1)
        .unwrap_or(1);
    let condition_expr = expr::parse(condition_source, line_number, column)?;

    *cursor += 1;
    let then_nodes = parse_nodes(lines, cursor, true, lets)?;

    if *cursor >= lines.len() {
        return Err(parse_diagnostic_line(line_number, "unclosed @if block"));
    }

    let close_line = lines[*cursor].trim();
    let mut else_nodes = Vec::new();

    if close_line == "}" {
        *cursor += 1;
        if *cursor < lines.len() && lines[*cursor].trim().starts_with("@else if ") {
            else_nodes.push(parse_else_if(lines, cursor, lets)?);
        } else if *cursor < lines.len() && lines[*cursor].trim() == "@else {" {
            *cursor += 1;
            else_nodes = parse_nodes(lines, cursor, true, lets)?;
            if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
                return Err(parse_diagnostic_line(*cursor + 1, "unclosed @else block"));
            }
            *cursor += 1;
        }
    } else if close_line.starts_with("} @else if ") {
        else_nodes.push(parse_inline_else_if(lines, cursor, lets)?);
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
            source: condition_source.to_string(),
            expr: condition_expr,
            line: line_number,
            column,
        },
        then_nodes,
        else_nodes,
    })
}

fn parse_else_if(
    lines: &[&str],
    cursor: &mut usize,
    lets: &mut Vec<LetDecl>,
) -> Result<TemplateNode, Diagnostic> {
    let raw_line = lines[*cursor];
    let line_number = *cursor + 1;
    let trimmed = raw_line.trim();
    let condition_source = trimmed
        .strip_prefix("@else if")
        .expect("@else if prefix checked")
        .trim()
        .strip_suffix('{')
        .ok_or_else(|| {
            parse_diagnostic_line(line_number, "@else if expects `@else if condition {`")
        })?
        .trim();

    parse_if_branch(lines, cursor, lets, raw_line, line_number, condition_source)
}

fn parse_inline_else_if(
    lines: &[&str],
    cursor: &mut usize,
    lets: &mut Vec<LetDecl>,
) -> Result<TemplateNode, Diagnostic> {
    let raw_line = lines[*cursor];
    let line_number = *cursor + 1;
    let trimmed = raw_line.trim();
    let condition_source = trimmed
        .strip_prefix("} @else if")
        .expect("} @else if prefix checked")
        .trim()
        .strip_suffix('{')
        .ok_or_else(|| {
            parse_diagnostic_line(line_number, "@else if expects `} @else if condition {`")
        })?
        .trim();

    parse_if_branch(lines, cursor, lets, raw_line, line_number, condition_source)
}

fn parse_if_branch(
    lines: &[&str],
    cursor: &mut usize,
    lets: &mut Vec<LetDecl>,
    raw_line: &str,
    line_number: usize,
    condition_source: &str,
) -> Result<TemplateNode, Diagnostic> {
    let column = raw_line
        .find(condition_source)
        .map(|index| index + 1)
        .unwrap_or(1);
    let condition_expr = expr::parse(condition_source, line_number, column)?;

    *cursor += 1;
    let then_nodes = parse_nodes(lines, cursor, true, lets)?;

    if *cursor >= lines.len() {
        return Err(parse_diagnostic_line(
            line_number,
            "unclosed @else if block",
        ));
    }

    let close_line = lines[*cursor].trim();
    let mut else_nodes = Vec::new();

    if close_line == "}" {
        *cursor += 1;
        if *cursor < lines.len() && lines[*cursor].trim().starts_with("@else if ") {
            else_nodes.push(parse_else_if(lines, cursor, lets)?);
        } else if *cursor < lines.len() && lines[*cursor].trim() == "@else {" {
            *cursor += 1;
            else_nodes = parse_nodes(lines, cursor, true, lets)?;
            if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
                return Err(parse_diagnostic_line(*cursor + 1, "unclosed @else block"));
            }
            *cursor += 1;
        }
    } else if close_line.starts_with("} @else if ") {
        else_nodes.push(parse_inline_else_if(lines, cursor, lets)?);
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
            source: condition_source.to_string(),
            expr: condition_expr,
            line: line_number,
            column,
        },
        then_nodes,
        else_nodes,
    })
}

fn parse_switch(
    lines: &[&str],
    cursor: &mut usize,
    lets: &mut Vec<LetDecl>,
) -> Result<TemplateNode, Diagnostic> {
    let raw_line = lines[*cursor];
    let line_number = *cursor + 1;
    let trimmed = raw_line.trim();
    let value_source = trimmed
        .strip_prefix("@switch")
        .expect("@switch prefix checked")
        .trim()
        .strip_suffix('{')
        .ok_or_else(|| parse_diagnostic_line(line_number, "@switch expects `@switch expr {`"))?
        .trim();
    let column = raw_line
        .find(value_source)
        .map(|index| index + 1)
        .unwrap_or(1);
    let value_expr = expr::parse(value_source, line_number, column)?;

    *cursor += 1;
    let mut cases = Vec::new();
    let mut default_nodes = Vec::new();

    while *cursor < lines.len() {
        let branch_line = lines[*cursor].trim();
        if branch_line == "}" {
            *cursor += 1;
            return Ok(TemplateNode::Switch {
                value: SourceExpr {
                    source: value_source.to_string(),
                    expr: value_expr,
                    line: line_number,
                    column,
                },
                cases,
                default_nodes,
            });
        }
        if branch_line.is_empty() {
            *cursor += 1;
            continue;
        }
        if branch_line.starts_with("@case ") {
            cases.push(parse_switch_case(lines, cursor, lets)?);
            continue;
        }
        if branch_line == "@default {" {
            if !default_nodes.is_empty() {
                return Err(parse_diagnostic_line(
                    *cursor + 1,
                    "duplicate @default block",
                ));
            }
            *cursor += 1;
            default_nodes = parse_nodes(lines, cursor, true, lets)?;
            if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
                return Err(parse_diagnostic_line(
                    *cursor + 1,
                    "unclosed @default block",
                ));
            }
            *cursor += 1;
            continue;
        }
        return Err(parse_diagnostic_line(
            *cursor + 1,
            "@switch expects @case, @default, or }",
        ));
    }

    Err(parse_diagnostic_line(line_number, "unclosed @switch block"))
}

fn parse_switch_case(
    lines: &[&str],
    cursor: &mut usize,
    lets: &mut Vec<LetDecl>,
) -> Result<SwitchCase, Diagnostic> {
    let raw_line = lines[*cursor];
    let line_number = *cursor + 1;
    let trimmed = raw_line.trim();
    let value_source = trimmed
        .strip_prefix("@case")
        .expect("@case prefix checked")
        .trim()
        .strip_suffix('{')
        .ok_or_else(|| parse_diagnostic_line(line_number, "@case expects `@case expr {`"))?
        .trim();
    let column = raw_line
        .find(value_source)
        .map(|index| index + 1)
        .unwrap_or(1);
    let value_expr = expr::parse(value_source, line_number, column)?;

    *cursor += 1;
    let nodes = parse_nodes(lines, cursor, true, lets)?;
    if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
        return Err(parse_diagnostic_line(line_number, "unclosed @case block"));
    }
    *cursor += 1;

    Ok(SwitchCase {
        value: SourceExpr {
            source: value_source.to_string(),
            expr: value_expr,
            line: line_number,
            column,
        },
        nodes,
    })
}

const CLIENT_EVENTS: &[&str] = &[
    "click", "input", "change", "submit", "keydown", "keyup", "focus", "blur",
];

fn parse_event_directive(rest: &str) -> Option<(String, bool, bool, usize)> {
    let after_at = rest.strip_prefix('@')?;
    for event in CLIENT_EVENTS {
        let Some(suffix) = after_at.strip_prefix(event) else {
            continue;
        };
        let mut prevent_default = false;
        let mut stop_propagation = false;
        let mut remainder = suffix;
        loop {
            if let Some(next) = remainder.strip_prefix(".prevent") {
                prevent_default = true;
                remainder = next;
            } else if let Some(next) = remainder.strip_prefix(".stop") {
                stop_propagation = true;
                remainder = next;
            } else {
                break;
            }
        }
        if remainder.starts_with('=') {
            let consumed = rest.len() - remainder.len();
            return Some((
                event.to_string(),
                prevent_default,
                stop_propagation,
                consumed,
            ));
        }
    }
    None
}

fn extract_event_bindings(
    line: &str,
    line_number: usize,
    emit_markers: bool,
) -> Result<(String, Vec<EventBinding>), Diagnostic> {
    let mut bindings = Vec::new();
    let mut output = String::new();
    let mut offset = 0;

    while offset < line.len() {
        let Some(relative) = line[offset..]
            .find('@')
            .filter(|index| parse_event_directive(&line[offset + index..]).is_some())
        else {
            output.push_str(&line[offset..]);
            break;
        };

        let start = offset + relative;
        output.push_str(&line[offset..start]);

        let rest = &line[start..];
        let (event, prevent_default, stop_propagation, directive_len) =
            parse_event_directive(rest).expect("matched client event prefix");

        let mut brace_start = directive_len;
        if rest.as_bytes().get(brace_start) == Some(&b'=') {
            brace_start += 1;
        }
        if rest.len() <= brace_start || rest.as_bytes()[brace_start] != b'{' {
            return Err(parse_diagnostic(
                line_number,
                start + 1,
                start + rest.len().min(start + 20),
                format!("@{event} expects `@{event}={{handler}}`"),
            ));
        }

        let handler_start = brace_start + 1;
        let mut depth = 1usize;
        let mut handler_end = None;
        for (index, ch) in rest[handler_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        handler_end = Some(handler_start + index);
                        break;
                    }
                }
                _ => {}
            }
        }

        let Some(handler_end) = handler_end else {
            return Err(parse_diagnostic(
                line_number,
                start + 1,
                line.len(),
                format!("unclosed `@{event}` handler"),
            ));
        };

        let handler_source = rest[handler_start..handler_end].trim().to_string();
        bindings.push(EventBinding {
            event,
            handler_source,
            line: line_number,
            column: start + 1,
            prevent_default,
            stop_propagation,
        });

        if emit_markers {
            let event_name = bindings.last().expect("binding").event.clone();
            output.push_str(&format!(" data-ws-{event_name}"));
        }
        offset = start + handler_end + 1;
    }

    Ok((output, bindings))
}

fn is_slot_tag(value: &str) -> bool {
    matches!(value.trim(), "<slot />" | "<slot/>")
}

fn parse_text_line(line: &str, line_number: usize) -> Result<Vec<TemplateNode>, Diagnostic> {
    let (line, event_bindings) = extract_event_bindings(line, line_number, true)?;

    let mut nodes = Vec::new();
    let mut offset = 0;

    while offset < line.len() {
        let Some(slot_start) = line[offset..].find("<slot").filter(|index| {
            let rest = &line[offset + index..];
            rest.starts_with("<slot />") || rest.starts_with("<slot/>")
        }) else {
            break;
        };

        let absolute_start = offset + slot_start;
        if absolute_start > offset {
            nodes.push(TemplateNode::Text(line[offset..absolute_start].to_string()));
        }

        let slot_len = if line[absolute_start..].starts_with("<slot />") {
            8
        } else {
            7
        };
        nodes.push(TemplateNode::Slot);
        offset = absolute_start + slot_len;
    }

    if offset < line.len() {
        let remainder = &line[offset..];
        offset = line.len();
        nodes.extend(parse_text_expressions(remainder, line_number)?);
    } else if nodes.is_empty() {
        nodes.extend(parse_text_expressions(&line, line_number)?);
    }

    for binding in event_bindings {
        nodes.push(TemplateNode::EventBinding(binding));
    }

    Ok(nodes)
}

fn parse_text_expressions(line: &str, line_number: usize) -> Result<Vec<TemplateNode>, Diagnostic> {
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
            return Ok(nodes);
        };

        let expr_end = expr_start + end;
        let source = line[expr_start..expr_end].trim();
        let column = line[expr_start..expr_end]
            .find(source)
            .map(|inner| expr_start + inner + 1)
            .unwrap_or(expr_start + 1);

        if source.is_empty() {
            nodes.push(TemplateNode::Text(
                line[absolute_start..=expr_end].to_string(),
            ));
        } else if let Ok(expr) = expr::parse(source, line_number, column) {
            nodes.push(TemplateNode::Expr(SourceExpr {
                source: source.to_string(),
                expr,
                line: line_number,
                column,
            }));
        } else {
            nodes.push(TemplateNode::Text(
                line[absolute_start..=expr_end].to_string(),
            ));
        }

        offset = expr_end + 1;
    }

    if offset < line.len() {
        nodes.push(TemplateNode::Text(line[offset..].to_string()));
    }

    Ok(nodes)
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

fn parse_let(line: &str, line_number: usize) -> Result<LetDecl, Diagnostic> {
    let rest = line
        .strip_prefix("@let")
        .expect("@let prefix already checked")
        .trim();
    let (left, right) = rest
        .split_once('=')
        .ok_or_else(|| parse_diagnostic_line(line_number, "@let expects `name = value`"))?;
    let (name, type_name) = match left.split_once(':') {
        Some((name, type_name)) => (name.trim(), Some(type_name.trim())),
        None => (left.trim(), None),
    };

    if !is_identifier(name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid identifier `{name}`"),
        ));
    }

    let value_text = right.trim();
    let value_col = line.find(value_text).map(|index| index + 1).unwrap_or(1);
    let value = match type_name {
        Some(type_name) if type_name.ends_with("[]") => LetValue::Static(parse_value(
            type_name,
            value_text,
            line_number,
            value_col,
            value_col + value_text.len(),
        )?),
        Some(_) | None => LetValue::Expr(expr::parse(value_text, line_number, value_col)?),
    };
    Ok(LetDecl {
        name: name.to_string(),
        type_name: type_name.map(str::to_string),
        value,
        line: line_number,
        value_start_col: value_col,
        value_end_col: value_col + value_text.len(),
    })
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

    if types::is_string_literal_type(type_name) {
        let parsed = parse_quoted(value).map(Value::String).ok_or_else(|| {
            parse_diagnostic(
                line_number,
                start_col,
                end_col,
                "string literal type values must be quoted",
            )
        })?;
        if types::value_matches_type(&parsed, type_name) {
            return Ok(parsed);
        }
        return Err(parse_diagnostic(
            line_number,
            start_col,
            end_col,
            format!("expected `{type_name}`, found `{}`", parsed.type_name()),
        ));
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
        "object" => match expr::parse(value, line_number, start_col)? {
            expr::Expr::Literal(Value::Object(fields)) => Ok(Value::Object(fields)),
            other => Err(parse_diagnostic(
                line_number,
                start_col,
                end_col,
                format!("object values must be object literals, found `{other:?}`"),
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

fn split_once_unquoted(value: &str, needle: char) -> Option<(&str, &str)> {
    let mut in_string = false;
    let mut escaped = false;
    for (index, char) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match char {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            _ if char == needle && !in_string => {
                return Some((&value[..index], &value[index + char.len_utf8()..]));
            }
            _ => {}
        }
    }
    None
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
    let mut escaped = false;
    let mut depth = 0usize;

    for (index, char) in value.char_indices() {
        if in_string {
            if char == '"' && !escaped {
                in_string = false;
            }
            escaped = char == '\\' && !escaped;
            if char != '\\' {
                escaped = false;
            }
            continue;
        }

        match char {
            '"' => in_string = true,
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
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
    !value.is_empty()
        && value.split('.').all(|segment| {
            segment
                .chars()
                .next()
                .is_some_and(|char| char.is_ascii_uppercase())
                && is_identifier(segment)
        })
}

#[cfg(test)]
mod tests {
    use super::{parse, LetValue, PropValue, TemplateNode, Value};
    use crate::expr;

    #[test]
    fn parses_page_lets_and_template_ast() {
        let parsed = parse(
            "@page \"/\"\n\n@let name: string = \"Ada\"\n@let visits: int = 3\n\n<h1>{name}</h1>",
        )
        .expect("valid page");

        assert_eq!(parsed.route.as_ref().expect("route").raw, "/");
        assert_eq!(parsed.lets[0].name, "name");
        assert_eq!(parsed.lets[0].type_name.as_deref(), Some("string"));
        assert_eq!(parsed.lets[1].name, "visits");
        assert_eq!(parsed.lets[1].type_name.as_deref(), Some("int"));
        assert!(matches!(parsed.template[1], TemplateNode::Expr(_)));
    }

    #[test]
    fn parses_typed_and_inferred_let_expressions() {
        let parsed = parse(
            "@page \"/\"\n\n@let name = \"Ada\"\n@let visits: int = 2 + 3\n@let label = \"Hello \" + name\n\n<h1>{label}</h1>",
        )
        .expect("valid page");

        assert_eq!(parsed.lets[0].name, "name");
        assert_eq!(parsed.lets[0].type_name, None);
        assert_eq!(parsed.lets[1].name, "visits");
        assert_eq!(parsed.lets[1].type_name.as_deref(), Some("int"));
        assert_eq!(parsed.lets[2].name, "label");
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
            "@page \"/\"\n\n@let visits: int = 3\n\n@if visits > 1 {\n<p>yes</p>\n} @else {\n<p>no</p>\n}",
        )
        .expect("valid page");

        assert!(matches!(parsed.template[0], TemplateNode::If { .. }));
    }

    #[test]
    fn parses_else_if_chain() {
        let parsed = parse(
            "@page \"/\"\n\n@let status = \"published\"\n\n@if status == \"draft\" {\n<p>draft</p>\n} @else if status == \"published\" {\n<p>published</p>\n} @else {\n<p>other</p>\n}",
        )
        .expect("valid page");

        let TemplateNode::If { else_nodes, .. } = &parsed.template[0] else {
            panic!("expected root if");
        };
        assert!(matches!(else_nodes.first(), Some(TemplateNode::If { .. })));
    }

    #[test]
    fn parses_switch_cases_and_default() {
        let parsed = parse(
            "@page \"/\"\n\n@let size = \"icon\"\n\n@switch size {\n  @case \"icon\" {\n    <span>icon</span>\n  }\n  @case \"lg\" {\n    <span>large</span>\n  }\n  @default {\n    <span>default</span>\n  }\n}",
        )
        .expect("valid page");

        let TemplateNode::Switch {
            cases,
            default_nodes,
            ..
        } = &parsed.template[0]
        else {
            panic!("expected switch");
        };
        assert_eq!(cases.len(), 2);
        assert!(!default_nodes.is_empty());
    }

    #[test]
    fn parses_arrays_and_for_blocks() {
        let parsed = parse(
            "@page \"/\"\n\n@let posts: string[] = [\"One\", \"Two\", \"Three\"]\n\n@for post in posts {\n<p>{post}</p>\n}",
        )
        .expect("valid page");

        assert!(matches!(
            &parsed.lets[0].value,
            LetValue::Static(Value::Array { values, .. }) if values.len() == 3
        ));
        assert!(
            matches!(&parsed.template[0], TemplateNode::For { item_name, .. } if item_name == "post")
        );
    }

    #[test]
    fn parses_object_literals_and_arrays_of_objects() {
        let parsed = parse(
            "@page \"/\"\n\n@let author = {\n  name: \"Ada\"\n  role: \"admin\"\n}\n@let posts = [\n  { title: \"Intro\", slug: \"intro\", featured: true },\n  { title: \"Launch\", slug: \"launch\", featured: false }\n]\n\n<h1>{author.name}</h1>",
        )
        .expect("valid page");

        assert!(matches!(
            &parsed.lets[0].value,
            LetValue::Expr(expr::Expr::Literal(Value::Object(fields)))
                if matches!(fields.get("name"), Some(Value::String(name)) if name == "Ada")
        ));
        assert!(matches!(
            &parsed.lets[1].value,
            LetValue::Expr(expr::Expr::Literal(Value::Array { element_type, values }))
                if element_type == "object" && values.len() == 2
        ));
    }

    #[test]
    fn parses_load_blocks() {
        let parsed = parse(
            "@page \"/fetch-demo\"\n\n@load {\n  status: int = 0\n  error: string = \"\"\n}\n\n<p>{status}</p>",
        )
        .expect("valid page");

        assert!(parsed.load.is_some());
        assert_eq!(parsed.load.as_ref().expect("load").statements.len(), 2);
    }

    #[test]
    fn parses_action_blocks() {
        let parsed = parse(
            "@page \"/\"\n\n@action increment {\n  if input.name == \"\" {\n    fail(\"Name is required\")\n  }\n  session.count = session.count + 1\n  redirect(\"/\")\n}\n\n<p>{session.count}</p>",
        )
        .expect("valid page");

        assert_eq!(parsed.actions[0].name, "increment");
        assert_eq!(parsed.actions[0].input_schema, None);
        assert_eq!(parsed.actions[0].statements.len(), 3);
    }

    #[test]
    fn parses_action_input_schema() {
        let parsed = parse(
            "@page \"/\"\n\n@action rememberName(input: ProfileInput) {\n  redirect(\"/\")\n}\n\n<p>ok</p>",
        )
        .expect("valid page");

        assert_eq!(parsed.actions[0].name, "rememberName");
        assert_eq!(
            parsed.actions[0].input_schema.as_deref(),
            Some("ProfileInput")
        );
    }

    #[test]
    fn rejects_missing_page() {
        let error = parse("<h1>No route</h1>").expect_err("missing page should fail");
        assert_eq!(
            error.message,
            "missing @page, @component, or @layout directive"
        );
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
    fn parses_string_literal_union_prop_defaults() {
        let component = parse(
            "@component Button {\n  variant: \"primary\" | \"secondary\" = \"primary\"\n  kind: \"icon\" = \"icon\"\n}\n\n<button>{variant}</button>",
        )
        .expect("valid component");

        let declaration = component.component.as_ref().expect("component");
        assert_eq!(declaration.props[0].type_name, r#""primary" | "secondary""#);
        assert!(matches!(
            declaration.props[0].default,
            Some(Value::String(ref value)) if value == "primary"
        ));
        assert_eq!(declaration.props[1].type_name, r#""icon""#);
    }

    #[test]
    fn rejects_string_literal_union_prop_default_mismatch() {
        let error = parse(
            "@component Button {\n  variant: \"primary\" | \"secondary\" = \"ghost\"\n}\n\n<button>{variant}</button>",
        )
        .expect_err("invalid literal default should fail");

        assert!(error
            .message
            .contains("expected `\"primary\" | \"secondary\"`"));
    }

    #[test]
    fn parses_let_after_template_start_without_rendering_directive_text() {
        let parsed = parse("@page \"/\"\n\n@let name: string = \"Ada\"\n<h1>{name}</h1>\n@let greeting: string = \"Hello\"\n<p>{greeting}</p>")
            .expect("valid page");

        assert!(parsed
            .lets
            .iter()
            .any(|let_decl| let_decl.name == "greeting"));
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
    fn parses_defer_with_placeholder() {
        let parsed = parse(
            "@page \"/\"\n\n@defer {\n  <SlowPanel />\n} @placeholder {\n  <Skeleton />\n}",
        )
        .expect("valid defer");

        let TemplateNode::Defer {
            prelude,
            body,
            placeholder,
            error_name,
            error_body,
            ..
        } = &parsed.template[0]
        else {
            panic!("expected defer node");
        };
        assert!(prelude.is_empty());
        assert!(body.iter().any(|node| matches!(node, TemplateNode::Text(t) if t.contains("SlowPanel"))));
        assert!(placeholder.iter().any(|node| matches!(node, TemplateNode::Text(t) if t.contains("Skeleton"))));
        assert!(error_name.is_none());
        assert!(error_body.is_empty());
    }

    #[test]
    fn parses_defer_with_prelude_and_error() {
        let parsed = parse(
            "@page \"/\"\n\n@defer {\n  count: int = 0\n  <p>{count}</p>\n} @placeholder {\n  <p>Loading...</p>\n} @error err {\n  <p>{err.message}</p>\n}",
        )
        .expect("valid defer with prelude");

        let TemplateNode::Defer {
            prelude,
            error_name,
            ..
        } = &parsed.template[0]
        else {
            panic!("expected defer node");
        };
        assert_eq!(prelude.len(), 1);
        assert_eq!(error_name.as_deref(), Some("err"));
    }

    #[test]
    fn rejects_defer_without_placeholder() {
        let error = parse("@page \"/\"\n\n@defer {\n  <p>Hi</p>\n}")
            .expect_err("defer without placeholder should fail");
        assert!(error.message.contains("@placeholder"));
    }

    #[test]
    fn parse_event_directive_finds_brace_after_equals() {
        let (_, _, _, directive_len) =
            super::parse_event_directive("@click={count++}").expect("click directive");
        let rest = "@click={count++}";
        assert_eq!(directive_len, 6);
        assert_eq!(rest.as_bytes()[directive_len + 1], b'{');
    }

    #[test]
    fn parse_event_directive_supports_prevent_modifier() {
        let (event, prevent, stop, _) =
            super::parse_event_directive("@submit.prevent={save}").expect("submit directive");
        assert_eq!(event, "submit");
        assert!(prevent);
        assert!(!stop);
    }

    #[test]
    fn parses_client_block_and_click_binding() {
        let component = parse(
            "@component Counter {\n  initial: int = 0\n}\n\n@client {\n  count: signal<int> = initial\n}\n\n<button @click={count++}>\n  {count}\n</button>",
        )
        .expect("valid component");

        let client = component.client.as_ref().expect("client block");
        assert_eq!(client.signals[0].name, "count");
        assert_eq!(client.signals[0].type_name, "int");
        assert!(matches!(
            client.signals[0].initial,
            super::ClientInitial::PropRef(ref name) if name == "initial"
        ));

        assert!(component.template.iter().any(|node| matches!(
            node,
            TemplateNode::Text(value) if value.contains("data-ws-click")
        )));
        assert!(component
            .template
            .iter()
            .any(|node| matches!(node, TemplateNode::EventBinding(binding) if binding.handler_source == "count++")));
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

    #[test]
    fn parses_scoped_style_after_markup() {
        let parsed = parse(
            "@page \"/\"\n\n<div class=\"card\">Hi</div>\n\n@style {\n  .card { color: red; }\n}",
        )
        .expect("valid page");

        assert_eq!(parsed.styles.len(), 1);
        assert!(!parsed.styles[0].global);
        assert!(parsed.styles[0].css.contains(".card"));
        assert!(parsed.styles[0].css.contains("color: red"));
    }

    #[test]
    fn parses_global_style_after_markup() {
        let parsed =
            parse("@page \"/\"\n\n<main></main>\n\n@style global {\n  body { margin: 0; }\n}")
                .expect("valid page");

        assert_eq!(parsed.styles.len(), 1);
        assert!(parsed.styles[0].global);
        assert!(parsed.styles[0].css.contains("body"));
    }

    #[test]
    fn parses_style_with_nested_braces() {
        let parsed = parse(
            "@component Card {}\n\n<div></div>\n\n@style {\n  @media (min-width: 600px) {\n    .card { color: blue; }\n  }\n}",
        )
        .expect("valid component");

        assert_eq!(parsed.styles.len(), 1);
        assert!(parsed.styles[0].css.contains("@media"));
        assert!(parsed.styles[0].css.contains(".card"));
    }

    #[test]
    fn rejects_style_before_markup() {
        let error = parse("@page \"/\"\n\n@style {\n  .a {}\n}\n\n<p></p>")
            .expect_err("style before markup should fail");

        assert_eq!(error.message, "@style must appear after markup");
    }

    #[test]
    fn rejects_markup_after_style() {
        let error = parse("@page \"/\"\n\n<p></p>\n\n@style {\n  .a {}\n}\n<p>late</p>")
            .expect_err("markup after style should fail");

        assert_eq!(error.message, "only @style blocks are allowed after markup");
    }

    #[test]
    fn parses_namespaced_component_declaration() {
        let component = parse(
            "@component UI.Button {\n  label: string = \"Click\"\n}\n\n<button>{label}</button>",
        )
        .expect("valid component");

        let declaration = component.component.as_ref().expect("component");
        assert_eq!(declaration.name, "UI.Button");
        assert_eq!(declaration.props[0].name, "label");
    }

    #[test]
    fn parses_namespaced_component_call() {
        let page = parse("@page \"/\"\n\n<UI.Button label=\"Save\" />").expect("valid page");

        let TemplateNode::Component(call) = &page.template[0] else {
            panic!("expected component call");
        };
        assert_eq!(call.name, "UI.Button");
        assert_eq!(call.props[0].name, "label");
        assert!(matches!(
            call.props[0].value,
            PropValue::Literal(Value::String(ref label)) if label == "Save"
        ));
    }

    #[test]
    fn parses_component_call_with_event_and_class() {
        let page = parse(
            "@page \"/\"\n\n<UI.Button label=\"+\" variant=\"outline\" size=\"sm\" class=\"w-full\" @click={count++} />",
        )
        .expect("valid page");

        let TemplateNode::Component(call) = &page.template[0] else {
            panic!("expected component call");
        };
        assert_eq!(call.name, "UI.Button");
        assert_eq!(call.event_bindings.len(), 1);
        assert_eq!(call.event_bindings[0].event, "click");
        assert_eq!(call.event_bindings[0].handler_source, "count++");
        assert!(call.class_expr.is_some());
        assert!(
            call.props.iter().all(|prop| prop.name != "class"),
            "class should be extracted as passthrough, not a prop"
        );
    }

    #[test]
    fn rejects_invalid_namespaced_component_declaration() {
        let error = parse("@component ui.Button {\n  label: string\n}\n\n<button />")
            .expect_err("lowercase namespace should fail");
        assert!(error.message.contains("component name"));
    }

    #[test]
    fn lowercase_dotted_tag_is_treated_as_html() {
        let page = parse("@page \"/\"\n\n<ui.Button />").expect("valid page");
        assert!(matches!(page.template[0], TemplateNode::Text(_)));
    }
}
