use crate::diagnostic::{Diagnostic, Span};
use crate::parser::{ComponentCall, ComponentProp, PropValue, SourceExpr, TemplateNode, Value, WebFile};
use std::collections::BTreeMap;

pub type Scope = BTreeMap<String, Value>;
pub type ComponentRegistry = BTreeMap<String, WebFile>;

pub fn render_with_components(
    file: &WebFile,
    params: &Scope,
    components: &ComponentRegistry,
) -> Result<String, Diagnostic> {
    let scope = scope_for(file, params);
    render_nodes(&file.template, &scope, components)
}

pub fn validate_with_components(file: &WebFile, components: &ComponentRegistry) -> Vec<Diagnostic> {
    let scope = scope_for(file, &Scope::new());
    let mut diagnostics = Vec::new();
    validate_nodes(&file.template, &scope, components, &mut diagnostics);
    diagnostics
}

fn scope_for(file: &WebFile, params: &Scope) -> Scope {
    let mut scope = Scope::new();

    if let Some(route) = &file.route {
        for param in &route.params {
            scope.insert(param.name.clone(), sample_for_type(&param.type_name));
        }
    }
    if let Some(component) = &file.component {
        for prop in &component.props {
            scope.insert(
                prop.name.clone(),
                prop.default
                    .clone()
                    .unwrap_or_else(|| sample_for_type(&prop.type_name)),
            );
        }
    }
    for (name, value) in params {
        scope.insert(name.clone(), value.clone());
    }
    for (name, value) in &file.lets {
        scope.insert(name.clone(), value.clone());
    }

    scope
}

fn render_nodes(
    nodes: &[TemplateNode],
    scope: &Scope,
    components: &ComponentRegistry,
) -> Result<String, Diagnostic> {
    let mut html = String::new();

    for node in nodes {
        match node {
            TemplateNode::Text(value) => html.push_str(value),
            TemplateNode::Expr(expr) => {
                let value = scope
                    .get(&expr.name)
                    .ok_or_else(|| unknown_expression(expr))?;
                html.push_str(&escape_html(&value.render()));
            }
            TemplateNode::Component(call) => {
                html.push_str(&render_component(call, scope, components)?);
            }
            TemplateNode::If {
                condition,
                then_nodes,
                else_nodes,
            } => {
                let value = scope
                    .get(&condition.name)
                    .ok_or_else(|| unknown_condition(condition))?;
                let Some(condition_value) = value.as_bool() else {
                    return Err(if_condition_not_bool(condition));
                };

                if condition_value {
                    html.push_str(&render_nodes(then_nodes, scope, components)?);
                } else {
                    html.push_str(&render_nodes(else_nodes, scope, components)?);
                }
            }
            TemplateNode::For {
                item_name,
                source,
                body,
            } => {
                let value = scope
                    .get(&source.name)
                    .ok_or_else(|| unknown_loop_source(source))?;
                let Some(items) = value.as_array() else {
                    return Err(for_source_not_array(source));
                };

                for item in items {
                    let mut loop_scope = scope.clone();
                    loop_scope.insert(item_name.clone(), item.clone());
                    html.push_str(&render_nodes(body, &loop_scope, components)?);
                }
            }
        }
    }

    Ok(html)
}

fn render_component(
    call: &ComponentCall,
    scope: &Scope,
    components: &ComponentRegistry,
) -> Result<String, Diagnostic> {
    let component = components
        .get(&call.name)
        .ok_or_else(|| unknown_component(call))?;
    let component_scope = component_scope(call, component, scope)?;
    render_nodes(&component.template, &component_scope, components)
}

fn component_scope(
    call: &ComponentCall,
    component: &WebFile,
    parent: &Scope,
) -> Result<Scope, Diagnostic> {
    let declaration = component
        .component
        .as_ref()
        .ok_or_else(|| not_a_component(call))?;
    let mut scope = Scope::new();

    for prop in &declaration.props {
        if let Some(default) = &prop.default {
            scope.insert(prop.name.clone(), default.clone());
        }
    }

    for call_prop in &call.props {
        let Some(declared) = declaration
            .props
            .iter()
            .find(|prop| prop.name == call_prop.name)
        else {
            return Err(unknown_prop(call_prop, &call.name));
        };
        let value = evaluate_prop_value(&call_prop.value, parent, call_prop)?;
        if !value_matches_type(&value, &declared.type_name) {
            return Err(prop_type_mismatch(
                call_prop,
                &call.name,
                &declared.type_name,
                &value.type_name(),
            ));
        }
        scope.insert(call_prop.name.clone(), value);
    }

    for prop in &declaration.props {
        if !scope.contains_key(&prop.name) {
            return Err(missing_prop(call, &prop.name));
        }
    }

    for (name, value) in &component.lets {
        scope.insert(name.clone(), value.clone());
    }

    Ok(scope)
}

