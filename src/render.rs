use crate::client::{
    build_event_handler, compile_handler_body, handler_body_is_async, index_event_attributes,
    js_literal, resolve_signal_initial, value_signal_from_field_handler, HandlerCompileContext,
    IfBinding, IslandManifest, NamedHandler, SignalBinding, TextBinding, ValueBinding,
};
use crate::diagnostic::{Diagnostic, Span};
use crate::expr;
use crate::parser::{
    ClientBlock, ComponentCall, ComponentProp, EventBinding, LayoutUse, LetDecl, LetValue,
    PropValue, SourceExpr, StyleBlock, TemplateNode, Value, WebFile,
};
use crate::style;
use crate::runtime::WebRuntime;
use crate::schema_runtime::SchemaRuntime;
use crate::stmt::{self, BlockOutcome, Statement};
use std::collections::{BTreeMap, BTreeSet};

pub type Scope = BTreeMap<String, Value>;
pub type ComponentRegistry = BTreeMap<String, WebFile>;
pub type LayoutRegistry = BTreeMap<String, WebFile>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderOutput {
    pub html: String,
    pub islands: Vec<IslandManifest>,
    pub global_styles: Vec<String>,
    pub scoped_styles: BTreeMap<String, String>,
}

#[derive(Default)]
struct RenderContext {
    islands: Vec<IslandManifest>,
    island_counts: BTreeMap<String, usize>,
    global_styles: Vec<String>,
    scoped_styles: BTreeMap<String, String>,
    page_route: String,
    page_actions: BTreeMap<String, Option<String>>,
}

struct IslandBuildState<'a> {
    signal_names: BTreeSet<String>,
    signal_types: BTreeMap<String, String>,
    handler_names: BTreeSet<String>,
    event_counts: BTreeMap<String, usize>,
    manifest: &'a mut IslandManifest,
    page_actions: &'a BTreeMap<String, Option<String>>,
    action_url: &'a str,
}

struct ClientValidateContext {
    signals: BTreeSet<String>,
    handlers: BTreeSet<String>,
}

struct ForwardContext {
    event_bindings: Vec<EventBinding>,
    class_expr: Option<SourceExpr>,
    value_signal: Option<String>,
    applied: bool,
}

pub fn render_with_components(
    file: &WebFile,
    params: &Scope,
    components: &ComponentRegistry,
    runtime: &WebRuntime,
) -> Result<RenderOutput, Diagnostic> {
    if file.load.is_some() {
        return Err(Diagnostic::error(
            Span::at(1, 1),
            "@load requires async rendering",
            None,
        ));
    }
    let mut scope = base_scope_for(file, params);
    evaluate_lets(&file.lets, &mut scope)?;
    let mut context = RenderContext::default();
    let mut island = None;
    let mut forward_state = None;
    let shadowed = BTreeSet::new();
    let html = render_nodes(
        &file.template,
        &scope,
        components,
        runtime,
        &mut context,
        &mut island,
        None,
        &mut forward_state,
        &shadowed,
    )?;
    let scope_id = scope_id_for_file(file);
    let html = apply_file_styles(file, &scope_id, &mut context, html);
    Ok(render_output(html, &context))
}

pub async fn render_with_components_async(
    file: &WebFile,
    params: &Scope,
    components: &ComponentRegistry,
    runtime: &WebRuntime,
) -> Result<RenderOutput, Diagnostic> {
    render_page_async(file, params, components, &LayoutRegistry::new(), None, runtime).await
}

pub async fn render_page_async(
    file: &WebFile,
    params: &Scope,
    components: &ComponentRegistry,
    layouts: &LayoutRegistry,
    default_layout: Option<&str>,
    runtime: &WebRuntime,
) -> Result<RenderOutput, Diagnostic> {
    let scope = scope_for_async(file, params, runtime).await?;
    let mut context = page_render_context(file);
    let mut island = None;
    let mut forward_state = None;
    let shadowed = BTreeSet::new();
    let page_html = render_nodes(
        &file.template,
        &scope,
        components,
        runtime,
        &mut context,
        &mut island,
        None,
        &mut forward_state,
        &shadowed,
    )?;

    let layout_name = match &file.layout_use {
        Some(LayoutUse::None) => None,
        Some(LayoutUse::Apply { name, .. }) => Some(name.as_str()),
        None => default_layout,
    };

    let page_scope_id = scope_id_for_file(file);
    let page_html = apply_file_styles(file, &page_scope_id, &mut context, page_html);

    let Some(layout_name) = layout_name else {
        return Ok(render_output(page_html, &context));
    };

    let layout_file = layouts.get(layout_name).ok_or_else(|| {
        Diagnostic::error(
            Span::at(1, 1),
            format!("unknown layout `{layout_name}`"),
            None,
        )
    })?;

    let layout_scope = layout_scope_for(
        layout_file,
        file.layout_use.as_ref(),
        &scope,
    )?;
    let mut layout_island = None;
    let mut forward_state = None;
    let shadowed = BTreeSet::new();
    let mut html = render_nodes(
        &layout_file.template,
        &layout_scope,
        components,
        runtime,
        &mut context,
        &mut layout_island,
        Some(&page_html),
        &mut forward_state,
        &shadowed,
    )?;

    let layout_scope_id = scope_id_for_file(layout_file);
    html = apply_file_styles(layout_file, &layout_scope_id, &mut context, html);

    Ok(render_output(html, &context))
}

fn render_output(html: String, context: &RenderContext) -> RenderOutput {
    RenderOutput {
        html,
        islands: context.islands.clone(),
        global_styles: context.global_styles.clone(),
        scoped_styles: context.scoped_styles.clone(),
    }
}

fn page_render_context(file: &WebFile) -> RenderContext {
    let page_route = file
        .route
        .as_ref()
        .map(|route| route.raw.clone())
        .unwrap_or_else(|| "/".to_string());
    let page_actions = file
        .actions
        .iter()
        .map(|action| (action.name.clone(), action.input_schema.clone()))
        .collect();
    RenderContext {
        page_route,
        page_actions,
        ..RenderContext::default()
    }
}

