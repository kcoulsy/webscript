use crate::diagnostic::{Diagnostic, Span};
use crate::parser::{ClientInitial, ClientSignalDecl, EventBinding, Value};

pub const RUNTIME_PATH: &str = "/.web/runtime.js";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IslandManifest {
    pub id: String,
    pub component: String,
    pub signals: Vec<SignalBinding>,
    pub click_handlers: Vec<ClickHandler>,
    pub text_bindings: Vec<TextBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalBinding {
    pub name: String,
    pub type_name: String,
    pub initial: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClickHandler {
    pub handler_source: String,
    pub js_body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBinding {
    pub signal: String,
}

pub fn client_runtime_script() -> &'static str {
    r#"window.WebScript = window.WebScript || {};
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
                format!("unknown prop `{name}` in @client signal `{signal_name}`", signal_name = signal.name),
                None,
            )
        }),
    }
}

pub fn compile_click_handler(source: &str, line: usize, column: usize) -> Result<String, Diagnostic> {
    let source = source.trim();
    if let Some(name) = source.strip_suffix("++") {
        let name = name.trim();
        if is_identifier(name) {
            return Ok(format!("signals.{name}.set(signals.{name}.get() + 1)"));
        }
    }
    if let Some(name) = source.strip_suffix("--") {
        let name = name.trim();
        if is_identifier(name) {
            return Ok(format!("signals.{name}.set(signals.{name}.get() - 1)"));
        }
    }

    if let Some((left, right)) = source.split_once('=') {
        let left = left.trim();
        let right = right.trim();
        if !is_identifier(left) {
            return Err(invalid_handler(source, line, column));
        }

        if right == format!("{left} + 1") {
            return Ok(format!("signals.{left}.set(signals.{left}.get() + 1)"));
        }
        if right == format!("{left} - 1") {
            return Ok(format!("signals.{left}.set(signals.{left}.get() - 1)"));
        }
    }

    Err(invalid_handler(source, line, column))
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
        format!("  const signals = {{ {} }};", signal_inits.join(", ")),
    ];

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

    for (index, handler) in manifest.click_handlers.iter().enumerate() {
        lines.push(format!(
            "  const click_{index} = root.querySelector('[data-ws-click]');"
        ));
        lines.push(format!(
            "  click_{index}?.addEventListener('click', () => {{ {body}; }});",
            body = handler.js_body
        ));
    }

    lines.push("})();".to_string());
    lines.push("</script>".to_string());
    lines.join("\n")
}

pub fn inject_client_scripts(html: &str, scripts: &str) -> String {
    if scripts.is_empty() {
        return html.to_string();
    }

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

pub fn build_click_handler(binding: &EventBinding) -> Result<ClickHandler, Diagnostic> {
    let js_body = compile_click_handler(
        &binding.handler_source,
        binding.line,
        binding.column,
    )?;
    Ok(ClickHandler {
        handler_source: binding.handler_source.clone(),
        js_body,
    })
}

fn js_literal(value: &Value) -> String {
    match value {
        Value::Int(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::String(value) => format!("\"{}\"", escape_js_string(value)),
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

fn invalid_handler(source: &str, line: usize, column: usize) -> Diagnostic {
    Diagnostic::error(
        Span::new(line, column, column + source.len()),
        format!("unsupported click handler `{source}`"),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_increment_handler() {
        let js = compile_click_handler("count++", 1, 1).expect("handler");
        assert_eq!(js, "signals.count.set(signals.count.get() + 1)");
    }

    #[test]
    fn compiles_assignment_increment_handler() {
        let js = compile_click_handler("count = count + 1", 1, 1).expect("handler");
        assert_eq!(js, "signals.count.set(signals.count.get() + 1)");
    }

    #[test]
    fn rejects_unknown_handler() {
        let error = compile_click_handler("save()", 1, 1).expect_err("handler");
        assert!(error.message.contains("unsupported click handler"));
    }

    #[test]
    fn renders_island_script_with_signal_and_click() {
        let manifest = IslandManifest {
            id: "Counter-0".to_string(),
            component: "Counter".to_string(),
            signals: vec![SignalBinding {
                name: "count".to_string(),
                type_name: "int".to_string(),
                initial: Value::Int(5),
            }],
            click_handlers: vec![ClickHandler {
                handler_source: "count++".to_string(),
                js_body: "signals.count.set(signals.count.get() + 1)".to_string(),
            }],
            text_bindings: vec![TextBinding {
                signal: "count".to_string(),
            }],
        };

        let script = render_island_script(&manifest);
        assert!(script.contains("data-ws-island=\"Counter-0\""));
        assert!(script.contains("WebScript.signal(5)"));
        assert!(script.contains("addEventListener('click'"));
        assert!(script.contains("data-ws-text=\"count\""));
    }
}
