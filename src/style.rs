use std::collections::BTreeMap;

pub fn scope_css(css: &str, scope_attr: &str) -> String {
    let prefix = format!(r#"[data-ws-style="{scope_attr}"] "#);
    let mut output = String::new();
    let mut index = 0usize;
    let len = css.len();

    while index < len {
        index = skip_whitespace(css, index);
        if index >= len {
            break;
        }

        let selector_start = index;
        while index < len && !starts_with_char(css, index, '{') {
            index += 1;
        }
        if index >= len {
            output.push_str(css[selector_start..].trim());
            break;
        }

        let selector = css[selector_start..index].trim();
        index += 1; // opening brace

        let (block, next_index) = read_brace_block(css, index);
        index = next_index;

        if selector.starts_with('@') {
            output.push_str(selector);
            output.push('{');
            output.push_str(&scope_css(block, scope_attr));
            output.push('}');
        } else if !selector.is_empty() {
            output.push_str(&prefix);
            output.push_str(selector);
            output.push('{');
            output.push_str(block);
            output.push('}');
        } else {
            output.push('{');
            output.push_str(block);
            output.push('}');
        }
    }

    output.trim().to_string()
}

fn skip_whitespace(css: &str, mut index: usize) -> usize {
    while index < css.len() {
        let char = css[index..].chars().next().unwrap();
        if !char.is_whitespace() {
            break;
        }
        index += char.len_utf8();
    }
    index
}

fn starts_with_char(css: &str, index: usize, target: char) -> bool {
    css[index..].starts_with(target)
}

fn read_brace_block(css: &str, start: usize) -> (&str, usize) {
    let mut depth = 1usize;
    let mut index = start;

    while index < css.len() && depth > 0 {
        let char = css[index..].chars().next().unwrap();
        let char_len = char.len_utf8();
        if char == '{' {
            depth += 1;
        } else if char == '}' {
            depth -= 1;
            if depth == 0 {
                return (&css[start..index], index + char_len);
            }
        }
        index += char_len;
    }

    (&css[start..], index)
}

fn escape_style_content(css: &str) -> String {
    css.replace("</style>", "<\\/style>")
}

pub fn render_style_tags(global: &[String], scoped: &BTreeMap<String, String>) -> String {
    let mut fragment = String::new();

    for css in global {
        if css.trim().is_empty() {
            continue;
        }
        fragment.push_str("<style data-ws-global>");
        fragment.push_str(&escape_style_content(css));
        fragment.push_str("</style>");
    }

    for (scope_id, css) in scoped {
        if css.trim().is_empty() {
            continue;
        }
        fragment.push_str(&format!(r#"<style data-ws-scoped="{scope_id}">"#));
        fragment.push_str(&escape_style_content(css));
        fragment.push_str("</style>");
    }

    fragment
}

pub fn inject_styles(html: &str, style_fragment: &str) -> String {
    if style_fragment.is_empty() {
        return html.to_string();
    }

    if let Some(index) = html.rfind("</body>") {
        let mut injected = String::with_capacity(html.len() + style_fragment.len());
        injected.push_str(&html[..index]);
        injected.push_str(style_fragment);
        injected.push_str(&html[index..]);
        injected
    } else {
        let mut injected = String::with_capacity(html.len() + style_fragment.len());
        injected.push_str(html);
        injected.push_str(style_fragment);
        injected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_simple_selector() {
        let scoped = scope_css(".card { color: red; }", "Counter");
        assert!(scoped.contains(r#"[data-ws-style="Counter"] .card"#));
        assert!(scoped.contains("color: red"));
    }

    #[test]
    fn scopes_rules_inside_media_query() {
        let scoped = scope_css(
            "@media (min-width: 600px) { .card { color: blue; } }",
            "Card",
        );
        assert!(scoped.contains("@media (min-width: 600px)"));
        assert!(scoped.contains(r#"[data-ws-style="Card"] .card"#));
    }

    #[test]
    fn renders_global_and_scoped_tags() {
        let mut scoped = BTreeMap::new();
        scoped.insert("Counter".to_string(), ".x {}".to_string());
        let tags = render_style_tags(&["body { margin: 0; }".to_string()], &scoped);
        assert!(tags.contains("data-ws-global"));
        assert!(tags.contains("data-ws-scoped=\"Counter\""));
    }

    #[test]
    fn injects_styles_before_body_close() {
        let html = "<html><body><main></main></body></html>";
        let injected = inject_styles(html, "<style>.a{}</style>");
        assert!(injected.contains("<style>.a{}</style></body>"));
    }
}
