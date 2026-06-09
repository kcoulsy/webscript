use crate::diagnostic::{Diagnostic, Span};
use crate::parser::Value;
use std::collections::BTreeMap;

pub type Env = BTreeMap<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Literal(Value),
    Path(Vec<String>),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Or,
    And,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Add,
    Sub,
}

pub fn parse(source: &str, line: usize, column: usize) -> Result<Expr, Diagnostic> {
    let tokens = tokenize(source, line, column)?;
    let mut parser = ExprParser { tokens, index: 0 };
    let expr = parser.parse_or()?;

    if let Some(token) = parser.peek() {
        return Err(Diagnostic::error(
            token.span.clone(),
            format!("unexpected token `{}` in expression", token.lexeme),
            None,
        ));
    }

    Ok(expr)
}

pub fn evaluate(expr: &Expr, env: &Env, line: usize, column: usize) -> Result<Value, Diagnostic> {
    match expr {
        Expr::Literal(value) => Ok(value.clone()),
        Expr::Path(path) => evaluate_path(path, env, line, column),
        Expr::Unary { op, expr } => {
            let value = evaluate(expr, env, line, column)?;
            match op {
                UnaryOp::Not => match value {
                    Value::Bool(value) => Ok(Value::Bool(!value)),
                    other => Err(type_error(
                        line,
                        column,
                        "operator `!` expects `bool`",
                        &other.type_name(),
                    )),
                },
            }
        }
        Expr::Binary { left, op, right } => {
            let left = evaluate(left, env, line, column)?;

            match op {
                BinaryOp::Or => {
                    let left = expect_bool(left, line, column, "operator `||`")?;
                    if left {
                        return Ok(Value::Bool(true));
                    }
                    let right = expect_bool(
                        evaluate(right, env, line, column)?,
                        line,
                        column,
                        "operator `||`",
                    )?;
                    Ok(Value::Bool(right))
                }
                BinaryOp::And => {
                    let left = expect_bool(left, line, column, "operator `&&`")?;
                    if !left {
                        return Ok(Value::Bool(false));
                    }
                    let right = expect_bool(
                        evaluate(right, env, line, column)?,
                        line,
                        column,
                        "operator `&&`",
                    )?;
                    Ok(Value::Bool(right))
                }
                _ => {
                    let right = evaluate(right, env, line, column)?;
                    evaluate_binary(left, *op, right, line, column)
                }
            }
        }
    }
}

fn evaluate_path(
    path: &[String],
    env: &Env,
    line: usize,
    column: usize,
) -> Result<Value, Diagnostic> {
    let Some(first) = path.first() else {
        return Err(Diagnostic::error(
            Span::new(line, column, column),
            "empty expression",
            None,
        ));
    };

    let Some(mut value) = env.get(first).cloned() else {
        return Err(Diagnostic::error(
            Span::identifier(line, column, first),
            format!("unknown expression `{first}`"),
            None,
        ));
    };

    for field in &path[1..] {
        value = match value {
            Value::Object(fields) => {
                let Some(field_value) = fields.get(field).cloned() else {
                    return Err(Diagnostic::error(
                        Span::identifier(line, column, &path.join(".")),
                        format!("unknown property `{field}` on `{}`", path.join(".")),
                        None,
                    ));
                };
                field_value
            }
            other => {
                return Err(Diagnostic::error(
                    Span::identifier(line, column, &path.join(".")),
                    format!(
                        "property access `{}` requires `object`, found `{}`",
                        path.join("."),
                        other.type_name()
                    ),
                    Some("only object values have fields".to_string()),
                ));
            }
        };
    }

    Ok(value)
}

