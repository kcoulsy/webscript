use ariadne::{Color, Label, Report, ReportKind, Source};
use std::io::{self, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    #[allow(dead_code)]
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub start_col: usize,
    pub end_col: usize,
}

impl Span {
    pub fn new(line: usize, start_col: usize, end_col: usize) -> Self {
        Self {
            line,
            start_col,
            end_col: end_col.max(start_col),
        }
    }

    pub fn identifier(line: usize, column: usize, name: &str) -> Self {
        Self::new(line, column, column + name.len())
    }

    #[allow(dead_code)]
    pub fn braced_expr(line: usize, column: usize, name: &str) -> Self {
        Self::new(line, column, column + name.len() + 2)
    }

    #[allow(dead_code)]
    pub fn at(line: usize, column: usize) -> Self {
        Self::new(line, column, column + 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub label: Option<String>,
}

impl Diagnostic {
    pub fn error(span: Span, message: impl Into<String>, label: Option<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            span,
            label,
        }
    }

    #[allow(dead_code)]
    pub fn warning(span: Span, message: impl Into<String>, label: Option<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            span,
            label,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileDiagnostic {
    pub file: PathBuf,
    pub source: String,
    pub diagnostic: Diagnostic,
}

impl FileDiagnostic {
    pub fn new(file: PathBuf, source: String, diagnostic: Diagnostic) -> Self {
        Self {
            file,
            source,
            diagnostic,
        }
    }

    pub fn report_stderr(&self) {
        render_one(self, io::stderr()).ok();
    }

    pub fn dev_error_html(&self) -> String {
        dev_error_page(&self.file, &self.source, &self.diagnostic)
    }
}

pub fn span_to_byte_range(source: &str, span: &Span) -> Range<usize> {
    let mut line_index = 1usize;
    let mut line_start = 0usize;

    for line in source.split_inclusive('\n') {
        if line_index == span.line {
            let line_body = line.strip_suffix('\n').unwrap_or(line);
            let start = line_start + col_to_byte_offset(line_body, span.start_col);
            let end = line_start + col_to_byte_offset(line_body, span.end_col);
            return start..end.max(start);
        }
        line_start += line.len();
        line_index += 1;
    }

    0..0
}

fn col_to_byte_offset(line: &str, column: usize) -> usize {
    if column <= 1 {
        return 0;
    }
    line.char_indices()
        .nth(column - 1)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}

pub fn render_all(diagnostics: &[FileDiagnostic]) {
    for file_diagnostic in diagnostics {
        render_one(file_diagnostic, io::stderr()).ok();
    }
}

pub fn render_one(file_diagnostic: &FileDiagnostic, writer: impl Write) -> io::Result<()> {
    let file_id = file_diagnostic.file.display().to_string();
    let range = span_to_byte_range(&file_diagnostic.source, &file_diagnostic.diagnostic.span);
    let kind = match file_diagnostic.diagnostic.severity {
        Severity::Error => ReportKind::Error,
        Severity::Warning => ReportKind::Warning,
    };

    let mut label = Label::new((file_id.as_str(), range.clone())).with_color(Color::Red);
    if let Some(text) = &file_diagnostic.diagnostic.label {
        label = label.with_message(text.clone());
    }

    let builder = Report::build(kind, file_id.as_str(), range.start)
        .with_message(file_diagnostic.diagnostic.message.clone())
        .with_label(label);

    builder.finish().write(
        (
            file_id.as_str(),
            Source::from(file_diagnostic.source.as_str()),
        ),
        writer,
    )
}

pub fn render_html_snippet(source: &str, diagnostic: &Diagnostic) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let Some(line) = lines.get(diagnostic.span.line.saturating_sub(1)) else {
        return escape_html(&diagnostic.message);
    };

    let start = col_to_byte_offset(line, diagnostic.span.start_col);
    let end = col_to_byte_offset(line, diagnostic.span.end_col).max(start);
    let before = escape_html(&line[..start]);
    let highlight = escape_html(&line[start..end]);
    let after = escape_html(&line[end..]);

    let mut html = format!(
        "<div class=\"error-message\">{}</div>",
        escape_html(&diagnostic.message)
    );
    if let Some(label) = &diagnostic.label {
        html.push_str(&format!(
            "<div class=\"error-label\">{}</div>",
            escape_html(label)
        ));
    }
    html.push_str(&format!(
        "<pre class=\"error-snippet\"><span class=\"line-num\">{:>4} </span>{before}<span class=\"highlight\">{highlight}</span>{after}</pre>",
        diagnostic.span.line
    ));
    html
}

pub fn diagnostic_to_markdown(file: &Path, source: &str, diagnostic: &Diagnostic) -> String {
    let file_path = file.display().to_string();
    let location = format!(
        "{}:{}:{}",
        file_path, diagnostic.span.line, diagnostic.span.start_col
    );

    let mut md = format!("# WebScript Error\n\n**File:** `{location}`\n\n");
    md.push_str(&format!(
        "**Severity:** {}\n\n",
        match diagnostic.severity {
            Severity::Error => "Error",
            Severity::Warning => "Warning",
        }
    ));
    md.push_str(&format!("**Message:** {}\n\n", diagnostic.message));
    if let Some(label) = &diagnostic.label {
        md.push_str(&format!("**Note:** {label}\n\n"));
    }

    let snippet = markdown_code_snippet(source, diagnostic);
    if !snippet.is_empty() {
        md.push_str("## Source\n\n");
        md.push_str(&snippet);
        md.push('\n');
    }

    md
}

fn markdown_code_snippet(source: &str, diagnostic: &Diagnostic) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let error_line = diagnostic.span.line;
    let Some(line) = lines.get(error_line.saturating_sub(1)) else {
        return String::new();
    };

    let start = col_to_byte_offset(line, diagnostic.span.start_col);
    let end = col_to_byte_offset(line, diagnostic.span.end_col).max(start);
    let highlight_len = (end - start).max(1);

    let context_start = error_line.saturating_sub(2).max(1);
    let context_end = (error_line + 2).min(lines.len());
    let width = context_end.to_string().len().max(4);

    let mut body = String::new();
    for line_no in context_start..=context_end {
        let Some(text) = lines.get(line_no.saturating_sub(1)) else {
            continue;
        };
        body.push_str(&format!("{:width$} | {text}\n", line_no + 1));
        if line_no + 1 == error_line {
            let caret_pad = " ".repeat(diagnostic.span.start_col.saturating_sub(1) + width + 3);
            body.push_str(&format!("{caret_pad}| {}\n", "^".repeat(highlight_len)));
        }
    }

    let fence = markdown_fence(&body);
    format!("{fence}\n{body}{fence}")
}

fn markdown_fence(content: &str) -> String {
    let mut ticks = 3;
    while content.contains(&"`".repeat(ticks)) {
        ticks += 1;
    }
    "`".repeat(ticks)
}

pub fn dev_error_page(file: &Path, source: &str, diagnostic: &Diagnostic) -> String {
    let snippet = render_html_snippet(source, diagnostic);
    let markdown = diagnostic_to_markdown(file, source, diagnostic);
    format!(
        "<!doctype html><html><head><title>WebScript Error</title><style>\
        body{{font-family:system-ui,sans-serif;margin:3rem;line-height:1.5}}\
        .error-header{{display:flex;align-items:center;gap:1rem;margin-bottom:1rem}}\
        .error-header h1{{margin:0}}\
        #copy-btn{{font:inherit;padding:0.4rem 0.75rem;border:1px solid #d1d5db;border-radius:6px;background:#fff;cursor:pointer}}\
        #copy-btn:hover{{background:#f9fafb}}\
        #copy-btn.copied{{border-color:#16a34a;color:#16a34a}}\
        .error-message{{font-weight:600;color:#b91c1c;margin-bottom:0.5rem}}\
        .error-label{{color:#6b7280;margin-bottom:1rem}}\
        pre.error-snippet{{background:#1f2937;color:#f9fafb;padding:1rem;border-radius:6px;overflow:auto}}\
        .line-num{{color:#9ca3af}}\
        .highlight{{background:#7f1d1d;text-decoration:underline wavy #fca5a5}}\
        .file{{color:#6b7280;font-size:0.875rem;margin-bottom:1rem}}\
        </style></head><body>\
        <div class=\"error-header\"><h1>WebScript Error</h1><button type=\"button\" id=\"copy-btn\">Copy</button></div>\
        <div class=\"file\">{}:{}:{}</div>\
        {snippet}\
        <textarea id=\"error-markdown\" hidden readonly>{markdown}</textarea>\
        <script>\
        document.getElementById('copy-btn').addEventListener('click',async()=>{{\
        const btn=document.getElementById('copy-btn');\
        const text=document.getElementById('error-markdown').value;\
        try{{await navigator.clipboard.writeText(text);}}\
        catch{{const ta=document.getElementById('error-markdown');ta.hidden=false;ta.select();document.execCommand('copy');ta.hidden=true;}}\
        btn.textContent='Copied!';btn.classList.add('copied');\
        setTimeout(()=>{{btn.textContent='Copy for LLM';btn.classList.remove('copied');}},2000);\
        }});\
        </script>\
        </body></html>",
        escape_html(&file.display().to_string()),
        diagnostic.span.line,
        diagnostic.span.start_col,
        markdown = escape_textarea(&markdown),
    )
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

fn escape_textarea(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let mut escaped = String::with_capacity(value.len());
    let mut index = 0usize;
    while let Some(found) = lower[index..].find("</textarea>") {
        let end = index + found;
        escaped.push_str(&escape_html(&value[index..end]));
        escaped.push_str("&lt;/textarea&gt;");
        index = end + "</textarea>".len();
    }
    escaped.push_str(&escape_html(&value[index..]));
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_to_byte_range_finds_expression() {
        let source = "@page \"/\"\n\n<h1>{name}</h1>";
        let range = span_to_byte_range(source, &Span::braced_expr(3, 5, "name"));
        assert_eq!(&source[range], "{name}");
    }

    #[test]
    fn diagnostic_to_markdown_includes_location_and_snippet() {
        let source = "@page \"/\"\n\n<h1>{name}</h1>";
        let diagnostic = Diagnostic::error(
            Span::braced_expr(3, 5, "name"),
            "unknown variable 'name'",
            Some("did you mean 'title'?".to_string()),
        );
        let markdown =
            diagnostic_to_markdown(Path::new("app/pages/index.web"), source, &diagnostic);

        assert!(markdown.contains("# WebScript Error"));
        assert!(markdown.contains("app/pages/index.web:3:5"));
        assert!(markdown.contains("unknown variable 'name'"));
        assert!(markdown.contains("did you mean 'title'?"));
        assert!(markdown.contains("<h1>{name}</h1>"));
        assert!(markdown.contains("^^^^^^"));
    }
}
