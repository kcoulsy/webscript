use crate::parser::{expressions, WebFile};

pub fn render(file: &WebFile) -> Result<String, String> {
    let mut html = file.template.clone();

    for expr in expressions(&file.template) {
        let value = file
            .lets
            .get(&expr)
            .ok_or_else(|| format!("unknown expression `{expr}`"))?;
        html = html.replace(&format!("{{{expr}}}"), &escape_html(&value.render()));
    }

    Ok(html)
}

pub fn validate(file: &WebFile) -> Vec<String> {
    expressions(&file.template)
        .into_iter()
        .filter(|expr| !file.lets.contains_key(expr))
        .map(|expr| format!("unknown expression `{expr}` in route {}", file.route))
        .collect()
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
    use super::render;
    use crate::parser::parse;

    #[test]
    fn renders_known_expressions_with_escaping() {
        let file = parse("@page \"/\"\n\n@let name: string = \"<Ada>\"\n\n<h1>{name}</h1>")
            .expect("valid page");

        assert_eq!(render(&file).expect("rendered"), "<h1>&lt;Ada&gt;</h1>");
    }

    #[test]
    fn rejects_unknown_expressions() {
        let file = parse("@page \"/\"\n\n<h1>{name}</h1>").expect("valid page");
        let error = render(&file).expect_err("unknown expression should fail");

        assert_eq!(error, "unknown expression `name`");
    }
}
