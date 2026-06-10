use crate::diagnostic::{Diagnostic, Span};
use crate::expr;
use crate::parser::Value;
use crate::stmt::{self, error_value, AssignTarget, BlockOutcome, Statement, StmtResult, WebError};
use async_recursion::async_recursion;
use reqwest::Client;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout, Duration};

pub type Env = BTreeMap<String, Value>;

static PROMISE_ID: AtomicU64 = AtomicU64::new(1);

type BoxFuture = Pin<Box<dyn Future<Output = Result<Value, WebError>> + Send>>;

#[derive(Clone)]
pub struct WebRuntime {
    client: Client,
    promises: Arc<Mutex<BTreeMap<u64, BoxFuture>>>,
}

impl WebRuntime {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            promises: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub async fn execute_block_async(
        &self,
        statements: &[Statement],
        scope: &mut Env,
        session: &mut BTreeMap<String, Value>,
    ) -> Result<Option<BlockOutcome>, Diagnostic> {
        stmt::register_fn_defs(statements, scope)?;
        match self
            .execute_statements(statements, scope, session, false)
            .await?
        {
            StmtResult::Done(outcome) => Ok(outcome),
            StmtResult::Thrown(error) => Err(Diagnostic::error(
                Span::at(1, 1),
                error.message,
                Some("uncaught throw".to_string()),
            )),
        }
    }

    #[async_recursion]
    async fn execute_statements(
        &self,
        statements: &[Statement],
        scope: &mut Env,
        session: &mut BTreeMap<String, Value>,
        inside_try: bool,
    ) -> Result<StmtResult, Diagnostic> {
        for statement in statements {
            match self
                .execute_statement(statement, scope, session, inside_try)
                .await?
            {
                ExecuteResult::Continue => {}
                ExecuteResult::Outcome(outcome) => {
                    return Ok(StmtResult::Done(Some(outcome)));
                }
                ExecuteResult::Thrown(error) => {
                    if inside_try {
                        return Ok(StmtResult::Thrown(error));
                    }
                    return Err(error_to_diagnostic_at(&error, 1, 1));
                }
            }
        }
        Ok(StmtResult::Done(None))
    }

