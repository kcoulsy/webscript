use crate::parser;
use crate::parser::Value;
use crate::render::{self, Scope};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Route {
    pub path: String,
    pub file: PathBuf,
}

pub fn create_project(root: &Path) -> Result<(), String> {
    if root.exists() {
        return Err(format!("{} already exists", root.display()));
    }

    fs::create_dir_all(root.join("app").join("pages")).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join("public")).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join("styles")).map_err(|error| error.to_string())?;

    fs::write(
        root.join("app").join("pages").join("index.web"),
        "@page \"/\"\n\n@let name: string = \"WebScript\"\n\n<main>\n  <h1>Hello {name}</h1>\n</main>\n",
    )
    .map_err(|error| error.to_string())?;

    fs::write(
        root.join("web.config"),
        "@deploy {\n  mode: \"runtime\"\n  adapter: \"node\"\n}\n",
    )
    .map_err(|error| error.to_string())?;

    println!("Created {}", root.display());
    Ok(())
}

pub fn discover_routes(root: &Path) -> Result<Vec<Route>, String> {
    let mut files = Vec::new();
    collect_web_files(&root.join("app"), &mut files)?;

    let mut routes = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).map_err(|error| error.to_string())?;
        let parsed =
            parser::parse(&source).map_err(|error| format!("{}: {error}", file.display()))?;
        routes.push(Route {
            path: parsed.route.raw,
            file,
        });
    }

    routes.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(routes)
}

pub fn check_project(root: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    collect_web_files(&root.join("app"), &mut files)?;

    let mut diagnostics = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).map_err(|error| error.to_string())?;
        match parser::parse(&source) {
            Ok(parsed) => {
                for diagnostic in render::validate(&parsed) {
                    diagnostics.push(format!("{}: {diagnostic}", file.display()));
                }
            }
            Err(error) => diagnostics.push(format!("{}: {error}", file.display())),
        }
    }

    Ok(diagnostics)
}

pub fn load_route(
    root: &Path,
    request_path: &str,
) -> Result<Option<(Route, parser::WebFile, Scope)>, String> {
    for route in discover_routes(root)? {
        let source = fs::read_to_string(&route.file).map_err(|error| error.to_string())?;
        let parsed =
            parser::parse(&source).map_err(|error| format!("{}: {error}", route.file.display()))?;

        if let Some(params) = match_route(&parsed, request_path) {
            return Ok(Some((route, parsed, params)));
        }
    }

    Ok(None)
}

fn match_route(file: &parser::WebFile, request_path: &str) -> Option<Scope> {
    let pattern_segments: Vec<&str> = file
        .route
        .raw
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let request_segments: Vec<&str> = request_path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    if pattern_segments.len() != request_segments.len() {
        return None;
    }

    let mut params = Scope::new();

    let mut param_index = 0;

    for (pattern, request) in pattern_segments.iter().zip(request_segments.iter()) {
        if pattern.starts_with('{') && pattern.ends_with('}') {
            let param = file.route.params.get(param_index)?;
            param_index += 1;

            let value = match param.type_name.as_str() {
                "string" => Value::String((*request).to_string()),
                "int" => Value::Int(request.parse().ok()?),
                _ => return None,
            };
            params.insert(param.name.clone(), value);
        } else if pattern != request {
            return None;
        }
    }

    Some(params)
}

fn collect_web_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_web_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "web") {
            files.push(path);
        }
    }

    Ok(())
}
