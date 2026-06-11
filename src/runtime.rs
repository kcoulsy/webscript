use crate::db_runtime::DbRuntime;
use crate::debugbar::{QuerySource, TaskKind, TaskTrace};
use crate::diagnostic::{Diagnostic, Span};
use crate::expr;
use crate::model_runtime::ModelRuntime;
use crate::parser::Value;
use crate::schema::is_schema_name;
use crate::schema::json_value_to_parser;
use crate::schema_runtime::SchemaRuntime;
use crate::stmt::{self, error_value, AssignTarget, BlockOutcome, Statement, StmtResult, WebError};
use crate::types;
use async_recursion::async_recursion;
use reqwest::Client;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{sleep, timeout, Duration};

pub type Env = BTreeMap<String, Value>;

static PROMISE_ID: AtomicU64 = AtomicU64::new(1);

type BoxFuture = Pin<Box<dyn Future<Output = Result<Value, WebError>> + Send>>;

#[derive(Clone)]
pub struct WebRuntime {
    client: Client,
    promises: Arc<AsyncMutex<BTreeMap<u64, BoxFuture>>>,
    models: Option<Arc<ModelRuntime>>,
    db: Option<Arc<DbRuntime>>,
    schemas: Option<Arc<SchemaRuntime>>,
    trace: Option<Arc<Mutex<TaskTrace>>>,
    promise_labels: Arc<Mutex<BTreeMap<u64, String>>>,
}

