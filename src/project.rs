use crate::diagnostic::FileDiagnostic;
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
    ) -> Result<Option<(Route, parser::WebFile, Scope)>, String> {
        Ok(self
            .load_route_with_source(request_path)?
            .map(|(file, _source, parsed, params)| {
                (
                    Route {
                        path: parsed.route.as_ref().map(|route| route.raw.clone()).unwrap_or_default(),
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
    ) -> Result<Option<(PathBuf, String, parser::WebFile, Scope)>, String> {
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

    pub fn load_components(&mut self) -> Result<render::ComponentRegistry, String> {
        let mut files = Vec::new();
        collect_web_files(&self.root.join("app"), &mut files)?;

        let mut components = render::ComponentRegistry::new();
        for file in files {
            let parsed = self.parse_file(&file)?;
            if let Some(component) = &parsed.component {
                if components
                    .insert(component.name.clone(), parsed.clone())
                    .is_some()
                {
                    return Err(format!("duplicate component `{}`", component.name));
                }
            }
        }

        Ok(components)
    }

    fn discover_routes(&mut self) -> Result<Vec<Route>, String> {
        let mut files = Vec::new();
        collect_web_files(&self.root.join("app"), &mut files)?;

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

    fn parse_file(&mut self, file: &Path) -> Result<parser::WebFile, String> {
        Ok(self.parse_file_with_source(file)?.0)
    }

    fn parse_file_with_source(&mut self, file: &Path) -> Result<(parser::WebFile, String), String> {
        let source = fs::read_to_string(file).map_err(|error| error.to_string())?;
        let source_hash = hash_source(&source);

        if let Some(cached) = self.parsed_files.get(file) {
            if cached.source_hash == source_hash {
                return Ok((cached.parsed.clone(), cached.source.clone()));
            }
        }

        let parsed = parser::parse(&source).map_err(|error| {
            format!(
                "{}:{}:{}: {}",
                file.display(),
                error.span.line,
                error.span.start_col,
                error.message
            )
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
    ProjectRuntime::new(root.to_path_buf()).discover_routes()
}

pub fn check_project(root: &Path) -> Result<Vec<FileDiagnostic>, String> {
    let mut files = Vec::new();
    collect_web_files(&root.join("app"), &mut files)?;
    let components = load_components(root)?;

    let mut diagnostics = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).map_err(|error| error.to_string())?;
        match parser::parse(&source) {
            Ok(parsed) => {
                for diagnostic in render::validate_with_components(&parsed, &components) {
                    diagnostics.push(FileDiagnostic {
                        file: file.clone(),
                        source: source.clone(),
                        diagnostic,
                    });
                }
            }
            Err(diagnostic) => diagnostics.push(FileDiagnostic {
                file: file.clone(),
                source,
                diagnostic,
            }),
        }
    }

    Ok(diagnostics)
}

pub fn load_components(root: &Path) -> Result<render::ComponentRegistry, String> {
    ProjectRuntime::new(root.to_path_buf()).load_components()
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