fn evaluate_binary(
    left: Value,
    op: BinaryOp,
    right: Value,
    line: usize,
    column: usize,
) -> Result<Value, Diagnostic> {
    match op {
        BinaryOp::Eq => Ok(Value::Bool(left == right)),
        BinaryOp::NotEq => Ok(Value::Bool(left != right)),
        BinaryOp::Add => match (left, right) {
            (Value::Int(left), Value::Int(right)) => Ok(Value::Int(left + right)),
            (Value::String(left), Value::String(right)) => {
                Ok(Value::String(format!("{left}{right}")))
            }
            (Value::String(left), right) => Ok(Value::String(format!("{left}{}", right.render()))),
            (left, Value::String(right)) => Ok(Value::String(format!("{}{right}", left.render()))),
            (left, right) => Err(binary_type_error(line, column, "+", &left, &right)),
        },
        BinaryOp::Sub => match (left, right) {
            (Value::Int(left), Value::Int(right)) => Ok(Value::Int(left - right)),
            (left, right) => Err(binary_type_error(line, column, "-", &left, &right)),
        },
        BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => match (left, right) {
            (Value::Int(left), Value::Int(right)) => {
                let value = match op {
                    BinaryOp::Lt => left < right,
                    BinaryOp::LtEq => left <= right,
                    BinaryOp::Gt => left > right,
                    BinaryOp::GtEq => left >= right,
                    _ => unreachable!(),
                };
                Ok(Value::Bool(value))
            }
            (left, right) => Err(binary_type_error(
                line,
                column,
                comparison_name(op),
                &left,
                &right,
            )),
        },
        BinaryOp::Or | BinaryOp::And => unreachable!("handled with short-circuiting"),
    }
}

fn expect_bool(
    value: Value,
    line: usize,
    column: usize,
    context: &str,
) -> Result<bool, Diagnostic> {
    match value {
        Value::Bool(value) => Ok(value),
        other => Err(type_error(
            line,
            column,
            format!("{context} expects `bool`"),
            &other.type_name(),
        )),
    }
}

fn type_error(line: usize, column: usize, message: impl Into<String>, found: &str) -> Diagnostic {
    Diagnostic::error(
        Span::new(line, column, column + 1),
        message,
        Some(format!("found `{found}`")),
    )
}

fn binary_type_error(
    line: usize,
    column: usize,
    op: &str,
    left: &Value,
    right: &Value,
) -> Diagnostic {
    Diagnostic::error(
        Span::new(line, column, column + 1),
        format!(
            "operator `{op}` cannot be applied to `{}` and `{}`",
            left.type_name(),
            right.type_name()
        ),
        None,
    )
}

fn comparison_name(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Lt => "<",
        BinaryOp::LtEq => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::GtEq => ">=",
        _ => unreachable!(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
    lexeme: String,
    span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Identifier,
    String,
    Int,
    True,
    False,
    Bang,
    Plus,
    Minus,
    Dot,
    Colon,
    Comma,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    OrOr,
    AndAnd,
    EqEq,
    BangEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
}

fn tokenize(source: &str, line: usize, column: usize) -> Result<Vec<Token>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut index = 0;
    let chars: Vec<char> = source.chars().collect();

    while index < chars.len() {
        let char = chars[index];
        let col = column + index;

        if char.is_whitespace() {
            index += 1;
            continue;
        }

        if char.is_ascii_alphabetic() || char == '_' {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
            {
                index += 1;
            }
            let lexeme: String = chars[start..index].iter().collect();
            let kind = match lexeme.as_str() {
                "true" => TokenKind::True,
                "false" => TokenKind::False,
                _ => TokenKind::Identifier,
            };
            tokens.push(Token {
                kind,
                span: Span::new(line, column + start, column + index),
                lexeme,
            });
            continue;
        }

        if char.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Int,
                span: Span::new(line, column + start, column + index),
                lexeme: chars[start..index].iter().collect(),
            });
            continue;
        }

        if char == '"' {
            let start = index;
            index += 1;
            let mut escaped = false;
            while index < chars.len() {
                let current = chars[index];
                if current == '"' && !escaped {
                    index += 1;
                    tokens.push(Token {
                        kind: TokenKind::String,
                        span: Span::new(line, column + start, column + index),
                        lexeme: chars[start..index].iter().collect(),
                    });
                    break;
                }
                escaped = current == '\\' && !escaped;
                if current != '\\' {
                    escaped = false;
                }
                index += 1;
            }
            if tokens
                .last()
                .is_none_or(|token| token.span.start_col != column + start)
            {
                return Err(Diagnostic::error(
                    Span::new(line, column + start, column + source.len()),
                    "unterminated string in expression",
                    None,
                ));
            }
            continue;
        }

        let two = if index + 1 < chars.len() {
            Some((char, chars[index + 1]))
        } else {
            None
        };
        let kind = match two {
            Some(('|', '|')) => Some((TokenKind::OrOr, 2)),
            Some(('&', '&')) => Some((TokenKind::AndAnd, 2)),
            Some(('=', '=')) => Some((TokenKind::EqEq, 2)),
            Some(('!', '=')) => Some((TokenKind::BangEq, 2)),
            Some(('<', '=')) => Some((TokenKind::LtEq, 2)),
            Some(('>', '=')) => Some((TokenKind::GtEq, 2)),
            _ => match char {
                '!' => Some((TokenKind::Bang, 1)),
                '+' => Some((TokenKind::Plus, 1)),
                '-' => Some((TokenKind::Minus, 1)),
                '.' => Some((TokenKind::Dot, 1)),
                ':' => Some((TokenKind::Colon, 1)),
                ',' => Some((TokenKind::Comma, 1)),
                '(' => Some((TokenKind::LParen, 1)),
                ')' => Some((TokenKind::RParen, 1)),
                '{' => Some((TokenKind::LBrace, 1)),
                '}' => Some((TokenKind::RBrace, 1)),
                '[' => Some((TokenKind::LBracket, 1)),
                ']' => Some((TokenKind::RBracket, 1)),
                '<' => Some((TokenKind::Lt, 1)),
                '>' => Some((TokenKind::Gt, 1)),
                _ => None,
            },
        };

        let Some((kind, len)) = kind else {
            return Err(Diagnostic::error(
                Span::new(line, col, col + 1),
                format!("unexpected character `{char}` in expression"),
                None,
            ));
        };
        tokens.push(Token {
            kind,
            span: Span::new(line, col, col + len),
            lexeme: chars[index..index + len].iter().collect(),
        });
        index += len;
    }

    Ok(tokens)
}

