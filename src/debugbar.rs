use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Model,
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

#[derive(Debug)]
pub struct TaskTrace {
    origin: Instant,
    tasks: Vec<AsyncTaskSpan>,
}

impl TaskTrace {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
            tasks: Vec::new(),
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

    pub fn spans(&self) -> &[AsyncTaskSpan] {
        &self.tasks
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
    pub total_ms: u64,
}

impl RequestMetrics {
    pub fn new(request_path: impl Into<String>) -> Self {
        Self {
            request_path: request_path.into(),
            route_file: None,
            entries: Vec::new(),
            tasks: Vec::new(),
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
    let pills = pills.join("");

    let rows = metrics
        .entries
        .iter()
        .map(|entry| {
            let detail = entry
                .detail
                .as_ref()
                .map(|value| {
                    format!(
                        r#"<td class="ws-debugbar-detail">{detail}</td>"#,
                        detail = html_escape(value)
                    )
                })
                .unwrap_or_else(|| r#"<td class="ws-debugbar-detail"></td>"#.to_string());

            format!(
                r#"<tr><td>{label}</td><td>{duration} ms</td>{detail}</tr>"#,
                label = html_escape(&entry.label),
                duration = entry.duration_ms,
                detail = detail
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let route_row = metrics
        .route_file
        .as_ref()
        .map(|file| {
            format!(
                r#"<tr><td>Route file</td><td colspan="2">{path}</td></tr>"#,
                path = html_escape(&file.display().to_string())
            )
        })
        .unwrap_or_default();

    let request_path = html_escape(&metrics.request_path);
    let gantt = render_gantt(&metrics.tasks, metrics.total_ms);

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
.ws-debugbar-task-sleep {{ background: linear-gradient(180deg, #c58af9, #a855f7); }}
.ws-debugbar-task-fetch {{ background: linear-gradient(180deg, #5bb974, #3d9e50); }}
.ws-debugbar-task-timeout {{ background: linear-gradient(180deg, #f5a742, #e37400); }}
.ws-debugbar-task-spawn {{ background: linear-gradient(180deg, #78d9ec, #24c1e0); }}
.ws-debugbar-task-await {{ background: linear-gradient(180deg, #9aa0a6, #6f757b); }}
.ws-debugbar-gantt-empty {{
  color: #9aa0a6;
  font-style: italic;
}}
</style>
<div class="ws-debugbar-summary" data-ws-debugbar-toggle>
  <span class="ws-debugbar-brand">WebScript</span>
  <span class="ws-debugbar-pill ws-debugbar-total">Total: {total} ms</span>
  {pills}
  <span class="ws-debugbar-toggle">&#9650;</span>
</div>
<div class="ws-debugbar-panel">
  <table>
    <thead>
      <tr><th>Phase</th><th>Time</th><th>Detail</th></tr>
    </thead>
    <tbody>
      <tr><td>Request</td><td colspan="2">{request_path}</td></tr>
      {route_row}
      {rows}
      <tr><td>Total</td><td colspan="2">{total} ms</td></tr>
    </tbody>
  </table>
  {gantt}
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
        request_path = request_path,
        route_row = route_row,
        rows = rows,
        gantt = gantt
    )
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
            format!(
                r#"<div class="ws-debugbar-gantt-row">
  <div class="ws-debugbar-gantt-label" title="{title}">{label}</div>
  <div class="ws-debugbar-gantt-track">
    <div class="ws-debugbar-gantt-bar {class}" style="left:{left:.2}%;width:{width:.2}%" title="{title}"></div>
  </div>
</div>"#,
                title = title,
                label = html_escape(&task.label),
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

#[cfg(test)]
mod tests {
    use super::{render_gantt, render_html, AsyncTaskSpan, RequestMetrics, TaskKind, TaskTrace};
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
                label: "await Todo.all".to_string(),
                kind: TaskKind::Await,
                start_ms: 1,
                duration_ms: 10,
            },
        ];

        let html = render_gantt(&tasks, 12);

        assert!(html.contains("Async timeline"));
        assert!(html.contains("Todo.all"));
        assert!(html.contains("ws-debugbar-task-model"));
        assert!(html.contains("ws-debugbar-task-await"));
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
}