    #[async_recursion]
    async fn execute_statement(
        &self,
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
                let value = match self
                    .evaluate_statement_expr(value, scope, session, *line, *column, inside_try)
                    .await?
                {
                    Ok(value) => value,
                    Err(error) => return Ok(ExecuteResult::Thrown(error)),
                };
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
                let value = match self
                    .evaluate_statement_expr(value, scope, session, *line, *column, inside_try)
                    .await?
                {
                    Ok(value) => value,
                    Err(error) => return Ok(ExecuteResult::Thrown(error)),
                };
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
                let value = match self
                    .evaluate_statement_expr(condition, scope, session, *line, *column, inside_try)
                    .await?
                {
                    Ok(value) => value,
                    Err(error) => return Ok(ExecuteResult::Thrown(error)),
                };
                let Some(condition_value) = value.as_bool() else {
                    return Err(condition_not_bool(*line, *column, &value));
                };
                if condition_value {
                    return match self
                        .execute_statements(statements, scope, session, inside_try)
                        .await?
                    {
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
                let value = match self
                    .evaluate_statement_expr(condition, scope, session, *line, *column, inside_try)
                    .await?
                {
                    Ok(value) => value,
                    Err(error) => return Ok(ExecuteResult::Thrown(error)),
                };
                let Some(condition_value) = value.as_bool() else {
                    return Err(condition_not_bool(*line, *column, &value));
                };
                if !condition_value {
                    return Ok(ExecuteResult::Continue);
                }
                match self
                    .execute_statements(statements, scope, session, inside_try)
                    .await?
                {
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
            } => match self
                .execute_statements(statements, scope, session, true)
                .await?
            {
                StmtResult::Done(outcome) => Ok(outcome
                    .map(ExecuteResult::Outcome)
                    .unwrap_or(ExecuteResult::Continue)),
                StmtResult::Thrown(error) => {
                    scope.insert(catch_name.clone(), error_value(error.message.clone()));
                    match self
                        .execute_statements(catch_body, scope, session, false)
                        .await?
                    {
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
                    Some(expr) => match self
                        .evaluate_statement_expr(expr, scope, session, *line, *column, inside_try)
                        .await?
                    {
                        Ok(value) => value,
                        Err(error) => return Ok(ExecuteResult::Thrown(error)),
                    },
                    None => Value::Object(BTreeMap::new()),
                };
                Ok(ExecuteResult::Outcome(BlockOutcome::Return(value)))
            }
            Statement::Throw {
                value,
                line,
                column,
            } => {
                let value = match self
                    .evaluate_statement_expr(value, scope, session, *line, *column, inside_try)
                    .await?
                {
                    Ok(value) => value,
                    Err(error) => return Ok(ExecuteResult::Thrown(error)),
                };
                Ok(ExecuteResult::Thrown(WebError {
                    message: value.render(),
                }))
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

    #[allow(dead_code)]
    pub fn evaluate_expr<'a>(
        &'a self,
        expression: &'a expr::Expr,
        scope: &'a Env,
        session: &'a BTreeMap<String, Value>,
        line: usize,
        column: usize,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Value, Diagnostic>> + Send + 'a>> {
        Box::pin(async move {
            self.evaluate_expr_inner(expression, scope, session, line, column)
                .await
                .map_err(|error| error.into_diagnostic(line, column))
        })
    }

    fn evaluate_expr_inner<'a>(
        &'a self,
        expression: &'a expr::Expr,
        scope: &'a Env,
        session: &'a BTreeMap<String, Value>,
        line: usize,
        column: usize,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Value, EvalError>> + Send + 'a>> {
        Box::pin(async move {
            match expression {
                expr::Expr::Await { expr } => {
                    let promise = self
                        .evaluate_expr_inner(expr, scope, session, line, column)
                        .await?;
                    self.await_value(promise, line, column).await
                }
                expr::Expr::Call { callee, args } => {
                    self.evaluate_call(callee, args, scope, session, line, column)
                        .await
                }
                other => expr::evaluate(other, scope, line, column).map_err(EvalError::Diagnostic),
            }
        })
    }

    async fn evaluate_statement_expr(
        &self,
        expression: &expr::Expr,
        scope: &Env,
        session: &BTreeMap<String, Value>,
        line: usize,
        column: usize,
        inside_try: bool,
    ) -> Result<Result<Value, WebError>, Diagnostic> {
        match self
            .evaluate_expr_inner(expression, scope, session, line, column)
            .await
        {
            Ok(value) => Ok(Ok(value)),
            Err(EvalError::Diagnostic(diagnostic)) => Err(diagnostic),
            Err(EvalError::Thrown(error)) if inside_try => Ok(Err(error)),
            Err(EvalError::Thrown(error)) => Err(error_to_diagnostic_at(&error, line, column)),
        }
    }

    fn evaluate_call<'a>(
        &'a self,
        callee: &'a expr::Expr,
        args: &'a [expr::Expr],
        scope: &'a Env,
        session: &'a BTreeMap<String, Value>,
        line: usize,
        column: usize,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Value, EvalError>> + Send + 'a>> {
        Box::pin(self.evaluate_call_inner(callee, args, scope, session, line, column))
    }

    async fn evaluate_call_inner(
        &self,
        callee: &expr::Expr,
        args: &[expr::Expr],
        scope: &Env,
        session: &BTreeMap<String, Value>,
        line: usize,
        column: usize,
    ) -> Result<Value, EvalError> {
        let Some(name) = simple_callee_name(callee) else {
            return Err(Diagnostic::error(
                Span::at(line, column),
                "only simple function calls are supported",
                None,
            )
            .into());
        };

        let mut evaluated_args = Vec::with_capacity(args.len());
        for arg in args {
            evaluated_args
                .push(Box::pin(self.evaluate_expr_inner(arg, scope, session, line, column)).await?);
        }

        if let Some(Value::Function { params, body, .. }) = scope.get(&name).cloned() {
            if params.len() != evaluated_args.len() {
                return Err(Diagnostic::error(
                    Span::at(line, column),
                    format!(
                        "function `{name}` expects {} arguments, found {}",
                        params.len(),
                        evaluated_args.len()
                    ),
                    None,
                )
                .into());
            }
            let mut call_scope = scope.clone();
            for (param, value) in params.iter().zip(evaluated_args.iter()) {
                call_scope.insert(param.name.clone(), value.clone());
            }
            let mut call_session = session.clone();
            let outcome = self
                .execute_block_async(&body, &mut call_scope, &mut call_session)
                .await
                .map_err(EvalError::Diagnostic)?;
            return match outcome {
                Some(BlockOutcome::Return(value)) => Ok(value),
                Some(BlockOutcome::Fail(message)) => Err(Diagnostic::error(
                    Span::at(line, column),
                    message,
                    Some("function returned fail".to_string()),
                )
                .into()),
                Some(BlockOutcome::Redirect(target)) => Err(Diagnostic::error(
                    Span::at(line, column),
                    format!("function redirected to `{target}`"),
                    None,
                )
                .into()),
                None => Ok(Value::Object(BTreeMap::new())),
            };
        }

        match name.as_str() {
            "sleep" => self
                .builtin_sleep(&evaluated_args, line, column)
                .await
                .map_err(Into::into),
            "fetch" => self
                .builtin_fetch(&evaluated_args, line, column)
                .await
                .map_err(Into::into),
            "spawn" => self
                .builtin_spawn(&evaluated_args, line, column)
                .await
                .map_err(Into::into),
            "timeout" => self
                .builtin_timeout(&evaluated_args, line, column)
                .await
                .map_err(Into::into),
            _ => Err(Diagnostic::error(
                Span::identifier(line, column, &name),
                format!("unknown function `{name}`"),
                None,
            )
            .into()),
        }
    }

    async fn builtin_sleep(
        &self,
        args: &[Value],
        line: usize,
        column: usize,
    ) -> Result<Value, Diagnostic> {
        let Value::Duration { ms } = expect_duration(args, line, column, "sleep")? else {
            unreachable!();
        };
        let id = self
            .insert_future(async move {
                sleep(Duration::from_millis(ms as u64)).await;
                Ok(Value::Object(BTreeMap::new()))
            })
            .await;
        Ok(Value::Promise { id })
    }

    async fn builtin_fetch(
        &self,
        args: &[Value],
        line: usize,
        column: usize,
    ) -> Result<Value, Diagnostic> {
        let url = expect_string(args, line, column, "fetch")?;
        let client = self.client.clone();
        let id = self
            .insert_future(async move {
                let response = client.get(&url).send().await.map_err(|error| WebError {
                    message: error.to_string(),
                })?;
                let status = response.status().as_u16() as i64;
                let body = response.text().await.map_err(|error| WebError {
                    message: error.to_string(),
                })?;
                let ok = (200..300).contains(&(status as i16));
                let mut fields = BTreeMap::new();
                fields.insert("status".to_string(), Value::Int(status));
                fields.insert("body".to_string(), Value::String(body));
                fields.insert("ok".to_string(), Value::Bool(ok));
                Ok(Value::Object(fields))
            })
            .await;
        Ok(Value::Promise { id })
    }

    async fn builtin_spawn(
        &self,
        args: &[Value],
        line: usize,
        column: usize,
    ) -> Result<Value, Diagnostic> {
        let Value::Promise { id } = expect_promise(args, line, column, "spawn")? else {
            unreachable!();
        };
        Ok(Value::Promise { id })
    }

    async fn builtin_timeout(
        &self,
        args: &[Value],
        line: usize,
        column: usize,
    ) -> Result<Value, Diagnostic> {
        if args.len() != 2 {
            return Err(Diagnostic::error(
                Span::at(line, column),
                "timeout expects 2 arguments",
                None,
            ));
        }
        let duration = match &args[0] {
            Value::Duration { ms } => Duration::from_millis(*ms as u64),
            other => {
                return Err(Diagnostic::error(
                    Span::at(line, column),
                    format!("timeout expects duration, found `{}`", other.type_name()),
                    None,
                ))
            }
        };
        let Value::Promise { id } = &args[1] else {
            return Err(Diagnostic::error(
                Span::at(line, column),
                "timeout expects promise as second argument",
                None,
            ));
        };
        let inner = self
            .take_future(*id)
            .await
            .ok_or_else(|| Diagnostic::error(Span::at(line, column), "unknown promise", None))?;
        let id = self
            .insert_future(async move {
                match timeout(duration, inner).await {
                    Ok(result) => result,
                    Err(_) => Err(WebError {
                        message: "timeout".to_string(),
                    }),
                }
            })
            .await;
        Ok(Value::Promise { id })
    }

    async fn insert_future<F>(&self, future: F) -> u64
    where
        F: Future<Output = Result<Value, WebError>> + Send + 'static,
    {
        let id = PROMISE_ID.fetch_add(1, Ordering::Relaxed);
        self.promises.lock().await.insert(id, Box::pin(future));
        id
    }

    async fn take_future(&self, id: u64) -> Option<BoxFuture> {
        self.promises.lock().await.remove(&id)
    }

    async fn await_value(
        &self,
        value: Value,
        line: usize,
        column: usize,
    ) -> Result<Value, EvalError> {
        let Value::Promise { id } = value else {
            return Err(Diagnostic::error(
                Span::at(line, column),
                format!("await expects promise, found `{}`", value.type_name()),
                None,
            )
            .into());
        };

        let Some(future) = self.take_future(id).await else {
            return Err(Diagnostic::error(Span::at(line, column), "unknown promise", None).into());
        };

        future.await.map_err(EvalError::Thrown)
    }
}

enum EvalError {
    Diagnostic(Diagnostic),
    Thrown(WebError),
}

impl EvalError {
    #[allow(dead_code)]
    fn into_diagnostic(self, line: usize, column: usize) -> Diagnostic {
        match self {
            Self::Diagnostic(diagnostic) => diagnostic,
            Self::Thrown(error) => error_to_diagnostic_at(&error, line, column),
        }
    }
}

impl From<Diagnostic> for EvalError {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::Diagnostic(diagnostic)
    }
}

fn error_to_diagnostic_at(error: &WebError, line: usize, column: usize) -> Diagnostic {
    Diagnostic::error(
        Span::at(line, column),
        error.message.clone(),
        Some("uncaught throw".to_string()),
    )
}

enum ExecuteResult {
    Continue,
    Outcome(BlockOutcome),
    Thrown(WebError),
}

fn simple_callee_name(expr: &expr::Expr) -> Option<String> {
    match expr {
        expr::Expr::Path(path) if path.len() == 1 => Some(path[0].clone()),
        _ => None,
    }
}

fn expect_string(
    args: &[Value],
    line: usize,
    column: usize,
    name: &str,
) -> Result<String, Diagnostic> {
    if args.len() != 1 {
        return Err(Diagnostic::error(
            Span::at(line, column),
            format!("{name} expects 1 argument"),
            None,
        ));
    }
    match &args[0] {
        Value::String(value) => Ok(value.clone()),
        other => Err(Diagnostic::error(
            Span::at(line, column),
            format!("{name} expects string, found `{}`", other.type_name()),
            None,
        )),
    }
}

fn expect_duration(
    args: &[Value],
    line: usize,
    column: usize,
    name: &str,
) -> Result<Value, Diagnostic> {
    if args.len() != 1 {
        return Err(Diagnostic::error(
            Span::at(line, column),
            format!("{name} expects 1 argument"),
            None,
        ));
    }
    match &args[0] {
        Value::Duration { .. } => Ok(args[0].clone()),
        other => Err(Diagnostic::error(
            Span::at(line, column),
            format!("{name} expects duration, found `{}`", other.type_name()),
            None,
        )),
    }
}

fn expect_promise(
    args: &[Value],
    line: usize,
    column: usize,
    name: &str,
) -> Result<Value, Diagnostic> {
    if args.len() != 1 {
        return Err(Diagnostic::error(
            Span::at(line, column),
            format!("{name} expects 1 argument"),
            None,
        ));
    }
    match &args[0] {
        Value::Promise { .. } => Ok(args[0].clone()),
        other => Err(Diagnostic::error(
            Span::at(line, column),
            format!("{name} expects promise, found `{}`", other.type_name()),
            None,
        )),
    }
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

impl Default for WebRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr;

    #[tokio::test]
    async fn sleep_resolves() {
        let runtime = WebRuntime::new();
        let expression = expr::parse("await sleep(10ms)", 1, 1).expect("parse");
        let scope = Env::new();
        let session = BTreeMap::new();
        let value = runtime
            .evaluate_expr(&expression, &scope, &session, 1, 1)
            .await
            .expect("sleep");
        assert!(matches!(value, Value::Object(_)));
    }
}
