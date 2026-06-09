use crate::parser::{TemplateNode, Value, WebFile};
use std::collections::BTreeMap;

pub type Scope = BTreeMap<String, Value>;

pub fn render(file: &WebFile, params: &Scope) -> Result<String, String> {
    let scope = scope_for(file, params);
    render_nodes(&file.template, &scope)
}

pub fn validate(file: &WebFile) -> Vec<String> {
    let scope = scope_for(file, &Scope::new());
    let mut diagnostics = Vec::new();
    validate_nodes(&file.template, &scope, &mut diagnostics);
    diagnostics
}

fn scope_for(file: &WebFile, params: &Scope) -> Scope {
    let mut scope = Scope::new();

    for param in &file.route.params {
        scope.insert(param.name.clone(), Value::String(String::new()));
    }
    for (name, value) in params {
        scope.insert(name.clone(), value.clone());
    }
    for (name, value) in &file.lets {
        scope.insert(name.clone(), value.clone());
    }

    scope
}

fn render_nodes(nodes: &[TemplateNode], scope: &Scope) -> Result<String, String> {
    let mut html = String::new();

    for node in nodes {
        match node {
            TemplateNode::Text(value) => html.push_str(value),
            TemplateNode::Expr(expr) => {
                let value = scope.get(&expr.name).ok_or_else(|| {
                    format!(
                        "line {}:{} unknown expression `{}`",
                        expr.line, expr.column, expr.name
                    )
                })?;
                html.push_str(&escape_html(&value.render()));
            }
            TemplateNode::If {
                condition,
                then_nodes,
                else_nodes,
            } => {
                let value = scope.get(&condition.name).ok_or_else(|| {
                    format!(
                        "line {}:{} unknown condition `{}`",
                        condition.line, condition.column, condition.name
                    )
                })?;
                let Some(condition_value) = value.as_bool() else {
                    return Err(format!(
                        "line {}:{} @if condition `{}` must be bool",
                        condition.line, condition.column, condition.name
                    ));
                };

                if condition_value {
                    html.push_str(&render_nodes(then_nodes, scope)?);
                } else {
                    html.push_str(&render_nodes(else_nodes, scope)?);
                }
            }
            TemplateNode::For {
                item_name,
                source,
                body,
            } => {
                let value = scope.get(&source.name).ok_or_else(|| {
                    format!(
                        "line {}:{} unknown loop source `{}`",
                        source.line, source.column, source.name
                    )
                })?;
                let Some(items) = value.as_array() else {
                    return Err(format!(
                        "line {}:{} @for source `{}` must be array",
                        source.line, source.column, source.name
                    ));
                };

                for item in items {
                    let mut loop_scope = scope.clone();
                    loop_scope.insert(item_name.clone(), item.clone());
                    html.push_str(&render_nodes(body, &loop_scope)?);
                }
            }
        }
    }

    Ok(html)
}

fn validate_nodes(nodes: &[TemplateNode], scope: &Scope, diagnostics: &mut Vec<String>) {
    for node in nodes {
        match node {
            TemplateNode::Text(_) => {}
            TemplateNode::Expr(expr) => {
                if !scope.contains_key(&expr.name) {
                    diagnostics.push(format!(
                        "line {}:{} unknown expression `{}`",
                        expr.line, expr.column, expr.name
                    ));
                }
            }
            TemplateNode::If {
                condition,
                then_nodes,
                else_nodes,
            } => {
                match scope.get(&condition.name) {
                    Some(value) if value.as_bool().is_some() => {}
                    Some(_) => diagnostics.push(format!(
                        "line {}:{} @if condition `{}` must be bool",
                        condition.line, condition.column, condition.name
                    )),
                    None => diagnostics.push(format!(
                        "line {}:{} unknown condition `{}`",
                        condition.line, condition.column, condition.name
                    )),
                }
                validate_nodes(then_nodes, scope, diagnostics);
                validate_nodes(else_nodes, scope, diagnostics);
            }
            TemplateNode::For {
                item_name,
                source,
                body,
            } => match scope.get(&source.name) {
                Some(value) if value.as_array().is_some() => {
                    let mut loop_scope = scope.clone();
                    if let Some(sample) = value.array_sample() {
                        loop_scope.insert(item_name.clone(), sample);
                    }
                    validate_nodes(body, &loop_scope, diagnostics);
                }
                Some(_) => diagnostics.push(format!(
                    "line {}:{} @for source `{}` must be array",
                    source.line, source.column, source.name
                )),
                None => diagnostics.push(format!(
                    "line {}:{} unknown loop source `{}`",
                    source.line, source.column, source.name
                )),
            },
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{render, Scope};
    use crate::parser::parse;

    #[test]
    fn renders_known_expressions_with_escaping() {
        let file = parse("@page \"/\"\n\n@let name: string = \"<Ada>\"\n\n<h1>{name}</h1>")
            .expect("valid page");

        assert_eq!(
            render(&file, &Scope::new()).expect("rendered"),
            "<h1>&lt;Ada&gt;</h1>"
        );
    }

    #[test]
    fn renders_if_else() {
        let file = parse(
            "@page \"/\"\n\n@let show: bool = false\n\n@if show {\n<p>yes</p>\n} @else {\n<p>no</p>\n}",
        )
        .expect("valid page");

        assert_eq!(render(&file, &Scope::new()).expect("rendered"), "<p>no</p>");
    }

    #[test]
    fn renders_for_blocks_with_scoped_items() {
        let file = parse(
            "@page \"/\"\n\n@let posts: string[] = [\"One\", \"<Two>\"]\n\n@for post in posts {\n<p>{post}</p>\n}\n<p>{post}</p>",
        )
        .expect("valid page");
        let error = render(&file, &Scope::new()).expect_err("post should be scoped to loop");

        assert_eq!(error, "line 8:5 unknown expression `post`");

        let file = parse(
            "@page \"/\"\n\n@let posts: string[] = [\"One\", \"<Two>\"]\n\n@for post in posts {\n<p>{post}</p>\n}",
        )
        .expect("valid page");

        assert_eq!(
            render(&file, &Scope::new()).expect("rendered"),
            "<p>One</p><p>&lt;Two&gt;</p>"
        );
    }

    #[test]
    fn validates_for_source_type() {
        let file = parse("@page \"/\"\n\n@let title: string = \"Nope\"\n\n@for post in title {\n<p>{post}</p>\n}")
            .expect("valid page");

        assert_eq!(
            super::validate(&file),
            vec!["line 5:14 @for source `title` must be array"]
        );
    }

    #[test]
    fn validates_unknown_loop_sources_and_scoping() {
        let file = parse("@page \"/\"\n\n@for post in posts {\n<p>{post}</p>\n}\n<p>{post}</p>")
            .expect("valid page");

        assert_eq!(
            super::validate(&file),
            vec![
                "line 3:14 unknown loop source `posts`",
                "line 6:5 unknown expression `post`",
            ]
        );
    }

    #[test]
    fn validates_if_requires_bool_inside_empty_bool_array_loop() {
        let file =
            parse("@page \"/\"\n\n@let flags: bool[] = []\n\n@for flag in flags {\n@if flag {\n<p>yes</p>\n}\n}")
                .expect("valid page");

        assert!(super::validate(&file).is_empty());
    }

    #[test]
    fn rejects_unknown_expressions_with_location() {
        let file = parse("@page \"/\"\n\n<h1>{name}</h1>").expect("valid page");
        let error = render(&file, &Scope::new()).expect_err("unknown expression should fail");

        assert_eq!(error, "line 3:6 unknown expression `name`");
    }
}
