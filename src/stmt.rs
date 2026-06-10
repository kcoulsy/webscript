use crate::diagnostic::{Diagnostic, Span};
use crate::expr;
use crate::parser::Value;
use std::collections::BTreeMap;

pub type Env = BTreeMap<String, Value>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockMode {
    AsyncCapable,
    SyncOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnParam {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignTarget {
    Name(String),
    SessionField(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Let {
        name: String,
        type_name: Option<String>,
        value: expr::Expr,
        line: usize,
        column: usize,
    },
    Assign {
        target: AssignTarget,
        value: expr::Expr,
        line: usize,
        column: usize,
    },
    If {
        condition: expr::Expr,
        statements: Vec<Statement>,
        line: usize,
        column: usize,
    },
    While {
        condition: expr::Expr,
        statements: Vec<Statement>,
        line: usize,
        column: usize,
    },
    Try {
        statements: Vec<Statement>,
        catch_name: String,
        catch_body: Vec<Statement>,
        line: usize,
        column: usize,
    },
    FnDef {
        name: String,
        params: Vec<FnParam>,
        return_type: Option<String>,
        body: Vec<Statement>,
        line: usize,
        column: usize,
    },
    Return {
        value: Option<expr::Expr>,
        line: usize,
        column: usize,
    },
    Throw {
        value: expr::Expr,
        line: usize,
        column: usize,
    },
    Fail {
        message: String,
        line: usize,
        column: usize,
    },
    Redirect {
        target: String,
        line: usize,
        column: usize,
    },
    ExprStmt {
        expr: expr::Expr,
        line: usize,
        column: usize,
    },
}

#[derive(Debug, Clone)]
pub struct WebError {
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum BlockOutcome {
    Return(Value),
    Fail(String),
    Redirect(String),
}

pub fn parse_server_block(
    lines: &[&str],
    cursor: &mut usize,
    block_line: usize,
    mode: BlockMode,
) -> Result<Vec<Statement>, Diagnostic> {
    let mut statements = Vec::new();

    while *cursor < lines.len() {
        let statement_line = *cursor + 1;
        let raw_statement = lines[*cursor];
        let trimmed = raw_statement.trim();

        if trimmed == "}" || trimmed.starts_with("} catch ") {
            return Ok(statements);
        }
        if trimmed.is_empty() {
            *cursor += 1;
            continue;
        }

        statements.push(parse_statement(
            lines,
            cursor,
            raw_statement,
            trimmed,
            statement_line,
            mode,
        )?);
    }

    Err(parse_diagnostic_line(block_line, "unclosed server block"))
}

fn parse_statement(
    lines: &[&str],
    cursor: &mut usize,
    raw_line: &str,
    trimmed: &str,
    line_number: usize,
    mode: BlockMode,
) -> Result<Statement, Diagnostic> {
    if trimmed.starts_with("fn ") {
        return parse_fn_def(lines, cursor, raw_line, trimmed, line_number, mode);
    }

    if trimmed.starts_with("if ") {
        return parse_if(lines, cursor, raw_line, trimmed, line_number, mode);
    }

    if trimmed.starts_with("while ") {
        return parse_while(lines, cursor, raw_line, trimmed, line_number, mode);
    }

    if trimmed == "try {" {
        return parse_try(lines, cursor, line_number, mode);
    }

    if trimmed.starts_with("return") {
        return parse_return(raw_line, trimmed, line_number, mode, cursor);
    }

    if trimmed.starts_with("throw(") || trimmed.starts_with("throw (") {
        return parse_throw(raw_line, trimmed, line_number, mode, cursor);
    }

    if let Some(rest) = trimmed.strip_prefix("session.") {
        if let Some((field, value_source)) = rest.split_once('=') {
            let field = field.trim();
            if is_identifier(field) {
                let value_source = value_source.trim();
                let column = raw_line
                    .find(value_source)
                    .map(|index| index + 1)
                    .unwrap_or(1);
                *cursor += 1;
                return Ok(Statement::Assign {
                    target: AssignTarget::SessionField(field.to_string()),
                    value: parse_expr(value_source, line_number, column, mode)?,
                    line: line_number,
                    column,
                });
            }
        }
    }

    if let Some((left, right)) = trimmed.split_once(":=") {
        let name = left.trim();
        if is_identifier(name) {
            let value_source = right.trim();
            let column = raw_line
                .find(value_source)
                .map(|index| index + 1)
                .unwrap_or(1);
            *cursor += 1;
            return Ok(Statement::Let {
                name: name.to_string(),
                type_name: None,
                value: parse_expr(value_source, line_number, column, mode)?,
                line: line_number,
                column,
            });
        }
    }

    if let Some((left, right)) = trimmed.split_once('=') {
        let left = left.trim();
        let right = right.trim();
        let column = raw_line.find(right).map(|index| index + 1).unwrap_or(1);
        if let Some((name, type_name)) = left.split_once(':') {
            let name = name.trim();
            let type_name = type_name.trim();
            if is_identifier(name) {
                *cursor += 1;
                return Ok(Statement::Let {
                    name: name.to_string(),
                    type_name: Some(type_name.to_string()),
                    value: parse_expr(right, line_number, column, mode)?,
                    line: line_number,
                    column,
                });
            }
        } else if is_identifier(left) {
            *cursor += 1;
            return Ok(Statement::Assign {
                target: AssignTarget::Name(left.to_string()),
                value: parse_expr(right, line_number, column, mode)?,
                line: line_number,
                column,
            });
        }
    }

    if let Some(rest) = trimmed.strip_prefix("fail(") {
        let message_source = rest.strip_suffix(')').ok_or_else(|| {
            parse_diagnostic_line(line_number, "fail statements use `fail(\"message\")`")
        })?;
        let message = parse_quoted(message_source).ok_or_else(|| {
            parse_diagnostic_line(line_number, "fail message must be a quoted string")
        })?;
        let column = raw_line.find("fail").map(|index| index + 1).unwrap_or(1);
        *cursor += 1;
        return Ok(Statement::Fail {
            message,
            line: line_number,
            column,
        });
    }

    if let Some(rest) = trimmed.strip_prefix("redirect(") {
        let target_source = rest.strip_suffix(')').ok_or_else(|| {
            parse_diagnostic_line(line_number, "redirect statements use `redirect(\"/path\")`")
        })?;
        let target = parse_quoted(target_source).ok_or_else(|| {
            parse_diagnostic_line(line_number, "redirect target must be a quoted path")
        })?;
        let column = raw_line
            .find("redirect")
            .map(|index| index + 1)
            .unwrap_or(1);
        *cursor += 1;
        return Ok(Statement::Redirect {
            target,
            line: line_number,
            column,
        });
    }

    *cursor += 1;
    Err(parse_diagnostic_line(
        line_number,
        format!("unsupported statement `{trimmed}`"),
    ))
}

fn parse_fn_def(
    lines: &[&str],
    cursor: &mut usize,
    raw_line: &str,
    trimmed: &str,
    line_number: usize,
    mode: BlockMode,
) -> Result<Statement, Diagnostic> {
    let header = trimmed
        .strip_prefix("fn ")
        .expect("fn prefix checked")
        .trim()
        .strip_suffix('{')
        .ok_or_else(|| parse_diagnostic_line(line_number, "fn expects `fn name(params) {`"))?
        .trim();

    let (signature, _) = header
        .split_once(')')
        .ok_or_else(|| parse_diagnostic_line(line_number, "fn expects `fn name(params) {`"))?;
    let (name, params_source) = signature
        .split_once('(')
        .ok_or_else(|| parse_diagnostic_line(line_number, "fn expects `fn name(params) {`"))?;
    let name = name.trim();
    if !is_identifier(name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid function name `{name}`"),
        ));
    }

    let (params, return_type) = parse_fn_signature(params_source.trim(), line_number)?;
    *cursor += 1;
    let body = parse_server_block(lines, cursor, line_number, mode)?;
    if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
        return Err(parse_diagnostic_line(line_number, "unclosed fn block"));
    }
    *cursor += 1;

    let column = raw_line.find("fn").map(|index| index + 1).unwrap_or(1);
    Ok(Statement::FnDef {
        name: name.to_string(),
        params,
        return_type,
        body,
        line: line_number,
        column,
    })
}

fn parse_fn_signature(
    params_source: &str,
    line_number: usize,
) -> Result<(Vec<FnParam>, Option<String>), Diagnostic> {
    let mut params = Vec::new();
    if !params_source.is_empty() {
        for param in params_source.split(',') {
            let param = param.trim();
            let (name, type_name) = param
                .split_once(':')
                .ok_or_else(|| parse_diagnostic_line(line_number, "fn params use `name: type`"))?;
            let name = name.trim();
            if !is_identifier(name) {
                return Err(parse_diagnostic_line(
                    line_number,
                    format!("invalid param name `{name}`"),
                ));
            }
            params.push(FnParam {
                name: name.to_string(),
                type_name: type_name.trim().to_string(),
            });
        }
    }
    Ok((params, None))
}

fn parse_if(
    lines: &[&str],
    cursor: &mut usize,
    raw_line: &str,
    trimmed: &str,
    line_number: usize,
    mode: BlockMode,
) -> Result<Statement, Diagnostic> {
    let condition_source = trimmed
        .strip_prefix("if")
        .expect("if prefix checked")
        .trim()
        .strip_suffix('{')
        .ok_or_else(|| parse_diagnostic_line(line_number, "if expects `if condition {`"))?
        .trim();
    let column = raw_line
        .find(condition_source)
        .map(|index| index + 1)
        .unwrap_or(1);
    let condition = parse_expr(condition_source, line_number, column, mode)?;
    *cursor += 1;
    let statements = parse_server_block(lines, cursor, line_number, mode)?;
    if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
        return Err(parse_diagnostic_line(line_number, "unclosed if block"));
    }
    *cursor += 1;
    Ok(Statement::If {
        condition,
        statements,
        line: line_number,
        column,
    })
}

