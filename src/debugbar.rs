use std::path::PathBuf;
use std::time::{Duration, Instant};

const DEVTOOLS_CLIENT_SCRIPT: &str = include_str!("devtools-client.js");

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

pub fn metrics_to_json(metrics: &RequestMetrics) -> serde_json::Value {
    serde_json::json!({
        "requestPath": metrics.request_path,
        "routeFile": metrics
            .route_file
            .as_ref()
            .map(|file| file.display().to_string())
            .unwrap_or_default(),
        "entries": metrics
            .entries
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "label": entry.label,
                    "duration": entry.duration_ms,
                    "detail": entry.detail.clone().unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>(),
        "tasks": metrics
            .tasks
            .iter()
            .map(|task| {
                serde_json::json!({
                    "label": task.label,
                    "kind": task_kind_name(task.kind),
                    "startMs": task.start_ms,
                    "durationMs": task.duration_ms,
                })
            })
            .collect::<Vec<_>>(),
        "queries": metrics
            .queries
            .iter()
            .map(|query| {
                serde_json::json!({
                    "label": query.label,
                    "source": query.source.as_str(),
                    "sql": query.sql,
                    "params": format_params(&query.params),
                    "duration": query.duration_ms,
                    "status": if query.success { "ok" } else { "error" },
                    "error": query.error.clone().unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>(),
        "total": metrics.total_ms,
    })
}

fn task_kind_name(kind: TaskKind) -> &'static str {
    match kind {
        TaskKind::Model => "model",
        TaskKind::Db => "db",
        TaskKind::Sleep => "sleep",
        TaskKind::Fetch => "fetch",
        TaskKind::Timeout => "timeout",
        TaskKind::Spawn => "spawn",
        TaskKind::Await => "await",
    }
}

pub fn render_html(metrics: &RequestMetrics) -> String {
    let metrics_json = metrics_to_json(metrics).to_string();

    format!(
        r#"<script type="application/json" id="ws-request-metrics">{metrics_json}</script>
<div id="webscript-debugbar" class="ws-debugbar">
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
  user-select: none;
}}
.ws-debugbar-summary[data-ws-debugbar-toggle] {{
  cursor: pointer;
}}
.ws-debugbar-path {{
  max-width: 240px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}}
.ws-debugbar-session-pill {{
  cursor: pointer;
}}
.ws-debugbar-session-row {{
  cursor: pointer;
}}
.ws-debugbar-session-row[aria-current="true"] {{
  background: #2b2d31;
}}
.ws-debugbar-session-row:hover {{
  background: #303238;
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
<div class="ws-debugbar-summary" data-ws-debugbar-summary data-ws-debugbar-toggle>
  <span class="ws-debugbar-brand">WebScript</span>
  <span class="ws-debugbar-pill ws-debugbar-total">Total: {total} ms</span>
  <span class="ws-debugbar-toggle">&#9650;</span>
</div>
<div class="ws-debugbar-panel"></div>
<script>{devtools_script}</script>
</div>"#,
        total = metrics.total_ms,
        metrics_json = metrics_json,
        devtools_script = DEVTOOLS_CLIENT_SCRIPT,
    )
}

fn format_params(params: &[String]) -> String {
    if params.is_empty() {
        return String::new();
    }
    params.join(", ")
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
        metrics_to_json, render_gantt, render_html, AsyncTaskSpan, DbQueryEntry, QuerySource,
        RequestMetrics, TaskKind, TaskTrace,
    };
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn render_html_includes_metrics_payload_and_shell() {
        let mut metrics = RequestMetrics::new("/posts");
        metrics.push("Route", Duration::from_millis(4), None);
        metrics.push("Components", Duration::from_millis(3), None);
        metrics.push("Render", Duration::from_millis(5), None);
        metrics.set_total(Duration::from_millis(12));

        let html = render_html(&metrics);

        assert!(html.contains(r#"id="ws-request-metrics""#));
        assert!(html.contains(r#""requestPath":"/posts""#));
        assert!(html.contains(r#""label":"Route""#));
        assert!(html.contains(r#""duration":4"#));
        assert!(html.contains("Total: 12 ms"));
        assert!(html.contains("webscript-debugbar"));
        assert!(html.contains("WebScript.devtools"));
        assert!(html.contains("color-scheme: dark"));
        assert!(html.contains(".ws-debugbar td"));
        assert!(html.contains("color: #e8eaed"));
    }

    #[test]
    fn metrics_to_json_includes_route_file_when_provided() {
        let mut metrics = RequestMetrics::new("/");
        metrics.route_file = Some(PathBuf::from("app/pages/index.web"));
        metrics.set_total(Duration::from_millis(1));

        let json = metrics_to_json(&metrics);

        assert_eq!(json["requestPath"], "/");
        assert_eq!(json["routeFile"], "app/pages/index.web");
        assert_eq!(json["total"], 1);
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
    fn metrics_to_json_includes_query_rows() {
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

        let json = metrics_to_json(&metrics);
        let queries = json["queries"].as_array().expect("queries array");

        assert_eq!(queries.len(), 2);
        assert_eq!(queries[0]["sql"], "SELECT * FROM Todo ORDER BY createdAt");
        assert_eq!(queries[1]["sql"], "SELECT nope FROM Missing");
        assert_eq!(queries[1]["error"], "no such table: Missing");
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
