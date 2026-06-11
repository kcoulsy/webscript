use crate::client::{self, render_island_script};
use crate::debugbar::{RequestMetrics, TaskTrace};
use crate::dev;
use crate::diagnostic::{self, FileDiagnostic};
use crate::project;
use crate::render;
use crate::runtime::WebRuntime;
use crate::tailwind;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::runtime::Handle;

type SessionStore = Arc<Mutex<std::collections::BTreeMap<String, render::Scope>>>;

pub async fn serve(root: PathBuf, host: String, port: u16) -> Result<(), String> {
    match project::check_project(&root) {
        Ok(diagnostics) if !diagnostics.is_empty() => {
            eprintln!("Type check errors found (server will still start):");
            diagnostic::render_all(&diagnostics);
            eprintln!();
        }
        Err(error) => return Err(error),
        _ => {}
    }

    let address = format!("{host}:{port}");
    let listener = TcpListener::bind(&address).map_err(|error| error.to_string())?;
    println!("WebScript dev server listening on http://{address}");
    let project_runtime = Arc::new(Mutex::new(project::ProjectRuntime::new(root.clone())));
    let tailwind_cache = Arc::new(Mutex::new(tailwind::TailwindCache::new()));
    let web_runtime = Arc::new(WebRuntime::with_database(root.clone())?);
    let sessions = Arc::new(Mutex::new(std::collections::BTreeMap::new()));
    let dev_server = dev::DevServer::start(root.clone());
    let runtime_handle = Handle::current();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let root = root.clone();
                let project_runtime = Arc::clone(&project_runtime);
                let tailwind_cache = Arc::clone(&tailwind_cache);
                let web_runtime = Arc::clone(&web_runtime);
                let sessions = Arc::clone(&sessions);
                let dev_server = dev_server.clone();
                let runtime_handle = runtime_handle.clone();
                thread::spawn(move || {
                    if let Err(diagnostic) = handle_connection(
                        &root,
                        &project_runtime,
                        &tailwind_cache,
                        &web_runtime,
                        &sessions,
                        &dev_server,
                        &runtime_handle,
                        stream,
                    ) {
                        diagnostic.report_stderr();
                    }
                });
            }
            Err(error) => eprintln!("connection error: {error}"),
        }
    }

    Ok(())
}

