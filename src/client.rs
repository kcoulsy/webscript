use crate::diagnostic::{Diagnostic, Span};
use crate::expr;
use crate::parser::{
    ClientInitial, ClientSignalDecl, ComponentCall, EventBinding, PropValue, SourceExpr,
    TemplateNode, Value, WebFile,
};
use crate::schema::parser_value_to_json;
use std::collections::{BTreeMap, BTreeSet};

pub const RUNTIME_PATH: &str = "/.web/runtime.js";
const DEFAULT_EVENT_PARAM: &str = "event";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IslandManifest {
    pub id: String,
    pub component: String,
    pub action_url: String,
    pub signals: Vec<SignalBinding>,
    pub event_handlers: Vec<EventHandler>,
    pub named_handlers: Vec<NamedHandler>,
    pub text_bindings: Vec<TextBinding>,
    pub value_bindings: Vec<ValueBinding>,
    pub html_bindings: Vec<HtmlBinding>,
    pub if_bindings: Vec<IfBinding>,
    pub attr_bindings: Vec<AttrBinding>,
    pub for_bindings: Vec<ForBinding>,
    pub bootstrap: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalBinding {
    pub name: String,
    pub type_name: String,
    pub initial: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedHandler {
    pub name: String,
    pub param_name: String,
    pub js_body: String,
    pub is_async: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventHandler {
    pub event: String,
    pub index: usize,
    pub handler_source: String,
    pub js_body: String,
    pub param_name: String,
    pub prevent_default: bool,
    pub stop_propagation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBinding {
    pub signal: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueBinding {
    pub signal: String,
    pub handler_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlBinding {
    pub signal: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfBinding {
    pub signal: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttrBinding {
    pub attribute: String,
    pub index: usize,
    pub js_expr: String,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForBinding {
    pub signal: String,
    pub item_name: String,
    pub js_fn_body: String,
}

#[derive(Debug, Clone)]
pub struct HandlerCompileContext<'a> {
    pub signals: &'a BTreeSet<String>,
    pub handlers: &'a BTreeSet<String>,
    pub locals: &'a BTreeSet<String>,
    pub page_actions: &'a BTreeMap<String, Option<String>>,
    pub action_url: &'a str,
    pub param: &'a str,
    pub is_submit_context: bool,
}

pub fn client_runtime_script() -> &'static str {
    r#"window.WebScript = window.WebScript || {};
WebScript.defer = {
  init() {},
  replace(id, html) {
    const el = document.querySelector(`[data-web-defer="${id}"]`);
    if (!el) return;
    const template = document.createElement("template");
    template.innerHTML = html;
    const replacement = template.content.firstElementChild;
    if (replacement) {
      el.replaceWith(replacement);
      WebScript.defer.hydrateIslands(replacement);
      return;
    }
    el.outerHTML = html;
    WebScript.defer.hydrateIslands(document);
  },
  hydrateIslands(root) {
    const scope = root instanceof Element ? root : document;
    const islands = scope.querySelectorAll("[data-ws-island]");
    for (const island of islands) {
      if (island.dataset.wsHydrated === "1") continue;
      island.dataset.wsHydrated = "1";
      const script = island.nextElementSibling;
      if (script && script.tagName === "SCRIPT" && !script.src) {
        try {
          (0, eval)(script.textContent);
        } catch (error) {
          console.error("WebScript island hydration failed", error);
        }
      }
    }
  },
};
WebScript.signal = (initial) => {
  let value = initial;
  const listeners = new Set();
  return {
    get() { return value; },
    set(next) {
      value = next;
      for (const listener of listeners) listener(value);
    },
    subscribe(listener) {
      listeners.add(listener);
      listener(value);
      return () => listeners.delete(listener);
    },
  };
};
WebScript.action = async (url, name, input) => {
  let body;
  let headers = {
    Accept: "application/json",
    "X-WebScript-Action": "1",
  };
  if (input instanceof FormData) {
    input.set("_action", name);
    body = new URLSearchParams(input).toString();
    headers["Content-Type"] = "application/x-www-form-urlencoded";
  } else if (input && typeof input === "object") {
    body = JSON.stringify({ _action: name, ...input });
    headers["Content-Type"] = "application/json";
  } else {
    body = new URLSearchParams({ _action: name }).toString();
    headers["Content-Type"] = "application/x-www-form-urlencoded";
  }
  const started = performance.now();
  const response = await fetch(url, {
    method: "POST",
    headers,
    body,
    credentials: "same-origin",
  });
  const clientDurationMs = performance.now() - started;
  const payload = await response.json().catch(() => ({}));
  if (WebScript.devtools?.record) {
    WebScript.devtools.record(
      {
        requestPath: `${url}#${name}`,
        routeFile: "",
        entries: [],
        tasks: [],
        queries: [],
        total: Math.round(clientDurationMs),
      },
      {
        kind: response.ok ? "action" : "action-error",
        path: `${url}#${name}`,
        clientDurationMs,
      },
    );
  }
  if (!response.ok) {
    throw new Error(payload.error ?? "Action failed");
  }
  if (payload.redirect) {
    await WebScript.navigate(payload.redirect);
    return null;
  }
  return payload.data ?? null;
};
WebScript.runScripts = (scripts) => {
  for (const code of scripts) {
    try {
      (0, eval)(code);
    } catch (error) {
      console.error("WebScript script execution failed", error);
    }
  }
};
WebScript.mergeHead = (doc) => {
  const title = doc.querySelector("title")?.textContent;
  if (title) {
    document.title = title;
  }
  const scopedKeys = new Set();
  const collectScoped = (root) => {
    for (const style of root.querySelectorAll("style[data-ws-scoped]")) {
      scopedKeys.add(style.getAttribute("data-ws-scoped"));
    }
  };
  collectScoped(doc.head);
  collectScoped(doc.body);
  for (const style of document.querySelectorAll("style[data-ws-scoped]")) {
    const key = style.getAttribute("data-ws-scoped");
    if (!scopedKeys.has(key)) {
      style.remove();
    }
  }
  for (const style of doc.querySelectorAll("style[data-ws-scoped]")) {
    const key = style.getAttribute("data-ws-scoped");
    const existing = document.querySelector(`style[data-ws-scoped="${key}"]`);
    if (existing) {
      if (existing.textContent !== style.textContent) {
        existing.textContent = style.textContent;
      }
      continue;
    }
    document.body.appendChild(style.cloneNode(true));
  }
};
WebScript.swapRegion = (selector, doc) => {
  const current = document.querySelector(selector);
  const next = doc.querySelector(selector);
  if (!current || !next) {
    return;
  }
  current.replaceWith(next.cloneNode(true));
};
WebScript.swapOutlet = (doc) => {
  const current = document.querySelector("[data-ws-outlet]");
  const next = doc.querySelector("[data-ws-outlet]");
  if (!current || !next) {
    return false;
  }
  current.replaceWith(next.cloneNode(true));
  return true;
};
WebScript.extractPageScripts = (doc) => {
  const scripts = [];
  for (const script of doc.body.querySelectorAll("script")) {
    const src = script.getAttribute("src") || "";
    if (src.endsWith("/.web/runtime.js") || src.endsWith("/.web/dev-client.js")) {
      script.remove();
      continue;
    }
    if (src) {
      script.remove();
      continue;
    }
    const code = script.textContent?.trim();
    if (code) {
      scripts.push(code);
    }
    script.remove();
  }
  return scripts;
};
WebScript.navigate = async (url, options = {}) => {
  const replace = options.replace === true;
  const resolved = new URL(url, window.location.href);
  if (resolved.origin !== window.location.origin) {
    window.location.assign(resolved.href);
    return;
  }
  const started = performance.now();
  const response = await fetch(resolved.pathname + resolved.search, {
    headers: { Accept: "text/html" },
    credentials: "same-origin",
  });
  const clientDurationMs = performance.now() - started;
  if (!response.ok) {
    window.location.assign(resolved.href);
    return;
  }
  const html = await response.text();
  if (html.includes("data-web-defer")) {
    window.location.assign(resolved.href);
    return;
  }
  const doc = new DOMParser().parseFromString(html, "text/html");
  if (!doc.querySelector("[data-ws-outlet]")) {
    window.location.assign(resolved.href);
    return;
  }
  const metrics = WebScript.devtools?.parseMetricsNode?.(doc);
  WebScript.devtools?.stripDebugbar?.(doc);
  const scripts = WebScript.extractPageScripts(doc);
  const apply = () => {
    if (!WebScript.swapOutlet(doc)) {
      return false;
    }
    WebScript.swapRegion("[data-ws-nav-region]", doc);
    WebScript.mergeHead(doc);
    WebScript.runScripts(scripts);
    window.scrollTo(0, 0);
    return true;
  };
  const ok = apply();
  if (!ok) {
    window.location.assign(resolved.href);
    return;
  }
  if (metrics && WebScript.devtools?.record) {
    WebScript.devtools.record(metrics, {
      kind: "navigation",
      path: resolved.pathname + resolved.search,
      clientDurationMs,
    });
  }
  if (replace) {
    history.replaceState({}, "", resolved.href);
  } else {
    history.pushState({}, "", resolved.href);
  }
};
WebScript.navigate.init = () => {
  if (WebScript.navigate._initialized) {
    return;
  }
  WebScript.navigate._initialized = true;
  let busy = false;
  document.addEventListener("click", (event) => {
    if (event.defaultPrevented) return;
    if (event.button !== 0) return;
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const anchor = event.target.closest("a[href]");
    if (!anchor) return;
    if (anchor.target === "_blank") return;
    if (anchor.hasAttribute("download")) return;
    if (anchor.getAttribute("data-ws-nav") === "reload") return;
    if (anchor.closest('[data-ws-nav="reload"]')) return;
    const href = anchor.getAttribute("href");
    if (!href || href.startsWith('#')) return;
    const target = new URL(href, window.location.href);
    if (target.origin !== window.location.origin) return;
    event.preventDefault();
    if (busy) return;
    busy = true;
    WebScript.navigate(target.href).finally(() => {
      busy = false;
    });
  });
  window.addEventListener("popstate", () => {
    if (busy) return;
    busy = true;
    WebScript.navigate(window.location.href, { replace: true }).finally(() => {
      busy = false;
    });
  });
};
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => WebScript.navigate.init());
} else {
  WebScript.navigate.init();
}
"#
}

pub fn is_runtime_path(path: &str) -> bool {
    path == RUNTIME_PATH
}

pub fn resolve_signal_initial(
    signal: &ClientSignalDecl,
    scope: &std::collections::BTreeMap<String, Value>,
) -> Result<Value, Diagnostic> {
    match &signal.initial {
        ClientInitial::Literal(value) => Ok(value.clone()),
        ClientInitial::PropRef(name) => scope.get(name).cloned().ok_or_else(|| {
            Diagnostic::error(
                Span::at(signal.line, 1),
                format!(
                    "unknown prop `{name}` in @client signal `{signal_name}`",
                    signal_name = signal.name
                ),
                None,
            )
        }),
    }
}

pub fn compile_handler_body(
    source: &str,
    ctx: &HandlerCompileContext<'_>,
    line: usize,
    column: usize,
) -> Result<String, Diagnostic> {
    compile_statements(source, ctx, line, column)
}

pub fn compile_attribute_expression(
    source: &str,
    ctx: &HandlerCompileContext<'_>,
    line: usize,
    column: usize,
) -> Result<String, Diagnostic> {
    compile_expression(source, ctx, line, column)
}

pub fn signal_refs_in_source(source: &str, signals: &BTreeSet<String>) -> Vec<String> {
    let mut found = BTreeSet::new();
    for signal in signals {
        if source == *signal
            || source.contains(&format!("{signal}."))
            || source.contains(&format!("{signal} "))
            || source.contains(&format!(" {signal}"))
            || source.contains(&format!("({signal}"))
            || source.contains(&format!("!{signal}"))
        {
            found.insert(signal.clone());
        }
    }
    found.into_iter().collect()
}

pub fn is_array_signal_type(type_name: &str) -> bool {
    type_name.ends_with("[]")
}

pub fn compile_for_body_js(
    body: &[TemplateNode],
    item_name: &str,
    components: &BTreeMap<String, WebFile>,
    handlers: &BTreeSet<String>,
    line: usize,
) -> Result<String, Diagnostic> {
    let ctx = ForItemCompileContext {
        item_name: item_name.to_string(),
        handlers: handlers.clone(),
        components,
        line,
        props: BTreeMap::new(),
        forward_bindings: None,
    };
    let parts = compile_template_nodes_js(body, &ctx)?;
    Ok(format!(
        "const escapeHtml = (value) => String(value ?? '').replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('\"', '&quot;'); return `{}`;",
        parts.join("")
    ))
}

struct ForItemCompileContext<'a> {
    item_name: String,
    handlers: BTreeSet<String>,
    components: &'a BTreeMap<String, WebFile>,
    line: usize,
    props: BTreeMap<String, String>,
    forward_bindings: Option<Vec<EventBinding>>,
}

fn compile_template_nodes_js(
    nodes: &[TemplateNode],
    ctx: &ForItemCompileContext<'_>,
) -> Result<Vec<String>, Diagnostic> {
    let mut parts = Vec::new();
    let mut forward = ctx.forward_bindings.as_deref();
    for node in nodes {
        parts.extend(compile_template_node_js(node, ctx, &mut forward)?);
    }
    Ok(parts)
}

fn compile_template_node_js(
    node: &TemplateNode,
    ctx: &ForItemCompileContext<'_>,
    forward: &mut Option<&[EventBinding]>,
) -> Result<Vec<String>, Diagnostic> {
    match node {
        TemplateNode::Text(text) => {
            if let Some(bindings) = forward.take() {
                if text.contains("data-ws-bind") {
                    return Ok(inject_forward_bindings_js(text, bindings, ctx)?);
                }
            }
            Ok(vec![escape_js_template(text)])
        }
        TemplateNode::Expr(expr) => {
            let js = compile_item_expr(&expr.expr, ctx)?;
            Ok(vec![format!("${{escapeHtml({js})}}")])
        }
        TemplateNode::If {
            condition,
            then_nodes,
            else_nodes,
        } => {
            let cond = compile_item_condition(condition, ctx)?;
            let then_parts = compile_template_nodes_js(then_nodes, ctx)?;
            let else_parts = compile_template_nodes_js(else_nodes, ctx)?;
            Ok(vec![format!(
                "${{({cond}) ? `{}` : `{}`}}",
                then_parts.join(""),
                else_parts.join("")
            )])
        }
        TemplateNode::Switch {
            value,
            cases,
            default_nodes,
        } => {
            let value_js = compile_item_expr(&value.expr, ctx)?;
            let mut chain = compile_template_nodes_js(default_nodes, ctx)?.join("");
            for case in cases.iter().rev() {
                let case_js = compile_item_expr(&case.value.expr, ctx)?;
                let body = compile_template_nodes_js(&case.nodes, ctx)?.join("");
                chain = format!("${{({value_js}) === ({case_js}) ? `{body}` : `{chain}`}}");
            }
            Ok(vec![chain])
        }
        TemplateNode::Component(call) => compile_component_js(call, ctx),
        _ => Ok(Vec::new()),
    }
}

fn compile_item_condition(
    condition: &SourceExpr,
    ctx: &ForItemCompileContext<'_>,
) -> Result<String, Diagnostic> {
    compile_item_expr(&condition.expr, ctx)
}

fn compile_item_expr(
    expression: &expr::Expr,
    ctx: &ForItemCompileContext<'_>,
) -> Result<String, Diagnostic> {
    match expression {
        expr::Expr::Literal(value) => Ok(js_literal(value)),
        expr::Expr::Path(path) if path.len() == 1 => {
            if let Some(prop) = ctx.props.get(&path[0]) {
                return Ok(prop.clone());
            }
            if path[0] == ctx.item_name {
                return Ok("item".to_string());
            }
            Err(Diagnostic::error(
                Span::at(ctx.line, 1),
                format!("unknown identifier `{}` in reactive @for body", path[0]),
                None,
            ))
        }
        expr::Expr::Path(path) if path.first() == Some(&ctx.item_name) => {
            let field = path[1..].join(".");
            Ok(format!("item.{field}"))
        }
        expr::Expr::Unary { op, expr } => {
            let inner = compile_item_expr(expr, ctx)?;
            let op = match op {
                expr::UnaryOp::Not => "!",
            };
            Ok(format!("({op}{inner})"))
        }
        expr::Expr::Binary { left, op, right } => {
            let left_js = compile_item_expr(left, ctx)?;
            let right_js = compile_item_expr(right, ctx)?;
            let op = match op {
                expr::BinaryOp::Or => "||",
                expr::BinaryOp::And => "&&",
                expr::BinaryOp::Eq => "==",
                expr::BinaryOp::NotEq => "!=",
                expr::BinaryOp::Lt => "<",
                expr::BinaryOp::LtEq => "<=",
                expr::BinaryOp::Gt => ">",
                expr::BinaryOp::GtEq => ">=",
                expr::BinaryOp::Add => "+",
                expr::BinaryOp::Sub => "-",
            };
            Ok(format!("({left_js} {op} {right_js})"))
        }
        expr::Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            let condition_js = compile_item_expr(condition, ctx)?;
            let then_js = compile_item_expr(then_expr, ctx)?;
            let else_js = compile_item_expr(else_expr, ctx)?;
            Ok(format!("({condition_js} ? {then_js} : {else_js})"))
        }
        _ => Err(Diagnostic::error(
            Span::at(ctx.line, 1),
            "unsupported expression in reactive @for body",
            None,
        )),
    }
}

fn compile_component_js(
    call: &ComponentCall,
    ctx: &ForItemCompileContext<'_>,
) -> Result<Vec<String>, Diagnostic> {
    let component = ctx.components.get(&call.name).ok_or_else(|| {
        Diagnostic::error(
            Span::identifier(call.line, call.column, &call.name),
            format!("unknown component `{name}`", name = call.name),
            None,
        )
    })?;

    let declaration = component.component.as_ref().ok_or_else(|| {
        Diagnostic::error(
            Span::identifier(call.line, call.column, &call.name),
            format!("`{name}` is not a component", name = call.name),
            None,
        )
    })?;

    let mut props = BTreeMap::new();
    for prop_decl in &declaration.props {
        if let Some(default) = &prop_decl.default {
            props.insert(prop_decl.name.clone(), js_literal(default));
        }
    }
    for prop in &call.props {
        let value = match &prop.value {
            PropValue::Expr(expr) => compile_item_expr(&expr.expr, ctx)?,
            PropValue::Literal(value) => js_literal(value),
        };
        props.insert(prop.name.clone(), value);
    }

    let forward_bindings = if call.event_bindings.is_empty() {
        None
    } else {
        Some(call.event_bindings.clone())
    };

    let child_ctx = ForItemCompileContext {
        item_name: ctx.item_name.clone(),
        handlers: ctx.handlers.clone(),
        components: ctx.components,
        line: ctx.line,
        props,
        forward_bindings,
    };

    compile_template_nodes_js(&component.template, &child_ctx)
}

fn inject_forward_bindings_js(
    text: &str,
    bindings: &[EventBinding],
    ctx: &ForItemCompileContext<'_>,
) -> Result<Vec<String>, Diagnostic> {
    const MARKER: &str = "data-ws-bind";
    let marker_pos = text.find(MARKER).ok_or_else(|| {
        Diagnostic::error(
            Span::at(ctx.line, 1),
            "forwarded event bindings require `data-ws-bind` on the component root element",
            None,
        )
    })?;
    let tag_start = text[..marker_pos]
        .rfind('<')
        .ok_or_else(|| Diagnostic::error(Span::at(ctx.line, 1), "malformed bind target", None))?;
    let after_marker = &text[marker_pos..];
    let tag_end = after_marker
        .find('>')
        .map(|index| marker_pos + index + 1)
        .ok_or_else(|| Diagnostic::error(Span::at(ctx.line, 1), "malformed bind target", None))?;

    let mut tag = text[tag_start..tag_end].to_string();
    if tag.ends_with('>') {
        tag.pop();
    }
    tag = tag.replace(" data-ws-bind", "");
    tag = tag.replace(MARKER, "");

    for binding in bindings {
        if binding.event == "click" {
            if let Some((handler, arg)) = compile_handler_call(&binding.handler_source, ctx) {
                tag.push_str(&format!(
                    r#" data-ws-handler="{handler}" data-ws-handler-arg="${{{arg}}}""#
                ));
            }
        }
    }
    tag.push('>');

    let mut parts = Vec::new();
    if tag_start > 0 {
        parts.push(escape_js_template(&text[..tag_start]));
    }
    parts.push(escape_js_template(&tag));
    if tag_end < text.len() {
        parts.push(escape_js_template(&text[tag_end..]));
    }
    Ok(parts)
}

fn compile_handler_call(
    source: &str,
    ctx: &ForItemCompileContext<'_>,
) -> Option<(String, String)> {
    let source = source.trim();
    if !source.contains('(') {
        if ctx.handlers.contains(source) {
            return Some((source.to_string(), "event".to_string()));
        }
        return None;
    }
    let open_paren = source.find('(')?;
    let name = source[..open_paren].trim();
    if !ctx.handlers.contains(name) {
        return None;
    }
    let args_str = source[open_paren + 1..].strip_suffix(')')?.trim();
    let arg_js = if args_str.is_empty() {
        "event".to_string()
    } else {
        let expr = expr::parse(args_str, ctx.line, 1).ok()?;
        compile_item_expr(&expr, ctx).ok()?
    };
    Some((name.to_string(), arg_js))
}

fn escape_js_template(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${")
}

pub fn handler_body_is_async(source: &str) -> bool {
    source.contains("await ")
}

pub struct CompiledHandler {
    pub param_name: String,
    pub js_body: String,
}

pub fn compile_event_handler(
    _event: &str,
    source: &str,
    line: usize,
    column: usize,
    ctx: &HandlerCompileContext<'_>,
) -> Result<CompiledHandler, Diagnostic> {
    let (param, body) = resolve_handler_lambda(source.trim(), ctx, line, column)?;
    let lambda_ctx = HandlerCompileContext {
        signals: ctx.signals,
        handlers: ctx.handlers,
        locals: ctx.locals,
        page_actions: ctx.page_actions,
        action_url: ctx.action_url,
        param: &param,
        is_submit_context: ctx.is_submit_context,
    };
    let js_body = compile_lambda_body(&body, &lambda_ctx, line, column)?;
    Ok(CompiledHandler {
        param_name: param,
        js_body,
    })
}

pub fn index_event_attributes(html: &str, handlers: &[EventHandler]) -> String {
    let mut output = html.to_string();

    for handler in handlers {
        let bare = format!(" data-ws-{}", handler.event);
        let indexed = format!(" data-ws-{}=\"{}\"", handler.event, handler.index);
        let mut search_from = 0;

        while let Some(relative) = output[search_from..].find(&bare) {
            let start = search_from + relative;
            let after = start + bare.len();
            if output[after..].starts_with('=') {
                search_from = after;
                continue;
            }
            output.replace_range(start..after, &indexed);
            break;
        }
    }

    output
}

pub fn render_island_script(manifest: &IslandManifest) -> String {
    let mut signal_inits = Vec::new();
    for signal in &manifest.signals {
        let literal = js_literal(&signal.initial);
        signal_inits.push(format!("{}: WebScript.signal({literal})", signal.name));
    }

    let mut lines = vec![
        "<script>".to_string(),
        "(() => {".to_string(),
        format!(
            "  const root = document.querySelector('[data-ws-island=\"{}\"]');",
            manifest.id
        ),
        "  if (!root) return;".to_string(),
    ];
    if !manifest.action_url.is_empty() {
        lines.push(format!(
            "  const actionUrl = {};",
            js_string_literal(&manifest.action_url)
        ));
    }
    lines.push(format!(
        "  const signals = {{ {} }};",
        signal_inits.join(", ")
    ));

    if !manifest.named_handlers.is_empty() {
        let mut handler_entries = Vec::new();
        for handler in &manifest.named_handlers {
            let params = if handler.param_name.is_empty() {
                "event".to_string()
            } else {
                handler.param_name.clone()
            };
            let fn_kw = if handler.is_async { "async " } else { "" };
            handler_entries.push(format!(
                "    {name}: {fn_kw}({params}) => {{ {body} }}",
                name = handler.name,
                body = handler.js_body,
            ));
        }
        lines.push(format!(
            "  const handlers = {{\n{}\n  }};",
            handler_entries.join(",\n")
        ));
    } else {
        lines.push("  const handlers = {};".to_string());
    }

    for text in &manifest.text_bindings {
        lines.push(format!(
            "  const text_{signal} = root.querySelector('[data-ws-text=\"{signal}\"]');",
            signal = text.signal
        ));
        lines.push(format!(
            "  signals.{signal}.subscribe((value) => {{ text_{signal}.textContent = String(value); }});",
            signal = text.signal
        ));
    }

    for value in &manifest.value_bindings {
        lines.push(format!(
            "  const value_{signal} = root.querySelector('[data-ws-value=\"{signal}\"]');",
            signal = value.signal
        ));
        lines.push(format!(
            "  signals.{signal}.subscribe((next) => {{ if (value_{signal} && value_{signal}.value !== String(next)) value_{signal}.value = String(next); }});",
            signal = value.signal
        ));
    }

    for html in &manifest.html_bindings {
        lines.push(format!(
            "  const html_{signal} = root.querySelector('[data-ws-html=\"{signal}\"]');",
            signal = html.signal
        ));
        lines.push(format!(
            "  signals.{signal}.subscribe((next) => {{ if (html_{signal}) html_{signal}.innerHTML = String(next); }});",
            signal = html.signal
        ));
    }

    for handler in &manifest.event_handlers {
        let attr = format!("data-ws-{}", handler.event);
        let param = &handler.param_name;
        let var = format!("event_{}_{}", handler.event, handler.index);
        lines.push(format!(
            "  const {var} = root.querySelector('[{attr}=\"{index}\"]');",
            index = handler.index
        ));
        let event_name = js_event_name(&handler.event);
        let mut body = String::new();
        if handler.prevent_default {
            body.push_str(&format!("{param}.preventDefault();"));
        }
        if handler.stop_propagation {
            body.push_str(&format!("{param}.stopPropagation();"));
        }
        body.push_str(&handler.js_body);
        let listener = if handler.js_body.contains("await ") {
            format!("({param}) => {{ void (async () => {{ {body} }})(); }}")
        } else {
            format!("({param}) => {{ {body} }}")
        };
        lines.push(format!(
            "  {var}?.addEventListener('{event_name}', {listener});",
        ));
    }

    lines.push("  root.addEventListener('click', (event) => {".to_string());
    lines.push("    const node = event.target.closest('[data-ws-handler]');".to_string());
    lines.push("    if (!node) return;".to_string());
    lines.push("    const name = node.getAttribute('data-ws-handler');".to_string());
    lines.push("    const arg = node.getAttribute('data-ws-handler-arg');".to_string());
    lines.push("    if (!name || !handlers[name]) return;".to_string());
    lines.push("    event.preventDefault();".to_string());
    lines.push("    void handlers[name](arg ?? event);".to_string());
    lines.push("  });".to_string());
    if let Some(bootstrap) = &manifest.bootstrap {
        lines.push(format!("  void ({bootstrap});"));
    }

    for if_binding in &manifest.if_bindings {
        let signal = &if_binding.signal;
        lines.push(format!(
            "  const if_{signal}_then = root.querySelectorAll('[data-ws-if=\"{signal}\"][data-ws-branch=\"then\"]');",
            signal = signal
        ));
        lines.push(format!(
            "  const if_{signal}_else = root.querySelectorAll('[data-ws-if=\"{signal}\"][data-ws-branch=\"else\"]');",
            signal = signal
        ));
        lines.push(format!(
            "  signals.{signal}.subscribe((value) => {{",
            signal = signal
        ));
        lines.push(format!(
            "    if_{signal}_then.forEach((node) => {{ node.style.display = value ? '' : 'none'; }});",
            signal = signal
        ));
        lines.push(format!(
            "    if_{signal}_else.forEach((node) => {{ node.style.display = value ? 'none' : ''; }});",
            signal = signal
        ));
        lines.push("  });".to_string());
    }

    for binding in &manifest.attr_bindings {
        let attr_key = binding.attribute.replace('-', "_");
        let var_name = format!("attr_{attr_key}_{}", binding.index);
        lines.push(format!(
            "  const {var_name} = root.querySelector('[data-ws-attr-{attr}=\"{index}\"]');",
            attr = binding.attribute,
            index = binding.index
        ));
        lines.push(format!(
            "  const update_{var_name} = () => {{ if ({var_name}) {var_name}.setAttribute('{attr}', String({expr})); }};",
            attr = binding.attribute,
            expr = binding.js_expr
        ));
        for signal in &binding.signals {
            lines.push(format!(
                "  signals.{signal}.subscribe(() => update_{var_name}());",
                signal = signal
            ));
        }
        lines.push(format!("  update_{var_name}();"));
    }

    for binding in &manifest.for_bindings {
        let signal = &binding.signal;
        lines.push(format!(
            "  const for_{signal} = root.querySelector('[data-ws-for=\"{signal}\"]');",
            signal = signal
        ));
        lines.push(format!(
            "  const renderFor_{signal} = (item) => {{ {body} }};",
            signal = signal,
            body = binding.js_fn_body
        ));
        lines.push(format!(
            "  const update_for_{signal} = (items) => {{ if (!for_{signal}) return; for_{signal}.innerHTML = Array.isArray(items) ? items.map(renderFor_{signal}).join('') : ''; }};",
            signal = signal
        ));
        lines.push(format!(
            "  signals.{signal}.subscribe((items) => update_for_{signal}(items));",
            signal = signal
        ));
        lines.push(format!(
            "  update_for_{signal}(signals.{signal}.get());",
            signal = signal
        ));
    }

    lines.push("})();".to_string());
    lines.push("</script>".to_string());
    lines.join("\n")
}

fn js_string_literal(value: &str) -> String {
    format!("\"{}\"", escape_js_string(value))
}

pub fn format_defer_chunk_script(id: &str, html: &str) -> String {
    format!(
        "<script data-web-defer-chunk>WebScript.defer.replace({}, {})</script>",
        js_string_literal(id),
        js_string_literal(html)
    )
}

pub fn inject_client_scripts(html: &str, scripts: &str) -> String {
    let runtime_tag = format!(r#"<script src="{RUNTIME_PATH}"></script>"#);
    let mut fragment = runtime_tag;
    fragment.push_str(scripts);

    if let Some(index) = html.rfind("</body>") {
        let mut injected = String::with_capacity(html.len() + fragment.len());
        injected.push_str(&html[..index]);
        injected.push_str(&fragment);
        injected.push_str(&html[index..]);
        injected
    } else {
        let mut injected = String::with_capacity(html.len() + fragment.len());
        injected.push_str(html);
        injected.push_str(&fragment);
        injected
    }
}

pub fn build_event_handler(
    binding: &EventBinding,
    index: usize,
    ctx: &HandlerCompileContext<'_>,
) -> Result<EventHandler, Diagnostic> {
    let compiled = compile_event_handler(
        &binding.event,
        &binding.handler_source,
        binding.line,
        binding.column,
        ctx,
    )?;
    Ok(EventHandler {
        event: binding.event.clone(),
        index,
        handler_source: binding.handler_source.clone(),
        js_body: compiled.js_body,
        param_name: compiled.param_name,
        prevent_default: binding.prevent_default,
        stop_propagation: binding.stop_propagation,
    })
}

fn resolve_handler_lambda(
    source: &str,
    ctx: &HandlerCompileContext<'_>,
    line: usize,
    column: usize,
) -> Result<(String, String), Diagnostic> {
    if let Some((param, body)) = parse_client_lambda(source) {
        return Ok((param, body));
    }

    if source.starts_with('|') {
        return Err(invalid_lambda(event_label(source), source, line, column));
    }

    if is_identifier(source) && ctx.handlers.contains(source) {
        return Ok((DEFAULT_EVENT_PARAM.to_string(), format!("{source}()")));
    }

    Ok((DEFAULT_EVENT_PARAM.to_string(), source.to_string()))
}

pub fn value_signal_from_field_handler(handler_source: &str) -> Option<String> {
    let body = if let Some((_, body)) = parse_client_lambda(handler_source) {
        body
    } else {
        handler_source.trim().to_string()
    };

    let eq_index = find_assignment_equals(&body)?;
    let left = body[..eq_index].trim();
    let right = body[eq_index + 1..].trim();
    if !is_identifier(left) {
        return None;
    }
    if right == "event.value" || right == "event.target.value" || right.ends_with(".target.value") {
        return Some(left.to_string());
    }
    None
}

fn parse_client_lambda(source: &str) -> Option<(String, String)> {
    let rest = source.strip_prefix('|')?;
    let pipe_end = rest.find('|')?;
    let param = rest[..pipe_end].trim();
    if !is_identifier(param) {
        return None;
    }
    let body = rest[pipe_end + 1..].trim();
    if body.is_empty() {
        return None;
    }

    let body = strip_block_body(body);
    Some((param.to_string(), body))
}

fn strip_block_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        let inner = &trimmed[1..trimmed.len() - 1];
        return inner.trim().to_string();
    }
    trimmed.to_string()
}

fn compile_lambda_body(
    body: &str,
    ctx: &HandlerCompileContext<'_>,
    line: usize,
    column: usize,
) -> Result<String, Diagnostic> {
    if let Some(js) = try_legacy_handler(body, ctx)? {
        return Ok(js);
    }
    compile_statements(body, ctx, line, column)
}

fn try_legacy_handler(
    source: &str,
    ctx: &HandlerCompileContext<'_>,
) -> Result<Option<String>, Diagnostic> {
    if let Some(name) = source.strip_suffix("++") {
        let name = name.trim();
        if is_identifier(name) && ctx.signals.contains(name) {
            return Ok(Some(format!(
                "signals.{name}.set(signals.{name}.get() + 1)"
            )));
        }
    }
    if let Some(name) = source.strip_suffix("--") {
        let name = name.trim();
        if is_identifier(name) && ctx.signals.contains(name) {
            return Ok(Some(format!(
                "signals.{name}.set(signals.{name}.get() - 1)"
            )));
        }
    }

    if let Some(eq_index) = find_assignment_equals(source) {
        let left = source[..eq_index].trim();
        let right = source[eq_index + 1..].trim();
        if !is_identifier(left) || !ctx.signals.contains(left) {
            return Ok(None);
        }

        let param = ctx.param;
        if right == format!("!{left}") {
            return Ok(Some(format!("signals.{left}.set(!signals.{left}.get())")));
        }
        if right == format!("{left} + 1") {
            return Ok(Some(format!(
                "signals.{left}.set(signals.{left}.get() + 1)"
            )));
        }
        if right == format!("{left} - 1") {
            return Ok(Some(format!(
                "signals.{left}.set(signals.{left}.get() - 1)"
            )));
        }
        if right == "event.value"
            || right == "event.target.value"
            || right == format!("{param}.value")
            || right == format!("{param}.target.value")
        {
            return Ok(Some(format!("signals.{left}.set({param}.target.value)")));
        }
        if is_simple_literal_rhs(right) {
            if let Ok(literal) = parse_literal_assignment(right) {
                return Ok(Some(format!("signals.{left}.set({literal})")));
            }
        }
    }

    Ok(None)
}

fn is_simple_literal_rhs(source: &str) -> bool {
    if source.contains('+')
        || source.contains("&&")
        || source.contains("||")
        || source.contains("==")
        || source.contains("!=")
        || source.contains('(')
    {
        return false;
    }
    if source.starts_with('"') {
        return source.ends_with('"') && !source[1..source.len() - 1].contains('"');
    }
    if source.starts_with('\'') {
        return source.ends_with('\'') && !source[1..source.len() - 1].contains('\'');
    }
    !source.contains('.')
}

fn compile_statements(
    source: &str,
    ctx: &HandlerCompileContext<'_>,
    line: usize,
    column: usize,
) -> Result<String, Diagnostic> {
    let parts = split_statements(source);
    if parts.is_empty() {
        return Err(invalid_handler("handler", source, line, column));
    }

    let mut locals = ctx.locals.clone();
    let mut compiled = Vec::new();
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let statement_ctx = HandlerCompileContext {
            locals: &locals,
            ..ctx.clone()
        };
        compiled.push(compile_statement(part, &statement_ctx, line, column)?);
        if let Some(name) = let_binding_name(part) {
            locals.insert(name);
        }
    }

    if compiled.is_empty() {
        return Err(invalid_handler("handler", source, line, column));
    }

    Ok(compiled.join("; "))
}

fn let_binding_name(source: &str) -> Option<String> {
    let rest = source.trim().strip_prefix("let ")?.trim();
    let name = rest.split_once('=')?.0.trim();
    if is_identifier(name) {
        Some(name.to_string())
    } else {
        None
    }
}

fn compile_statement(
    source: &str,
    ctx: &HandlerCompileContext<'_>,
    line: usize,
    column: usize,
) -> Result<String, Diagnostic> {
    if let Some(js) = try_legacy_handler(source, ctx)? {
        return Ok(js);
    }

    if let Some(rest) = source.strip_prefix("let ") {
        if let Some(eq_index) = find_assignment_equals(rest) {
            let name = rest[..eq_index].trim();
            let right = rest[eq_index + 1..].trim();
            if is_identifier(name) {
                let expr = compile_expression(right, ctx, line, column)?;
                return Ok(format!("const {name} = {expr}"));
            }
        }
    }

    if let Some(eq_index) = find_assignment_equals(source) {
        let left = source[..eq_index].trim();
        let right = source[eq_index + 1..].trim();
        if is_identifier(left) && ctx.signals.contains(left) {
            let expr = compile_expression(right, ctx, line, column)?;
            return Ok(format!("signals.{left}.set({expr})"));
        }
    }

    compile_expression(source, ctx, line, column)
}

fn compile_action_call(
    args: &[String],
    ctx: &HandlerCompileContext<'_>,
) -> Result<String, Diagnostic> {
    let Some(action_name) = args.first() else {
        return Err(Diagnostic::error(
            Span::at(1, 1),
            "action() requires an action name",
            None,
        ));
    };
    let Some(action_key) = action_name_from_arg(action_name) else {
        return Err(Diagnostic::error(
            Span::at(1, 1),
            "action() name must be a string literal",
            None,
        ));
    };
    if !ctx.page_actions.contains_key(&action_key) {
        return Err(Diagnostic::error(
            Span::at(1, 1),
            format!("unknown page action `{action_key}`"),
            None,
        ));
    }

    let input = if args.len() >= 2 {
        args[1].clone()
    } else if ctx.is_submit_context {
        format!(
            "{}.target && {}.target.tagName === 'FORM' ? Object.fromEntries(new FormData({}.target)) : null",
            ctx.param, ctx.param, ctx.param
        )
    } else {
        "null".to_string()
    };

    Ok(format!(
        "WebScript.action(actionUrl, {action_name}, {input})"
    ))
}

fn action_name_from_arg(arg: &str) -> Option<String> {
    let trimmed = arg.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        return Some(trimmed[1..trimmed.len() - 1].to_string());
    }
    None
}

fn compile_expression(
    source: &str,
    ctx: &HandlerCompileContext<'_>,
    line: usize,
    column: usize,
) -> Result<String, Diagnostic> {
    let tokens = tokenize(source, line, column)?;
    let mut parser = ExprParser {
        tokens,
        index: 0,
        ctx,
        line,
        column,
    };
    let expr = parser.parse_ternary()?;
    if parser.peek().is_some() {
        return Err(invalid_handler("expression", source, line, column));
    }
    Ok(expr)
}

fn find_assignment_equals(source: &str) -> Option<usize> {
    let mut in_string = None::<char>;
    let mut depth_paren = 0usize;
    let mut depth_brace = 0usize;
    let mut byte_index = 0usize;

    for ch in source.chars() {
        match in_string {
            Some(quote) if ch == quote => in_string = None,
            Some(_) => {}
            None if ch == '"' || ch == '\'' => in_string = Some(ch),
            None if ch == '(' => depth_paren += 1,
            None if ch == ')' => depth_paren = depth_paren.saturating_sub(1),
            None if ch == '{' => depth_brace += 1,
            None if ch == '}' => depth_brace = depth_brace.saturating_sub(1),
            None if ch == '='
                && depth_paren == 0
                && depth_brace == 0
                && !source[byte_index..].starts_with("==") =>
            {
                return Some(byte_index);
            }
            None => {}
        }
        byte_index += ch.len_utf8();
    }

    None
}

fn split_statements(source: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth_paren = 0usize;
    let mut depth_brace = 0usize;
    let mut in_string = None::<char>;

    for ch in source.chars() {
        match in_string {
            Some(quote) if ch == quote => {
                in_string = None;
                current.push(ch);
            }
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => {
                in_string = Some(ch);
                current.push(ch);
            }
            None if ch == '(' => {
                depth_paren += 1;
                current.push(ch);
            }
            None if ch == ')' => {
                depth_paren = depth_paren.saturating_sub(1);
                current.push(ch);
            }
            None if ch == '{' => {
                depth_brace += 1;
                current.push(ch);
            }
            None if ch == '}' => {
                depth_brace = depth_brace.saturating_sub(1);
                current.push(ch);
            }
            None if (ch == ';' || ch == '\n') && depth_paren == 0 && depth_brace == 0 => {
                if !current.trim().is_empty() {
                    parts.push(current.trim().to_string());
                }
                current.clear();
            }
            None => current.push(ch),
        }
    }

    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }

    parts
}

