use crate::db;
use crate::diagnostic::{Diagnostic, FileDiagnostic, Span};
use crate::schema;
use crate::validate;
use crate::parser;
use crate::parser::Value;
use crate::render::{self, Scope};
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Route {
    pub path: String,
    pub file: PathBuf,
}

#[derive(Debug)]
pub struct ProjectRuntime {
    root: PathBuf,
    parsed_files: BTreeMap<PathBuf, CachedWebFile>,
}

#[derive(Debug, Clone)]
struct CachedWebFile {
    source_hash: u64,
    source: String,
    parsed: parser::WebFile,
}

impl ProjectRuntime {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            parsed_files: BTreeMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn load_route(
        &mut self,
        request_path: &str,
    ) -> Result<Option<(Route, parser::WebFile, Scope)>, FileDiagnostic> {
        Ok(self
            .load_route_with_source(request_path)?
            .map(|(file, _source, parsed, params)| {
                (
                    Route {
                        path: parsed
                            .route
                            .as_ref()
                            .map(|route| route.raw.clone())
                            .unwrap_or_default(),
                        file,
                    },
                    parsed,
                    params,
                )
            }))
    }

    pub fn load_route_with_source(
        &mut self,
        request_path: &str,
    ) -> Result<Option<(PathBuf, String, parser::WebFile, Scope)>, FileDiagnostic> {
        for route in self.discover_routes()? {
            let (parsed, source) = self.parse_file_with_source(&route.file)?;

            if parsed.route.is_some() {
                if let Some(params) = match_route(&parsed, request_path) {
                    return Ok(Some((route.file, source, parsed, params)));
                }
            }
        }

        Ok(None)
    }

    pub fn load_components(&mut self) -> Result<render::ComponentRegistry, FileDiagnostic> {
        let mut files = Vec::new();
        collect_web_files(&self.root.join("app"), &mut files).map_err(read_dir_diagnostic)?;

        let mut components = render::ComponentRegistry::new();
        for file in files {
            let (parsed, source) = self.parse_file_with_source(&file)?;
            if parsed.layout.is_some() {
                continue;
            }
            if let Some(component) = &parsed.component {
                if components
                    .insert(component.name.clone(), parsed.clone())
                    .is_some()
                {
                    return Err(duplicate_component_diagnostic(
                        &file,
                        &source,
                        &component.name,
                    ));
                }
            }
        }

        Ok(components)
    }

    pub fn load_layouts(&mut self) -> Result<render::LayoutRegistry, FileDiagnostic> {
        let mut files = Vec::new();
        collect_web_files(&self.root.join("app"), &mut files).map_err(read_dir_diagnostic)?;

        let mut layouts = render::LayoutRegistry::new();
        for file in files {
            let (parsed, source) = self.parse_file_with_source(&file)?;
            if let Some(layout) = &parsed.layout {
                if layouts
                    .insert(layout.name.clone(), parsed.clone())
                    .is_some()
                {
                    return Err(duplicate_layout_diagnostic(&file, &source, &layout.name));
                }
            }
        }

        Ok(layouts)
    }

    pub fn default_layout(&self) -> Option<String> {
        load_default_layout(&self.root)
    }

    fn discover_routes(&mut self) -> Result<Vec<Route>, FileDiagnostic> {
        let mut files = Vec::new();
        collect_web_files(&self.root.join("app"), &mut files).map_err(read_dir_diagnostic)?;

        let mut routes = Vec::new();
        for file in files {
            let parsed = self.parse_file(&file)?;
            if let Some(route) = parsed.route {
                routes.push(Route {
                    path: route.raw,
                    file,
                });
            }
        }

        routes.sort_by(compare_routes);
        Ok(routes)
    }

    fn parse_file(&mut self, file: &Path) -> Result<parser::WebFile, FileDiagnostic> {
        Ok(self.parse_file_with_source(file)?.0)
    }

    fn parse_file_with_source(
        &mut self,
        file: &Path,
    ) -> Result<(parser::WebFile, String), FileDiagnostic> {
        let source = fs::read_to_string(file).map_err(|error| read_file_diagnostic(file, error))?;
        let source_hash = hash_source(&source);

        if let Some(cached) = self.parsed_files.get(file) {
            if cached.source_hash == source_hash {
                return Ok((cached.parsed.clone(), cached.source.clone()));
            }
        }

        let parsed = parser::parse(&source).map_err(|diagnostic| {
            FileDiagnostic::new(file.to_path_buf(), source.clone(), diagnostic)
        })?;
        self.parsed_files.insert(
            file.to_path_buf(),
            CachedWebFile {
                source_hash,
                source: source.clone(),
                parsed: parsed.clone(),
            },
        );

        Ok((parsed, source))
    }
}