fn handle_connection(
    root: &Path,
    project_runtime: &Arc<Mutex<project::ProjectRuntime>>,
    tailwind_cache: &Arc<Mutex<tailwind::TailwindCache>>,
    web_runtime: &Arc<WebRuntime>,
    sessions: &SessionStore,
    dev_server: &dev::DevServer,
    runtime_handle: &Handle,
    mut stream: TcpStream,
) -> Result<(), FileDiagnostic> {
    let request = read_request(&mut stream).map_err(io_diagnostic)?;
    let Some(first_line) = request.raw.lines().next() else {
        write_response(&mut stream, 400, "text/plain", "Bad Request")
            .map_err(|error| io_diagnostic(error))?;
        return Ok(());
    };

    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let request_target = parts.next().unwrap_or("/");

    if method != "GET" && method != "POST" {
        write_response(&mut stream, 405, "text/plain", "Method Not Allowed")
            .map_err(|error| io_diagnostic(error))?;
        return Ok(());
    }

    let (path, query) = split_request_target(request_target);
    if dev::DevServer::is_hot_reload_path(path) {
        return dev_server
            .handle_hot_reload(stream, &request.raw)
            .map_err(io_diagnostic);
    }
    if dev::DevServer::is_dev_client_path(path) {
        write_response(
            &mut stream,
            200,
            "application/javascript; charset=utf-8",
            dev::DevServer::dev_client_script(),
        )
        .map_err(io_diagnostic)?;
        return Ok(());
    }
    if client::is_runtime_path(path) {
        write_response(
            &mut stream,
            200,
            "application/javascript; charset=utf-8",
            client::client_runtime_script(),
        )
        .map_err(io_diagnostic)?;
        return Ok(());
    }
    if path == tailwind::STYLESHEET_PATH {
        if !tailwind::enabled(root) {
            write_response(&mut stream, 404, "text/plain", "Not Found")
                .map_err(|error| io_diagnostic(error))?;
            return Ok(());
        }
        let css = tailwind_cache
            .lock()
            .map_err(|_| io_diagnostic("tailwind cache lock poisoned".to_string()))?
            .stylesheet(root)
            .map_err(io_diagnostic)?;
        write_response(&mut stream, 200, "text/css; charset=utf-8", &css).map_err(io_diagnostic)?;
        return Ok(());
    }

    if method == "GET" {
        if let Some(asset) = public_asset(root, path).map_err(|error| io_diagnostic(error))? {
            write_response(&mut stream, 200, content_type(path), &asset)
                .map_err(|error| io_diagnostic(error))?;
            return Ok(());
        }
    }

    let started = Instant::now();
    let mut metrics = RequestMetrics::new(path);

    let mut project_runtime = project_runtime
        .lock()
        .map_err(|_| io_diagnostic("project runtime lock poisoned".to_string()))?;

    let route_started = Instant::now();
    let (route_file, source, parsed, params) = match project_runtime.load_route_with_source(path) {
        Ok(Some(value)) => value,
        Ok(None) => {
            write_response(&mut stream, 404, "text/plain", "Not Found")
                .map_err(|error| io_diagnostic(error))?;
            return Ok(());
        }
        Err(diagnostic) => {
            metrics.push("Route", route_started.elapsed(), None);
            metrics.set_total(started.elapsed());
            return respond_diagnostic(&mut stream, diagnostic, Some(metrics));
        }
    };
    metrics.push("Route", route_started.elapsed(), None);
    metrics.route_file = Some(route_file.clone());

    let components_started = Instant::now();
    let components = match project_runtime.load_components() {
        Ok(components) => components,
        Err(diagnostic) => {
            metrics.push("Components", components_started.elapsed(), None);
            metrics.set_total(started.elapsed());
            return respond_diagnostic(&mut stream, diagnostic, Some(metrics));
        }
    };
    metrics.push("Components", components_started.elapsed(), None);

    let layouts_started = Instant::now();
    let layouts = match project_runtime.load_layouts() {
        Ok(layouts) => layouts,
        Err(diagnostic) => {
            metrics.push("Layouts", layouts_started.elapsed(), None);
            metrics.set_total(started.elapsed());
            return respond_diagnostic(&mut stream, diagnostic, Some(metrics));
        }
    };
    metrics.push("Layouts", layouts_started.elapsed(), None);
    let default_layout = project_runtime.default_layout();

    let (session_id, mut session, is_new_session) =
        load_session(sessions, request.headers.get("cookie").map(String::as_str))
            .map_err(io_diagnostic)?;

    if method == "POST" {
        let is_fetch_action = request
            .headers
            .get("x-webscript-action")
            .is_some_and(|value| value == "1");
        let input = parse_post_input(
            request.headers.get("content-type").map(String::as_str),
            &request.body,
        )
        .map_err(io_diagnostic)?;
        let action_name = input
            .get("_action")
            .map(|value| value.render())
            .or_else(|| query_value(query, "_action"))
            .ok_or_else(|| io_diagnostic("POST requests require `_action`".to_string()))?;
        let action_result = runtime_handle.block_on(render::execute_action(
            &parsed,
            &action_name,
            &params,
            &input,
            &mut session,
            web_runtime,
        ));
        match action_result {
            Ok(outcome) if is_fetch_action => {
                if !matches!(outcome, render::ActionOutcome::Fail(_)) {
                    save_session(sessions, &session_id, session).map_err(io_diagnostic)?;
                }
                write_action_json(&mut stream, path, &outcome, &session_id, is_new_session)
                    .map_err(io_diagnostic)?;
                return Ok(());
            }
            Ok(render::ActionOutcome::Redirect(target)) => {
                save_session(sessions, &session_id, session).map_err(io_diagnostic)?;
                write_redirect(
                    &mut stream,
                    &resolve_redirect(path, &target),
                    &session_id,
                    is_new_session,
                )
                .map_err(io_diagnostic)?;
                return Ok(());
            }
            Ok(render::ActionOutcome::Json(_)) => {
                save_session(sessions, &session_id, session).map_err(io_diagnostic)?;
                write_redirect(
                    &mut stream,
                    &resolve_redirect(path, "."),
                    &session_id,
                    is_new_session,
                )
                .map_err(io_diagnostic)?;
                return Ok(());
            }
            Ok(render::ActionOutcome::Fail(message)) => {
                write_response_with_headers(
                    &mut stream,
                    422,
                    "text/html; charset=utf-8",
                    &action_failure_html(&message),
                    &session_headers(&session_id, is_new_session),
                )
                .map_err(io_diagnostic)?;
                return Ok(());
            }
            Err(error) => {
                metrics.set_total(started.elapsed());
                return respond_diagnostic(
                    &mut stream,
                    FileDiagnostic::new(route_file, source, error),
                    Some(metrics),
                );
            }
        }
    }

    let render_started = Instant::now();
    let mut render_params = params.clone();
    render_params.insert("session".to_string(), crate::parser::Value::Object(session));
    let task_trace = Arc::new(Mutex::new(TaskTrace::new()));
    let request_runtime = web_runtime.for_request(Arc::clone(&task_trace));
    let render_result = runtime_handle.block_on(render::render_page_async(
        &parsed,
        &render_params,
        &components,
        &layouts,
        default_layout.as_deref(),
        &request_runtime,
    ));
    match render_result {
        Ok(output) => {
            metrics.push("Render", render_started.elapsed(), None);
            if let Ok(trace) = task_trace.lock() {
                metrics.tasks = trace.spans().to_vec();
                metrics.queries = trace.queries().to_vec();
            }
            metrics.set_total(started.elapsed());
            let style_fragment =
                crate::style::render_style_tags(&output.global_styles, &output.scoped_styles);
            let html = if tailwind::enabled(root) {
                crate::style::inject_head_fragment(&output.html, tailwind::STYLESHEET_LINK)
            } else {
                output.html.clone()
            };
            let html = crate::style::inject_styles(&html, &style_fragment);
            let client_scripts = output
                .islands
                .iter()
                .map(render_island_script)
                .collect::<String>();
            let html = client::inject_client_scripts(&html, &client_scripts);
            write_response_with_headers(
                &mut stream,
                200,
                "text/html; charset=utf-8",
                &dev::DevServer::inject_dev_tools(&html, Some(&metrics)),
                &session_headers(&session_id, is_new_session),
            )
            .map_err(|error| io_diagnostic(error))?;
            Ok(())
        }
        Err(error) => {
            metrics.push("Render", render_started.elapsed(), None);
            if let Ok(trace) = task_trace.lock() {
                metrics.tasks = trace.spans().to_vec();
                metrics.queries = trace.queries().to_vec();
            }
            metrics.set_total(started.elapsed());
            respond_diagnostic(
                &mut stream,
                FileDiagnostic::new(route_file, source, error),
                Some(metrics),
            )
        }
    }
}

