use crate::diagnostic::{Diagnostic, Span};
use crate::expr;
use crate::parser::WebFile;
use crate::schema::is_schema_name;
use crate::stmt::Statement;
use std::collections::BTreeSet;

pub fn validate_schema_calls(file: &WebFile, schema_names: &BTreeSet<String>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if let Some(load) = &file.load {
        validate_statements(&load.statements, schema_names, &mut diagnostics);
    }
    for action in &file.actions {
        validate_statements(&action.statements, schema_names, &mut diagnostics);
    }
    diagnostics
}

fn validate_statements(
    statements: &[Statement],
    schema_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in statements {
        match statement {
            Statement::Let { value, line, column, .. }
            | Statement::Assign { value, line, column, .. }
            | Statement::Throw { value, line, column, .. } => {
                validate_expr(value, *line, *column, schema_names, diagnostics);
            }
            Statement::If {
                condition,
                statements,
                line,
                column,
            }
            | Statement::While {
                condition,
                statements,
                line,
                column,
            } => {
                validate_expr(condition, *line, *column, schema_names, diagnostics);
                validate_statements(statements, schema_names, diagnostics);
            }
            Statement::Try {
                statements,
                catch_body,
                ..
            } => {
                validate_statements(statements, schema_names, diagnostics);
                validate_statements(catch_body, schema_names, diagnostics);
            }
            Statement::FnDef { body, .. } => {
                validate_statements(body, schema_names, diagnostics);
            }
            Statement::Return { value: Some(value), line, column, .. } => {
                validate_expr(value, *line, *column, schema_names, diagnostics);
            }
            Statement::ExprStmt { expr, line, column } => {
                validate_expr(expr, *line, *column, schema_names, diagnostics);
            }
            _ => {}
        }
    }
}

fn validate_expr(
    expression: &expr::Expr,
    line: usize,
    column: usize,
    schema_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        expr::Expr::Await { expr } => {
            if let expr::Expr::Call { callee, args } = expr.as_ref() {
                validate_schema_call(callee, args, line, column, schema_names, diagnostics);
            }
            validate_expr(expr, line, column, schema_names, diagnostics);
        }
        expr::Expr::Call { callee, args } => {
            for arg in args {
                validate_expr(arg, line, column, schema_names, diagnostics);
            }
            validate_expr(callee, line, column, schema_names, diagnostics);
        }
        expr::Expr::Unary { expr, .. } => {
            validate_expr(expr, line, column, schema_names, diagnostics);
        }
        expr::Expr::Binary { left, right, .. } => {
            validate_expr(left, line, column, schema_names, diagnostics);
            validate_expr(right, line, column, schema_names, diagnostics);
        }
        _ => {}
    }
}

fn validate_schema_call(
    callee: &expr::Expr,
    args: &[expr::Expr],
    line: usize,
    column: usize,
    schema_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if is_fetch_call(callee) {
        if args.len() != 2 {
            diagnostics.push(Diagnostic::error(
                Span::at(line, column),
                "fetch expects 2 arguments (url, Schema)",
                None,
            ));
            return;
        }
        validate_schema_arg(&args[1], line, column, schema_names, "fetch", diagnostics);
        return;
    }

    if is_db_query_call(callee) {
        if args.is_empty() || args.len() > 3 {
            diagnostics.push(Diagnostic::error(
                Span::at(line, column),
                "db.query expects (sql, Schema) or (sql, params, Schema)",
                None,
            ));
            return;
        }
        validate_schema_arg(
            args.last().expect("checked length"),
            line,
            column,
            schema_names,
            "db.query",
            diagnostics,
        );
    }
}

fn validate_schema_arg(
    expression: &expr::Expr,
    line: usize,
    column: usize,
    schema_names: &BTreeSet<String>,
    callee_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(name) = schema_ref_name(expression) else {
        diagnostics.push(Diagnostic::error(
            Span::at(line, column),
            format!("{callee_name} requires a schema name as the last argument"),
            None,
        ));
        return;
    };
    if !schema_names.contains(name) {
        diagnostics.push(Diagnostic::error(
            Span::at(line, column),
            format!("unknown schema `{name}`"),
            None,
        ));
    }
}

fn schema_ref_name(expression: &expr::Expr) -> Option<&str> {
    match expression {
        expr::Expr::Path(path) if path.len() == 1 && is_schema_name(&path[0]) => Some(&path[0]),
        _ => None,
    }
}

fn is_fetch_call(callee: &expr::Expr) -> bool {
    matches!(callee, expr::Expr::Path(path) if path == &["fetch"])
}

fn is_db_query_call(callee: &expr::Expr) -> bool {
    matches!(callee, expr::Expr::Path(path) if path == &["db", "query"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use std::collections::BTreeSet;

    #[test]
    fn rejects_fetch_without_schema() {
        let file = parse(
            "@page \"/\"\n\n@load {\n  _: object = await fetch(\"https://example.com\")\n}\n\n<p>ok</p>",
        )
        .expect("parse");
        let diagnostics = validate_schema_calls(&file, &BTreeSet::new());
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("2 arguments")));
    }

    #[test]
    fn rejects_unknown_schema_name() {
        let file = parse(
            "@page \"/\"\n\n@load {\n  _: object = await fetch(\"https://example.com\", Missing)\n}\n\n<p>ok</p>",
        )
        .expect("parse");
        let mut names = BTreeSet::new();
        names.insert("Known".to_string());
        let diagnostics = validate_schema_calls(&file, &names);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unknown schema")));
    }
}
