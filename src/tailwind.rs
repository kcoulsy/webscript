use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use tailwind_rs_core::CssGenerator;

pub const STYLESHEET_PATH: &str = "/.web/tailwind.css";
pub const STYLESHEET_LINK: &str = r#"<link rel="stylesheet" href="/.web/tailwind.css">"#;

#[derive(Debug, Default)]
pub struct TailwindCache {
    cached: Option<CachedStylesheet>,
}

#[derive(Debug, Clone)]
struct CachedStylesheet {
    source_hash: u64,
    css: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityReport {
    pub supported: Vec<String>,
    pub unsupported: Vec<String>,
}

impl TailwindCache {
    pub fn new() -> Self {
        Self { cached: None }
    }

    pub fn stylesheet(&mut self, root: &Path) -> Result<String, String> {
        let sources = ProjectSources::load(root)?;
        let source_hash = sources.hash();

        if let Some(cached) = &self.cached {
            if cached.source_hash == source_hash {
                return Ok(cached.css.clone());
            }
        }

        let css = generate_css_from_sources(&sources)?;
        self.cached = Some(CachedStylesheet {
            source_hash,
            css: css.clone(),
        });
        Ok(css)
    }
}

pub fn enabled(root: &Path) -> bool {
    let Ok(config) = fs::read_to_string(root.join("web.config")) else {
        return false;
    };
    tailwind_enabled_in_config(&config)
}

pub fn generate_project_css(root: &Path) -> Result<String, String> {
    let sources = ProjectSources::load(root)?;
    generate_css_from_sources(&sources)
}

pub fn compatibility_report() -> CompatibilityReport {
    let mut supported = Vec::new();
    let mut unsupported = Vec::new();

    for class in COMPATIBILITY_CLASSES {
        let mut generator = CssGenerator::new();
        match generator.add_class(class) {
            Ok(()) if css_contains_selector(&generator.generate_css(), class) => {
                supported.push((*class).to_string());
            }
            _ => unsupported.push((*class).to_string()),
        }
    }

    CompatibilityReport {
        supported,
        unsupported,
    }
}

fn generate_css_from_sources(sources: &ProjectSources) -> Result<String, String> {
    let classes = extract_classes_from_sources(&sources.files);
    generate_css_for_classes(classes)
}

fn generate_css_for_classes(classes: BTreeSet<String>) -> Result<String, String> {
    let mut generator = CssGenerator::new();

    for class in classes {
        match generator.add_class(&class) {
            Ok(()) => {}
            Err(error) if is_unknown_class_error(&error.to_string()) => {}
            Err(error) => return Err(format!("tailwind generation failed for `{class}`: {error}")),
        }
    }

    Ok(generator.generate_css())
}

fn is_unknown_class_error(message: &str) -> bool {
    message.contains("Unknown class:")
}

fn extract_classes_from_sources(files: &[(PathBuf, String)]) -> BTreeSet<String> {
    let mut classes = BTreeSet::new();

    for (_path, source) in files {
        for literal in string_literals(source) {
            for token in literal.split_whitespace() {
                let token = clean_candidate(token);
                if is_class_candidate(token) {
                    classes.insert(token.to_string());
                }
            }
        }
    }

    classes
}

fn string_literals(source: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;

    for char in source.chars() {
        if in_string {
            if escaped {
                current.push(char);
                escaped = false;
            } else if char == '\\' {
                escaped = true;
            } else if char == '"' {
                literals.push(current.clone());
                current.clear();
                in_string = false;
            } else {
                current.push(char);
            }
        } else if char == '"' {
            in_string = true;
        }
    }

    literals
}

fn clean_candidate(token: &str) -> &str {
    token.trim_matches(|char: char| {
        matches!(
            char,
            '\'' | '"' | '`' | '{' | '}' | '(' | ')' | '<' | '>' | ',' | ';'
        )
    })
}

fn is_class_candidate(token: &str) -> bool {
    if token.is_empty() || token.len() > 120 || token.contains('{') || token.contains('}') {
        return false;
    }

    token.chars().all(|char| {
        char.is_ascii_alphanumeric()
            || matches!(
                char,
                '-' | '_' | ':' | '/' | '.' | '[' | ']' | '#' | '%' | '@' | '!' | '(' | ')'
            )
    })
}

fn css_contains_selector(css: &str, class: &str) -> bool {
    let base = class.rsplit(':').next().unwrap_or(class);
    css.contains(&format!(".{base}"))
}

fn tailwind_enabled_in_config(config: &str) -> bool {
    let mut in_tailwind = false;

    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("@tailwind") {
            in_tailwind = true;
            continue;
        }
        if in_tailwind {
            if trimmed == "}" {
                break;
            }
            if let Some(value) = trimmed.strip_prefix("enabled:") {
                return value.trim() == "true";
            }
        }
    }

