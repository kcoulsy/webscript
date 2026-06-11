use crate::parser::Value;

pub fn value_matches_type(value: &Value, expected: &str) -> bool {
    if let Some(literals) = parse_string_literal_union(expected) {
        return match value {
            Value::String(value) => literals.iter().any(|literal| literal == value),
            _ => false,
        };
    }

    type_names_match(&value.type_name(), expected)
}

pub fn type_names_match(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }

    if parse_string_literal_union(expected).is_some() {
        return false;
    }

    if is_object_type_name(expected) {
        return actual == "object" || actual.starts_with('{');
    }

    if let (Some(actual_element), Some(expected_element)) =
        (actual.strip_suffix("[]"), expected.strip_suffix("[]"))
    {
        return type_names_match(actual_element, expected_element);
    }

    actual.starts_with('{') && expected.starts_with('{') && actual == expected
}

pub fn parse_string_literal_union(type_name: &str) -> Option<Vec<String>> {
    let mut rest = type_name.trim();
    let mut literals = Vec::new();

    loop {
        let (literal, consumed) = parse_string_literal_prefix(rest)?;
        literals.push(literal);
        rest = rest[consumed..].trim_start();
        if rest.is_empty() {
            return Some(literals);
        }
        let after_pipe = rest.strip_prefix('|')?;
        rest = after_pipe.trim_start();
        if rest.is_empty() {
            return None;
        }
    }
}

pub fn is_string_literal_type(type_name: &str) -> bool {
    parse_string_literal_union(type_name).is_some()
}

pub fn is_object_type_name(type_name: &str) -> bool {
    type_name == "object"
        || type_name
            .chars()
            .next()
            .is_some_and(|char| char.is_ascii_uppercase())
}

fn parse_string_literal_prefix(source: &str) -> Option<(String, usize)> {
    let mut chars = source.char_indices();
    if chars.next()? != (0, '"') {
        return None;
    }

    let mut literal = String::new();
    let mut escaped = false;
    for (index, char) in chars {
        if escaped {
            literal.push(char);
            escaped = false;
            continue;
        }
        match char {
            '\\' => escaped = true,
            '"' => return Some((literal, index + char.len_utf8())),
            _ => literal.push(char),
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_string_literal_unions() {
        assert_eq!(
            parse_string_literal_union(r#""primary" | "secondary""#),
            Some(vec!["primary".to_string(), "secondary".to_string()])
        );
        assert_eq!(
            parse_string_literal_union(r#""icon""#),
            Some(vec!["icon".to_string()])
        );
        assert_eq!(parse_string_literal_union("string"), None);
        assert_eq!(parse_string_literal_union(r#""primary" |"#), None);
    }

    #[test]
    fn matches_values_against_literal_union() {
        assert!(value_matches_type(
            &Value::String("primary".to_string()),
            r#""primary" | "secondary""#
        ));
        assert!(!value_matches_type(
            &Value::String("ghost".to_string()),
            r#""primary" | "secondary""#
        ));
        assert!(!type_names_match("string", r#""primary" | "secondary""#));
    }
}