fn parse_while(
    lines: &[&str],
    cursor: &mut usize,
    raw_line: &str,
    trimmed: &str,
    line_number: usize,
    mode: BlockMode,
) -> Result<Statement, Diagnostic> {
    let condition_source = trimmed
        .strip_prefix("while")
        .expect("while prefix checked")
        .trim()
        .strip_suffix('{')
        .ok_or_else(|| parse_diagnostic_line(line_number, "while expects `while condition {`"))?
        .trim();
    let column = raw_line
        .find(condition_source)
        .map(|index| index + 1)
        .unwrap_or(1);
    let condition = parse_expr(condition_source, line_number, column, mode)?;
    *cursor += 1;
    let statements = parse_server_block(lines, cursor, line_number, mode)?;
    if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
        return Err(parse_diagnostic_line(line_number, "unclosed while block"));
    }
    *cursor += 1;
    Ok(Statement::While {
        condition,
        statements,
        line: line_number,
        column,
    })
}

fn parse_try(
    lines: &[&str],
    cursor: &mut usize,
    line_number: usize,
    mode: BlockMode,
) -> Result<Statement, Diagnostic> {
    *cursor += 1;
    let statements = parse_server_block(lines, cursor, line_number, mode)?;
    if *cursor >= lines.len() {
        return Err(parse_diagnostic_line(line_number, "unclosed try block"));
    }

    let catch_line = lines[*cursor].trim();
    if !catch_line.starts_with("} catch ") || !catch_line.ends_with('{') {
        return Err(parse_diagnostic_line(
            line_number,
            "try expects `} catch name {`",
        ));
    }

    let catch_name = catch_line
        .strip_prefix("} catch ")
        .and_then(|rest| rest.strip_suffix('{'))
        .map(str::trim)
        .ok_or_else(|| parse_diagnostic_line(line_number, "try expects `} catch name {`"))?;

    if !is_identifier(catch_name) {
        return Err(parse_diagnostic_line(
            line_number,
            format!("invalid catch variable `{catch_name}`"),
        ));
    }

    if mode == BlockMode::SyncOnly {
        return Err(parse_diagnostic_line(
            line_number,
            "`try/catch` is not allowed in `@do` blocks",
        ));
    }

    *cursor += 1;
    let catch_body = parse_server_block(lines, cursor, line_number, mode)?;
    if *cursor >= lines.len() || lines[*cursor].trim() != "}" {
        return Err(parse_diagnostic_line(line_number, "unclosed catch block"));
    }
    *cursor += 1;

    Ok(Statement::Try {
        statements,
        catch_name: catch_name.to_string(),
        catch_body,
        line: line_number,
        column: 1,
    })
}