#[derive(Debug, Clone)]
struct Token {
    lexeme: String,
}

struct ExprParser<'a, 'b> {
    tokens: Vec<Token>,
    index: usize,
    ctx: &'a HandlerCompileContext<'b>,
    line: usize,
    column: usize,
}

impl ExprParser<'_, '_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn bump(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    fn parse_ternary(&mut self) -> Result<String, Diagnostic> {
        let mut value = self.parse_or()?;
        if self.match_lexeme("?") {
            let then_value = self.parse_or()?;
            self.expect_lexeme(":")?;
            let else_value = self.parse_ternary()?;
            value = format!("({value} ? {then_value} : {else_value})");
        }
        Ok(value)
    }

    fn parse_or(&mut self) -> Result<String, Diagnostic> {
        let mut value = self.parse_and()?;
        while self.match_lexeme("||") {
            let right = self.parse_and()?;
            value = format!("({value} || {right})");
        }
        Ok(value)
    }

    fn parse_and(&mut self) -> Result<String, Diagnostic> {
        let mut value = self.parse_equality()?;
        while self.match_lexeme("&&") {
            let right = self.parse_equality()?;
            value = format!("({value} && {right})");
        }
        Ok(value)
    }

    fn parse_equality(&mut self) -> Result<String, Diagnostic> {
        let mut value = self.parse_additive()?;
        while let Some(op) = self.match_one(&["==", "!="]) {
            let right = self.parse_additive()?;
            value = format!("({value} {op} {right})");
        }
        Ok(value)
    }

    fn parse_additive(&mut self) -> Result<String, Diagnostic> {
        let mut value = self.parse_multiplicative()?;
        while let Some(op) = self.match_one(&["+", "-"]) {
            let right = self.parse_multiplicative()?;
            value = format!("({value} {op} {right})");
        }
        Ok(value)
    }

    fn parse_multiplicative(&mut self) -> Result<String, Diagnostic> {
        let mut value = self.parse_unary()?;
        while let Some(op) = self.match_one(&["*", "/"]) {
            let right = self.parse_unary()?;
            value = format!("({value} {op} {right})");
        }
        Ok(value)
    }

    fn parse_unary(&mut self) -> Result<String, Diagnostic> {
        if self.match_lexeme("await") {
            let value = self.parse_unary()?;
            return Ok(format!("await {value}"));
        }
        if self.match_lexeme("!") {
            let value = self.parse_unary()?;
            return Ok(format!("(!{value})"));
        }
        if self.match_lexeme("-") {
            let value = self.parse_unary()?;
            return Ok(format!("(-{value})"));
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<String, Diagnostic> {
        let mut value = self.parse_primary()?;
        let param = self.ctx.param;
        loop {
            if self.match_lexeme(".") {
                let property = self.expect_ident("property")?;
                if self.match_lexeme("(") {
                    let args = self.parse_call_args()?;
                    self.expect_lexeme(")")?;
                    if (value == "event" || value == param)
                        && property == "preventDefault"
                        && args.is_empty()
                    {
                        value = format!("{param}.preventDefault()");
                    } else if (value == "event" || value == param)
                        && property == "stopPropagation"
                        && args.is_empty()
                    {
                        value = format!("{param}.stopPropagation()");
                    } else if args.is_empty() && self.ctx.handlers.contains(&property) {
                        value = format!("handlers.{property}({param})");
                    } else if args.is_empty() {
                        value = format!("{value}.{property}()");
                    } else {
                        value = format!("{value}.{property}({})", args.join(", "));
                    }
                } else if (value == "event" || value == param) && property == "value" {
                    value = format!("{param}.target.value");
                } else {
                    value = format!("{value}.{property}");
                }
            } else if self.match_lexeme("(") {
                let args = self.parse_call_args()?;
                self.expect_lexeme(")")?;
                if value == "action" {
                    value = compile_action_call(&args, self.ctx)?;
                } else if let Some(name) = value.strip_prefix("handlers.") {
                    let call_args = if args.is_empty() {
                        param.to_string()
                    } else {
                        args.join(", ")
                    };
                    value = format!("handlers.{name}({call_args})");
                } else if self.ctx.handlers.contains(&value) {
                    let call_args = if args.is_empty() {
                        param.to_string()
                    } else {
                        args.join(", ")
                    };
                    value = format!("handlers.{value}({call_args})");
                } else {
                    value = format!("{value}({})", args.join(", "));
                }
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_object_literal(&mut self) -> Result<String, Diagnostic> {
        let mut fields = Vec::new();
        if !self.match_lexeme("}") {
            loop {
                let key = self.expect_ident("field")?;
                self.expect_lexeme(":")?;
                let value = self.parse_or()?;
                fields.push(format!("{key}: {value}"));
                if !self.match_lexeme(",") {
                    break;
                }
            }
            self.expect_lexeme("}")?;
        }
        Ok(format!("{{ {} }}", fields.join(", ")))
    }

    fn parse_primary(&mut self) -> Result<String, Diagnostic> {
        if self.match_lexeme("(") {
            let value = self.parse_or()?;
            self.expect_lexeme(")")?;
            return Ok(value);
        }

        if self.match_lexeme("{") {
            return self.parse_object_literal();
        }

        let Some(token) = self.bump() else {
            return Err(invalid_handler("expression", "", self.line, self.column));
        };

        let param = self.ctx.param;
        match token.lexeme.as_str() {
            "true" | "false" => Ok(token.lexeme),
            value if value.starts_with('"') || value.starts_with('\'') => Ok(token.lexeme),
            value
                if value
                    .chars()
                    .all(|char| char.is_ascii_digit() || char == '.') =>
            {
                Ok(token.lexeme)
            }
            "event" => Ok(param.to_string()),
            name if self.ctx.locals.contains(name) => Ok(name.to_string()),
            name if self.ctx.signals.contains(name) => Ok(format!("signals.{name}.get()")),
            name if self.ctx.handlers.contains(name) => Ok(format!("handlers.{name}")),
            name if is_identifier(name) => Ok(name.to_string()),
            _ => Err(invalid_handler(
                "expression",
                &token.lexeme,
                self.line,
                self.column,
            )),
        }
    }

    fn parse_call_args(&mut self) -> Result<Vec<String>, Diagnostic> {
        let mut args = Vec::new();
        if self.peek().is_some_and(|token| token.lexeme == ")") {
            return Ok(args);
        }
        loop {
            args.push(self.parse_or()?);
            if !self.match_lexeme(",") {
                break;
            }
        }
        Ok(args)
    }

    fn match_lexeme(&mut self, lexeme: &str) -> bool {
        if self.peek().is_some_and(|token| token.lexeme == lexeme) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn match_one(&mut self, lexemes: &[&str]) -> Option<String> {
        for lexeme in lexemes {
            if self.match_lexeme(lexeme) {
                return Some((*lexeme).to_string());
            }
        }
        None
    }

    fn expect_lexeme(&mut self, lexeme: &str) -> Result<(), Diagnostic> {
        if self.match_lexeme(lexeme) {
            Ok(())
        } else {
            Err(invalid_handler(
                "expression",
                lexeme,
                self.line,
                self.column,
            ))
        }
    }

    fn expect_ident(&mut self, label: &str) -> Result<String, Diagnostic> {
        let Some(token) = self.bump() else {
            return Err(invalid_handler("expression", label, self.line, self.column));
        };
        if is_identifier(&token.lexeme) {
            Ok(token.lexeme)
        } else {
            Err(invalid_handler(
                "expression",
                &token.lexeme,
                self.line,
                self.column,
            ))
        }
    }
}

fn tokenize(source: &str, line: usize, column: usize) -> Result<Vec<Token>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut byte_index = 0usize;
    let source_bytes = source.as_bytes();

    while byte_index < source_bytes.len() {
        let ch = source[byte_index..].chars().next().expect("byte index");
        if ch.is_whitespace() {
            byte_index += ch.len_utf8();
            continue;
        }

        if ch == '"' || ch == '\'' {
            let quote = ch;
            let start = byte_index;
            byte_index += ch.len_utf8();
            while byte_index < source_bytes.len() {
                let current = source[byte_index..].chars().next().expect("byte index");
                if current == quote {
                    byte_index += current.len_utf8();
                    break;
                }
                if current == '\\' {
                    byte_index += 1;
                    if byte_index < source_bytes.len() {
                        byte_index += source[byte_index..]
                            .chars()
                            .next()
                            .map_or(0, |next| next.len_utf8());
                    }
                } else {
                    byte_index += current.len_utf8();
                }
            }
            tokens.push(Token {
                lexeme: source[start..byte_index].to_string(),
            });
            continue;
        }

        if ch.is_ascii_digit() {
            let start = byte_index;
            byte_index += ch.len_utf8();
            while byte_index < source_bytes.len() {
                let next = source[byte_index..].chars().next().expect("byte index");
                if next.is_ascii_digit() || next == '.' {
                    byte_index += next.len_utf8();
                } else {
                    break;
                }
            }
            tokens.push(Token {
                lexeme: source[start..byte_index].to_string(),
            });
            continue;
        }

        if ch == '_' || ch.is_ascii_alphabetic() {
            let start = byte_index;
            byte_index += ch.len_utf8();
            while byte_index < source_bytes.len() {
                let next = source[byte_index..].chars().next().expect("byte index");
                if next == '_' || next.is_ascii_alphanumeric() {
                    byte_index += next.len_utf8();
                } else {
                    break;
                }
            }
            tokens.push(Token {
                lexeme: source[start..byte_index].to_string(),
            });
            continue;
        }

        let two = source_bytes.get(byte_index + 1).copied();
        let lexeme = match (ch, two) {
            ('=', Some(b'=')) => {
                byte_index += 2;
                "=="
            }
            ('!', Some(b'=')) => {
                byte_index += 2;
                "!="
            }
            ('&', Some(b'&')) => {
                byte_index += 2;
                "&&"
            }
            ('|', Some(b'|')) => {
                byte_index += 2;
                "||"
            }
            _ => {
                byte_index += ch.len_utf8();
                match ch {
                    '+' => "+",
                    '-' => "-",
                    '*' => "*",
                    '/' => "/",
                    '(' => "(",
                    ')' => ")",
                    '{' => "{",
                    '}' => "}",
                    ':' => ":",
                    '?' => "?",
                    '.' => ".",
                    ',' => ",",
                    '!' => "!",
                    _ => {
                        return Err(Diagnostic::error(
                            Span::new(line, column + byte_index, column + byte_index + 1),
                            format!("unexpected character `{ch}` in client expression"),
                            None,
                        ));
                    }
                }
            }
        };
        tokens.push(Token {
            lexeme: lexeme.to_string(),
        });
    }

    Ok(tokens)
}

fn parse_literal_assignment(source: &str) -> Result<String, Diagnostic> {
    if source == "true" || source == "false" {
        return Ok(source.to_string());
    }
    if source
        .chars()
        .all(|char| char.is_ascii_digit() || char == '-' || char == '.')
    {
        return Ok(source.to_string());
    }
    if (source.starts_with('"') && source.ends_with('"'))
        || (source.starts_with('\'') && source.ends_with('\''))
    {
        let inner = &source[1..source.len() - 1];
        return Ok(format!("\"{}\"", escape_js_string(inner)));
    }
    Err(Diagnostic::error(
        Span::at(1, 1),
        format!("unsupported literal `{source}`"),
        None,
    ))
}

fn js_event_name(event: &str) -> &'static str {
    match event {
        "click" => "click",
        "input" => "input",
        "change" => "change",
        "submit" => "submit",
        "keydown" => "keydown",
        "keyup" => "keyup",
        "focus" => "focus",
        "blur" => "blur",
        _ => "click",
    }
}

pub fn js_literal(value: &Value) -> String {
    match value {
        Value::Int(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::String(value) => format!("\"{}\"", escape_js_string(value)),
        Value::Object(_) | Value::Array { .. } => parser_value_to_json(value)
            .map(|json| json.to_string())
            .unwrap_or_else(|_| "null".to_string()),
        _ => "null".to_string(),
    }
}

fn escape_js_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|char| char == '_' || char.is_ascii_alphanumeric())
}

fn event_label(_source: &str) -> &str {
    "handler"
}

fn invalid_handler(event: &str, source: &str, line: usize, column: usize) -> Diagnostic {
    Diagnostic::error(
        Span::new(line, column, column + source.len().max(1)),
        format!("unsupported {event} handler `{source}`"),
        None,
    )
}

fn invalid_lambda(event: &str, source: &str, line: usize, column: usize) -> Diagnostic {
    Diagnostic::error(
        Span::new(line, column, column + source.len().max(1)),
        format!("invalid {event} lambda `{source}`; use `|param| body` or a simple expression"),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(
        signals: &'a BTreeSet<String>,
        handlers: &'a BTreeSet<String>,
        page_actions: &'a BTreeMap<String, Option<String>>,
        locals: &'a BTreeSet<String>,
    ) -> HandlerCompileContext<'a> {
        HandlerCompileContext {
            signals,
            handlers,
            locals,
            page_actions,
            action_url: "/",
            param: DEFAULT_EVENT_PARAM,
            is_submit_context: false,
        }
    }

    fn compile(source: &str, compile_ctx: &HandlerCompileContext<'_>) -> CompiledHandler {
        compile_event_handler("click", source, 1, 1, compile_ctx).expect("handler")
    }

    #[test]
    fn value_signal_from_pipe_lambda_field_handler() {
        assert_eq!(
            value_signal_from_field_handler("|event| note = event.target.value"),
            Some("note".to_string())
        );
        assert_eq!(
            value_signal_from_field_handler("name = event.value"),
            Some("name".to_string())
        );
    }

    #[test]
    fn compiles_increment_handler() {
        let signals = BTreeSet::from(["count".to_string()]);
        let handlers = BTreeSet::new();
        let page_actions = BTreeMap::new();
        let locals = BTreeSet::new();
        let compile_ctx = ctx(&signals, &handlers, &page_actions, &locals);
        let compiled = compile("count++", &compile_ctx);
        assert_eq!(
            compiled.js_body,
            "signals.count.set(signals.count.get() + 1)"
        );
        assert_eq!(compiled.param_name, "event");
    }

    #[test]
    fn compiles_bool_toggle_handler() {
        let signals = BTreeSet::from(["open".to_string()]);
        let handlers = BTreeSet::new();
        let page_actions = BTreeMap::new();
        let locals = BTreeSet::new();
        let compile_ctx = ctx(&signals, &handlers, &page_actions, &locals);
        let compiled = compile("open = !open", &compile_ctx);
        assert_eq!(compiled.js_body, "signals.open.set(!signals.open.get())");
    }

    #[test]
    fn compiles_input_handler() {
        let signals = BTreeSet::from(["name".to_string()]);
        let handlers = BTreeSet::new();
        let page_actions = BTreeMap::new();
        let locals = BTreeSet::new();
        let compile_ctx = ctx(&signals, &handlers, &page_actions, &locals);
        let compiled = compile("name = event.value", &compile_ctx);
        assert_eq!(compiled.js_body, "signals.name.set(event.target.value)");
    }

    #[test]
    fn compiles_reset_handler() {
        let signals = BTreeSet::from(["count".to_string()]);
        let handlers = BTreeSet::new();
        let page_actions = BTreeMap::new();
        let locals = BTreeSet::new();
        let compile_ctx = ctx(&signals, &handlers, &page_actions, &locals);
        let compiled = compile("count = 0", &compile_ctx);
        assert_eq!(compiled.js_body, "signals.count.set(0)");
    }

    #[test]
    fn compiles_named_handler_reference() {
        let signals = BTreeSet::new();
        let handlers = BTreeSet::from(["save".to_string()]);
        let page_actions = BTreeMap::new();
        let locals = BTreeSet::new();
        let compile_ctx = ctx(&signals, &handlers, &page_actions, &locals);
        let compiled =
            compile_event_handler("submit", "save", 1, 1, &compile_ctx).expect("handler");
        assert_eq!(compiled.js_body, "handlers.save(event)");
    }

    #[test]
    fn compiles_general_expression_assignment() {
        let signals = BTreeSet::from(["count".to_string(), "message".to_string()]);
        let handlers = BTreeSet::new();
        let page_actions = BTreeMap::new();
        let locals = BTreeSet::new();
        let compile_ctx = ctx(&signals, &handlers, &page_actions, &locals);
        let compiled = compile("message = \"Count: \" + count", &compile_ctx);
        assert_eq!(
            compiled.js_body,
            "signals.message.set((\"Count: \" + signals.count.get()))"
        );
    }

    #[test]
    fn compiles_event_key_expression() {
        let signals = BTreeSet::from(["log".to_string()]);
        let handlers = BTreeSet::new();
        let page_actions = BTreeMap::new();
        let locals = BTreeSet::new();
        let compile_ctx = ctx(&signals, &handlers, &page_actions, &locals);
        let compiled = compile("log = \"Key: \" + event.key", &compile_ctx);
        assert_eq!(compiled.js_body, "signals.log.set((\"Key: \" + event.key))");
    }

    #[test]
    fn compiles_handler_body_with_multiple_statements() {
        let signals = BTreeSet::from(["count".to_string(), "message".to_string()]);
        let handlers = BTreeSet::new();
        let page_actions = BTreeMap::new();
        let locals = BTreeSet::new();
        let compile_ctx = ctx(&signals, &handlers, &page_actions, &locals);
        let js = compile_handler_body("count = 0\nmessage = \"reset\"", &compile_ctx, 1, 1)
            .expect("handler body");
        assert_eq!(js, "signals.count.set(0); signals.message.set(\"reset\")");
    }

    #[test]
    fn compiles_pipe_lambda_with_custom_param() {
        let signals = BTreeSet::from(["count".to_string()]);
        let handlers = BTreeSet::new();
        let page_actions = BTreeMap::new();
        let locals = BTreeSet::new();
        let compile_ctx = ctx(&signals, &handlers, &page_actions, &locals);
        let compiled = compile("|e| count++", &compile_ctx);
        assert_eq!(compiled.param_name, "e");
        assert_eq!(
            compiled.js_body,
            "signals.count.set(signals.count.get() + 1)"
        );
    }

    #[test]
    fn compiles_pipe_lambda_block_body() {
        let signals = BTreeSet::from(["a".to_string(), "b".to_string()]);
        let handlers = BTreeSet::new();
        let page_actions = BTreeMap::new();
        let locals = BTreeSet::new();
        let compile_ctx = ctx(&signals, &handlers, &page_actions, &locals);
        let compiled = compile("|event| { a = 1; b = 2 }", &compile_ctx);
        assert_eq!(compiled.js_body, "signals.a.set(1); signals.b.set(2)");
    }

    #[test]
    fn compiles_pipe_lambda_with_event_key() {
        let signals = BTreeSet::from(["log".to_string()]);
        let handlers = BTreeSet::from(["save".to_string()]);
        let page_actions = BTreeMap::new();
        let locals = BTreeSet::new();
        let compile_ctx = ctx(&signals, &handlers, &page_actions, &locals);
        let compiled = compile("|event| event.key == \"Enter\" && save()", &compile_ctx);
        assert_eq!(
            compiled.js_body,
            "((event.key == \"Enter\") && handlers.save(event))"
        );
    }

    #[test]
    fn indexes_multiple_click_attributes() {
        let handlers = vec![
            EventHandler {
                event: "click".to_string(),
                index: 0,
                handler_source: "count++".to_string(),
                js_body: String::new(),
                param_name: DEFAULT_EVENT_PARAM.to_string(),
                prevent_default: false,
                stop_propagation: false,
            },
            EventHandler {
                event: "click".to_string(),
                index: 1,
                handler_source: "count--".to_string(),
                js_body: String::new(),
                param_name: DEFAULT_EVENT_PARAM.to_string(),
                prevent_default: false,
                stop_propagation: false,
            },
        ];
        let html = index_event_attributes(
            "<button data-ws-click>+</button><button data-ws-click>-</button>",
            &handlers,
        );
        assert!(html.contains("data-ws-click=\"0\""));
        assert!(html.contains("data-ws-click=\"1\""));
    }

    #[test]
    fn renders_island_script_with_signal_and_click() {
        let manifest = IslandManifest {
            id: "Counter-0".to_string(),
            component: "Counter".to_string(),
            action_url: "/counter".to_string(),
            signals: vec![SignalBinding {
                name: "count".to_string(),
                type_name: "int".to_string(),
                initial: Value::Int(5),
            }],
            event_handlers: vec![EventHandler {
                event: "click".to_string(),
                index: 0,
                handler_source: "count++".to_string(),
                js_body: "signals.count.set(signals.count.get() + 1)".to_string(),
                param_name: DEFAULT_EVENT_PARAM.to_string(),
                prevent_default: false,
                stop_propagation: false,
            }],
            named_handlers: Vec::new(),
            text_bindings: vec![TextBinding {
                signal: "count".to_string(),
            }],
            value_bindings: Vec::new(),
            html_bindings: Vec::new(),
            if_bindings: Vec::new(),
            attr_bindings: Vec::new(),
            for_bindings: Vec::new(),
            bootstrap: None,
        };

        let script = render_island_script(&manifest);
        assert!(script.contains("data-ws-island=\"Counter-0\""));
        assert!(script.contains("WebScript.signal(5)"));
        assert!(script.contains("data-ws-click=\"0\""));
        assert!(script.contains("data-ws-text=\"count\""));
        assert!(script.contains("(event) => { signals.count.set"));
    }

    #[test]
    fn runtime_excludes_app_specific_helpers() {
        let runtime = client_runtime_script();
        assert!(!runtime.contains("passwordToggle"));
        assert!(!runtime.contains("renderTodos"));
        assert!(!runtime.contains("escapeHtml"));
    }

    #[test]
    fn js_literal_serializes_arrays() {
        let value = Value::Array {
            element_type: "Todo".to_string(),
            values: vec![Value::Object(BTreeMap::from([
                ("id".to_string(), Value::Int(1)),
                ("title".to_string(), Value::String("Ship".to_string())),
                ("done".to_string(), Value::Bool(false)),
            ]))],
        };
        let literal = js_literal(&value);
        assert!(literal.contains("\"title\":\"Ship\""));
        assert!(literal.starts_with('['));
    }

    #[test]
    fn let_binding_shadows_signal_name_in_handler() {
        let signals = BTreeSet::from(["todos".to_string(), "title".to_string()]);
        let handlers = BTreeSet::from(["applyTodos".to_string()]);
        let page_actions =
            BTreeMap::from([("addTodo".to_string(), Some("AddTodoInput".to_string()))]);
        let locals = BTreeSet::new();
        let compile_ctx = HandlerCompileContext {
            signals: &signals,
            handlers: &handlers,
            locals: &locals,
            page_actions: &page_actions,
            action_url: "/todos/live",
            param: "event",
            is_submit_context: false,
        };
        let js = compile_handler_body(
            "let todos = await action('addTodo', { title: title })\napplyTodos(todos)\ntitle = \"\"",
            &compile_ctx,
            1,
            1,
        )
        .expect("handler");
        assert!(js.contains("const todos = await WebScript.action"));
        assert!(js.contains("handlers.applyTodos(todos)"));
        assert!(!js.contains("handlers.applyTodos(signals.todos.get())"));
    }

    #[test]
    fn compiles_action_call_with_object_literal() {
        let signals = BTreeSet::from(["title".to_string()]);
        let handlers = BTreeSet::new();
        let page_actions =
            BTreeMap::from([("addTodo".to_string(), Some("AddTodoInput".to_string()))]);
        let locals = BTreeSet::new();
        let compile_ctx = HandlerCompileContext {
            signals: &signals,
            handlers: &handlers,
            locals: &locals,
            page_actions: &page_actions,
            action_url: "/todos/live",
            param: "event",
            is_submit_context: false,
        };
        let js = compile_handler_body(
            "let todos = await action('addTodo', { title: title })",
            &compile_ctx,
            1,
            1,
        )
        .expect("handler");
        assert!(js.contains("WebScript.action(actionUrl, 'addTodo'"));
        assert!(js.contains("signals.title.get()"));
    }

    #[test]
    fn renders_submit_with_prevent_default_and_custom_param() {
        let manifest = IslandManifest {
            id: "Form-0".to_string(),
            component: "Form".to_string(),
            action_url: "/".to_string(),
            signals: Vec::new(),
            event_handlers: vec![EventHandler {
                event: "submit".to_string(),
                index: 0,
                handler_source: "|e| save()".to_string(),
                js_body: "handlers.save(e)".to_string(),
                param_name: "e".to_string(),
                prevent_default: true,
                stop_propagation: false,
            }],
            named_handlers: vec![NamedHandler {
                name: "save".to_string(),
                param_name: String::new(),
                js_body: "signals.saved.set(true)".to_string(),
                is_async: false,
            }],
            text_bindings: Vec::new(),
            value_bindings: Vec::new(),
            html_bindings: Vec::new(),
            if_bindings: Vec::new(),
            attr_bindings: Vec::new(),
            for_bindings: Vec::new(),
            bootstrap: None,
        };

        let script = render_island_script(&manifest);
        assert!(script.contains("addEventListener('submit'"));
        assert!(script.contains("(e) => { e.preventDefault();handlers.save(e)"));
        assert!(script.contains("save: (event) =>"));
    }
}