fn read_file_diagnostic(file: &Path, error: std::io::Error) -> FileDiagnostic {
    FileDiagnostic::new(
        file.to_path_buf(),
        String::new(),
        Diagnostic::error(
            Span::at(1, 1),
            format!("could not read file: {error}"),
            None,
        ),
    )
}

fn read_dir_diagnostic(error: String) -> FileDiagnostic {
    FileDiagnostic::new(
        PathBuf::from("app"),
        String::new(),
        Diagnostic::error(Span::at(1, 1), error, None),
    )
}

fn load_default_layout(root: &Path) -> Option<String> {
    let source = fs::read_to_string(root.join("web.config")).ok()?;
    let mut in_defaults = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("@defaults") {
            in_defaults = true;
            continue;
        }
        if in_defaults {
            if trimmed == "}" {
                break;
            }
            if let Some(name) = trimmed.strip_prefix("layout:") {
                let name = name.trim().trim_matches('"');
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

fn duplicate_layout_diagnostic(file: &Path, source: &str, name: &str) -> FileDiagnostic {
    let (line, column) = source
        .lines()
        .enumerate()
        .find_map(|(index, line)| {
            line.trim()
                .strip_prefix("@layout")
                .map(|_| (index + 1, line.find('@').unwrap_or(0) + 1))
        })
        .unwrap_or((1, 1));

    FileDiagnostic::new(
        file.to_path_buf(),
        source.to_string(),
        Diagnostic::error(
            Span::new(line, column, column + name.len()),
            format!("duplicate layout `{name}`"),
            None,
        ),
    )
}

fn duplicate_component_diagnostic(file: &Path, source: &str, name: &str) -> FileDiagnostic {
    let (line, column) = source
        .lines()
        .enumerate()
        .find_map(|(index, line)| {
            line.trim()
                .strip_prefix("@component")
                .map(|_| (index + 1, line.find('@').unwrap_or(0) + 1))
        })
        .unwrap_or((1, 1));

    FileDiagnostic::new(
        file.to_path_buf(),
        source.to_string(),
        Diagnostic::error(
            Span::identifier(line, column, name),
            format!("duplicate component `{name}`"),
            None,
        ),
    )
}

fn hash_source(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

pub fn create_project(root: &Path) -> Result<(), String> {
    if root.exists() {
        return Err(format!("{} already exists", root.display()));
    }

    fs::create_dir_all(root.join("app").join("pages")).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join("app").join("models")).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join("db").join("migrations")).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join("public")).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join("styles")).map_err(|error| error.to_string())?;

    fs::write(
        root.join("app").join("pages").join("index.web"),
        "@page \"/\"\n\n@let name: string = \"WebScript\"\n\n<main>\n  <h1>Hello {name}</h1>\n</main>\n",
    )
    .map_err(|error| error.to_string())?;

    fs::write(
        root.join("web.config"),
        "@deploy {\n  mode: \"runtime\"\n  adapter: \"node\"\n}\n\n@defaults {\n  layout: AppLayout\n}\n",
    )
    .map_err(|error| error.to_string())?;

    println!("Created {}", root.display());
    Ok(())
}

pub fn discover_routes(root: &Path) -> Result<Vec<Route>, FileDiagnostic> {
    ProjectRuntime::new(root.to_path_buf()).discover_routes()
}

pub fn check_project(root: &Path) -> Result<Vec<FileDiagnostic>, String> {
    let mut files = Vec::new();
    collect_web_files(&root.join("app"), &mut files)?;

    let mut diagnostics = Vec::new();
    let mut parsed_by_file = BTreeMap::new();

    for file in files {
        let source = fs::read_to_string(&file).map_err(|error| error.to_string())?;
        match parser::parse(&source) {
            Ok(parsed) => {
                parsed_by_file.insert(file, (parsed, source));
            }
            Err(diagnostic) => {
                diagnostics.push(FileDiagnostic::new(file, source, diagnostic));
            }
        }
    }

    let mut components = render::ComponentRegistry::new();
    let mut layouts = render::LayoutRegistry::new();
    for (file, (parsed, source)) in &parsed_by_file {
        if let Some(layout) = &parsed.layout {
            if layouts
                .insert(layout.name.clone(), parsed.clone())
                .is_some()
            {
                diagnostics.push(duplicate_layout_diagnostic(file, source, &layout.name));
            }
            continue;
        }
        if let Some(component) = &parsed.component {
            if components
                .insert(component.name.clone(), parsed.clone())
                .is_some()
            {
                diagnostics.push(duplicate_component_diagnostic(
                    file,
                    source,
                    &component.name,
                ));
            }
        }
    }

    let default_layout = load_default_layout(root);

    let schema_names: std::collections::BTreeSet<String> = match schema::discover_schemas(root) {
        Ok(schemas) => schemas.into_iter().map(|schema| schema.name).collect(),
        Err(schema::SchemaLoadError::Diagnostic(diagnostic)) => {
            diagnostics.push(diagnostic);
            std::collections::BTreeSet::new()
        }
        Err(schema::SchemaLoadError::Io(error)) => return Err(error),
    };

    let models = match db::discover_models(root) {
        Ok(models) => models
            .into_iter()
            .map(|model| (model.name.clone(), model))
            .collect(),
        Err(db::ModelLoadError::Diagnostic(file_diagnostic)) => {
            diagnostics.push(file_diagnostic);
            BTreeMap::new()
        }
        Err(db::ModelLoadError::Io(error)) => return Err(error),
    };

    for (file, (parsed, source)) in &parsed_by_file {
        for diagnostic in render::validate_with_components(
            parsed,
            &components,
            &layouts,
            default_layout.as_deref(),
            &models,
        ) {
            diagnostics.push(FileDiagnostic::new(
                file.clone(),
                source.clone(),
                diagnostic,
            ));
        }
        for diagnostic in validate::validate_schema_calls(parsed, &schema_names) {
            diagnostics.push(FileDiagnostic::new(
                file.clone(),
                source.clone(),
                diagnostic,
            ));
        }
    }

    diagnostics.extend(db::check_models(root)?);
    diagnostics.extend(schema::check_schemas(root)?);

    Ok(diagnostics)
}

