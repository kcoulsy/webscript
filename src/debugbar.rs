use std::path::PathBuf;
use std::time::Duration;

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
    pub total_ms: u64,
}

impl RequestMetrics {
    pub fn new(request_path: impl Into<String>) -> Self {
        Self {
            request_path: request_path.into(),
            route_file: None,
            entries: Vec::new(),
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
    let pills = metrics
        .entries
        .iter()
        .map(|entry| {
            format!(
                r#"<span class="ws-debugbar-pill">{label}: {duration} ms</span>"#,
                label = html_escape(&entry.label),
                duration = entry.duration_ms
            )
        })
        .collect::<Vec<_>>()
        .join("");

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
  max-height: 220px;
  overflow: auto;
  background: #25262a;
  border-top: 1px solid #3a3b3f;
  padding: 8px 12px 12px;
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
}}
.ws-debugbar th {{
  color: #9aa0a6;
  font-weight: 500;
}}
.ws-debugbar-detail {{
  color: #9aa0a6;
  word-break: break-all;
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
        rows = rows
    )
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
    use super::{render_html, RequestMetrics};
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
}
