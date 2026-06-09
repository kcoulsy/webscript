use crate::diagnostic::{Diagnostic, Span};
use crate::expr;
use crate::parser::{
    ComponentCall, ComponentProp, LetDecl, LetValue, PropValue, SourceExpr, TemplateNode, Value,
    WebFile,
};
use std::collections::BTreeMap;

pub type Scope = BTreeMap<String, Value>;
pub type ComponentRegistry = BTreeMap<String, WebFile>;

pub fn render_with_components(
    file: &WebFile,
    params: &Scope,
    components: &ComponentRegistry,
) -> Result<String, Diagnostic> {
    let scope = scope_for(file, params)?;
    render_nodes(&file.template, &scope, components)
}

pub fn validate_with_components(file: &WebFile, components: &ComponentRegistry) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let scope = match scope_for(file, &Scope::new()) {
        Ok(scope) => scope,
        Err(error) => {
            diagnostics.push(error);
            base_scope_for(file, &Scope::new())
        }
    };
    validate_nodes(&file.template, &scope, components, &mut diagnostics);
    diagnostics
}

fn base_scope_for(file: &WebFile, params: &Scope) -> Scope {
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
                    .unwrap_or_else(|| sample_for_prop_type(&prop.type_name)),
            );
        }
    }
    for (name, value) in params {
        scope.insert(name.clone(), value.clone());
    }

    scope
}

fn scope_for(file: &WebFile, params: &Scope) -> Result<Scope, Diagnostic> {
    let mut scope = base_scope_for(file, params);
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
) -> Result<String, Diagnostic> {
    let mut html = String::new();

    for node in nodes {
        match node {
            TemplateNode::Text(value) => html.push_str(value),
            TemplateNode::Expr(expr) => {
                let value = evaluate_source_expr(expr, scope)?;
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
                let value = evaluate_source_expr(condition, scope)?;
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
                let value = evaluate_source_expr(source, scope)?;
                let Some(items) = value.as_array() else {
                    return Err(for_source_not_array(source));
                };

                for item in items.to_vec() {
                    let mut loop_scope = scope.clone();
                    loop_scope.insert(item_name.clone(), item);
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
    for node in nodes {
        match node {
            TemplateNode::Text(_) => {}
            TemplateNode::Expr(expr) => {
                if let Err(error) = evaluate_source_expr(expr, scope) {
                    diagnostics.push(error);
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
                match evaluate_source_expr(condition, scope) {
                    Ok(value) if value.as_bool().is_some() => {}
                    Ok(_) => diagnostics.push(if_condition_not_bool(condition)),
                    Err(error) => diagnostics.push(error),
                }
                validate_nodes(then_nodes, scope, components, diagnostics);
                validate_nodes(else_nodes, scope, components, diagnostics);
            }
            TemplateNode::For {
                item_name,
                source,
                body,
            } => match evaluate_source_expr(source, scope) {
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
    fn renders_expression_interpolation() {
        let file = parse(
            "@page \"/\"\n\n@let name = \"Ada\"\n@let visits: int = 2\n\n<h1>{\"Hello \" + name}</h1>\n<p>{visits + 1}</p>",
        )
        .expect("valid page");

        assert_eq!(
            render_with_components(&file, &Scope::new(), &ComponentRegistry::new())
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
            render_with_components(&file, &params, &ComponentRegistry::new()).expect("rendered"),
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

        assert_eq!(error.message, "unknown expression `post`");
        assert_eq!(error.span, Span::identifier(8, 5, "post"));

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
    fn renders_object_properties_in_markup_loops_and_components() {
        let component = parse(
            "@component PostPreview {\n  title: string\n  featured: bool = false\n}\n\n<article>{title}:{featured}</article>",
        )
        .expect("valid component");
        let file = parse(
            "@page \"/\"\n\n@let author = {\n  name: \"Ada\"\n  role: \"admin\"\n}\n@let posts = [\n  { title: \"Intro\", slug: \"intro\", featured: true },\n  { title: \"Launch\", slug: \"launch\", featured: false }\n]\n\n<h1>{author.name}</h1>\n@for post in posts {\n<PostPreview\n  title={post.title}\n  featured={post.featured}\n/>\n}",
        )
        .expect("valid page");
        let mut components = ComponentRegistry::new();
        components.insert("PostPreview".to_string(), component);

        assert_eq!(
            render_with_components(&file, &Scope::new(), &components).expect("rendered"),
            "<h1>Ada</h1>\n<article>Intro:true</article><article>Launch:false</article>"
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
        assert_eq!(diagnostics[0].message, "unknown expression `posts`");
        assert_eq!(diagnostics[1].message, "unknown expression `post`");
    }

    #[test]
    fn renders_components_with_typed_props_and_defaults() {
        let component = parse(
            "@component UserCard {\n  name: string\n  visits: int = 0\n}\n\n<article>{name}: {visits}</article>",
        )
        .expect("valid component");
        let page = parse(
            "@page \"/\"\n\n@let name: string = \"Ada\"\n\n<UserCard name={\"Dr. \" + name} />",
        )
        .expect("valid page");
        let mut components = ComponentRegistry::new();
        components.insert("UserCard".to_string(), component);

        assert_eq!(
            render_with_components(&page, &Scope::new(), &components).expect("rendered"),
            "<article>Dr. Ada: 0</article>"
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
        assert_eq!(
            diagnostics[1].message,
            "unknown prop `extra` for component `UserCard`"
        );
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
        assert_eq!(error.span, Span::identifier(3, 6, "name"));
    }
}