impl WebRuntime {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            promises: Arc::new(AsyncMutex::new(BTreeMap::new())),
            models: None,
            db: None,
            schemas: None,
            trace: None,
            promise_labels: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn with_database(root: std::path::PathBuf) -> Result<Self, String> {
        let schemas = SchemaRuntime::new(root.clone())?;
        Ok(Self {
            client: Client::new(),
            promises: Arc::new(AsyncMutex::new(BTreeMap::new())),
            models: Some(ModelRuntime::new(root.clone())?),
            db: Some(DbRuntime::new(root, schemas.clone())?),
            schemas: Some(schemas),
            trace: None,
            promise_labels: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub fn for_request(&self, trace: Arc<Mutex<TaskTrace>>) -> Self {
        Self {
            client: self.client.clone(),
            promises: Arc::clone(&self.promises),
            models: self.models.clone(),
            db: self.db.clone(),
            schemas: self.schemas.clone(),
            trace: Some(trace),
            promise_labels: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn schemas(&self) -> Option<&SchemaRuntime> {
        self.schemas.as_deref()
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
                    if !types::value_matches_type(&value, type_name) {
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
            Statement::ExprStmt { expr, line, column } => {
                match self
                    .evaluate_statement_expr(expr, scope, session, *line, *column, inside_try)
                    .await?
                {
                    Ok(_) => Ok(ExecuteResult::Continue),
                    Err(error) => Ok(ExecuteResult::Thrown(error)),
                }
            }
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
        if db_callee_name(callee).as_deref() == Some("query") {
            return self
                .evaluate_db_query(args, scope, session, line, column)
                .await;
        }

        if simple_callee_name(callee).as_deref() == Some("fetch") {
            return self
                .evaluate_fetch(args, scope, session, line, column)
                .await;
        }

        let mut evaluated_args = Vec::with_capacity(args.len());
        for arg in args {
            evaluated_args
                .push(Box::pin(self.evaluate_expr_inner(arg, scope, session, line, column)).await?);
        }

        if let Some(method) = db_callee_name(callee) {
            let Some(db) = self.db.clone() else {
                return Err(Diagnostic::error(
                    Span::at(line, column),
                    format!("database helper `db.{method}` requires a project database"),
                    None,
                )
                .into());
            };
            let label = db_task_label(&method, &evaluated_args);
            let (sql, params) = db_query_parts(&evaluated_args);
            let trace = self.trace.clone();
            let id = self
                .insert_future(label.clone(), TaskKind::Db, async move {
                    let query_index =
                        begin_query(trace.as_ref(), label, QuerySource::Raw, sql, params);
                    let result = db.call(&method, &evaluated_args);
                    finish_query(trace.as_ref(), query_index, &result);
                    result.map_err(|error| WebError { message: error })
                })
                .await;
            return Ok(Value::Promise { id });
        }

        if let Some((model, method)) = model_callee_name(callee) {
            let Some(models) = self.models.clone() else {
                return Err(Diagnostic::error(
                    Span::at(line, column),
                    format!("database model `{model}.{method}` requires a project database"),
                    None,
                )
                .into());
            };
            let label = format!("{model}.{method}");
            let trace = self.trace.clone();
            let id = self
                .insert_future(label, TaskKind::Model, async move {
                    models
                        .call_with_trace(&model, &method, &evaluated_args, trace.as_ref())
                        .map_err(|error| WebError { message: error })
                })
                .await;
            return Ok(Value::Promise { id });
        }

        let Some(name) = simple_callee_name(callee) else {
            return Err(Diagnostic::error(
                Span::at(line, column),
                "only simple function calls are supported",
                None,
            )
            .into());
        };

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
            "fetch" => Err(Diagnostic::error(
                Span::at(line, column),
                "fetch must be called with a schema: fetch(url, Schema)",
                None,
            )
            .into()),
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
        let label = format!("sleep({ms}ms)");
        let id = self
            .insert_future(label, TaskKind::Sleep, async move {
                sleep(Duration::from_millis(ms as u64)).await;
                Ok(Value::Object(BTreeMap::new()))
            })
            .await;
        Ok(Value::Promise { id })
    }

    async fn evaluate_fetch(
        &self,
        args: &[expr::Expr],
        scope: &Env,
        session: &BTreeMap<String, Value>,
        line: usize,
        column: usize,
    ) -> Result<Value, EvalError> {
        if args.len() != 2 {
            return Err(Diagnostic::error(
                Span::at(line, column),
                "fetch expects 2 arguments (url, Schema)",
                None,
            )
            .into());
        }
        let schema_name = schema_ref_name(&args[1]).ok_or_else(|| {
            Diagnostic::error(
                Span::at(line, column),
                "fetch second argument must be a schema name such as ApiResponse",
                None,
            )
        })?;
        let url_value =
            Box::pin(self.evaluate_expr_inner(&args[0], scope, session, line, column)).await?;
        let Value::String(url) = url_value else {
            return Err(Diagnostic::error(
                Span::at(line, column),
                format!(
                    "fetch expects string url, found `{}`",
                    url_value.type_name()
                ),
                None,
            )
            .into());
        };
        self.builtin_fetch(url, schema_name.to_string(), line, column)
            .await
            .map_err(Into::into)
    }

    async fn evaluate_db_query(
        &self,
        args: &[expr::Expr],
        scope: &Env,
        session: &BTreeMap<String, Value>,
        line: usize,
        column: usize,
    ) -> Result<Value, EvalError> {
        if args.is_empty() || args.len() > 3 {
            return Err(Diagnostic::error(
                Span::at(line, column),
                "db.query expects (sql, Schema) or (sql, params, Schema)",
                None,
            )
            .into());
        }
        let schema_name =
            schema_ref_name(args.last().expect("checked length")).ok_or_else(|| {
                Diagnostic::error(
                    Span::at(line, column),
                    "db.query requires a schema as the last argument",
                    None,
                )
            })?;
        let Some(db) = self.db.clone() else {
            return Err(Diagnostic::error(
                Span::at(line, column),
                "database helper `db.query` requires a project database",
                None,
            )
            .into());
        };

        let mut evaluated_args = Vec::with_capacity(args.len() - 1);
        for arg in &args[..args.len() - 1] {
            evaluated_args
                .push(Box::pin(self.evaluate_expr_inner(arg, scope, session, line, column)).await?);
        }
        if evaluated_args.is_empty() {
            return Err(Diagnostic::error(
                Span::at(line, column),
                "db.query expects a SQL string as the first argument",
                None,
            )
            .into());
        }

        let label = db_task_label("query", &evaluated_args);
        let (sql, params) = db_query_parts(&evaluated_args);
        let schema_name = schema_name.to_string();
        let trace = self.trace.clone();
        let id = self
            .insert_future(label.clone(), TaskKind::Db, async move {
                let query_index = begin_query(trace.as_ref(), label, QuerySource::Raw, sql, params);
                let result = db.call_query(&evaluated_args, &schema_name);
                finish_query(trace.as_ref(), query_index, &result);
                result.map_err(|error| WebError { message: error })
            })
            .await;
        Ok(Value::Promise { id })
    }

    async fn builtin_fetch(
        &self,
        url: String,
        schema_name: String,
        line: usize,
        column: usize,
    ) -> Result<Value, Diagnostic> {
        let Some(schemas) = self.schemas.clone() else {
            return Err(Diagnostic::error(
                Span::at(line, column),
                "fetch requires loaded project schemas",
                None,
            ));
        };
        let label = format!("fetch({})", truncate_label(&url, 48));
        let client = self.client.clone();
        let id = self
            .insert_future(label, TaskKind::Fetch, async move {
                let response = client.get(&url).send().await.map_err(|error| WebError {
                    message: error.to_string(),
                })?;
                let status = response.status().as_u16() as i64;
                if !(200..300).contains(&(status as i16)) {
                    return Err(WebError {
                        message: format!("fetch returned {status}"),
                    });
                }
                let body = response.text().await.map_err(|error| WebError {
                    message: error.to_string(),
                })?;
                let json = serde_json::from_str(&body).map_err(|error| WebError {
                    message: format!("invalid JSON response: {error}"),
                })?;
                let value =
                    json_value_to_parser(json).map_err(|error| WebError { message: error })?;
                schemas
                    .validate(&schema_name, value)
                    .map_err(|error| WebError { message: error })
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
        let spawn_label = self
            .promise_labels
            .lock()
            .ok()
            .and_then(|labels| labels.get(&id).cloned())
            .map(|label| format!("spawn({label})"));
        if let Some(label) = spawn_label {
            if let Ok(mut labels) = self.promise_labels.lock() {
                labels.insert(id, label.clone());
            }
            if let Some(trace) = &self.trace {
                if let Ok(mut guard) = trace.lock() {
                    let index = guard.begin(label, TaskKind::Spawn);
                    guard.finish(index);
                }
            }
        }
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
        let label = format!("timeout({}ms)", duration.as_millis());
        let id = self
            .insert_future(label, TaskKind::Timeout, async move {
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

    async fn insert_future<F>(&self, label: impl Into<String>, kind: TaskKind, future: F) -> u64
    where
        F: Future<Output = Result<Value, WebError>> + Send + 'static,
    {
        let id = PROMISE_ID.fetch_add(1, Ordering::Relaxed);
        let label = label.into();
        if let Ok(mut labels) = self.promise_labels.lock() {
            labels.insert(id, label.clone());
        }

        let trace = self.trace.clone();
        let task_label = label;
        let wrapped = async move {
            let task_index = trace.as_ref().and_then(|trace| {
                trace
                    .lock()
                    .ok()
                    .map(|mut guard| guard.begin(&task_label, kind))
            });
            let result = future.await;
            if let (Some(trace), Some(task_index)) = (trace.as_ref(), task_index) {
                if let Ok(mut guard) = trace.lock() {
                    guard.finish(task_index);
                }
            }
            result
        };

        self.promises.lock().await.insert(id, Box::pin(wrapped));
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

        let await_label = self
            .promise_labels
            .lock()
            .ok()
            .and_then(|labels| labels.get(&id).cloned())
            .map(|label| format!("await {label}"))
            .unwrap_or_else(|| format!("await promise #{id}"));
        let await_index = self.trace.as_ref().and_then(|trace| {
            trace
                .lock()
                .ok()
                .map(|mut guard| guard.begin(&await_label, TaskKind::Await))
        });

        let result = future.await.map_err(EvalError::Thrown);

        if let (Some(trace), Some(await_index)) = (self.trace.as_ref(), await_index) {
            if let Ok(mut guard) = trace.lock() {
                guard.finish(await_index);
            }
        }

        result
    }
}

fn truncate_label(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }
    let truncated: String = value.chars().take(max_len.saturating_sub(1)).collect();
    format!("{truncated}…")
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

fn db_callee_name(expr: &expr::Expr) -> Option<String> {
    match expr {
        expr::Expr::Path(path)
            if path.len() == 2
                && path[0] == "db"
                && matches!(path[1].as_str(), "query" | "execute") =>
        {
            Some(path[1].clone())
        }
        _ => None,
    }
}

fn db_task_label(method: &str, args: &[Value]) -> String {
    let sql = args
        .first()
        .and_then(|value| match value {
            Value::String(text) => Some(text.as_str()),
            _ => None,
        })
        .unwrap_or("");
    format!("db.{method}(\"{sql}\")")
}

fn db_query_parts(args: &[Value]) -> (String, Vec<String>) {
    let sql = args
        .first()
        .and_then(|value| match value {
            Value::String(text) => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let params = args
        .get(1)
        .and_then(|value| match value {
            Value::Array { values, .. } => Some(values.iter().map(Value::render).collect()),
            _ => None,
        })
        .unwrap_or_default();
    (sql, params)
}

fn begin_query(
    trace: Option<&Arc<Mutex<TaskTrace>>>,
    label: String,
    source: QuerySource,
    sql: String,
    params: Vec<String>,
) -> Option<usize> {
    trace.and_then(|trace| {
        trace
            .lock()
            .ok()
            .map(|mut guard| guard.begin_query(label, source, sql, params))
    })
}

fn finish_query(
    trace: Option<&Arc<Mutex<TaskTrace>>>,
    query_index: Option<usize>,
    result: &Result<Value, String>,
) {
    if let (Some(trace), Some(query_index)) = (trace, query_index) {
        if let Ok(mut guard) = trace.lock() {
            guard.finish_query(query_index, result.is_ok(), result.as_ref().err().cloned());
        }
    }
}

fn schema_ref_name(expr: &expr::Expr) -> Option<&str> {
    match expr {
        expr::Expr::Path(path) if path.len() == 1 && is_schema_name(&path[0]) => Some(&path[0]),
        _ => None,
    }
}

fn model_callee_name(expr: &expr::Expr) -> Option<(String, String)> {
    match expr {
        expr::Expr::Path(path) if path.len() == 2 && path[0] != "db" => {
            Some((path[0].clone(), path[1].clone()))
        }
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

impl Default for WebRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr;
    use std::sync::{Arc, Mutex};

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

    #[tokio::test]
    async fn db_query_resolves() {
        use crate::db;
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("webscript-runtime-db-{nanos}"));
        fs::create_dir_all(root.join("app/models")).expect("models dir");
        fs::create_dir_all(root.join("app/schemas")).expect("schemas dir");
        fs::write(
            root.join("app/models/Todo.web"),
            "@model Todo {\n  id: int @primary @auto\n  title: string\n  done: bool @default(false)\n  createdAt: datetime @default(now)\n}\n",
        )
        .expect("write model");
        fs::write(
            root.join("app/schemas/CountRow.web"),
            "@schema CountRow {\n  n: int\n}\n",
        )
        .expect("write schema");
        db::generate(&root, Some("schema")).expect("generate");
        db::migrate(&root).expect("migrate");

        let runtime = WebRuntime::with_database(root.clone()).expect("runtime");
        let trace = Arc::new(Mutex::new(TaskTrace::new()));
        let runtime = runtime.for_request(Arc::clone(&trace));
        let expression =
            expr::parse("await db.query(\"SELECT 1 AS n\", CountRow)", 1, 1).expect("parse");
        let scope = Env::new();
        let session = BTreeMap::new();
        let value = runtime
            .evaluate_expr(&expression, &scope, &session, 1, 1)
            .await
            .expect("db query");
        let Value::Array {
            values,
            element_type,
        } = value
        else {
            panic!("expected array");
        };
        assert_eq!(element_type, "CountRow");
        assert_eq!(values.len(), 1);
        let Value::Object(fields) = &values[0] else {
            panic!("expected object row");
        };
        assert_eq!(fields.get("n"), Some(&Value::Int(1)));

        let trace = trace.lock().expect("trace");
        assert_eq!(trace.queries().len(), 1);
        let query = &trace.queries()[0];
        assert_eq!(query.source, QuerySource::Raw);
        assert_eq!(query.sql, "SELECT 1 AS n");
        assert!(query.success);
        assert!(query.duration_ms >= 1);

        let _ = fs::remove_dir_all(root);
    }
}
