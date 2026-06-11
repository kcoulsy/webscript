use crate::parser::{self, Value};
use crate::runtime::WebRuntime;
use crate::{client, render, style};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const DEVTOOLS_COMPONENT: &str = include_str!("devtools.web");
const DEVTOOLS_PAGE: &str = r#"@page "/__webscript/devtools"
@layout none

<WebScriptDevtools
  requestPath={requestPath}
  routeFile={routeFile}
  entries={entries}
  tasks={tasks}
  ticks={ticks}
  queries={queries}
  total={total}
/>
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Model,
    Db,
    Sleep,
    Fetch,
    Timeout,
    Spawn,
    Await,
}

impl TaskKind {
    fn css_class(self) -> &'static str {
        match self {
            Self::Model => "ws-debugbar-task-model",
            Self::Db => "ws-debugbar-task-db",
            Self::Sleep => "ws-debugbar-task-sleep",
            Self::Fetch => "ws-debugbar-task-fetch",
            Self::Timeout => "ws-debugbar-task-timeout",
            Self::Spawn => "ws-debugbar-task-spawn",
            Self::Await => "ws-debugbar-task-await",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncTaskSpan {
    pub label: String,
    pub kind: TaskKind,
    pub start_ms: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuerySource {
    Raw,
    Model,
}

impl QuerySource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Model => "model",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbQueryEntry {
    pub label: String,
    pub source: QuerySource,
    pub sql: String,
    pub params: Vec<String>,
    pub duration_ms: u64,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct TaskTrace {
    origin: Instant,
    tasks: Vec<AsyncTaskSpan>,
    queries: Vec<DbQueryEntry>,
}

impl TaskTrace {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
            tasks: Vec::new(),
            queries: Vec::new(),
        }
    }

    pub fn begin(&mut self, label: impl Into<String>, kind: TaskKind) -> usize {
        let index = self.tasks.len();
        self.tasks.push(AsyncTaskSpan {
            label: label.into(),
            kind,
            start_ms: self.origin.elapsed().as_millis() as u64,
            duration_ms: 0,
        });
        index
    }

    pub fn finish(&mut self, index: usize) {
        if let Some(task) = self.tasks.get_mut(index) {
            let end_ms = self.origin.elapsed().as_millis() as u64;
            task.duration_ms = end_ms.saturating_sub(task.start_ms).max(1);
        }
    }

    pub fn begin_query(
        &mut self,
        label: impl Into<String>,
        source: QuerySource,
        sql: impl Into<String>,
        params: Vec<String>,
    ) -> usize {
        let index = self.queries.len();
        self.queries.push(DbQueryEntry {
            label: label.into(),
            source,
            sql: sql.into(),
            params,
            duration_ms: self.origin.elapsed().as_millis() as u64,
            success: true,
            error: None,
        });
        index
    }

    pub fn finish_query(&mut self, index: usize, success: bool, error: Option<String>) {
        let end_ms = self.origin.elapsed().as_millis() as u64;
        if let Some(query) = self.queries.get_mut(index) {
            query.duration_ms = end_ms.saturating_sub(query.duration_ms).max(1);
            query.success = success;
            query.error = error;
        }
    }

    pub fn spans(&self) -> &[AsyncTaskSpan] {
        &self.tasks
    }

    pub fn queries(&self) -> &[DbQueryEntry] {
        &self.queries
    }
}

impl Default for TaskTrace {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimingEntry {
    pub label: String,
    pub duration_ms: u64,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestMetrics {
    pub request_path: String,
    pub route_file: Option<PathBuf>,
    pub entries: Vec<TimingEntry>,
    pub tasks: Vec<AsyncTaskSpan>,
    pub queries: Vec<DbQueryEntry>,
    pub total_ms: u64,
}

impl RequestMetrics {
    pub fn new(request_path: impl Into<String>) -> Self {
        Self {
            request_path: request_path.into(),
            route_file: None,
            entries: Vec::new(),
            tasks: Vec::new(),
            queries: Vec::new(),
            total_ms: 0,
        }
    }

    pub fn push(&mut self, label: impl Into<String>, duration: Duration, detail: Option<String>) {
        self.entries.push(TimingEntry {
            label: label.into(),
            duration_ms: duration.as_millis() as u64,
            detail,
        });
    }

    pub fn set_total(&mut self, duration: Duration) {
        self.total_ms = duration.as_millis() as u64;
    }
}

pub fn render_html(metrics: &RequestMetrics) -> String {
    let mut pills = metrics
        .entries
        .iter()
        .map(|entry| {
            format!(
                r#"<span class="ws-debugbar-pill">{label}: {duration} ms</span>"#,
                label = html_escape(&entry.label),
                duration = entry.duration_ms
            )
        })
        .collect::<Vec<_>>();
    if !metrics.tasks.is_empty() {
        pills.push(format!(
            r#"<span class="ws-debugbar-pill">Tasks: {count}</span>"#,
            count = metrics.tasks.len()
        ));
    }
    if !metrics.queries.is_empty() {
        pills.push(format!(
            r#"<span class="ws-debugbar-pill">Queries: {count}</span>"#,
            count = metrics.queries.len()
        ));
    }
    let pills = pills.join("");

    let panel = render_devtools_panel(metrics).unwrap_or_else(|error| {
        format!(
            r#"<p class="ws-debugbar-empty">Devtools failed to render: {}</p>"#,
            html_escape(&error)
        )
    });

    format!(
        r#"<div id="webscript-debugbar" class="ws-debugbar">
<style>
.ws-debugbar {{
  position: fixed;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 99999;
  font: 12px/1.4 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  color: #e8eaed;
  color-scheme: dark;
}}
.ws-debugbar-summary {{
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 12px;
  background: #1e1f22;
  border-top: 1px solid #3a3b3f;
  cursor: pointer;
  user-select: none;
}}
.ws-debugbar-brand {{
  font-weight: 600;
  color: #8ab4f8;
}}
.ws-debugbar-pill {{
  padding: 2px 8px;
  border-radius: 999px;
  background: #2b2d31;
  border: 1px solid #3a3b3f;
}}
.ws-debugbar-total {{
  font-weight: 600;
}}
.ws-debugbar-toggle {{
  margin-left: auto;
  opacity: 0.7;
}}
.ws-debugbar-panel {{
  display: none;
  max-height: 360px;
  overflow: auto;
  background: #25262a;
  border-top: 1px solid #3a3b3f;
  padding: 8px 12px 12px;
  color: #e8eaed;
}}
.ws-debugbar.open .ws-debugbar-panel {{
  display: block;
}}
.ws-debugbar.open .ws-debugbar-toggle {{
  transform: rotate(180deg);
}}
.ws-debugbar table {{
  width: 100%;
  border-collapse: collapse;
}}
.ws-debugbar th,
.ws-debugbar td {{
  text-align: left;
  padding: 4px 8px;
  border-bottom: 1px solid #3a3b3f;
  color: #e8eaed;
}}
.ws-debugbar th {{
  color: #9aa0a6;
  font-weight: 500;
}}
.ws-debugbar-detail {{
  color: #9aa0a6;
  word-break: break-all;
}}
.ws-debugbar-section {{
  margin-top: 12px;
}}
.ws-debugbar-section h3 {{
  margin: 0 0 8px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: #9aa0a6;
}}
.ws-debugbar-gantt {{
  display: flex;
  flex-direction: column;
  gap: 4px;
}}
.ws-debugbar-gantt-axis {{
  display: grid;
  grid-template-columns: 160px 1fr;
  gap: 8px;
  margin-bottom: 2px;
  color: #9aa0a6;
  font-size: 10px;
}}
.ws-debugbar-gantt-axis-track {{
  position: relative;
  height: 14px;
  border-bottom: 1px solid #3a3b3f;
}}
.ws-debugbar-gantt-tick {{
  position: absolute;
  bottom: 0;
  transform: translateX(-50%);
  padding-bottom: 2px;
}}
.ws-debugbar-gantt-tick::before {{
  content: "";
  position: absolute;
  left: 50%;
  bottom: 100%;
  width: 1px;
  height: 6px;
  background: #3a3b3f;
}}
.ws-debugbar-gantt-row {{
  display: grid;
  grid-template-columns: 160px 1fr;
  gap: 8px;
  align-items: center;
  min-height: 22px;
}}
.ws-debugbar-gantt-label {{
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: #c4c7c5;
}}
.ws-debugbar-gantt-track {{
  position: relative;
  height: 18px;
  background: #1e1f22;
  border: 1px solid #3a3b3f;
  border-radius: 4px;
}}
.ws-debugbar-gantt-bar {{
  position: absolute;
  top: 2px;
  bottom: 2px;
  min-width: 2px;
  border-radius: 3px;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
}}
.ws-debugbar-task-model {{ background: linear-gradient(180deg, #669df6, #4c7fe6); }}
.ws-debugbar-task-db {{ background: linear-gradient(180deg, #3db9a8, #2a9d8f); }}
.ws-debugbar-task-sleep {{ background: linear-gradient(180deg, #c58af9, #a855f7); }}
.ws-debugbar-task-fetch {{ background: linear-gradient(180deg, #5bb974, #3d9e50); }}
.ws-debugbar-task-timeout {{ background: linear-gradient(180deg, #f5a742, #e37400); }}
.ws-debugbar-task-spawn {{ background: linear-gradient(180deg, #78d9ec, #24c1e0); }}
.ws-debugbar-task-await {{ background: linear-gradient(180deg, #9aa0a6, #6f757b); }}
.ws-debugbar-gantt-empty {{
  color: #9aa0a6;
  font-style: italic;
}}
.ws-debugbar-sql {{
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace;
  word-break: break-word;
}}
.ws-debugbar-error {{
  color: #f28b82;
  word-break: break-word;
}}
</style>
<div class="ws-debugbar-summary" data-ws-debugbar-toggle>
  <span class="ws-debugbar-brand">WebScript</span>
  <span class="ws-debugbar-pill ws-debugbar-total">Total: {total} ms</span>
  {pills}
  <span class="ws-debugbar-toggle">&#9650;</span>
</div>
<div class="ws-debugbar-panel">
  {panel}
</div>
<script>
(() => {{
  const bar = document.getElementById("webscript-debugbar");
  const toggle = bar?.querySelector("[data-ws-debugbar-toggle]");
  toggle?.addEventListener("click", () => bar.classList.toggle("open"));
}})();
</script>
</div>"#,
        total = metrics.total_ms,
        pills = pills,
        panel = panel
    )
}

fn render_devtools_panel(metrics: &RequestMetrics) -> Result<String, String> {
    let component = parser::parse(DEVTOOLS_COMPONENT).map_err(|error| error.message)?;
    let page = parser::parse(DEVTOOLS_PAGE).map_err(|error| error.message)?;
    let mut components = render::ComponentRegistry::new();
    components.insert("WebScriptDevtools".to_string(), component);

    let runtime = WebRuntime::new();
    let output = render::render_with_components(&page, &metrics_scope(metrics), &components, &runtime)
        .map_err(|error| error.message)?;
    let style_fragment = style::render_style_tags(&output.global_styles, &output.scoped_styles);
    let html = style::inject_styles(&output.html, &style_fragment);
    let scripts = output
        .islands
        .iter()
        .map(client::render_island_script)
        .collect::<String>();
    Ok(client::inject_client_scripts(&html, &scripts))
}

fn metrics_scope(metrics: &RequestMetrics) -> render::Scope {
    let mut scope = render::Scope::new();
    scope.insert(
        "requestPath".to_string(),
        Value::String(metrics.request_path.clone()),
    );
    scope.insert(
        "routeFile".to_string(),
        Value::String(
            metrics
                .route_file
                .as_ref()
                .map(|file| file.display().to_string())
                .unwrap_or_default(),
        ),
    );
    scope.insert("entries".to_string(), timing_entries_value(&metrics.entries));
    let timeline_ms = timeline_ms(&metrics.tasks, metrics.total_ms);
    scope.insert("tasks".to_string(), task_entries_value(&metrics.tasks, timeline_ms));
    scope.insert("ticks".to_string(), tick_entries_value(timeline_ms));
    scope.insert("queries".to_string(), query_entries_value(&metrics.queries));
    scope.insert("total".to_string(), Value::Int(metrics.total_ms as i64));
    scope
}

fn timing_entries_value(entries: &[TimingEntry]) -> Value {
    Value::Array {
        element_type: "object".to_string(),
        values: entries
            .iter()
            .map(|entry| {
                object_value([
                    ("label", Value::String(entry.label.clone())),
                    ("duration", Value::Int(entry.duration_ms as i64)),
                    (
                        "detail",
                        Value::String(entry.detail.clone().unwrap_or_default()),
                    ),
                ])
            })
            .collect(),
    }
}

fn task_entries_value(tasks: &[AsyncTaskSpan], timeline_ms: u64) -> Value {
    Value::Array {
        element_type: "object".to_string(),
        values: tasks
            .iter()
            .map(|task| {
                let left = (task.start_ms as f64 / timeline_ms as f64) * 100.0;
                let width = ((task.duration_ms as f64 / timeline_ms as f64) * 100.0).max(0.4);
                let title = format!("{} - {} ms", task.label, task.duration_ms);
                object_value([
                    ("label", Value::String(truncate_gantt_label(&task.label, 80))),
                    ("title", Value::String(title)),
                    ("cssClass", Value::String(task.kind.css_class().to_string())),
                    (
                        "style",
                        Value::String(format!("left:{left:.2}%;width:{width:.2}%")),
                    ),
                ])
            })
            .collect(),
    }
}

fn tick_entries_value(timeline_ms: u64) -> Value {
    Value::Array {
        element_type: "object".to_string(),
        values: gantt_ticks(timeline_ms)
            .into_iter()
            .map(|ms| {
                let left = (ms as f64 / timeline_ms as f64) * 100.0;
                object_value([
                    ("label", Value::String(format!("{ms} ms"))),
                    ("style", Value::String(format!("left:{left:.2}%"))),
                ])
            })
            .collect(),
    }
}

fn query_entries_value(queries: &[DbQueryEntry]) -> Value {
    Value::Array {
        element_type: "object".to_string(),
        values: queries
            .iter()
            .map(|query| {
                let status = if query.success { "ok" } else { "error" };
                object_value([
                    ("label", Value::String(query.label.clone())),
                    ("source", Value::String(query.source.as_str().to_string())),
                    ("sql", Value::String(query.sql.clone())),
                    ("params", Value::String(format_params(&query.params))),
                    ("duration", Value::Int(query.duration_ms as i64)),
                    ("status", Value::String(status.to_string())),
                    ("error", Value::String(query.error.clone().unwrap_or_default())),
                ])
            })
            .collect(),
    }
}

fn object_value<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn format_params(params: &[String]) -> String {
    if params.is_empty() {
        return String::new();
    }
    params.join(", ")
}

fn timeline_ms(tasks: &[AsyncTaskSpan], total_ms: u64) -> u64 {
    tasks
        .iter()
        .map(|task| task.start_ms.saturating_add(task.duration_ms))
        .max()
        .unwrap_or(0)
        .max(total_ms)
        .max(1)
}

fn render_gantt(tasks: &[AsyncTaskSpan], total_ms: u64) -> String {
    if tasks.is_empty() {
        return String::new();
    }

    let timeline_ms = tasks
        .iter()
        .map(|task| task.start_ms.saturating_add(task.duration_ms))
        .max()
        .unwrap_or(0)
        .max(total_ms)
        .max(1);

    let ticks = gantt_ticks(timeline_ms);
    let tick_marks = ticks
        .iter()
        .map(|ms| {
            let left = (*ms as f64 / timeline_ms as f64) * 100.0;
            format!(
                r#"<span class="ws-debugbar-gantt-tick" style="left:{left:.2}%">{ms} ms</span>"#
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let rows = tasks
        .iter()
        .map(|task| {
            let left = (task.start_ms as f64 / timeline_ms as f64) * 100.0;
            let width = (task.duration_ms as f64 / timeline_ms as f64) * 100.0;
            let title = html_escape(&format!(
                "{} — {} ms",
                task.label,
                task.duration_ms
            ));
            let display_label = html_escape(&truncate_gantt_label(&task.label, 80));
            format!(
                r#"<div class="ws-debugbar-gantt-row">
  <div class="ws-debugbar-gantt-label" title="{title}">{label}</div>
  <div class="ws-debugbar-gantt-track">
    <div class="ws-debugbar-gantt-bar {class}" style="left:{left:.2}%;width:{width:.2}%" title="{title}"></div>
  </div>
</div>"#,
                title = title,
                label = display_label,
                class = task.kind.css_class(),
                left = left,
                width = width.max(0.4)
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r#"<div class="ws-debugbar-section">
  <h3>Async timeline</h3>
  <div class="ws-debugbar-gantt">
    <div class="ws-debugbar-gantt-axis">
      <div></div>
      <div class="ws-debugbar-gantt-axis-track">{tick_marks}</div>
    </div>
    {rows}
  </div>
</div>"#,
        tick_marks = tick_marks,
        rows = rows
    )
}

fn gantt_ticks(timeline_ms: u64) -> Vec<u64> {
    let step = match timeline_ms {
        0..=20 => 5,
        21..=100 => 10,
        101..=500 => 50,
        501..=2000 => 200,
        _ => 500,
    };
    let mut ticks = Vec::new();
    let mut value = 0;
    while value <= timeline_ms {
        ticks.push(value);
        value += step;
    }
    if ticks.last().copied() != Some(timeline_ms) {
        ticks.push(timeline_ms);
    }
    ticks
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn truncate_gantt_label(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }
    let truncated: String = value.chars().take(max_len.saturating_sub(1)).collect();
    format!("{truncated}…")
}

#[cfg(test)]
mod tests {
    use super::{
        render_gantt, render_html, AsyncTaskSpan, DbQueryEntry, QuerySource, RequestMetrics,
        TaskKind, TaskTrace,
    };
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn render_html_includes_timing_labels_and_values() {
        let mut metrics = RequestMetrics::new("/posts");
        metrics.push("Route", Duration::from_millis(4), None);
        metrics.push("Components", Duration::from_millis(3), None);
        metrics.push("Render", Duration::from_millis(5), None);
        metrics.set_total(Duration::from_millis(12));

        let html = render_html(&metrics);

        assert!(html.contains("Route: 4 ms"));
        assert!(html.contains("Components: 3 ms"));
        assert!(html.contains("Render: 5 ms"));
        assert!(html.contains("Total: 12 ms"));
        assert!(html.contains("Timings"));
        assert!(html.contains("Async Timeline"));
        assert!(html.contains("Queries"));
        assert!(html.contains("/posts"));
        assert!(html.contains("color-scheme: dark"));
        assert!(html.contains(".ws-debugbar td"));
        assert!(html.contains("color: #e8eaed"));
    }

    #[test]
    fn render_html_includes_route_file_when_provided() {
        let mut metrics = RequestMetrics::new("/");
        metrics.route_file = Some(PathBuf::from("app/pages/index.web"));
        metrics.set_total(Duration::from_millis(1));

        let html = render_html(&metrics);

        assert!(html.contains("app/pages/index.web"));
        assert!(html.contains("Route file"));
    }

    #[test]
    fn render_gantt_includes_task_bars() {
        let tasks = vec![
            AsyncTaskSpan {
                label: "Todo.all".to_string(),
                kind: TaskKind::Model,
                start_ms: 2,
                duration_ms: 8,
            },
            AsyncTaskSpan {
                label: "db.query(\"SELECT * FROM Todo\")".to_string(),
                kind: TaskKind::Db,
                start_ms: 3,
                duration_ms: 6,
            },
            AsyncTaskSpan {
                label: "await Todo.all".to_string(),
                kind: TaskKind::Await,
                start_ms: 1,
                duration_ms: 10,
            },
        ];

        let html = render_gantt(&tasks, 12);

        assert!(html.contains("Async timeline"));
        assert!(html.contains("Todo.all"));
        assert!(html.contains("db.query"));
        assert!(html.contains("ws-debugbar-task-model"));
        assert!(html.contains("ws-debugbar-task-db"));
        assert!(html.contains("ws-debugbar-task-await"));
    }

    #[test]
    fn render_html_includes_query_rows() {
        let mut metrics = RequestMetrics::new("/todos/live");
        metrics.queries.push(DbQueryEntry {
            label: "Todo.all".to_string(),
            source: QuerySource::Model,
            sql: "SELECT * FROM Todo ORDER BY createdAt".to_string(),
            params: Vec::new(),
            duration_ms: 4,
            success: true,
            error: None,
        });
        metrics.queries.push(DbQueryEntry {
            label: "db.query".to_string(),
            source: QuerySource::Raw,
            sql: "SELECT nope FROM Missing".to_string(),
            params: vec!["1".to_string()],
            duration_ms: 2,
            success: false,
            error: Some("no such table: Missing".to_string()),
        });
        metrics.set_total(Duration::from_millis(6));

        let html = render_html(&metrics);

        assert!(html.contains("Queries: 2"));
        assert!(html.contains("SELECT * FROM Todo ORDER BY createdAt"));
        assert!(html.contains("SELECT nope FROM Missing"));
        assert!(html.contains("no such table: Missing"));
    }

    #[test]
    fn task_trace_records_span_duration() {
        let mut trace = TaskTrace::new();
        let index = trace.begin("sleep(10ms)", TaskKind::Sleep);
        std::thread::sleep(Duration::from_millis(5));
        trace.finish(index);

        let spans = trace.spans();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].label, "sleep(10ms)");
        assert!(spans[0].duration_ms >= 1);
    }

    #[test]
    fn task_trace_records_query_duration_and_error() {
        let mut trace = TaskTrace::new();
        let index = trace.begin_query(
            "db.query",
            QuerySource::Raw,
            "SELECT 1",
            vec!["1".to_string()],
        );
        std::thread::sleep(Duration::from_millis(5));
        trace.finish_query(index, false, Some("boom".to_string()));

        let queries = trace.queries();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].source, QuerySource::Raw);
        assert_eq!(queries[0].sql, "SELECT 1");
        assert!(!queries[0].success);
        assert_eq!(queries[0].error.as_deref(), Some("boom"));
        assert!(queries[0].duration_ms >= 1);
    }
}
