use crate::parser;
use crate::render;
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
            path: parsed.route,
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
) -> Result<Option<(Route, parser::WebFile)>, String> {
    for route in discover_routes(root)? {
        if route.path == request_path {
            let source = fs::read_to_string(&route.file).map_err(|error| error.to_string())?;
            let parsed = parser::parse(&source)
                .map_err(|error| format!("{}: {error}", route.file.display()))?;
            return Ok(Some((route, parsed)));
        }
    }

    Ok(None)
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