fn evaluate_prop_value(
    value: &PropValue,
    scope: &Scope,
    prop: &ComponentProp,
) -> Result<Value, Diagnostic> {
    match value {
        PropValue::Literal(value) => Ok(value.clone()),
        PropValue::Expr(expr) => scope
            .get(&expr.name)
            .cloned()
            .ok_or_else(|| unknown_prop_expression(expr, prop)),
    }
}

fn validate_nodes(
    nodes: &[TemplateNode],
    scope: &Scope,
    components: &ComponentRegistry,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for node in nodes {
        match node {
            TemplateNode::Text(_) => {}
            TemplateNode::Expr(expr) => {
                if !scope.contains_key(&expr.name) {
                    diagnostics.push(unknown_expression(expr));
                }
            }
            TemplateNode::Component(call) => {
                validate_component_call(call, scope, components, diagnostics)
            }
            TemplateNode::If {
                condition,
                then_nodes,
                else_nodes,
            } => {
                match scope.get(&condition.name) {
                    Some(value) if value.as_bool().is_some() => {}
                    Some(_) => diagnostics.push(if_condition_not_bool(condition)),
                    None => diagnostics.push(unknown_condition(condition)),
                }
                validate_nodes(then_nodes, scope, components, diagnostics);
                validate_nodes(else_nodes, scope, components, diagnostics);
            }
            TemplateNode::For {
                item_name,
                source,
                body,
            } => match scope.get(&source.name) {
                Some(value) if value.as_array().is_some() => {
                    let mut loop_scope = scope.clone();
                    if let Some(sample) = value.array_sample() {
                        loop_scope.insert(item_name.clone(), sample);
                    }
                    validate_nodes(body, &loop_scope, components, diagnostics);
                }
                Some(_) => diagnostics.push(for_source_not_array(source)),
                None => diagnostics.push(unknown_loop_source(source)),
            },
        }
    }
}

fn validate_component_call(
    call: &ComponentCall,
    scope: &Scope,
    components: &ComponentRegistry,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(component) = components.get(&call.name) else {
        diagnostics.push(unknown_component(call));
        return;
    };
    let Some(declaration) = component.component.as_ref() else {
        diagnostics.push(not_a_component(call));
        return;
    };

    let mut provided = BTreeMap::new();
    for prop in &call.props {
        let Some(declared) = declaration
            .props
            .iter()
            .find(|candidate| candidate.name == prop.name)
        else {
            diagnostics.push(unknown_prop(prop, &call.name));
            continue;
        };

        match prop_value_type(&prop.value, scope, prop) {
            Ok(actual) if type_names_match(&actual, &declared.type_name) => {}
            Ok(actual) => diagnostics.push(prop_type_mismatch(
                prop,
                &call.name,
                &declared.type_name,
                &actual,
            )),
            Err(error) => diagnostics.push(error),
        }

        provided.insert(prop.name.clone(), true);
    }

    for prop in &declaration.props {
        if prop.default.is_none() && !provided.contains_key(&prop.name) {
            diagnostics.push(missing_prop(call, &prop.name));
        }
    }
}

fn prop_value_type(
    value: &PropValue,
    scope: &Scope,
    prop: &ComponentProp,
) -> Result<String, Diagnostic> {
    match value {
        PropValue::Literal(value) => Ok(value.type_name()),
        PropValue::Expr(expr) => scope
            .get(&expr.name)
            .map(Value::type_name)
            .ok_or_else(|| unknown_prop_expression(expr, prop)),
    }
}

fn unknown_expression(expr: &SourceExpr) -> Diagnostic {
    Diagnostic::error(
        Span::braced_expr(expr.line, expr.column.saturating_sub(1), &expr.name),
        format!("unknown expression `{name}`", name = expr.name),
        None,
    )
}

fn unknown_condition(condition: &SourceExpr) -> Diagnostic {
    Diagnostic::error(
        Span::identifier(condition.line, condition.column, &condition.name),
        format!("unknown condition `{name}`", name = condition.name),
        None,
    )
}

fn if_condition_not_bool(condition: &SourceExpr) -> Diagnostic {
    Diagnostic::error(
        Span::identifier(condition.line, condition.column, &condition.name),
        format!("@if condition `{name}` must be bool", name = condition.name),
        Some("expected `bool`".to_string()),
    )
}