fn respond_diagnostic(
    stream: &mut TcpStream,
    diagnostic: FileDiagnostic,
    metrics: Option<RequestMetrics>,
) -> Result<(), FileDiagnostic> {
    write_response(
        stream,
        500,
        "text/html; charset=utf-8",
        &dev::DevServer::inject_dev_tools(&diagnostic.dev_error_html(), metrics.as_ref()),
    )
    .map_err(|error| io_diagnostic(error))?;
    Err(diagnostic)
}

fn io_diagnostic(message: String) -> FileDiagnostic {
    FileDiagnostic::new(
        PathBuf::from("<server>"),
        String::new(),
        crate::diagnostic::Diagnostic::error(crate::diagnostic::Span::at(1, 1), message, None),
    )
}

struct HttpRequest {
    raw: String,
    headers: std::collections::BTreeMap<String, String>,
    body: String,
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];

    loop {
        let size = stream
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if size == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..size]);
        if let Some(header_end) = find_header_end(&bytes) {
            let headers_text = String::from_utf8_lossy(&bytes[..header_end]).to_string();
            let content_length = parse_content_length(&headers_text).unwrap_or(0);
            let expected = header_end + 4 + content_length;
            while bytes.len() < expected {
                let size = stream
                    .read(&mut buffer)
                    .map_err(|error| error.to_string())?;
                if size == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..size]);
            }
            break;
        }
        if bytes.len() > 1024 * 1024 {
            return Err("request is too large".to_string());
        }
    }

    let raw = String::from_utf8_lossy(&bytes).to_string();
    let (head, body) = raw
        .split_once("\r\n\r\n")
        .map(|(head, body)| (head.to_string(), body.to_string()))
        .unwrap_or_else(|| (raw.clone(), String::new()));
    let mut headers = std::collections::BTreeMap::new();
    for line in head.lines().skip(1) {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    Ok(HttpRequest { raw, headers, body })
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(headers: &str) -> Option<usize> {
    headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("content-length") {
            value.trim().parse().ok()
        } else {
            None
        }
    })
}

