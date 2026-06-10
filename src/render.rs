use crate::diagnostic::{Diagnostic, Span};
use crate::expr;
use crate::parser::{
    ComponentCall, ComponentProp, LetDecl, LetValue, PropValue, SourceExpr, TemplateNode, Value,
    WebFile,
};
use crate::runtime::WebRuntime;
use crate::stmt::{self, BlockOutcome, Statement};
use std::collections::BTreeMap;

pub type Scope = BTreeMap<String, Value>;
pub type ComponentRegistry = BTreeMap<String, WebFile>;

pub fn render_with_components(
    file: &WebFile,
    params: &Scope,
    components: &ComponentRegistry,
    runtime: &WebRuntime,
) -> Result<String, Diagnostic> {
    if file.load.is_some() {
        return Err(Diagnostic::error(
            Span::at(1, 1),
            "@load requires async rendering",
            None,
        ));
    }
    let mut scope = base_scope_for(file, params);
    evaluate_lets(&file.lets, &mut scope)?;
    render_nodes(&file.template, &scope, components, runtime)
}

pub async fn render_with_components_async(
    file: &WebFile,
    params: &Scope,
    components: &ComponentRegistry,
    runtime: &WebRuntime,
) -> Result<String, Diagnostic> {
    let scope = scope_for_async(file, params, runtime).await?;
    render_nodes(&file.template, &scope, components, runtime)
}

pub fn validate_with_components(file: &WebFile, components: &ComponentRegistry) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut scope = base_scope_for(file, &Scope::new());
    merge_load_samples(file, &mut scope);
    if let Err(error) = evaluate_lets(&file.lets, &mut scope) {
        diagnostics.push(error);
    }
    validate_nodes(&file.template, &scope, components, &mut diagnostics);
    if let Some(load) = &file.load {
        validate_server_block(load, &mut diagnostics);
    }
    for action in &file.actions {
        validate_server_block_statements(&action.statements, &mut diagnostics);
    }
    diagnostics
}

fn merge_load_samples(file: &WebFile, scope: &mut Scope) {
    let Some(load) = &file.load else {
        return;
    };
    for statement in &load.statements {
        if let Statement::Let {
            name, type_name, ..
        } = statement
        {
            let type_name = type_name.as_deref().unwrap_or("string");
            scope.insert(name.clone(), sample_for_type(type_name));
        }
    }
}

fn validate_server_block(load: &crate::parser::ServerBlock, diagnostics: &mut Vec<Diagnostic>) {
    validate_server_block_statements(&load.statements, diagnostics);
}

fn validate_server_block_statements(statements: &[Statement], diagnostics: &mut Vec<Diagnostic>) {
    for statement in statements {
        if let Statement::Try {
            statements,
            catch_body,
            ..
        } = statement
        {
            validate_server_block_statements(statements, diagnostics);
            validate_server_block_statements(catch_body, diagnostics);
        }
    }
}

pub enum ActionOutcome {
    Redirect(String),
    Fail(String),
}

pub async fn execute_action(
    file: &WebFile,
    action_name: &str,
    params: &Scope,
    input: &Scope,
    session: &mut BTreeMap<String, Value>,
    runtime: &WebRuntime,
) -> Result<ActionOutcome, Diagnostic> {
    let action = file
        .actions
        .iter()
        .find(|action| action.name == action_name)
        .ok_or_else(|| {
            Diagnostic::error(
                Span::identifier(1, 1, action_name),
                format!("unknown action `{action_name}`"),
                None,
            )
        })?;
    let mut scope = Scope::new();
    for (name, value) in params {
        scope.insert(name.clone(), value.clone());
    }
    scope.insert("input".to_string(), Value::Object(input.clone()));
    scope.insert("session".to_string(), Value::Object(session.clone()));

    let outcome = runtime
        .execute_block_async(&action.statements, &mut scope, session)
        .await?;

    Ok(match outcome {
        Some(BlockOutcome::Redirect(target)) => ActionOutcome::Redirect(target),
        Some(BlockOutcome::Fail(message)) => ActionOutcome::Fail(message),
        Some(BlockOutcome::Return(_)) => ActionOutcome::Redirect(".".to_string()),
        None => ActionOutcome::Redirect(".".to_string()),
    })
}