fn unknown_loop_source(source: &SourceExpr) -> Diagnostic {
    Diagnostic::error(
        Span::identifier(source.line, source.column, &source.name),
        format!("unknown loop source `{name}`", name = source.name),
        None,
    )
}

fn for_source_not_array(source: &SourceExpr) -> Diagnostic {
    Diagnostic::error(
        Span::identifier(source.line, source.column, &source.name),
        format!("@for source `{name}` must be array", name = source.name),
        Some("expected array type".to_string()),
    )
}

fn unknown_component(call: &ComponentCall) -> Diagnostic {
    Diagnostic::error(
        Span::identifier(call.line, call.column, &call.name),
        format!("unknown component `{name}`", name = call.name),
        None,
    )
}

fn not_a_component(call: &ComponentCall) -> Diagnostic {
    Diagnostic::error(
        Span::identifier(call.line, call.column, &call.name),
        format!("`{name}` is not a component", name = call.name),
        None,
    )
}

fn unknown_prop(prop: &ComponentProp, component_name: &str) -> Diagnostic {
    Diagnostic::error(
        Span::identifier(prop.line, prop.column, &prop.name),
        format!(
            "unknown prop `{prop}` for component `{component_name}`",
            prop = prop.name
        ),
        None,
    )
}

fn missing_prop(call: &ComponentCall, prop_name: &str) -> Diagnostic {
    Diagnostic::error(
        Span::identifier(call.line, call.column, &call.name),
        format!(
            "missing prop `{prop_name}` for component `{component}`",
            component = call.name
        ),
        None,
    )
}

fn prop_type_mismatch(
    prop: &ComponentProp,
    component_name: &str,
    expected: &str,
    found: &str,
) -> Diagnostic {
    Diagnostic::error(
        Span::new(prop.line, prop.value_start_col, prop.value_end_col),
        format!(
            "prop `{prop}` for component `{component_name}` expects `{expected}`, found `{found}`",
            prop = prop.name
        ),
        Some(format!("expected `{expected}`, found `{found}`")),
    )
}

fn unknown_prop_expression(expr: &SourceExpr, prop: &ComponentProp) -> Diagnostic {
    Diagnostic::error(
        Span::new(prop.line, prop.value_start_col, prop.value_end_col),
        format!("unknown prop expression `{name}`", name = expr.name),
        None,
    )
}

fn value_matches_type(value: &Value, expected: &str) -> bool {
    type_names_match(&value.type_name(), expected)
}

fn type_names_match(actual: &str, expected: &str) -> bool {
    actual == expected
}