fn parse_return(
    raw_line: &str,
    trimmed: &str,
    line_number: usize,
    mode: BlockMode,
    cursor: &mut usize,
) -> Result<Statement, Diagnostic> {
    let column = raw_line.find("return").map(|index| index + 1).unwrap_or(1);
    let value = if trimmed == "return" {
        None
    } else {
        let rest = trimmed
            .strip_prefix("return")
            .expect("return prefix")
            .trim();
        Some(parse_expr(rest, line_number, column + 6, mode)?)
    };
    *cursor += 1;
    Ok(Statement::Return {
        value,
        line: line_number,
        column,
    })
}

fn parse_throw(
    raw_line: &str,
    trimmed: &str,
    line_number: usize,
    mode: BlockMode,
    cursor: &mut usize,
) -> Result<Statement, Diagnostic> {
    if mode == BlockMode::SyncOnly {
        return Err(parse_diagnostic_line(
            line_number,
            "`throw` is not allowed in `@do` blocks",
        ));
    }

    let open = trimmed.find('(').expect("throw(");
    let close = trimmed
        .rfind(')')
        .ok_or_else(|| parse_diagnostic_line(line_number, "throw expects `throw(expr)`"))?;
    let value_source = trimmed[open + 1..close].trim();
    let column = raw_line
        .find(value_source)
        .map(|index| index + 1)
        .unwrap_or(1);
    *cursor += 1;
    Ok(Statement::Throw {
        value: parse_expr(value_source, line_number, column, mode)?,
        line: line_number,
        column,
    })
}