fn base_scope_for(file: &WebFile, params: &Scope) -> Scope {
    let mut scope = Scope::new();
    scope.insert("session".to_string(), default_session_value());

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
                    .unwrap_or_else(|| sample_for_prop_type(&prop.type_name)),
            );
        }
    }
    for (name, value) in params {
        if name == "session" {
            scope.insert(name.clone(), merge_session_defaults(value));
        } else {
            scope.insert(name.clone(), value.clone());
        }
    }

    scope
}

fn default_session_value() -> Value {
    let mut fields = BTreeMap::new();
    fields.insert("count".to_string(), Value::Int(0));
    fields.insert("name".to_string(), Value::String(String::new()));
    Value::Object(fields)
}

fn merge_session_defaults(value: &Value) -> Value {
    let Value::Object(mut defaults) = default_session_value() else {
        return value.clone();
    };
    if let Value::Object(fields) = value {
        for (name, field_value) in fields {
            defaults.insert(name.clone(), field_value.clone());
        }
    }
    Value::Object(defaults)
}

async fn scope_for_async(
    file: &WebFile,
    params: &Scope,
    runtime: &WebRuntime,
) -> Result<Scope, Diagnostic> {
    let mut scope = base_scope_for(file, params);
    let mut session = match scope.get("session") {
        Some(Value::Object(session)) => session.clone(),
        _ => BTreeMap::new(),
    };

    if let Some(load) = &file.load {
        runtime
            .execute_block_async(&load.statements, &mut scope, &mut session)
            .await?;
        scope.insert("session".to_string(), Value::Object(session));
    }

    evaluate_lets(&file.lets, &mut scope)?;
    Ok(scope)
}

fn evaluate_lets(lets: &[LetDecl], scope: &mut Scope) -> Result<(), Diagnostic> {
    for let_decl in lets {
        let value = match &let_decl.value {
            LetValue::Static(value) => value.clone(),
            LetValue::Expr(expr) => {
                expr::evaluate(expr, scope, let_decl.line, let_decl.value_start_col)?
            }
        };

        if let Some(type_name) = &let_decl.type_name {
            if !type_names_match(&value.type_name(), type_name) {
                return Err(Diagnostic::error(
                    Span::new(
                        let_decl.line,
                        let_decl.value_start_col,
                        let_decl.value_end_col,
                    ),
                    format!(
                        "@let `{}` expects `{type_name}`, found `{}`",
                        let_decl.name,
                        value.type_name()
                    ),
                    Some(format!(
                        "expected `{type_name}`, found `{}`",
                        value.type_name()
                    )),
                ));
            }
        }

        scope.insert(let_decl.name.clone(), value);
    }

    Ok(())
}

fn render_nodes(
    nodes: &[TemplateNode],
    scope: &Scope,
    components: &ComponentRegistry,
    runtime: &WebRuntime,
) -> Result<String, Diagnostic> {
    let mut html = String::new();
    let mut scope = scope.clone();

    for node in nodes {
        match node {
            TemplateNode::Text(value) => html.push_str(value),
            TemplateNode::Expr(expr) => {
                let value = evaluate_source_expr(expr, &scope)?;
                html.push_str(&escape_html(&value.render()));
            }
            TemplateNode::Component(call) => {
                html.push_str(&render_component(call, &scope, components, runtime)?);
            }
            TemplateNode::If {
                condition,
                then_nodes,
                else_nodes,
            } => {
                let value = evaluate_source_expr(condition, &scope)?;
                let Some(condition_value) = value.as_bool() else {
                    return Err(if_condition_not_bool(condition));
                };

                if condition_value {
                    html.push_str(&render_nodes(then_nodes, &scope, components, runtime)?);
                } else {
                    html.push_str(&render_nodes(else_nodes, &scope, components, runtime)?);
                }
            }
            TemplateNode::For {
                item_name,
                source,
                body,
            } => {
                let value = evaluate_source_expr(source, &scope)?;
                let Some(items) = value.as_array() else {
                    return Err(for_source_not_array(source));
                };

                for item in items.to_vec() {
                    let mut loop_scope = scope.clone();
                    loop_scope.insert(item_name.clone(), item);
                    html.push_str(&render_nodes(body, &loop_scope, components, runtime)?);
                }
            }
            TemplateNode::Do { statements, .. } => {
                let mut session = match scope.get("session") {
                    Some(Value::Object(session)) => session.clone(),
                    _ => BTreeMap::new(),
                };
                stmt::execute_sync(statements, &mut scope, &mut session)?;
                scope.insert("session".to_string(), Value::Object(session));
            }
        }
    }

    Ok(html)
}

