use crate::diagnostic::{Diagnostic, Span};
use crate::parser::Value;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand_core::OsRng;

pub fn method_name(path: &[String]) -> Option<String> {
    match path.len() {
        2 if path[0] == "crypto"
            && matches!(path[1].as_str(), "hashPassword" | "verifyPassword") =>
        {
            Some(path[1].clone())
        }
        _ => None,
    }
}

pub fn call(
    method: &str,
    args: &[Value],
    line: usize,
    column: usize,
) -> Result<Value, Diagnostic> {
    match method {
        "hashPassword" => hash_password(args, line, column),
        "verifyPassword" => verify_password(args, line, column),
        other => Err(Diagnostic::error(
            Span::at(line, column),
            format!("unknown method `crypto.{other}`"),
            None,
        )),
    }
}

fn hash_password(args: &[Value], line: usize, column: usize) -> Result<Value, Diagnostic> {
    let password = expect_string(args, line, column, "crypto.hashPassword", 1)?;
    if password.is_empty() {
        return Err(Diagnostic::error(
            Span::at(line, column),
            "crypto.hashPassword requires a non-empty password",
            None,
        ));
    }

    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|error| {
            Diagnostic::error(
                Span::at(line, column),
                format!("crypto.hashPassword failed: {error}"),
                None,
            )
        })?;

    Ok(Value::String(hash.to_string()))
}

fn verify_password(args: &[Value], line: usize, column: usize) -> Result<Value, Diagnostic> {
    if args.len() != 2 {
        return Err(Diagnostic::error(
            Span::at(line, column),
            "crypto.verifyPassword expects 2 arguments",
            None,
        ));
    }
    let password = expect_string_at(args, 0, line, column, "crypto.verifyPassword")?;
    let hash = expect_string_at(args, 1, line, column, "crypto.verifyPassword")?;

    let parsed = match PasswordHash::new(&hash) {
        Ok(parsed) => parsed,
        Err(_) => return Ok(Value::Bool(false)),
    };

    let ok = Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok();

    Ok(Value::Bool(ok))
}

fn expect_string(
    args: &[Value],
    line: usize,
    column: usize,
    name: &str,
    expected: usize,
) -> Result<String, Diagnostic> {
    if args.len() != expected {
        return Err(Diagnostic::error(
            Span::at(line, column),
            format!("{name} expects {expected} argument{}", if expected == 1 { "" } else { "s" }),
            None,
        ));
    }
    expect_string_at(args, 0, line, column, name)
}

fn expect_string_at(
    args: &[Value],
    index: usize,
    line: usize,
    column: usize,
    name: &str,
) -> Result<String, Diagnostic> {
    match &args[index] {
        Value::String(value) => Ok(value.clone()),
        other => Err(Diagnostic::error(
            Span::at(line, column),
            format!("{name} expects string, found `{}`", other.type_name()),
            None,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_password_returns_argon2id_phc_string() {
        let hash = hash_password(
            &[Value::String("secret".to_string())],
            1,
            1,
        )
        .expect("hash");
        let Value::String(text) = hash else {
            panic!("expected string");
        };
        assert!(text.starts_with("$argon2id$"));
    }

    #[test]
    fn hash_password_uses_unique_salts() {
        let first = match hash_password(&[Value::String("secret".to_string())], 1, 1).expect("hash")
        {
            Value::String(text) => text,
            _ => panic!("expected string"),
        };
        let second = match hash_password(&[Value::String("secret".to_string())], 1, 1).expect("hash")
        {
            Value::String(text) => text,
            _ => panic!("expected string"),
        };
        assert_ne!(first, second);
    }

    #[test]
    fn verify_password_accepts_matching_password() {
        let hash = match hash_password(&[Value::String("secret".to_string())], 1, 1).expect("hash")
        {
            Value::String(text) => text,
            _ => panic!("expected string"),
        };
        let ok = verify_password(
            &[
                Value::String("secret".to_string()),
                Value::String(hash),
            ],
            1,
            1,
        )
        .expect("verify");
        assert_eq!(ok, Value::Bool(true));
    }

    #[test]
    fn verify_password_rejects_wrong_password() {
        let hash = match hash_password(&[Value::String("secret".to_string())], 1, 1).expect("hash")
        {
            Value::String(text) => text,
            _ => panic!("expected string"),
        };
        let ok = verify_password(
            &[
                Value::String("wrong".to_string()),
                Value::String(hash),
            ],
            1,
            1,
        )
        .expect("verify");
        assert_eq!(ok, Value::Bool(false));
    }

    #[test]
    fn verify_password_rejects_invalid_hash() {
        let ok = verify_password(
            &[
                Value::String("secret".to_string()),
                Value::String("not-a-hash".to_string()),
            ],
            1,
            1,
        )
        .expect("verify");
        assert_eq!(ok, Value::Bool(false));
    }

    #[test]
    fn hash_password_rejects_empty_password() {
        let error = hash_password(&[Value::String(String::new())], 1, 1).expect_err("hash");
        assert!(error.message.contains("non-empty password"));
    }
}