fn parse_expr(
    source: &str,
    line: usize,
    column: usize,
    mode: BlockMode,
) -> Result<expr::Expr, Diagnostic> {
    let expr = expr::parse(source, line, column)?;
    if mode == BlockMode::SyncOnly {
        validate_sync_expr(&expr, line, column)?;
    }
    Ok(expr)
}

fn validate_sync_expr(expr: &expr::Expr, line: usize, column: usize) -> Result<(), Diagnostic> {
    match expr {
        expr::Expr::Await { .. } => Err(parse_diagnostic_line(
            line,
            "`await` is not allowed in `@do` blocks",
        )),
        expr::Expr::Call { callee, args } => {
            if let expr::Expr::Path(path) = callee.as_ref() {
                if let Some(name) = path.first() {
                    if matches!(name.as_str(), "fetch" | "spawn" | "timeout" | "sleep") {
                        return Err(parse_diagnostic_line(
                            line,
                            format!("`{name}` is not allowed in `@do` blocks"),
                        ));
                    }
                }
            }
            for arg in args {
                validate_sync_expr(arg, line, column)?;
            }
            Ok(())
        }
        expr::Expr::Unary { expr, .. } => validate_sync_expr(expr, line, column),
        expr::Expr::Binary { left, right, .. } => {
            validate_sync_expr(left, line, column)?;
            validate_sync_expr(right, line, column)
        }
        _ => Ok(()),
    }
}

pub fn error_value(message: String) -> Value {
    let mut fields = BTreeMap::new();
    fields.insert("message".to_string(), Value::String(message));
    Value::Object(fields)
}

pub fn execute_sync(
    statements: &[Statement],
    scope: &mut Env,
    session: &mut BTreeMap<String, Value>,
) -> Result<Option<BlockOutcome>, Diagnostic> {
    register_fn_defs(statements, scope)?;
    match execute_statements(statements, scope, session, false)? {
        StmtResult::Done(outcome) => Ok(outcome),
        StmtResult::Thrown(error) => Err(Diagnostic::error(
            Span::at(1, 1),
            error.message,
            Some("uncaught throw".to_string()),
        )),
    }
}

pub fn register_fn_defs(statements: &[Statement], scope: &mut Env) -> Result<(), Diagnostic> {
    for statement in statements {
        if let Statement::FnDef {
            name,
            params,
            return_type,
            body,
            ..
        } = statement
        {
            scope.insert(
                name.clone(),
                Value::Function {
                    name: name.clone(),
                    params: params.clone(),
                    return_type: return_type.clone(),
                    body: body.clone(),
                },
            );
        }
    }
    Ok(())
}