fn scope_id_for_file(file: &WebFile) -> String {
    if let Some(route) = &file.route {
        return route.raw.clone();
    }
    if let Some(component) = &file.component {
        return component.name.clone();
    }
    if let Some(layout) = &file.layout {
        return layout.name.clone();
    }
    "page".to_string()
}

fn register_style_blocks(
    styles: &[StyleBlock],
    scope_id: &str,
    context: &mut RenderContext,
) -> bool {
    let mut has_scoped = false;
    for block in styles {
        if block.global {
            context.global_styles.push(block.css.clone());
        } else {
            has_scoped = true;
            let scoped_css = style::scope_css(&block.css, scope_id);
            context
                .scoped_styles
                .entry(scope_id.to_string())
                .and_modify(|existing| {
                    if !existing.is_empty() {
                        existing.push('\n');
                    }
                    existing.push_str(&scoped_css);
                })
                .or_insert(scoped_css);
        }
    }
    has_scoped
}

fn apply_file_styles(
    file: &WebFile,
    scope_id: &str,
    context: &mut RenderContext,
    html: String,
) -> String {
    let has_scoped = register_style_blocks(&file.styles, scope_id, context);
    if has_scoped {
        format!(r#"<div data-ws-style="{scope_id}">{html}</div>"#)
    } else {
        html
    }
}

pub fn validate_with_components(
    file: &WebFile,
    components: &ComponentRegistry,
    layouts: &LayoutRegistry,
    default_layout: Option<&str>,
    models: &BTreeMap<String, crate::db::ModelDecl>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut scope = base_scope_for(file, &Scope::new());
    merge_load_samples(file, &mut scope);
    if let Err(error) = evaluate_lets(&file.lets, &mut scope) {
        diagnostics.push(error);
    }
    merge_client_samples(file, &mut scope, &mut diagnostics);
    let client_ctx = client_validate_context(file);
    let allow_slot = file.layout.is_some();
    validate_nodes(
        &file.template,
        &scope,
        components,
        allow_slot,
        client_ctx.as_ref(),
        models,
        &mut diagnostics,
    );
    if let Some(load) = &file.load {
        validate_server_block(load, &mut diagnostics);
    }
    for action in &file.actions {
        validate_server_block_statements(&action.statements, &mut diagnostics);
    }
    if file.route.is_some() {
        validate_page_layout(file, layouts, default_layout, &mut diagnostics);
    }
    diagnostics
}

fn validate_page_layout(
    file: &WebFile,
    layouts: &LayoutRegistry,
    default_layout: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let layout_name = match &file.layout_use {
        Some(LayoutUse::None) => return,
        Some(LayoutUse::Apply { name, .. }) => name.as_str(),
        None => match default_layout {
            Some(name) => name,
            None => return,
        },
    };

    if !layouts.contains_key(layout_name) {
        let line = match &file.layout_use {
            Some(LayoutUse::Apply { line, .. }) => *line,
            _ => 1,
        };
        diagnostics.push(Diagnostic::error(
            Span::at(line, 1),
            format!("unknown layout `{layout_name}`"),
            None,
        ));
    }
}

fn layout_scope_for(
    layout_file: &WebFile,
    layout_use: Option<&LayoutUse>,
    page_scope: &Scope,
) -> Result<Scope, Diagnostic> {
    let declaration = layout_file.layout.as_ref().ok_or_else(|| {
        Diagnostic::error(
            Span::at(1, 1),
            "layout file is missing @layout declaration",
            None,
        )
    })?;

    let mut scope = Scope::new();
    for prop in &declaration.props {
        scope.insert(
            prop.name.clone(),
            prop.default
                .clone()
                .unwrap_or_else(|| sample_for_prop_type(&prop.type_name)),
        );
    }

    if let Some(LayoutUse::Apply { props, .. }) = layout_use {
        for prop in props {
            let value = evaluate_prop_value(&prop.value, page_scope, prop)?;
            scope.insert(prop.name.clone(), value);
        }
    }

    for prop in &declaration.props {
        if !scope.contains_key(&prop.name) && prop.default.is_none() {
            return Err(Diagnostic::error(
                Span::at(1, 1),
                format!("missing layout prop `{}`", prop.name),
                None,
            ));
        }
    }

    Ok(scope)
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
            scope.insert(name.clone(), sample_for_prop_type(type_name));
        }
    }
}

fn merge_client_samples(file: &WebFile, scope: &mut Scope, diagnostics: &mut Vec<Diagnostic>) {
    let Some(client) = &file.client else {
        return;
    };
    for signal in &client.signals {
        match resolve_signal_initial(signal, scope) {
            Ok(initial) => {
                scope.insert(signal.name.clone(), initial);
            }
            Err(error) => diagnostics.push(error),
        }
    }
}

fn client_validate_context(file: &WebFile) -> Option<ClientValidateContext> {
    let client = file.client.as_ref()?;
    let mut signals = BTreeSet::new();
    let mut handlers = BTreeSet::new();
    for signal in &client.signals {
        signals.insert(signal.name.clone());
    }
    for handler in &client.handlers {
        handlers.insert(handler.name.clone());
    }
    Some(ClientValidateContext { signals, handlers })
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
    Json(Value),
}