struct ExprParser {
    tokens: Vec<Token>,
    index: usize,
}

impl ExprParser {
    fn parse_or(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_binary(Self::parse_and, &[TokenKind::OrOr])
    }

    fn parse_and(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_binary(Self::parse_equality, &[TokenKind::AndAnd])
    }

    fn parse_equality(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_binary(
            Self::parse_comparison,
            &[TokenKind::EqEq, TokenKind::BangEq],
        )
    }

    fn parse_comparison(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_binary(
            Self::parse_additive,
            &[
                TokenKind::Lt,
                TokenKind::LtEq,
                TokenKind::Gt,
                TokenKind::GtEq,
            ],
        )
    }

    fn parse_additive(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_binary(Self::parse_unary, &[TokenKind::Plus, TokenKind::Minus])
    }

    fn parse_binary(
        &mut self,
        next: fn(&mut Self) -> Result<Expr, Diagnostic>,
        kinds: &[TokenKind],
    ) -> Result<Expr, Diagnostic> {
        let mut expr = next(self)?;
        while let Some(token) = self.peek() {
            if !kinds.contains(&token.kind) {
                break;
            }
            let op = binary_op(token.kind);
            self.index += 1;
            let right = next(self)?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.match_token(TokenKind::Bang) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let Some(token) = self.advance().cloned() else {
            return Err(Diagnostic::error(
                Span::new(1, 1, 1),
                "expected expression",
                None,
            ));
        };

        match token.kind {
            TokenKind::String => Ok(Expr::Literal(Value::String(
                token.lexeme[1..token.lexeme.len() - 1].to_string(),
            ))),
            TokenKind::Int => Ok(Expr::Literal(Value::Int(
                token.lexeme.parse().expect("int token contains digits"),
            ))),
            TokenKind::True => Ok(Expr::Literal(Value::Bool(true))),
            TokenKind::False => Ok(Expr::Literal(Value::Bool(false))),
            TokenKind::Identifier => {
                let mut path = vec![token.lexeme.clone()];
                while self.match_token(TokenKind::Dot) {
                    let Some(next) = self.advance().cloned() else {
                        return Err(Diagnostic::error(
                            token.span.clone(),
                            "expected property name after `.`",
                            None,
                        ));
                    };
                    if next.kind != TokenKind::Identifier {
                        return Err(Diagnostic::error(
                            next.span,
                            "expected property name after `.`",
                            None,
                        ));
                    }
                    path.push(next.lexeme);
                }
                Ok(Expr::Path(path))
            }
            TokenKind::LParen => {
                let expr = self.parse_or()?;
                self.expect(TokenKind::RParen, "expected `)`", token.span.clone())?;
                Ok(expr)
            }
            TokenKind::LBrace => self.parse_object_literal(token.span),
            TokenKind::LBracket => self.parse_array_literal(token.span),
            _ => Err(Diagnostic::error(
                token.span,
                format!("expected expression, found `{}`", token.lexeme),
                None,
            )),
        }
    }

    fn parse_object_literal(&mut self, open_span: Span) -> Result<Expr, Diagnostic> {
        let mut fields = BTreeMap::new();

        if self.match_token(TokenKind::RBrace) {
            return Ok(Expr::Literal(Value::Object(fields)));
        }

        loop {
            let Some(name) = self.advance().cloned() else {
                return Err(Diagnostic::error(
                    open_span,
                    "expected object field name",
                    None,
                ));
            };
            if name.kind != TokenKind::Identifier {
                return Err(Diagnostic::error(
                    name.span,
                    "expected object field name",
                    None,
                ));
            }

            self.expect(
                TokenKind::Colon,
                "expected `:` after object field name",
                name.span,
            )?;
            let value = self.parse_or()?;
            let Expr::Literal(value) = value else {
                return Err(Diagnostic::error(
                    self.previous_span(),
                    "object literal fields must be literal values",
                    Some("field expressions will come with helper functions".to_string()),
                ));
            };
            fields.insert(name.lexeme, value);

            if self.match_token(TokenKind::Comma) {
                if self.match_token(TokenKind::RBrace) {
                    break;
                }
                continue;
            }

            if self
                .peek()
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            {
                continue;
            }

            self.expect(
                TokenKind::RBrace,
                "expected `}` after object literal",
                open_span,
            )?;
            break;
        }

        Ok(Expr::Literal(Value::Object(fields)))
    }

    fn parse_array_literal(&mut self, open_span: Span) -> Result<Expr, Diagnostic> {
        let mut values = Vec::new();

        if self.match_token(TokenKind::RBracket) {
            return Ok(Expr::Literal(Value::Array {
                element_type: "unknown".to_string(),
                values,
            }));
        }

        loop {
            let value = self.parse_or()?;
            let Expr::Literal(value) = value else {
                return Err(Diagnostic::error(
                    self.previous_span(),
                    "array literal items must be literal values",
                    Some("array expressions will come with helper functions".to_string()),
                ));
            };
            values.push(value);

            if self.match_token(TokenKind::Comma) {
                if self.match_token(TokenKind::RBracket) {
                    break;
                }
                continue;
            }

            self.expect(
                TokenKind::RBracket,
                "expected `]` after array literal",
                open_span,
            )?;
            break;
        }

        let element_type = infer_array_element_type(&values);
        Ok(Expr::Literal(Value::Array {
            element_type,
            values,
        }))
    }

    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.peek().is_some_and(|token| token.kind == kind) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.index);
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    fn expect(
        &mut self,
        kind: TokenKind,
        message: &str,
        fallback_span: Span,
    ) -> Result<Token, Diagnostic> {
        let Some(token) = self.advance().cloned() else {
            return Err(Diagnostic::error(fallback_span, message, None));
        };
        if token.kind != kind {
            return Err(Diagnostic::error(token.span, message, None));
        }
        Ok(token)
    }