fn render_component(
    call: &ComponentCall,
    scope: &Scope,
    components: &ComponentRegistry,
    runtime: &WebRuntime,
) -> Result<String, Diagnostic> {
    let component = components
        .get(&call.name)
        .ok_or_else(|| unknown_component(call))?;
    let component_scope = component_scope(call, component, scope)?;
    render_nodes(&component.template, &component_scope, components, runtime)
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

    evaluate_lets(&component.lets, &mut scope)?;

    Ok(scope)
}

fn evaluate_prop_value(
    value: &PropValue,
    scope: &Scope,
    prop: &ComponentProp,
) -> Result<Value, Diagnostic> {
    match value {
        PropValue::Literal(value) => Ok(value.clone()),
        PropValue::Expr(expr) => evaluate_source_expr(expr, scope).map_err(|error| {
            Diagnostic::error(
                Span::new(prop.line, prop.value_start_col, prop.value_end_col),
                error.message,
                error.label,
            )
        }),
    }
}

fn validate_nodes(
    nodes: &[TemplateNode],
    scope: &Scope,
    components: &ComponentRegistry,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut scope = scope.clone();
    for node in nodes {
        match node {
            TemplateNode::Text(_) => {}
            TemplateNode::Expr(expr) => {
                if let Err(error) = evaluate_source_expr(expr, &scope) {
                    diagnostics.push(error);
                }
            }
            TemplateNode::Component(call) => {
                validate_component_call(call, &scope, components, diagnostics)
            }
            TemplateNode::If {
                condition,
                then_nodes,
                else_nodes,
            } => {
                match evaluate_source_expr(condition, &scope) {
                    Ok(value) if value.as_bool().is_some() => {}
                    Ok(_) => diagnostics.push(if_condition_not_bool(condition)),
                    Err(error) => diagnostics.push(error),
                }
                validate_nodes(then_nodes, &scope, components, diagnostics);
                validate_nodes(else_nodes, &scope, components, diagnostics);
            }
            TemplateNode::For {
                item_name,
                source,
                body,
            } => match evaluate_source_expr(source, &scope) {
                Ok(value) if value.as_array().is_some() => {
                    let mut loop_scope = scope.clone();
                    if let Some(sample) = value.array_sample() {
                        loop_scope.insert(item_name.clone(), sample);
                    }
                    validate_nodes(body, &loop_scope, components, diagnostics);
                }
                Ok(_) => diagnostics.push(for_source_not_array(source)),
                Err(error) => diagnostics.push(error),
            },
            TemplateNode::Do { statements, .. } => {
                validate_server_block_statements(statements, diagnostics);
                let mut session = match scope.get("session") {
                    Some(Value::Object(session)) => session.clone(),
                    _ => BTreeMap::new(),
                };
                if let Err(error) = stmt::execute_sync(statements, &mut scope, &mut session) {
                    diagnostics.push(error);
                } else {
                    scope.insert("session".to_string(), Value::Object(session));
                }
            }
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
        PropValue::Expr(expr) => evaluate_source_expr(expr, scope)
            .map(|value| value.type_name())
            .map_err(|error| {
                Diagnostic::error(
                    Span::new(prop.line, prop.value_start_col, prop.value_end_col),
                    error.message,
                    error.label,
                )
            }),
    }
}

fn evaluate_source_expr(expr: &SourceExpr, scope: &Scope) -> Result<Value, Diagnostic> {
    expr::evaluate(&expr.expr, scope, expr.line, expr.column)
}

fn if_condition_not_bool(condition: &SourceExpr) -> Diagnostic {
    Diagnostic::error(
        Span::new(
            condition.line,
            condition.column,
            condition.column + condition.source.len(),
        ),
        format!("@if condition `{}` must be bool", condition.source),
        Some("expected `bool`".to_string()),
    )
}

fn for_source_not_array(source: &SourceExpr) -> Diagnostic {
    Diagnostic::error(
        Span::new(
            source.line,
            source.column,
            source.column + source.source.len(),
        ),
        format!("@for source `{}` must be array", source.source),
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

fn value_matches_type(value: &Value, expected: &str) -> bool {
    type_names_match(&value.type_name(), expected)
}

fn type_names_match(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }

    if is_object_type_name(expected) {
        return actual == "object" || actual.starts_with('{');
    }

    if let (Some(actual_element), Some(expected_element)) =
        (actual.strip_suffix("[]"), expected.strip_suffix("[]"))
    {
        return type_names_match(actual_element, expected_element);
    }

    actual.starts_with('{') && expected.starts_with('{') && actual == expected
}

fn is_object_type_name(type_name: &str) -> bool {
    type_name == "object"
        || type_name
            .chars()
            .next()
            .is_some_and(|char| char.is_ascii_uppercase())
}

fn sample_for_prop_type(type_name: &str) -> Value {
    if is_object_type_name(type_name) {
        return Value::Object(BTreeMap::new());
    }
    sample_for_type(type_name)
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
        "object" => Value::Object(BTreeMap::new()),
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
    use crate::runtime::WebRuntime;

    #[test]
    fn renders_known_expressions_with_escaping() {
        let file = parse("@page \"/\"\n\n@let name: string = \"<Ada>\"\n\n<h1>{name}</h1>")
            .expect("valid page");

        assert_eq!(
            render_with_components(
                &file,
                &Scope::new(),
                &ComponentRegistry::new(),
                &WebRuntime::new()
            )
            .expect("rendered"),
            "<h1>&lt;Ada&gt;</h1>"
        );
    }

    #[test]
    fn renders_expression_interpolation() {
        let file = parse(
            "@page \"/\"\n\n@let name = \"Ada\"\n@let visits: int = 2\n\n<h1>{\"Hello \" + name}</h1>\n<p>{visits + 1}</p>",
        )
        .expect("valid page");

        assert_eq!(
            render_with_components(
                &file,
                &Scope::new(),
                &ComponentRegistry::new(),
                &WebRuntime::new()
            )
            .expect("rendered"),
            "<h1>Hello Ada</h1>\n<p>3</p>"
        );
    }

    #[test]
    fn renders_lets_that_depend_on_route_params() {
        let file = parse(
            "@page \"/posts/{slug:string}\"\n\n@let title = \"Post: \" + slug\n@let isIntro = slug == \"intro\"\n\n<h1>{title}</h1>\n@if isIntro {\n<p>intro</p>\n}",
        )
        .expect("valid page");
        let mut params = Scope::new();
        params.insert(
            "slug".to_string(),
            crate::parser::Value::String("intro".to_string()),
        );

        assert_eq!(
            render_with_components(
                &file,
                &params,
                &ComponentRegistry::new(),
                &WebRuntime::new()
            )
            .expect("rendered"),
            "<h1>Post: intro</h1>\n<p>intro</p>"
        );
    }

    #[test]
    fn renders_if_else() {
        let file = parse(
            "@page \"/\"\n\n@let visits: int = 1\n\n@if visits > 1 {\n<p>yes</p>\n} @else {\n<p>no</p>\n}",
        )
        .expect("valid page");

        assert_eq!(
            render_with_components(
                &file,
                &Scope::new(),
                &ComponentRegistry::new(),
                &WebRuntime::new()
            )
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
        let error = render_with_components(
            &file,
            &Scope::new(),
            &ComponentRegistry::new(),
            &WebRuntime::new(),
        )
        .expect_err("post should be scoped to loop");

        assert_eq!(error.message, "unknown expression `post`");
        assert_eq!(error.span, Span::identifier(8, 5, "post"));

        let file = parse(
            "@page \"/\"\n\n@let posts: string[] = [\"One\", \"<Two>\"]\n\n@for post in posts {\n<p>{post}</p>\n}",
        )
        .expect("valid page");

        assert_eq!(
            render_with_components(
                &file,
                &Scope::new(),
                &ComponentRegistry::new(),
                &WebRuntime::new()
            )
            .expect("rendered"),
            "<p>One</p><p>&lt;Two&gt;</p>"
        );
    }

    #[tokio::test]
    async fn executes_action_and_mutates_session() {
        let file = parse(
            "@page \"/\"\n\n@action increment {\n  session.count = session.count + 1\n  redirect(\"/\")\n}\n\n<p>{session.count}</p>",
        )
        .expect("valid page");
        let mut session = Scope::new();
        session.insert("count".to_string(), crate::parser::Value::Int(1));
        let runtime = WebRuntime::new();

        let outcome = super::execute_action(
            &file,
            "increment",
            &Scope::new(),
            &Scope::new(),
            &mut session,
            &runtime,
        )
        .await
        .expect("action");

        assert!(matches!(outcome, super::ActionOutcome::Redirect(target) if target == "/"));
        assert!(matches!(
            session.get("count"),
            Some(crate::parser::Value::Int(2))
        ));
    }

    #[tokio::test]
    async fn renders_load_bindings_in_template() {
        let file =
            parse("@page \"/\"\n\n@load {\n  title: string = \"Loaded\"\n}\n\n<h1>{title}</h1>")
                .expect("valid page");
        let runtime = WebRuntime::new();

        let html = super::render_with_components_async(
            &file,
            &Scope::new(),
            &ComponentRegistry::new(),
            &runtime,
        )
        .await
        .expect("rendered");

        assert_eq!(html, "<h1>Loaded</h1>");
    }

    #[tokio::test]
    async fn load_try_catch_captures_throw() {
        let file = parse(
            "@page \"/\"\n\n@load {\n  error: string = \"\"\n  try {\n    throw(\"boom\")\n  } catch err {\n    error = err.message\n  }\n}\n\n<p>{error}</p>",
        )
        .expect("valid page");
        let runtime = WebRuntime::new();
        let html = super::render_with_components_async(
            &file,
            &Scope::new(),
            &ComponentRegistry::new(),
            &runtime,
        )
        .await
        .expect("rendered");

        assert!(html.contains("boom"));
    }

    #[tokio::test]
    async fn load_try_catch_captures_async_timeout() {
        let file = parse(
            "@page \"/\"\n\n@load {\n  timedOut: bool = false\n  task := spawn(sleep(50ms))\n  try {\n    _: object = await timeout(1ms, task)\n  } catch err {\n    timedOut = err.message == \"timeout\"\n  }\n}\n\n<p>{timedOut}</p>",
        )
        .expect("valid page");
        let runtime = WebRuntime::new();
        let html = super::render_with_components_async(
            &file,
            &Scope::new(),
            &ComponentRegistry::new(),
            &runtime,
        )
        .await
        .expect("rendered");

        assert!(html.contains("true"));
    }

    #[tokio::test]
    #[ignore = "requires network access to httpbin.org"]
    async fn fetch_demo_catches_http_error() {
        let file = parse(include_str!("../app/pages/fetch-demo.web")).expect("valid page");
        let runtime = WebRuntime::new();
        let html = super::render_with_components_async(
            &file,
            &Scope::new(),
            &ComponentRegistry::new(),
            &runtime,
        )
        .await
        .expect("rendered");

        assert!(html.contains("upstream returned 404") || html.contains("404"));
    }

    #[tokio::test]
    async fn executes_action_failures_inside_if_blocks() {
        let file = parse(
            "@page \"/\"\n\n@action rememberName {\n  if input.name == \"\" {\n    fail(\"Name is required\")\n  }\n  session.name = input.name\n  redirect(\"/\")\n}\n\n<p>{session.name}</p>",
        )
        .expect("valid page");
        let mut input = Scope::new();
        input.insert(
            "name".to_string(),
            crate::parser::Value::String(String::new()),
        );
        let mut session = Scope::new();
        let runtime = WebRuntime::new();

        let outcome = super::execute_action(
            &file,
            "rememberName",
            &Scope::new(),
            &input,
            &mut session,
            &runtime,
        )
        .await
        .expect("action");

        assert!(
            matches!(outcome, super::ActionOutcome::Fail(message) if message == "Name is required")
        );
        assert!(!session.contains_key("name"));
    }
}