fn sample_for_type(type_name: &str) -> Value {
    if let Some(element_type) = type_name.strip_suffix("[]") {
        return Value::Array {
            element_type: element_type.to_string(),
            values: Vec::new(),
        };
    }

    match type_name {
        "int" => Value::Int(0),
        "bool" => Value::Bool(false),
        _ => Value::String(String::new()),
    }
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for char in value.chars() {
        match char {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(char),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{render_with_components, ComponentRegistry, Scope};
    use crate::diagnostic::Span;
    use crate::parser::parse;

    #[test]
    fn renders_known_expressions_with_escaping() {
        let file = parse("@page \"/\"\n\n@let name: string = \"<Ada>\"\n\n<h1>{name}</h1>")
            .expect("valid page");

        assert_eq!(
            render_with_components(&file, &Scope::new(), &ComponentRegistry::new())
                .expect("rendered"),
            "<h1>&lt;Ada&gt;</h1>"
        );
    }

    #[test]
    fn renders_if_else() {
        let file = parse(
            "@page \"/\"\n\n@let show: bool = false\n\n@if show {\n<p>yes</p>\n} @else {\n<p>no</p>\n}",
        )
        .expect("valid page");

        assert_eq!(
            render_with_components(&file, &Scope::new(), &ComponentRegistry::new())
                .expect("rendered"),
            "<p>no</p>"
        );
    }

    #[test]
    fn renders_for_blocks_with_scoped_items() {
        let file = parse(
            "@page \"/\"\n\n@let posts: string[] = [\"One\", \"<Two>\"]\n\n@for post in posts {\n<p>{post}</p>\n}\n<p>{post}</p>",
        )
        .expect("valid page");
        let error = render_with_components(&file, &Scope::new(), &ComponentRegistry::new())
            .expect_err("post should be scoped to loop");

        assert_eq!(
            error.message,
            "unknown expression `post`"
        );
        assert_eq!(error.span, Span::braced_expr(8, 4, "post"));

        let file = parse(
            "@page \"/\"\n\n@let posts: string[] = [\"One\", \"<Two>\"]\n\n@for post in posts {\n<p>{post}</p>\n}",
        )
        .expect("valid page");

        assert_eq!(
            render_with_components(&file, &Scope::new(), &ComponentRegistry::new())
                .expect("rendered"),
            "<p>One</p><p>&lt;Two&gt;</p>"
        );
    }

    #[test]
    fn validates_for_source_type() {
        let file = parse("@page \"/\"\n\n@let title: string = \"Nope\"\n\n@for post in title {\n<p>{post}</p>\n}")
            .expect("valid page");

        let diagnostics = super::validate_with_components(&file, &ComponentRegistry::new());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "@for source `title` must be array");
        assert_eq!(diagnostics[0].span, Span::identifier(5, 14, "title"));
    }

    #[test]
    fn validates_unknown_loop_sources_and_scoping() {
        let file = parse("@page \"/\"\n\n@for post in posts {\n<p>{post}</p>\n}\n<p>{post}</p>")
            .expect("valid page");

        let diagnostics = super::validate_with_components(&file, &ComponentRegistry::new());
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].message, "unknown loop source `posts`");
        assert_eq!(diagnostics[1].message, "unknown expression `post`");
    }

    #[test]
    fn renders_components_with_typed_props_and_defaults() {
        let component = parse(
            "@component UserCard {\n  name: string\n  visits: int = 0\n}\n\n<article>{name}: {visits}</article>",
        )
        .expect("valid component");
        let page = parse("@page \"/\"\n\n@let name: string = \"Ada\"\n\n<UserCard name={name} />")
            .expect("valid page");
        let mut components = ComponentRegistry::new();
        components.insert("UserCard".to_string(), component);

        assert_eq!(
            render_with_components(&page, &Scope::new(), &components).expect("rendered"),
            "<article>Ada: 0</article>"
        );
    }

    #[test]
    fn validates_missing_and_wrong_component_props() {
        let component =
            parse("@component UserCard {\n  name: string\n  visits: int\n}\n\n<p>{name}</p>")
                .expect("valid component");
        let page = parse(
            "@page \"/\"\n\n@let visits: string = \"three\"\n\n<UserCard visits={visits} extra=\"nope\" />",
        )
        .expect("valid page");
        let mut components = ComponentRegistry::new();
        components.insert("UserCard".to_string(), component);

        let diagnostics = super::validate_with_components(&page, &components);
        assert_eq!(diagnostics.len(), 3);
        assert_eq!(
            diagnostics[0].message,
            "prop `visits` for component `UserCard` expects `int`, found `string`"
        );
        assert_eq!(
            diagnostics[0].label.as_deref(),
            Some("expected `int`, found `string`")
        );
        assert_eq!(diagnostics[1].message, "unknown prop `extra` for component `UserCard`");
        assert_eq!(
            diagnostics[2].message,
            "missing prop `name` for component `UserCard`"
        );
    }

    #[test]
    fn validates_unknown_component() {
        let page = parse("@page \"/\"\n\n<Missing />").expect("valid page");

        let diagnostics = super::validate_with_components(&page, &ComponentRegistry::new());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "unknown component `Missing`");
    }

    #[test]
    fn validates_component_template_against_its_props() {
        let component = parse(
            "@component UserCard {\n  name: string\n  active: bool\n}\n\n@if active {\n<p>{name}</p>\n}",
        )
        .expect("valid component");

        assert!(super::validate_with_components(&component, &ComponentRegistry::new()).is_empty());
    }

    #[test]
    fn validates_if_requires_bool_inside_empty_bool_array_loop() {
        let file =
            parse("@page \"/\"\n\n@let flags: bool[] = []\n\n@for flag in flags {\n@if flag {\n<p>yes</p>\n}\n}")
                .expect("valid page");

        assert!(super::validate_with_components(&file, &ComponentRegistry::new()).is_empty());
    }

    #[test]
    fn rejects_unknown_expressions_with_location() {
        let file = parse("@page \"/\"\n\n<h1>{name}</h1>").expect("valid page");
        let error = render_with_components(&file, &Scope::new(), &ComponentRegistry::new())
            .expect_err("unknown expression should fail");

        assert_eq!(error.message, "unknown expression `name`");
        assert_eq!(error.span, Span::braced_expr(3, 5, "name"));
    }
}
