use crate::diagnostic;
use crate::project;
use crate::render;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

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
    let mut runtime = project::ProjectRuntime::new(root.clone());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_connection(&root, &mut runtime, stream) {
                    eprintln!("request error: {error}");
                }
            }
            Err(error) => eprintln!("connection error: {error}"),
        }
    }

    Ok(())
}

fn handle_connection(
    root: &Path,
    runtime: &mut project::ProjectRuntime,
    mut stream: TcpStream,
) -> Result<(), String> {
    let mut buffer = [0; 4096];
    let size = stream
        .read(&mut buffer)
        .map_err(|error| error.to_string())?;
    let request = String::from_utf8_lossy(&buffer[..size]);
    let Some(first_line) = request.lines().next() else {
        return write_response(&mut stream, 400, "text/plain", "Bad Request");
    };

    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or("/");

    if method != "GET" {
        return write_response(&mut stream, 405, "text/plain", "Method Not Allowed");
    }

    let path = path.split('?').next().unwrap_or("/");
    if let Some(asset) = public_asset(root, path)? {
        return write_response(&mut stream, 200, content_type(path), &asset);
    }

    match runtime.load_route_with_source(path)? {
        Some((route_file, source, parsed, params)) => {
            match render::render_with_components(&parsed, &params, &runtime.load_components()?) {
                Ok(html) => write_response(&mut stream, 200, "text/html; charset=utf-8", &html),
                Err(error) => {
                    let file_diagnostic = diagnostic::FileDiagnostic {
                        file: route_file,
                        source,
                        diagnostic: error.clone(),
                    };
                    diagnostic::render_one(&file_diagnostic, std::io::stderr()).ok();
                    write_response(
                        &mut stream,
                        500,
                        "text/html; charset=utf-8",
                        &diagnostic::dev_error_page(
                            &file_diagnostic.file,
                            &file_diagnostic.source,
                            &error,
                        ),
                    )
                }
            }
        }
        None => write_response(&mut stream, 404, "text/plain", "Not Found"),
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
