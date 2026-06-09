use crate::debugbar::RequestMetrics;
use crate::dev;
use crate::diagnostic::{self, FileDiagnostic};
use crate::project;
use crate::render;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

pub fn serve(root: PathBuf, host: String, port: u16) -> Result<(), String> {
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
    let runtime = Arc::new(Mutex::new(project::ProjectRuntime::new(root.clone())));
    let dev_server = dev::DevServer::start(root.clone());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let root = root.clone();
                let runtime = Arc::clone(&runtime);
                let dev_server = dev_server.clone();
                thread::spawn(move || {
                    if let Err(diagnostic) = handle_connection(&root, &runtime, &dev_server, stream)
                    {
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
    runtime: &Arc<Mutex<project::ProjectRuntime>>,
    dev_server: &dev::DevServer,
    mut stream: TcpStream,
) -> Result<(), FileDiagnostic> {
    let mut buffer = [0; 4096];
    let size = stream
        .read(&mut buffer)
        .map_err(|error| io_diagnostic(error.to_string()))?;
    let request = String::from_utf8_lossy(&buffer[..size]);
    let Some(first_line) = request.lines().next() else {
        write_response(&mut stream, 400, "text/plain", "Bad Request")
            .map_err(|error| io_diagnostic(error))?;
        return Ok(());
    };

    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or("/");

    if method != "GET" {
        write_response(&mut stream, 405, "text/plain", "Method Not Allowed")
            .map_err(|error| io_diagnostic(error))?;
        return Ok(());
    }

    let path = path.split('?').next().unwrap_or("/");
    if dev::DevServer::is_hot_reload_path(path) {
        return dev_server
            .handle_hot_reload(stream, &request)
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

    if let Some(asset) = public_asset(root, path).map_err(|error| io_diagnostic(error))? {
        write_response(&mut stream, 200, content_type(path), &asset)
            .map_err(|error| io_diagnostic(error))?;
        return Ok(());
    }

    let started = Instant::now();
    let mut metrics = RequestMetrics::new(path);

    let mut runtime = runtime
        .lock()
        .map_err(|_| io_diagnostic("project runtime lock poisoned".to_string()))?;

    let route_started = Instant::now();
    let (route_file, source, parsed, params) = match runtime.load_route_with_source(path) {
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
    let components = match runtime.load_components() {
        Ok(components) => components,
        Err(diagnostic) => {
            metrics.push("Components", components_started.elapsed(), None);
            metrics.set_total(started.elapsed());
            return respond_diagnostic(&mut stream, diagnostic, Some(metrics));
        }
    };
    metrics.push("Components", components_started.elapsed(), None);

    let render_started = Instant::now();
    match render::render_with_components(&parsed, &params, &components) {
        Ok(html) => {
            metrics.push("Render", render_started.elapsed(), None);
            metrics.set_total(started.elapsed());
            write_response(
                &mut stream,
                200,
                "text/html; charset=utf-8",
                &dev::DevServer::inject_dev_tools(&html, Some(&metrics)),
            )
            .map_err(|error| io_diagnostic(error))?;
            Ok(())
        }
        Err(error) => {
            metrics.push("Render", render_started.elapsed(), None);
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
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "OK",
    };

    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.as_bytes().len()
    );

    stream
        .write_all(response.as_bytes())
        .map_err(|error| error.to_string())
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