fn split_request_target(target: &str) -> (&str, &str) {
    match target.split_once('?') {
        Some((path, query)) => (path, query),
        None => (target, ""),
    }
}

fn query_value(query: &str, name: &str) -> Option<String> {
    form_value(query, name)
}

fn parse_post_input(content_type: Option<&str>, body: &str) -> Result<render::Scope, String> {
    if content_type.is_some_and(|value| value.starts_with("application/json")) {
        return json_scope(body);
    }
    Ok(form_scope(body))
}

fn json_scope(body: &str) -> Result<render::Scope, String> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|error| format!("invalid JSON body: {error}"))?;
    let serde_json::Value::Object(fields) = json else {
        return Err("action JSON body must be an object".to_string());
    };

    let mut scope = render::Scope::new();
    for (key, value) in fields {
        scope.insert(
            key,
            crate::schema::json_value_to_parser(value)
                .map_err(|error| format!("invalid JSON field: {error}"))?,
        );
    }
    Ok(scope)
}

fn write_action_json(
    stream: &mut TcpStream,
    current_path: &str,
    outcome: &render::ActionOutcome,
    session_id: &str,
    is_new_session: bool,
) -> Result<(), String> {
    let (status, body) = match outcome {
        render::ActionOutcome::Redirect(target) => (
            200,
            serde_json::json!({
                "redirect": resolve_redirect(current_path, target),
            })
            .to_string(),
        ),
        render::ActionOutcome::Fail(message) => {
            (422, serde_json::json!({ "error": message }).to_string())
        }
        render::ActionOutcome::Json(value) => {
            let data = crate::schema::parser_value_to_json(value)
                .map_err(|error| format!("failed to serialize action result: {error}"))?;
            (200, serde_json::json!({ "data": data }).to_string())
        }
    };

    write_response_with_headers(
        stream,
        status,
        "application/json; charset=utf-8",
        &body,
        &session_headers(session_id, is_new_session),
    )
}

fn form_scope(body: &str) -> render::Scope {
    let mut scope = render::Scope::new();
    for pair in body.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = url_decode(key);
        if key.is_empty() {
            continue;
        }
        scope.insert(key, crate::parser::Value::String(url_decode(value)));
    }
    scope
}

fn form_value(body: &str, name: &str) -> Option<String> {
    for pair in body.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if url_decode(key) == name {
            return Some(url_decode(value));
        }
    }
    None
}

fn url_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &value[index + 1..index + 3];
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    output.push(byte);
                    index += 3;
                } else {
                    output.push(bytes[index]);
                    index += 1;
                }
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8_lossy(&output).to_string()
}