    false
}

#[derive(Debug)]
struct ProjectSources {
    files: Vec<(PathBuf, String)>,
}

impl ProjectSources {
    fn load(root: &Path) -> Result<Self, String> {
        let mut paths = Vec::new();
        collect_web_files(&root.join("app"), &mut paths)?;
        paths.sort();

        let mut files = Vec::new();
        for path in paths {
            let source = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            files.push((path, source));
        }

        Ok(Self { files })
    }

    fn hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for (path, source) in &self.files {
            path.hash(&mut hasher);
            source.hash(&mut hasher);
        }
        hasher.finish()
    }
}

fn collect_web_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !directory.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
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

const COMPATIBILITY_CLASSES: &[&str] = &[
    "flex",
    "grid",
    "hidden",
    "block",
    "px-4",
    "py-2",
    "m-4",
    "gap-4",
    "text-sm",
    "text-white",
    "bg-blue-500",
    "rounded-lg",
    "border",
    "shadow",
    "hover:bg-blue-600",
    "md:grid-cols-2",
    "w-[137px]",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reads_tailwind_enabled_config() {
        assert!(tailwind_enabled_in_config(
            "@deploy {\n}\n\n@tailwind {\n  enabled: true\n}\n"
        ));
        assert!(!tailwind_enabled_in_config(
            "@tailwind {\n  enabled: false\n}\n"
        ));
    }

    #[test]
    fn extracts_classes_from_markup_and_string_literals() {
        let files = vec![(
            PathBuf::from("app/pages/index.web"),
            r#"@page "/"

@load {
  let state = "hidden md:grid-cols-2";
}

<main class="flex px-4">
  <div class={"py-2 " + variant}>Hello</div>
</main>
"#
            .to_string(),
        )];

        let classes = extract_classes_from_sources(&files);
        assert!(classes.contains("flex"));
        assert!(classes.contains("px-4"));
        assert!(classes.contains("py-2"));
        assert!(classes.contains("hidden"));
        assert!(classes.contains("md:grid-cols-2"));
    }

    #[test]
    fn cache_invalidates_when_sources_change() {
        let root = temp_project("tailwind-cache");
        fs::create_dir_all(root.join("app/pages")).expect("app pages");
        fs::write(root.join("web.config"), "@tailwind {\n  enabled: true\n}\n").expect("config");
        fs::write(
            root.join("app/pages/index.web"),
            "@page \"/\"\n\n<main class=\"flex\"></main>\n",
        )
        .expect("page");

        let mut cache = TailwindCache::new();
        let first = cache.stylesheet(&root).expect("first css");
        assert!(first.contains("display: flex"));

        fs::write(
            root.join("app/pages/index.web"),
            "@page \"/\"\n\n<main class=\"grid\"></main>\n",
        )
        .expect("updated page");

        let second = cache.stylesheet(&root).expect("second css");
        assert!(second.contains("display: grid"));
        assert_ne!(first, second);
    }

    #[test]
    fn compatibility_gate_covers_required_utility_set() {
        let report = compatibility_report();
        assert!(
            report.unsupported.is_empty(),
            "unsupported Tailwind-like classes: {:?}",
            report.unsupported
        );
    }

    fn temp_project(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("webscript-{label}-{unique}"));
        fs::create_dir_all(&root).expect("temp root");
        root
    }
}