pub enum StmtResult {
    Done(Option<BlockOutcome>),
    Thrown(WebError),
}

fn execute_statements(
    statements: &[Statement],
    scope: &mut Env,
    session: &mut BTreeMap<String, Value>,
    inside_try: bool,
) -> Result<StmtResult, Diagnostic> {
    for statement in statements {
        match execute_statement(statement, scope, session, inside_try)? {
            ExecuteResult::Continue => {}
            ExecuteResult::Outcome(outcome) => {
                return Ok(StmtResult::Done(Some(outcome)));
            }
            ExecuteResult::Thrown(error) => {
                if inside_try {
                    return Ok(StmtResult::Thrown(error));
                }
                return Err(error_to_diagnostic(&error, statement));
            }
        }
    }
    Ok(StmtResult::Done(None))
}

enum ExecuteResult {
    Continue,
    Outcome(BlockOutcome),
    Thrown(WebError),
}

fn execute_statement(
    statement: &Statement,
    scope: &mut Env,
    session: &mut BTreeMap<String, Value>,
    inside_try: bool,
) -> Result<ExecuteResult, Diagnostic> {
    match statement {
        Statement::FnDef { .. } => Ok(ExecuteResult::Continue),
        Statement::Let {
            name,
            type_name,
            value,
            line,
            column,
        } => {
            let value = expr::evaluate(value, scope, *line, *column)?;
            if let Some(type_name) = type_name {
                if !type_names_match(&value.type_name(), type_name) {
                    return Err(type_mismatch(*line, *column, type_name, &value.type_name()));
                }
            }
            scope.insert(name.clone(), value);
            Ok(ExecuteResult::Continue)
        }
        Statement::Assign {
            target,
            value,
            line,
            column,
        } => {
            let value = expr::evaluate(value, scope, *line, *column)?;
            match target {
                AssignTarget::Name(name) => {
                    scope.insert(name.clone(), value);
                }
                AssignTarget::SessionField(field) => {
                    session.insert(field.clone(), value.clone());
                    scope.insert("session".to_string(), Value::Object(session.clone()));
                }
            }
            Ok(ExecuteResult::Continue)
        }
        Statement::If {
            condition,
            statements,
            line,
            column,
        } => {
            let value = expr::evaluate(condition, scope, *line, *column)?;
            let Some(condition_value) = value.as_bool() else {
                return Err(condition_not_bool(*line, *column, &value));
            };
            if condition_value {
                return match execute_statements(statements, scope, session, inside_try)? {
                    StmtResult::Done(Some(outcome)) => Ok(ExecuteResult::Outcome(outcome)),
                    StmtResult::Done(None) => Ok(ExecuteResult::Continue),
                    StmtResult::Thrown(error) => Ok(ExecuteResult::Thrown(error)),
                };
            }
            Ok(ExecuteResult::Continue)
        }
        Statement::While {
            condition,
            statements,
            line,
            column,
        } => loop {
            let value = expr::evaluate(condition, scope, *line, *column)?;
            let Some(condition_value) = value.as_bool() else {
                return Err(condition_not_bool(*line, *column, &value));
            };
            if !condition_value {
                return Ok(ExecuteResult::Continue);
            }
            match execute_statements(statements, scope, session, inside_try)? {
                StmtResult::Done(Some(outcome)) => {
                    return Ok(ExecuteResult::Outcome(outcome));
                }
                StmtResult::Done(None) => {}
                StmtResult::Thrown(error) => return Ok(ExecuteResult::Thrown(error)),
            }
        },
        Statement::Try {
            statements,
            catch_name,
            catch_body,
            ..
        } => match execute_statements(statements, scope, session, true)? {
            StmtResult::Done(outcome) => Ok(outcome
                .map(ExecuteResult::Outcome)
                .unwrap_or(ExecuteResult::Continue)),
            StmtResult::Thrown(error) => {
                scope.insert(catch_name.clone(), error_value(error.message.clone()));
                match execute_statements(catch_body, scope, session, false)? {
                    StmtResult::Done(Some(outcome)) => Ok(ExecuteResult::Outcome(outcome)),
                    StmtResult::Done(None) => Ok(ExecuteResult::Continue),
                    StmtResult::Thrown(error) => Ok(ExecuteResult::Thrown(error)),
                }
            }
        },
        Statement::Return {
            value,
            line,
            column,
        } => {
            let value = match value {
                Some(expr) => expr::evaluate(expr, scope, *line, *column)?,
                None => Value::Object(BTreeMap::new()),
            };
            Ok(ExecuteResult::Outcome(BlockOutcome::Return(value)))
        }
        Statement::Throw {
            value,
            line,
            column,
        } => {
            let value = expr::evaluate(value, scope, *line, *column)?;
            let message = value.render();
            let error = WebError { message };
            if inside_try {
                return Ok(ExecuteResult::Thrown(error));
            }
            Err(error_to_diagnostic(&error, statement))
        }
        Statement::Fail {
            message,
            line,
            column,
            ..
        } => {
            if message.is_empty() {
                return Err(Diagnostic::error(
                    Span::at(*line, *column),
                    "fail message cannot be empty",
                    None,
                ));
            }
            Ok(ExecuteResult::Outcome(BlockOutcome::Fail(message.clone())))
        }
        Statement::Redirect {
            target,
            line,
            column,
            ..
        } => {
            if target.is_empty() {
                return Err(Diagnostic::error(
                    Span::at(*line, *column),
                    "redirect target cannot be empty",
                    None,
                ));
            }
            Ok(ExecuteResult::Outcome(BlockOutcome::Redirect(
                target.clone(),
            )))
        }
        Statement::ExprStmt { .. } => Ok(ExecuteResult::Continue),
    }
}