fn load_session(
    sessions: &SessionStore,
    cookie_header: Option<&str>,
) -> Result<(String, render::Scope, bool), String> {
    let cookie_session_id = cookie_header.and_then(session_id_from_cookie);
    let mut sessions = sessions
        .lock()
        .map_err(|_| "session store lock poisoned".to_string())?;

    if let Some(session_id) = cookie_session_id {
        if let Some(session) = sessions.get(&session_id) {
            return Ok((session_id, session.clone(), false));
        }
    }

    let session_id = new_session_id();
    let mut session = render::Scope::new();
    session.insert("count".to_string(), crate::parser::Value::Int(0));
    session.insert(
        "name".to_string(),
        crate::parser::Value::String(String::new()),
    );
    sessions.insert(session_id.clone(), session.clone());
    Ok((session_id, session, true))
}

fn save_session(
    sessions: &SessionStore,
    session_id: &str,
    session: render::Scope,
) -> Result<(), String> {
    let mut sessions = sessions
        .lock()
        .map_err(|_| "session store lock poisoned".to_string())?;
    sessions.insert(session_id.to_string(), session);
    Ok(())
}

fn session_id_from_cookie(cookie_header: &str) -> Option<String> {
    cookie_header.split(';').find_map(|cookie| {
        let (name, value) = cookie.trim().split_once('=')?;
        if name == "webscript_session" && is_session_id(value) {
            Some(value.to_string())
        } else {
            None
        }
    })
}

fn is_session_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '-')
}

fn new_session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("sess_{:x}_{:x}", std::process::id(), nanos)
}

fn session_headers(session_id: &str, is_new_session: bool) -> Vec<(&'static str, String)> {
    if is_new_session {
        vec![("Set-Cookie", session_cookie(session_id))]
    } else {
        Vec::new()
    }
}

fn session_cookie(session_id: &str) -> String {
    format!("webscript_session={session_id}; Path=/; HttpOnly; SameSite=Lax")
}

fn resolve_redirect(current_path: &str, target: &str) -> String {
    if target == "." {
        current_path.to_string()
    } else {
        target.to_string()
    }
}

fn public_asset(root: &Path, request_path: &str) -> Result<Option<String>, String> {
    if request_path == "/" || request_path.contains("..") {
        return Ok(None);
    }

    let relative = request_path.trim_start_matches('/');
    let path = root.join("public").join(relative);
    if path.is_file() {
        fs::read_to_string(path)
            .map(Some)
            .map_err(|error| error.to_string())
    } else {
        Ok(None)
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
) -> Result<(), String> {
    write_response_with_headers(stream, status, content_type, body, &[])
}

fn write_redirect(
    stream: &mut TcpStream,
    location: &str,
    session_id: &str,
    is_new_session: bool,
) -> Result<(), String> {
    let mut headers = vec![("Location", location.to_string())];
    headers.extend(session_headers(session_id, is_new_session));
    write_response_with_headers(stream, 303, "text/plain; charset=utf-8", "", &headers)
}

fn write_response_with_headers(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
    headers: &[(&'static str, String)],
) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        303 => "See Other",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        422 => "Unprocessable Entity",
        500 => "Internal Server Error",
        _ => "OK",
    };

    let mut response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.as_bytes().len()
    );
    for (name, value) in headers {
        response.push_str(name);
        response.push_str(": ");
        response.push_str(value);
        response.push_str("\r\n");
    }
    response.push_str("\r\n");
    response.push_str(body);

    stream
        .write_all(response.as_bytes())
        .map_err(|error| error.to_string())
}

fn action_failure_html(message: &str) -> String {
    format!(
        "<!doctype html><html><head><title>Action Failed</title></head><body><h1>Action Failed</h1><p>{}</p></body></html>",
        escape_html(message)
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

fn content_type(path: &str) -> &'static str {
    if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else {
        "text/plain; charset=utf-8"
    }
}