pub fn prepare_action_input(
    action: &crate::parser::ActionDecl,
    raw_input: &Scope,
    schemas: Option<&SchemaRuntime>,
) -> Result<Scope, Diagnostic> {
    let mut input = raw_input.clone();
    input.remove("_action");

    let Some(schema_name) = &action.input_schema else {
        return Ok(input);
    };

    let schemas = schemas.ok_or_else(|| {
        Diagnostic::error(
            Span::at(1, 1),
            format!(
                "action `{}` requires schema `{schema_name}` but schemas are unavailable",
                action.name
            ),
            None,
        )
    })?;

    let raw = Value::Object(input);
    let validated = schemas.validate(schema_name, raw).map_err(|message| {
        Diagnostic::error(
            Span::identifier(1, 1, &action.name),
            message,
            None,
        )
    })?;

    let Value::Object(fields) = validated else {
        return Err(Diagnostic::error(
            Span::identifier(1, 1, &action.name),
            format!("schema `{schema_name}` must validate to an object"),
            None,
        ));
    };

    Ok(fields)
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
    let validated_input = prepare_action_input(action, input, runtime.schemas())?;
    let mut scope = Scope::new();
    for (name, value) in params {
        scope.insert(name.clone(), value.clone());
    }
    scope.insert("input".to_string(), Value::Object(validated_input));
    scope.insert("session".to_string(), Value::Object(session.clone()));

    let outcome = runtime
        .execute_block_async(&action.statements, &mut scope, session)
        .await?;

    Ok(match outcome {
        Some(BlockOutcome::Redirect(target)) => ActionOutcome::Redirect(target),
        Some(BlockOutcome::Fail(message)) => ActionOutcome::Fail(message),
        Some(BlockOutcome::Return(value)) => ActionOutcome::Json(value),
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
    context: &mut RenderContext,
    island: &mut Option<&mut IslandBuildState<'_>>,
    slot_content: Option<&str>,
    forward_state: &mut Option<ForwardContext>,
    shadowed_props: &BTreeSet<String>,
) -> Result<String, Diagnostic> {
    let mut html = String::new();
    let mut scope = scope.clone();

    for node in nodes {
        match node {
            TemplateNode::Text(value) => html.push_str(value),
            TemplateNode::Slot => {
                if let Some(content) = slot_content {
                    html.push_str(content);
                }
            }
            TemplateNode::Expr(expr) => {
                let in_value_attr = html.ends_with("value=");
                html.push_str(&render_expr(
                    expr, &scope, island, in_value_attr, shadowed_props,
                )?);
            }
            TemplateNode::Component(call) => {
                html.push_str(&render_component(
                    call, &scope, components, runtime, context, island,
                )?);
            }
            TemplateNode::If {
                condition,
                then_nodes,
                else_nodes,
            } => {
                let client_if_signal = island.as_ref().and_then(|state| {
                    signal_expr_name(&condition.expr).and_then(|name| {
                        if state.signal_types.get(name) == Some(&"bool".to_string()) {
                            Some(name.to_string())
                        } else {
                            None
                        }
                    })
                });

                if let Some(signal_name) = client_if_signal {
                    let value = evaluate_source_expr(condition, &scope)?;
                    let Some(condition_value) = value.as_bool() else {
                        return Err(if_condition_not_bool(condition));
                    };

                    if let Some(island_state) = island.as_deref_mut() {
                        if !island_state
                            .manifest
                            .if_bindings
                            .iter()
                            .any(|binding| binding.signal == signal_name)
                        {
                            island_state.manifest.if_bindings.push(IfBinding {
                                signal: signal_name.clone(),
                            });
                        }
                    }

                    let then_display = if condition_value { "" } else { "none" };
                    html.push_str(&format!(
                        "<div data-ws-if=\"{signal_name}\" data-ws-branch=\"then\" style=\"display:{then_display}\">"
                    ));
                    html.push_str(&render_nodes(
                        then_nodes,
                        &scope,
                        components,
                        runtime,
                        context,
                        island,
                        slot_content,
                        forward_state,
                        shadowed_props,
                    )?);
                    html.push_str("</div>");

                    if !else_nodes.is_empty() {
                        let else_display = if condition_value { "none" } else { "" };
                        html.push_str(&format!(
                            "<div data-ws-if=\"{signal_name}\" data-ws-branch=\"else\" style=\"display:{else_display}\">"
                        ));
                        html.push_str(&render_nodes(
                            else_nodes,
                            &scope,
                            components,
                            runtime,
                            context,
                            island,
                            slot_content,
                            forward_state,
                            shadowed_props,
                        )?);
                        html.push_str("</div>");
                    }
                    continue;
                }

                let value = evaluate_source_expr(condition, &scope)?;
                let Some(condition_value) = value.as_bool() else {
                    return Err(if_condition_not_bool(condition));
                };

                if condition_value {
                    html.push_str(&render_nodes(
                        then_nodes,
                        &scope,
                        components,
                        runtime,
                        context,
                        island,
                        slot_content,
                        forward_state,
                        shadowed_props,
                    )?);
                } else {
                    html.push_str(&render_nodes(
                        else_nodes,
                        &scope,
                        components,
                        runtime,
                        context,
                        island,
                        slot_content,
                        forward_state,
                        shadowed_props,
                    )?);
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
                    html.push_str(&render_nodes(
                        body,
                        &loop_scope,
                        components,
                        runtime,
                        context,
                        island,
                        slot_content,
                        forward_state,
                        shadowed_props,
                    )?);
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
            TemplateNode::EventBinding(binding) => {
                register_event_binding(binding, island, false)?;
            }
        }
    }

    Ok(html)
}

fn render_expr(
    expr: &SourceExpr,
    scope: &Scope,
    island: &mut Option<&mut IslandBuildState<'_>>,
    in_value_attr: bool,
    shadowed_props: &BTreeSet<String>,
) -> Result<String, Diagnostic> {
    if let Some(island) = island.as_deref_mut() {
        if let Some(signal_name) = signal_expr_name(&expr.expr) {
            if island.signal_names.contains(signal_name) && !shadowed_props.contains(signal_name) {
                let value = evaluate_source_expr(expr, scope)?;
                let type_name = island
                    .signal_types
                    .get(signal_name)
                    .map(String::as_str)
                    .unwrap_or("string");

                if in_value_attr && type_name == "string" {
                    if !island.manifest.value_bindings.iter().any(|binding| {
                        binding.signal == signal_name
                    }) {
                        island.manifest.value_bindings.push(ValueBinding {
                            signal: signal_name.to_string(),
                            handler_index: 0,
                        });
                    }
                    return Ok(escape_html(&value.render()));
                }

                if type_name == "string" {
                    if !island
                        .manifest
                        .text_bindings
                        .iter()
                        .any(|binding| binding.signal == signal_name)
                    {
                        island.manifest.text_bindings.push(TextBinding {
                            signal: signal_name.to_string(),
                        });
                    }
                    return Ok(format!(
                        "<span data-ws-text=\"{signal_name}\">{}</span>",
                        escape_html(&value.render())
                    ));
                }

                if !island
                    .manifest
                    .text_bindings
                    .iter()
                    .any(|binding| binding.signal == signal_name)
                {
                    island.manifest.text_bindings.push(TextBinding {
                        signal: signal_name.to_string(),
                    });
                }
                return Ok(format!(
                    "<span data-ws-text=\"{signal_name}\">{}</span>",
                    escape_html(&value.render())
                ));
            }
        }
    }

    let value = evaluate_source_expr(expr, scope)?;
    Ok(escape_html(&value.render()))
}

fn signal_expr_name(expr: &expr::Expr) -> Option<&str> {
    match expr {
        expr::Expr::Path(path) if path.len() == 1 => Some(&path[0]),
        _ => None,
    }
}

fn apply_forward_bindings(
    html: &str,
    forward: &mut ForwardContext,
    scope: &Scope,
    island: &mut Option<&mut IslandBuildState<'_>>,
) -> Result<String, Diagnostic> {
    const MARKER: &str = "data-ws-bind";
    let Some(marker_pos) = html.find(MARKER) else {
        return Ok(html.to_string());
    };

    let tag_start = html[..marker_pos]
        .rfind('<')
        .ok_or_else(|| forward_bind_target_missing_at(1))?;
    let after_marker = &html[marker_pos..];
    let (tag_end, self_closing) = if let Some(index) = after_marker.find("/>") {
        (marker_pos + index + 2, true)
    } else if let Some(index) = after_marker.find('>') {
        (marker_pos + index + 1, false)
    } else {
        return Err(forward_bind_target_missing_at(1));
    };

    let mut tag = html[tag_start..tag_end].to_string();
    if self_closing {
        let trimmed = tag.trim_end();
        tag = trimmed
            .strip_suffix("/>")
            .or_else(|| trimmed.strip_suffix('>').and_then(|value| value.strip_suffix('/')))
            .unwrap_or(trimmed)
            .trim_end()
            .to_string();
    } else if tag.ends_with('>') {
        tag.pop();
    } else {
        return Err(forward_bind_target_missing_at(1));
    }

    tag = tag.replace(" data-ws-bind", "");
    if tag.contains(MARKER) {
        tag = tag.replace(MARKER, "");
    }

    if let Some(class_expr) = &forward.class_expr {
        let extra = evaluate_source_expr(class_expr, scope)?;
        tag = merge_class_on_tag(&tag, &extra.render());
    }

    for binding in &forward.event_bindings {
        tag.push_str(&format!(" data-ws-{}", binding.event));
        register_event_binding(binding, island, true)?;
    }

    if let Some(signal) = &forward.value_signal {
        tag.push_str(&format!(r#" data-ws-value="{signal}""#));
    }

    if self_closing {
        tag.push_str(" />");
    } else {
        tag.push('>');
    }
    forward.applied = true;
    Ok(format!(
        "{}{}{}",
        &html[..tag_start],
        tag,
        &html[tag_end..]
    ))
}

fn merge_class_on_tag(tag: &str, extra: &str) -> String {
    if extra.is_empty() {
        return tag.to_string();
    }

    let Some(class_pos) = tag.find("class=") else {
        return format!(r#"{tag} class="{extra}""#);
    };

    let value_start = class_pos + 6;
    if tag.as_bytes().get(value_start) != Some(&b'"') {
        return format!(r#"{tag} class="{extra}""#);
    }

    let inner_start = value_start + 1;
    let Some(relative_end) = tag[inner_start..].find('"') else {
        return format!(r#"{tag} class="{extra}""#);
    };
    let inner_end = inner_start + relative_end;
    let existing = &tag[inner_start..inner_end];
    let merged = if existing.is_empty() {
        extra.to_string()
    } else {
        format!("{existing} {extra}")
    };
    format!(
        "{}class=\"{}{}",
        &tag[..class_pos],
        merged,
        &tag[inner_end..]
    )
}

fn register_event_binding(
    binding: &EventBinding,
    island: &mut Option<&mut IslandBuildState<'_>>,
    require_island: bool,
) -> Result<(), Diagnostic> {
    let Some(island) = island.as_deref_mut() else {
        if require_island {
            return Err(forwarded_event_outside_client(binding));
        }
        return Ok(());
    };

    let index = *island
        .event_counts
        .entry(binding.event.clone())
        .or_insert(0);
    island.event_counts.insert(binding.event.clone(), index + 1);
    let compile_ctx = HandlerCompileContext {
        signals: &island.signal_names,
        handlers: &island.handler_names,
        page_actions: island.page_actions,
        action_url: island.action_url,
        param: "event",
        is_submit_context: binding.event == "submit",
    };
    let handler = build_event_handler(binding, index, &compile_ctx)?;
    if binding.event == "input" || binding.event == "change" {
        if let Some(signal) = value_signal_from_field_handler(&binding.handler_source) {
            if island.signal_names.contains(&signal)
                && !island
                    .manifest
                    .value_bindings
                    .iter()
                    .any(|value| value.signal == signal && value.handler_index == index)
            {
                island.manifest.value_bindings.push(ValueBinding {
                    signal,
                    handler_index: index,
                });
            }
        }
    }
    island.manifest.event_handlers.push(handler);
    Ok(())
}

fn forwarded_event_outside_client(binding: &EventBinding) -> Diagnostic {
    Diagnostic::error(
        Span::new(binding.line, binding.column, binding.column + 1),
        format!(
            "@{event} on a component call requires a parent `@client` block",
            event = binding.event
        ),
        None,
    )
}

fn forward_bind_target_missing(call: &ComponentCall) -> Diagnostic {
    Diagnostic::error(
        Span::identifier(call.line, call.column, &call.name),
        format!(
            "component `{name}` has no `data-ws-bind` target for forwarded attributes",
            name = call.name
        ),
        Some("add `data-ws-bind` to the element that should receive events and class".to_string()),
    )
}

fn forward_bind_target_missing_at(line: usize) -> Diagnostic {
    Diagnostic::error(
        Span::at(line, 1),
        "malformed `data-ws-bind` target",
        None,
    )
}

fn component_template_has_bind_target(component: &WebFile) -> bool {
    component
        .template
        .iter()
        .any(|node| matches!(node, TemplateNode::Text(text) if text.contains("data-ws-bind")))
}

fn render_component(
    call: &ComponentCall,
    scope: &Scope,
    components: &ComponentRegistry,
    runtime: &WebRuntime,
    context: &mut RenderContext,
    island: &mut Option<&mut IslandBuildState<'_>>,
) -> Result<String, Diagnostic> {
    let component = components
        .get(&call.name)
        .ok_or_else(|| unknown_component(call))?;
    let component_scope = component_scope(call, component, scope)?;

    if let Some(client) = &component.client {
        return render_client_component(
            call,
            component,
            &component_scope,
            client,
            components,
            runtime,
            context,
        );
    }

    let declaration = component.component.as_ref().expect("component checked");
    let shadowed: BTreeSet<String> = declaration
        .props
        .iter()
        .map(|prop| prop.name.clone())
        .collect();
    let value_signal = call
        .props
        .iter()
        .find(|prop| prop.name == "value")
        .and_then(|prop| {
            if let PropValue::Expr(expr) = &prop.value {
                signal_expr_name(&expr.expr).and_then(|signal| {
                    island
                        .as_ref()
                        .and_then(|state| state.signal_names.contains(signal).then_some(signal))
                        .map(str::to_string)
                })
            } else {
                None
            }
        });
    let has_forward = !call.event_bindings.is_empty()
        || call.class_expr.is_some()
        || value_signal.is_some();
    let mut forward_state = has_forward.then(|| ForwardContext {
        event_bindings: call.event_bindings.clone(),
        class_expr: call.class_expr.clone(),
        value_signal,
        applied: false,
    });
    let mut html = render_nodes(
        &component.template,
        &component_scope,
        components,
        runtime,
        context,
        island,
        None,
        &mut forward_state,
        &shadowed,
    )?;
    if let Some(forward) = &mut forward_state {
        if !forward.applied && has_forward {
            html = apply_forward_bindings(&html, forward, scope, island)?;
        }
        if !forward.applied && has_forward {
            return Err(forward_bind_target_missing(call));
        }
    }
    let scope_id = component
        .component
        .as_ref()
        .map(|decl| decl.name.as_str())
        .unwrap_or(&call.name);
    Ok(apply_file_styles(component, scope_id, context, html))
}

fn render_client_component(
    call: &ComponentCall,
    component: &WebFile,
    component_scope: &Scope,
    client: &ClientBlock,
    components: &ComponentRegistry,
    runtime: &WebRuntime,
    context: &mut RenderContext,
) -> Result<String, Diagnostic> {
    let island_index = context
        .island_counts
        .entry(call.name.clone())
        .and_modify(|count| *count += 1)
        .or_insert(0);
    let island_id = format!("{}-{}", call.name, island_index);

    let mut signal_bindings = Vec::new();
    let mut render_scope = component_scope.clone();
    let mut signal_names = BTreeSet::new();
    let mut signal_types = BTreeMap::new();

    for signal in &client.signals {
        let initial = resolve_signal_initial(signal, component_scope)?;
        signal_names.insert(signal.name.clone());
        signal_types.insert(signal.name.clone(), signal.type_name.clone());
        signal_bindings.push(SignalBinding {
            name: signal.name.clone(),
            type_name: signal.type_name.clone(),
            initial: initial.clone(),
        });
        render_scope.insert(signal.name.clone(), initial);
    }

    let mut handler_names = BTreeSet::new();
    for handler in &client.handlers {
        handler_names.insert(handler.name.clone());
    }

    let page_route = context.page_route.clone();
    let page_actions = context.page_actions.clone();
    let mut named_handlers = Vec::new();
    for handler in &client.handlers {
        let handler_ctx = HandlerCompileContext {
            signals: &signal_names,
            handlers: &handler_names,
            page_actions: &page_actions,
            action_url: &page_route,
            param: handler.param_name.as_deref().unwrap_or("event"),
            is_submit_context: false,
        };
        let js_body = compile_handler_body(&handler.body, &handler_ctx, handler.line, 1)?;
        named_handlers.push(NamedHandler {
            name: handler.name.clone(),
            param_name: handler.param_name.clone().unwrap_or_default(),
            js_body,
            is_async: handler_body_is_async(&handler.body),
        });
    }

    let bootstrap = component_scope
        .get("initialTodos")
        .filter(|_| handler_names.contains("applyTodos"))
        .map(|value| format!("handlers.applyTodos({})", js_literal(value)));

    let mut manifest = IslandManifest {
        id: island_id.clone(),
        component: call.name.clone(),
        action_url: page_route.clone(),
        signals: signal_bindings,
        event_handlers: Vec::new(),
        named_handlers,
        text_bindings: Vec::new(),
        value_bindings: Vec::new(),
        html_bindings: Vec::new(),
        if_bindings: Vec::new(),
        bootstrap,
    };

    let mut island_state = IslandBuildState {
        signal_names: signal_names.clone(),
        signal_types,
        handler_names: handler_names.clone(),
        event_counts: BTreeMap::new(),
        manifest: &mut manifest,
        page_actions: &page_actions,
        action_url: &page_route,
    };

    let mut island_ref = Some(&mut island_state);
    let mut forward_state = None;
    let shadowed = BTreeSet::new();
    let inner = render_nodes(
        &component.template,
        &render_scope,
        components,
        runtime,
        context,
        &mut island_ref,
        None,
        &mut forward_state,
        &shadowed,
    )?;

    let inner = index_event_attributes(&inner, &manifest.event_handlers);
    let inner = annotate_value_bindings(&inner, &manifest.value_bindings);
    for signal in &signal_names {
        let marker = format!(r#"data-ws-html="{signal}""#);
        if inner.contains(&marker) {
            manifest.html_bindings.push(crate::client::HtmlBinding {
                signal: signal.to_string(),
            });
        }
    }

    context.islands.push(manifest);

    let scope_id = call.name.as_str();
    let has_scoped = register_style_blocks(&component.styles, scope_id, context);
    if has_scoped {
        Ok(format!(
            r#"<div data-ws-island="{island_id}" data-ws-component="{component}" data-ws-style="{scope_id}">{inner}</div>"#,
            component = call.name
        ))
    } else {
        Ok(format!(
            "<div data-ws-island=\"{island_id}\" data-ws-component=\"{component}\">{inner}</div>",
            component = call.name
        ))
    }
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
    allow_slot: bool,
    client_ctx: Option<&ClientValidateContext>,
    models: &BTreeMap<String, crate::db::ModelDecl>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut scope = scope.clone();
    for node in nodes {
        match node {
            TemplateNode::Text(_) => {}
            TemplateNode::Slot if !allow_slot => diagnostics.push(Diagnostic::error(
                Span::at(1, 1),
                "<slot /> is only allowed in layout templates",
                None,
            )),
            TemplateNode::Slot => {}
            TemplateNode::Expr(expr) => {
                if let Err(error) = evaluate_source_expr(expr, &scope) {
                    diagnostics.push(error);
                }
            }
            TemplateNode::Component(call) => {
                validate_component_call(call, &scope, components, client_ctx, diagnostics)
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
                validate_nodes(
                    then_nodes,
                    &scope,
                    components,
                    allow_slot,
                    client_ctx,
                    models,
                    diagnostics,
                );
                validate_nodes(
                    else_nodes,
                    &scope,
                    components,
                    allow_slot,
                    client_ctx,
                    models,
                    diagnostics,
                );
            }
            TemplateNode::For {
                item_name,
                source,
                body,
            } => match evaluate_source_expr(source, &scope) {
                Ok(value) if value.as_array().is_some() => {
                    let mut loop_scope = scope.clone();
                    if let Some(sample) = array_loop_sample(&value, models) {
                        loop_scope.insert(item_name.clone(), sample);
                    }
                    validate_nodes(
                        body,
                        &loop_scope,
                        components,
                        allow_slot,
                        client_ctx,
                        models,
                        diagnostics,
                    );
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
            TemplateNode::EventBinding(binding) => {
                let empty_signals = BTreeSet::new();
                let empty_handlers = BTreeSet::new();
                let (signals, handlers) = match client_ctx {
                    Some(ctx) => (&ctx.signals, &ctx.handlers),
                    None => (&empty_signals, &empty_handlers),
                };
                let empty_actions = BTreeMap::new();
                let compile_ctx = HandlerCompileContext {
                    signals,
                    handlers,
                    page_actions: &empty_actions,
                    action_url: "/",
                    param: "event",
                    is_submit_context: binding.event == "submit",
                };
                if let Err(error) = build_event_handler(binding, 0, &compile_ctx) {
                    diagnostics.push(error);
                }
            }
        }
    }
}

fn validate_component_call(
    call: &ComponentCall,
    scope: &Scope,
    components: &ComponentRegistry,
    client_ctx: Option<&ClientValidateContext>,
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

    if let Some(class_expr) = &call.class_expr {
        if let Err(error) = evaluate_source_expr(class_expr, scope) {
            diagnostics.push(error);
        }
    }

    if client_ctx.is_none() {
        for binding in &call.event_bindings {
            diagnostics.push(forwarded_event_outside_client(binding));
        }
    } else if !call.event_bindings.is_empty() {
        let ctx = client_ctx.expect("client context checked");
        let empty_actions = BTreeMap::new();
        let compile_ctx = HandlerCompileContext {
            signals: &ctx.signals,
            handlers: &ctx.handlers,
            page_actions: &empty_actions,
            action_url: "/",
            param: "event",
            is_submit_context: false,
        };
        for binding in &call.event_bindings {
            if let Err(error) = build_event_handler(binding, 0, &compile_ctx) {
                diagnostics.push(error);
            }
        }
    }

    let has_forward = !call.event_bindings.is_empty() || call.class_expr.is_some();
    if has_forward && !component_template_has_bind_target(component) {
        diagnostics.push(forward_bind_target_missing(call));
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
    if type_name.ends_with("[]") {
        return sample_for_type(type_name);
    }
    if is_object_type_name(type_name) {
        return Value::Object(BTreeMap::new());
    }
    sample_for_type(type_name)
}

fn sample_for_element_type(
    element_type: &str,
    models: &BTreeMap<String, crate::db::ModelDecl>,
) -> Value {
    if let Some(model) = models.get(element_type) {
        let mut fields = BTreeMap::new();
        for field in &model.fields {
            fields.insert(field.name.clone(), sample_for_prop_type(&field.type_name));
        }
        return Value::Object(fields);
    }
    sample_for_prop_type(element_type)
}

fn array_loop_sample(
    value: &Value,
    models: &BTreeMap<String, crate::db::ModelDecl>,
) -> Option<Value> {
    let Value::Array {
        element_type,
        values,
    } = value
    else {
        return None;
    };
    values
        .first()
        .cloned()
        .or_else(|| Some(sample_for_element_type(element_type, models)))
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

fn annotate_value_bindings(html: &str, bindings: &[ValueBinding]) -> String {
    if bindings.is_empty() {
        return html.to_string();
    }

    let mut output = html.to_string();
    for binding in bindings {
        let marker = format!("data-ws-value=\"{}\"", binding.signal);
        if output.contains(&marker) {
            continue;
        }

        let indexed_attrs = [
            format!("data-ws-input=\"{}\"", binding.handler_index),
            format!("data-ws-change=\"{}\"", binding.handler_index),
        ];
        let mut inserted = false;
        for indexed in &indexed_attrs {
            if let Some(pos) = output.find(indexed) {
                let insert_at = pos + indexed.len();
                output.insert_str(insert_at, &format!(" {marker}"));
                inserted = true;
                break;
            }
        }
        if inserted {
            continue;
        }

        if let Some(pos) = output.find("data-ws-input") {
            let insert_at = pos + "data-ws-input".len();
            if output[insert_at..].starts_with('=') {
                if let Some(end) = output[insert_at + 1..].find('"') {
                    let insert_at = insert_at + 1 + end + 1;
                    output.insert_str(insert_at, &format!(" {marker}"));
                }
            } else {
                output.insert_str(insert_at, &format!(" {marker}"));
            }
        }
    }
    output
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
    use crate::parser::{parse, WebFile};
    use crate::runtime::WebRuntime;

    fn register_ui_primitives(components: &mut ComponentRegistry) {
        let primitives = [
            ("UI.Button", include_str!("../app/components/UI/Button.web")),
            ("UI.Input", include_str!("../app/components/UI/Input.web")),
            ("UI.Label", include_str!("../app/components/UI/Label.web")),
            ("UI.Card", include_str!("../app/components/UI/Card.web")),
            ("UI.Separator", include_str!("../app/components/UI/Separator.web")),
        ];
        for (name, source) in primitives {
            let file = parse(source).expect("ui primitive");
            components.insert(name.to_string(), file);
        }
    }

    fn with_ui_primitives(component: WebFile) -> ComponentRegistry {
        let mut components = ComponentRegistry::new();
        register_ui_primitives(&mut components);
        let name = component
            .component
            .as_ref()
            .expect("component declaration")
            .name
            .clone();
        components.insert(name, component);
        components
    }

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
            .expect("rendered")
            .html,
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
            .expect("rendered")
            .html,
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
            .expect("rendered")
            .html,
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
            .expect("rendered")
            .html,
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
            .expect("rendered")
            .html,
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

        let output = super::render_with_components_async(
            &file,
            &Scope::new(),
            &ComponentRegistry::new(),
            &runtime,
        )
        .await
        .expect("rendered");

        assert_eq!(output.html, "<h1>Loaded</h1>");
    }

    #[tokio::test]
    async fn load_try_catch_captures_throw() {
        let file = parse(
            "@page \"/\"\n\n@load {\n  error: string = \"\"\n  try {\n    throw(\"boom\")\n  } catch err {\n    error = err.message\n  }\n}\n\n<p>{error}</p>",
        )
        .expect("valid page");
        let runtime = WebRuntime::new();
        let output = super::render_with_components_async(
            &file,
            &Scope::new(),
            &ComponentRegistry::new(),
            &runtime,
        )
        .await
        .expect("rendered");

        assert!(output.html.contains("boom"));
    }

    #[tokio::test]
    async fn load_try_catch_captures_async_timeout() {
        let file = parse(
            "@page \"/\"\n\n@load {\n  timedOut: bool = false\n  task := spawn(sleep(50ms))\n  try {\n    _: object = await timeout(1ms, task)\n  } catch err {\n    timedOut = err.message == \"timeout\"\n  }\n}\n\n<p>{timedOut}</p>",
        )
        .expect("valid page");
        let runtime = WebRuntime::new();
        let output = super::render_with_components_async(
            &file,
            &Scope::new(),
            &ComponentRegistry::new(),
            &runtime,
        )
        .await
        .expect("rendered");

        assert!(output.html.contains("true"));
    }

    #[tokio::test]
    #[ignore = "requires network access to jsonplaceholder.typicode.com"]
    async fn fetch_demo_fetches_json_with_schema() {
        let file = parse(include_str!("../app/pages/fetch-demo.web")).expect("valid page");
        let root = std::env::current_dir().expect("cwd");
        let runtime = WebRuntime::with_database(root).expect("runtime");
        let output = super::render_with_components_async(
            &file,
            &Scope::new(),
            &ComponentRegistry::new(),
            &runtime,
        )
        .await
        .expect("rendered");

        assert!(output.html.contains("delectus") || output.html.contains("Title:"));
    }

    #[test]
    fn renders_namespaced_component() {
        let button = parse(
            "@component UI.Button {\n  label: string = \"Click\"\n}\n\n<button>{label}</button>",
        )
        .expect("button");
        let page = parse("@page \"/\"\n\n<UI.Button label=\"Save\" />").expect("page");
        let mut components = ComponentRegistry::new();
        components.insert("UI.Button".to_string(), button);

        let output = render_with_components(
            &page,
            &Scope::new(),
            &components,
            &WebRuntime::new(),
        )
        .expect("rendered");

        assert!(output.html.contains("<button>Save</button>"));
    }

    #[test]
    fn renders_client_counter_island() {
        let counter = parse(include_str!("../app/components/Counter.web")).expect("counter");
        let page = parse("@page \"/counter\"\n\n<Counter initial={5} label=\"Score\" />")
            .expect("page");
        let components = with_ui_primitives(counter);

        let output = render_with_components(
            &page,
            &Scope::new(),
            &components,
            &WebRuntime::new(),
        )
        .expect("rendered");

        assert!(output.html.contains("data-ws-island=\"Counter-0\""));
        assert!(output.html.contains("data-ws-click=\"0\""));
        assert!(output.html.contains("data-ws-click=\"1\""));
        assert!(
            !output.html.contains(r#""> data-ws-click="#),
            "click handler must be on the opening tag, not in button text: {}",
            output.html
        );
        assert!(output.html.contains("data-ws-text=\"count\">5</span>"));
        assert_eq!(output.islands.len(), 1);
        assert_eq!(output.islands[0].signals[0].initial, crate::parser::Value::Int(5));
        assert_eq!(output.islands[0].event_handlers.len(), 3);

        let script = crate::client::render_island_script(&output.islands[0]);
        assert!(script.contains("WebScript.signal(5)"));
        assert!(script.contains("addEventListener('click'"));
    }

    #[test]
    fn renders_client_details_toggle_island() {
        let details = parse(include_str!("../app/components/Details.web")).expect("details");
        let page = parse("@page \"/\"\n\n<Details title=\"Notes\" />").expect("page");
        let components = with_ui_primitives(details);

        let output = render_with_components(
            &page,
            &Scope::new(),
            &components,
            &WebRuntime::new(),
        )
        .expect("rendered");

        assert!(output.html.contains("data-ws-if=\"open\""));
        assert_eq!(output.islands[0].if_bindings.len(), 1);
        let script = crate::client::render_island_script(&output.islands[0]);
        assert!(script.contains("if_open_then.forEach"));
        assert!(output.html.contains("details-panel"));
    }

    #[test]
    fn renders_client_greeting_input_island() {
        let greeting = parse(include_str!("../app/components/Greeting.web")).expect("greeting");
        let page = parse("@page \"/\"\n\n<Greeting />").expect("page");
        let components = with_ui_primitives(greeting);

        let output = render_with_components(
            &page,
            &Scope::new(),
            &components,
            &WebRuntime::new(),
        )
        .expect("rendered");

        assert!(
            output.html.contains("data-ws-input=\"0\" data-ws-value=\"name\"")
                || output.html.contains("data-ws-value=\"name\" data-ws-input=\"0\"")
        );
        assert!(
            !output.html.contains(r#"name="<span data-ws-text"#),
            "input name prop must not bind as text spans: {}",
            output.html
        );
        assert!(
            output.html.contains("id=greeting-name")
                && output.html.contains("data-ws-input=\"0\"")
                && output.html.contains("data-ws-value=\"name\""),
            "expected intact input with forwarded bindings: {}",
            output.html
        );
        let script = crate::client::render_island_script(&output.islands[0]);
        assert!(script.contains("addEventListener('input'"));
        assert!(script.contains("value_name"));
    }

    #[test]
    fn renders_event_demo_submit_prevent() {
        let demo = parse(include_str!("../app/components/EventDemo.web")).expect("event demo");
        let page = parse("@page \"/\"\n\n<EventDemo />").expect("page");
        let components = with_ui_primitives(demo);

        let output = render_with_components(
            &page,
            &Scope::new(),
            &components,
            &WebRuntime::new(),
        )
        .expect("rendered");

        assert!(output.html.contains("data-ws-submit=\"0\""));
        assert!(
            output.html.contains("data-ws-change=\"0\" data-ws-value=\"note\"")
                || output.html.contains("data-ws-value=\"note\" data-ws-change=\"0\"")
        );
        let script = crate::client::render_island_script(&output.islands[0]);
        assert!(script.contains("event.preventDefault();handlers.save(event)"));
        assert!(script.contains("keyCount"));
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

    #[tokio::test]
    async fn wraps_page_content_in_layout_slot() {
        let layout = parse(include_str!("../app/layouts/AppLayout.web")).expect("layout");
        let page = parse("@page \"/\"\n\n<main><h1>Hello</h1></main>").expect("page");
        let mut layouts = super::LayoutRegistry::new();
        layouts.insert("AppLayout".to_string(), layout);
        let runtime = WebRuntime::new();

        let output = super::render_page_async(
            &page,
            &Scope::new(),
            &ComponentRegistry::new(),
            &layouts,
            Some("AppLayout"),
            &runtime,
        )
        .await
        .expect("rendered");

        assert!(output.html.contains("app-layout"));
        assert!(output.html.contains("<main><h1>Hello</h1></main>"));
    }

    #[test]
    fn renders_scoped_component_styles() {
        let counter = parse(include_str!("../app/components/Counter.web")).expect("counter");
        let page = parse("@page \"/counter\"\n\n<Counter initial={5} label=\"Score\" />")
            .expect("page");
        let components = with_ui_primitives(counter);

        let output = render_with_components(
            &page,
            &Scope::new(),
            &components,
            &WebRuntime::new(),
        )
        .expect("rendered");

        assert!(output.html.contains(r#"data-ws-style="Counter""#));
        assert!(output.scoped_styles.contains_key("Counter"));
        assert!(output
            .scoped_styles
            .get("Counter")
            .expect("counter styles")
            .contains(r#"[data-ws-style="Counter"] .counter"#));
    }

    #[test]
    fn renders_global_page_styles() {
        let page = parse(
            "@page \"/demo\"\n\n<main class=\"demo\"></main>\n\n@style global {\n  body { margin: 0; }\n}",
        )
        .expect("page");

        let output = render_with_components(
            &page,
            &Scope::new(),
            &ComponentRegistry::new(),
            &WebRuntime::new(),
        )
        .expect("rendered");

        assert_eq!(output.global_styles.len(), 1);
        assert!(output.global_styles[0].contains("body"));
        assert!(output.scoped_styles.is_empty());
    }

    #[test]
    fn dedupes_scoped_styles_for_multiple_component_instances() {
        let counter = parse(include_str!("../app/components/Counter.web")).expect("counter");
        let page = parse(
            "@page \"/counter\"\n\n<Counter initial={1} />\n<Counter initial={2} />",
        )
        .expect("page");
        let components = with_ui_primitives(counter);

        let output = render_with_components(
            &page,
            &Scope::new(),
            &components,
            &WebRuntime::new(),
        )
        .expect("rendered");

        assert!(output.scoped_styles.contains_key("Counter"));
        assert!(output.scoped_styles.contains_key("UI.Button"));
        assert_eq!(output.html.matches(r#"data-ws-style="Counter""#).count(), 2);
    }
}