fn error_to_diagnostic(error: &WebError, statement: &Statement) -> Diagnostic {
    let (line, column) = match statement {
        Statement::Throw { line, column, .. } => (*line, *column),
        Statement::Try { line, column, .. } => (*line, *column),
        _ => (1, 1),
    };
    Diagnostic::error(
        Span::at(line, column),
        error.message.clone(),
        Some("uncaught throw".to_string()),
    )
}

fn condition_not_bool(line: usize, column: usize, value: &Value) -> Diagnostic {
    Diagnostic::error(
        Span::at(line, column),
        "condition must be bool",
        Some(format!("found `{}`", value.type_name())),
    )
}

fn type_mismatch(line: usize, column: usize, expected: &str, found: &str) -> Diagnostic {
    Diagnostic::error(
        Span::at(line, column),
        format!("expected `{expected}`, found `{found}`"),
        None,
    )
}

fn type_names_match(actual: &str, expected: &str) -> bool {
    actual == expected || (expected == "object" && (actual == "object" || actual.starts_with('{')))
}

fn parse_diagnostic_line(line: usize, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(Span::at(line, 1), message, None)
}

fn parse_quoted(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        Some(value[1..value.len() - 1].to_string())
    } else {
        None
    }
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|char| char == '_' || char.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_block(source: &str) -> Vec<Statement> {
        let wrapped = format!("{source}\n}}");
        let lines: Vec<&str> = wrapped.lines().collect();
        let mut cursor = 0;
        parse_server_block(&lines, &mut cursor, 1, BlockMode::AsyncCapable).expect("parse")
    }

    #[test]
    fn parses_let_assign_and_while() {
        let stmts =
            parse_block("attempts := 0\nwhile attempts < 3 {\n  attempts = attempts + 1\n}");
        assert_eq!(stmts.len(), 2);
        assert!(matches!(stmts[0], Statement::Let { .. }));
        assert!(matches!(stmts[1], Statement::While { .. }));
    }

    #[test]
    fn parses_try_catch() {
        let stmts =
            parse_block("try {\n  throw(\"boom\")\n} catch err {\n  result = err.message\n}");
        assert!(matches!(stmts[0], Statement::Try { .. }));
    }

    #[test]
    fn throw_is_caught_by_try() {
        let stmts = parse_block(
            "result := \"\"\ntry {\n  throw(\"boom\")\n} catch err {\n  result = err.message\n}",
        );
        let mut scope = Env::new();
        let mut session = BTreeMap::new();
        scope.insert("result".to_string(), Value::String(String::new()));
        execute_sync(&stmts, &mut scope, &mut session).expect("execute");
        assert!(matches!(
            scope.get("result"),
            Some(Value::String(message)) if message == "boom"
        ));
    }
}