    fn previous_span(&self) -> Span {
        self.tokens
            .get(self.index.saturating_sub(1))
            .map(|token| token.span.clone())
            .unwrap_or_else(|| Span::new(1, 1, 1))
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }
}

fn infer_array_element_type(values: &[Value]) -> String {
    let Some(first) = values.first() else {
        return "unknown".to_string();
    };
    if values.iter().all(|value| matches!(value, Value::Object(_))) {
        return "object".to_string();
    }

    let first_type = first.type_name();
    if values
        .iter()
        .skip(1)
        .all(|value| type_names_match(&value.type_name(), &first_type))
    {
        first_type
    } else {
        "mixed".to_string()
    }
}

fn type_names_match(actual: &str, expected: &str) -> bool {
    actual == expected || (expected == "object" && actual.starts_with('{'))
}

fn binary_op(kind: TokenKind) -> BinaryOp {
    match kind {
        TokenKind::OrOr => BinaryOp::Or,
        TokenKind::AndAnd => BinaryOp::And,
        TokenKind::EqEq => BinaryOp::Eq,
        TokenKind::BangEq => BinaryOp::NotEq,
        TokenKind::Lt => BinaryOp::Lt,
        TokenKind::LtEq => BinaryOp::LtEq,
        TokenKind::Gt => BinaryOp::Gt,
        TokenKind::GtEq => BinaryOp::GtEq,
        TokenKind::Plus => BinaryOp::Add,
        TokenKind::Minus => BinaryOp::Sub,
        _ => unreachable!("not a binary operator"),
    }
}