fn match_route(file: &parser::WebFile, request_path: &str) -> Option<Scope> {
    let route = file.route.as_ref()?;
    let pattern_segments: Vec<&str> = file
        .route
        .as_ref()?
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
            let param = route.params.get(param_index)?;
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

fn compare_routes(left: &Route, right: &Route) -> std::cmp::Ordering {
    let left_score = route_score(&left.path);
    let right_score = route_score(&right.path);

    left_score
        .dynamic_segments
        .cmp(&right_score.dynamic_segments)
        .then_with(|| right_score.static_segments.cmp(&left_score.static_segments))
        .then_with(|| right_score.total_segments.cmp(&left_score.total_segments))
        .then_with(|| left.path.cmp(&right.path))
}

#[derive(Debug, PartialEq, Eq)]
struct RouteScore {
    dynamic_segments: usize,
    static_segments: usize,
    total_segments: usize,
}

fn route_score(path: &str) -> RouteScore {
    let segments: Vec<&str> = path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let dynamic_segments = segments
        .iter()
        .filter(|segment| segment.starts_with('{') && segment.ends_with('}'))
        .count();

    RouteScore {
        dynamic_segments,
        static_segments: segments.len() - dynamic_segments,
        total_segments: segments.len(),
    }
}

fn collect_web_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|name| name == "models" || name == "schemas")
            {
                continue;
            }
            collect_web_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "web") {
            files.push(path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{compare_routes, match_route, ProjectRuntime, Route};
    use crate::parser::{parse, Value};
    use std::cmp::Ordering;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn exact_routes_sort_before_dynamic_routes() {
        let exact = Route {
            path: "/posts/new".to_string(),
            file: PathBuf::from("new.web"),
        };
        let dynamic = Route {
            path: "/posts/{slug:string}".to_string(),
            file: PathBuf::from("slug.web"),
        };

        assert_eq!(compare_routes(&exact, &dynamic), Ordering::Less);
        assert_eq!(compare_routes(&dynamic, &exact), Ordering::Greater);
    }

    #[test]
    fn dynamic_route_extracts_params() {
        let file = parse("@page \"/posts/{slug:string}\"\n\n<h1>{slug}</h1>").expect("valid route");
        let params = match_route(&file, "/posts/hello").expect("matched");

        assert!(matches!(params.get("slug"), Some(Value::String(value)) if value == "hello"));
    }

    #[test]
    fn runtime_reloads_route_after_source_changes() {
        let root = temp_project_root("webscript-reload");
        fs::create_dir_all(root.join("app").join("pages")).expect("create pages dir");
        let page = root.join("app").join("pages").join("index.web");

        fs::write(&page, "@page \"/\"\n\n<h1>First</h1>").expect("write first page");
        let mut runtime = ProjectRuntime::new(root.clone());
        let (_, parsed, _) = runtime
            .load_route("/")
            .expect("route load")
            .expect("route match");
        assert!(matches!(
            parsed.template.first(),
            Some(crate::parser::TemplateNode::Text(value)) if value == "<h1>First</h1>"
        ));

        fs::write(&page, "@page \"/\"\n\n<h1>Second</h1>").expect("write second page");
        let (_, parsed, _) = runtime
            .load_route("/")
            .expect("route reload")
            .expect("route match");
        assert!(matches!(
            parsed.template.first(),
            Some(crate::parser::TemplateNode::Text(value)) if value == "<h1>Second</h1>"
        ));

        fs::remove_dir_all(root).expect("cleanup temp project");
    }

    fn temp_project_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{}-{nanos}", std::process::id()))
    }
}
