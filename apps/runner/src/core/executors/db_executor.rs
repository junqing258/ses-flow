use std::env;
use std::time::{Duration, Instant};

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use serde_json::{Map, Number, Value, json};
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{Column, Row, TypeInfo};
use tokio::runtime::{Builder, Handle};

use super::{NodeExecutor, resolve_mapping};
use crate::core::definition::{NodeDefinition, NodeType};
use crate::core::runtime::{NodeExecutionContext, NodeExecutionResult};
use crate::error::RunnerError;

pub(super) struct DbQueryExecutor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DbQueryMode {
    Read,
    Write,
}

impl DbQueryMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

#[derive(Debug)]
struct PreparedSql {
    sql: String,
    parameter_names: Vec<String>,
}

impl NodeExecutor for DbQueryExecutor {
    fn node_type(&self) -> NodeType {
        NodeType::DbQuery
    }

    fn execute(
        &self,
        node: &NodeDefinition,
        context: &NodeExecutionContext<'_>,
    ) -> Result<NodeExecutionResult, RunnerError> {
        let mode = resolve_mode(node)?;
        let sql = resolve_sql(node)?;
        validate_sql_mode(&sql, mode)?;
        let connection_url = resolve_connection_url(node)?;
        let parameters = resolve_parameters(node, context)?;
        let prepared = prepare_named_sql(&sql)?;
        let started_at = Instant::now();
        let output = execute_db_query(&connection_url, &prepared, &parameters, mode, node.timeout_ms)?;
        let duration_ms = started_at.elapsed().as_millis() as u64;

        Ok(NodeExecutionResult::success(json!({
            "mode": mode.as_str(),
            "rowCount": output.row_count,
            "rows": output.rows,
            "columns": output.columns,
            "durationMs": duration_ms
        })))
    }
}

struct DbQueryOutput {
    row_count: u64,
    rows: Value,
    columns: Value,
}

fn resolve_mode(node: &NodeDefinition) -> Result<DbQueryMode, RunnerError> {
    let mode = node
        .config
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("read")
        .trim()
        .to_ascii_lowercase();

    match mode.as_str() {
        "" | "read" => Ok(DbQueryMode::Read),
        "write" => Ok(DbQueryMode::Write),
        _ => Err(RunnerError::InvalidDbConfig(format!(
            "db_query node config.mode must be read or write, got {mode}"
        ))),
    }
}

fn resolve_sql(node: &NodeDefinition) -> Result<String, RunnerError> {
    let sql = node.config.get("sql").and_then(Value::as_str).unwrap_or("").trim();

    if sql.is_empty() {
        return Err(RunnerError::InvalidDbConfig(
            "db_query node config.sql is required".to_string(),
        ));
    }

    Ok(sql.to_string())
}

fn resolve_connection_url(node: &NodeDefinition) -> Result<String, RunnerError> {
    let connection_key = node
        .config
        .get("connectionKey")
        .and_then(Value::as_str)
        .unwrap_or("default")
        .trim();
    let env_name = connection_key_to_env_name(connection_key);

    env::var(&env_name).map_err(|_| {
        RunnerError::InvalidDbConfig(format!(
            "db_query connectionKey {connection_key:?} requires environment variable {env_name}"
        ))
    })
}

fn connection_key_to_env_name(connection_key: &str) -> String {
    let normalized = connection_key
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    format!(
        "SES_FLOW_DB_{}_URL",
        if normalized.is_empty() {
            "DEFAULT".to_string()
        } else {
            normalized
        }
    )
}

fn resolve_parameters(
    node: &NodeDefinition,
    context: &NodeExecutionContext<'_>,
) -> Result<Map<String, Value>, RunnerError> {
    let parameters = resolve_mapping(node, context);

    match parameters {
        Value::Null => Ok(Map::new()),
        Value::Object(map) => Ok(map),
        _ => Err(RunnerError::InvalidDbConfig(
            "db_query inputMapping must resolve to an object".to_string(),
        )),
    }
}

fn validate_sql_mode(sql: &str, mode: DbQueryMode) -> Result<(), RunnerError> {
    let keyword = first_sql_keyword(sql)
        .ok_or_else(|| RunnerError::InvalidDbConfig("db_query config.sql must contain a statement".to_string()))?;

    match mode {
        DbQueryMode::Read if keyword == "select" || keyword == "with" => Ok(()),
        DbQueryMode::Read => Err(RunnerError::InvalidDbConfig(format!(
            "db_query read mode only supports select/with statements, got {keyword}"
        ))),
        DbQueryMode::Write if matches!(keyword.as_str(), "insert" | "update" | "delete") => Ok(()),
        DbQueryMode::Write => Err(RunnerError::InvalidDbConfig(format!(
            "db_query write mode only supports insert/update/delete statements, got {keyword}"
        ))),
    }
}

fn first_sql_keyword(sql: &str) -> Option<String> {
    strip_leading_sql_comments(sql)
        .split(|character: char| !character.is_ascii_alphabetic())
        .find(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
}

fn strip_leading_sql_comments(mut sql: &str) -> &str {
    loop {
        let trimmed = sql.trim_start();
        if let Some(rest) = trimmed.strip_prefix("--") {
            sql = rest.split_once('\n').map(|(_, after)| after).unwrap_or("");
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("/*") {
            sql = rest.split_once("*/").map(|(_, after)| after).unwrap_or("");
            continue;
        }

        return trimmed;
    }
}

fn execute_db_query(
    connection_url: &str,
    prepared: &PreparedSql,
    parameters: &Map<String, Value>,
    mode: DbQueryMode,
    timeout_ms: Option<u64>,
) -> Result<DbQueryOutput, RunnerError> {
    let operation = async {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(connection_url)
            .await
            .map_err(|error| RunnerError::DbQuery(format!("failed to connect PostgreSQL: {error}")))?;

        match mode {
            DbQueryMode::Read => {
                let rows = bind_parameters(sqlx::query(&prepared.sql), &prepared.parameter_names, parameters)?
                    .fetch_all(&pool)
                    .await
                    .map_err(|error| RunnerError::DbQuery(error.to_string()))?;
                let columns = rows.first().map(row_columns).unwrap_or_else(|| json!([]));
                let row_count = rows.len() as u64;
                let rows = Value::Array(rows.iter().map(row_to_json).collect::<Result<Vec<_>, _>>()?);

                Ok(DbQueryOutput {
                    row_count,
                    rows,
                    columns,
                })
            }
            DbQueryMode::Write => {
                if sql_has_returning_clause(&prepared.sql) {
                    let rows = bind_parameters(sqlx::query(&prepared.sql), &prepared.parameter_names, parameters)?
                        .fetch_all(&pool)
                        .await
                        .map_err(|error| RunnerError::DbQuery(error.to_string()))?;
                    let columns = rows.first().map(row_columns).unwrap_or_else(|| json!([]));
                    let row_count = rows.len() as u64;
                    let rows = Value::Array(rows.iter().map(row_to_json).collect::<Result<Vec<_>, _>>()?);

                    return Ok(DbQueryOutput {
                        row_count,
                        rows,
                        columns,
                    });
                }

                let result = bind_parameters(sqlx::query(&prepared.sql), &prepared.parameter_names, parameters)?
                    .execute(&pool)
                    .await
                    .map_err(|error| RunnerError::DbQuery(error.to_string()))?;

                Ok(DbQueryOutput {
                    row_count: result.rows_affected(),
                    rows: json!([]),
                    columns: json!([]),
                })
            }
        }
    };
    let future = async {
        if let Some(timeout_ms) = timeout_ms {
            return tokio::time::timeout(Duration::from_millis(timeout_ms), operation)
                .await
                .map_err(|_| RunnerError::DbQuery(format!("db_query exceeded timeout of {timeout_ms}ms")))?;
        }

        operation.await
    };

    match Handle::try_current() {
        Ok(handle) => handle.block_on(future),
        Err(_) => Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| RunnerError::DbQuery(error.to_string()))?
            .block_on(future),
    }
}

fn bind_parameters<'q>(
    mut query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    parameter_names: &[String],
    parameters: &'q Map<String, Value>,
) -> Result<sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>, RunnerError> {
    for name in parameter_names {
        let Some(value) = parameters.get(name) else {
            return Err(RunnerError::InvalidDbConfig(format!(
                "db_query missing SQL parameter :{name}"
            )));
        };
        query = bind_json_value(query, value)?;
    }

    Ok(query)
}

fn bind_json_value<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    value: &'q Value,
) -> Result<sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>, RunnerError> {
    Ok(match value {
        Value::Null => query.bind(None::<String>),
        Value::Bool(value) => query.bind(*value),
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                query.bind(value)
            } else if let Some(value) = number.as_u64() {
                let value = i64::try_from(value).map_err(|_| {
                    RunnerError::InvalidDbConfig(format!("db_query numeric parameter {number} exceeds i64"))
                })?;
                query.bind(value)
            } else if let Some(value) = number.as_f64() {
                query.bind(value)
            } else {
                return Err(RunnerError::InvalidDbConfig(format!(
                    "db_query unsupported numeric parameter {number}"
                )));
            }
        }
        Value::String(value) => query.bind(value),
        Value::Array(_) | Value::Object(_) => query.bind(sqlx::types::Json(value)),
    })
}

fn row_columns(row: &PgRow) -> Value {
    Value::Array(
        row.columns()
            .iter()
            .map(|column| {
                json!({
                    "name": column.name(),
                    "type": column.type_info().name()
                })
            })
            .collect(),
    )
}

fn row_to_json(row: &PgRow) -> Result<Value, RunnerError> {
    let mut output = Map::new();

    for (index, column) in row.columns().iter().enumerate() {
        output.insert(
            column.name().to_string(),
            column_value_to_json(row, index, column.type_info().name())?,
        );
    }

    Ok(Value::Object(output))
}

fn column_value_to_json(row: &PgRow, index: usize, type_name: &str) -> Result<Value, RunnerError> {
    let normalized_type = type_name.trim_start_matches('_').to_ascii_lowercase();
    let value = match normalized_type.as_str() {
        "bool" => row
            .try_get::<Option<bool>, _>(index)
            .map(option_to_json)
            .map_err(db_decode_error)?,
        "int2" => row
            .try_get::<Option<i16>, _>(index)
            .map(option_number_to_json)
            .map_err(db_decode_error)?,
        "int4" => row
            .try_get::<Option<i32>, _>(index)
            .map(option_number_to_json)
            .map_err(db_decode_error)?,
        "int8" => row
            .try_get::<Option<i64>, _>(index)
            .map(option_number_to_json)
            .map_err(db_decode_error)?,
        "float4" => row
            .try_get::<Option<f32>, _>(index)
            .map(option_float_to_json)
            .map_err(db_decode_error)?,
        "float8" | "numeric" => row
            .try_get::<Option<f64>, _>(index)
            .map(option_float_to_json)
            .map_err(db_decode_error)?,
        "json" | "jsonb" => row
            .try_get::<Option<Value>, _>(index)
            .map(|value| value.unwrap_or(Value::Null))
            .map_err(db_decode_error)?,
        "timestamptz" => row
            .try_get::<Option<DateTime<Utc>>, _>(index)
            .map(|value| {
                value
                    .map(|timestamp| json!(timestamp.to_rfc3339()))
                    .unwrap_or(Value::Null)
            })
            .map_err(db_decode_error)?,
        "timestamp" => row
            .try_get::<Option<NaiveDateTime>, _>(index)
            .map(|value| {
                value
                    .map(|timestamp| json!(timestamp.to_string()))
                    .unwrap_or(Value::Null)
            })
            .map_err(db_decode_error)?,
        "date" => row
            .try_get::<Option<NaiveDate>, _>(index)
            .map(|value| value.map(|date| json!(date.to_string())).unwrap_or(Value::Null))
            .map_err(db_decode_error)?,
        "time" => row
            .try_get::<Option<NaiveTime>, _>(index)
            .map(|value| value.map(|time| json!(time.to_string())).unwrap_or(Value::Null))
            .map_err(db_decode_error)?,
        _ => row
            .try_get::<Option<String>, _>(index)
            .map(|value| value.map(Value::String).unwrap_or(Value::Null))
            .map_err(db_decode_error)?,
    };

    Ok(value)
}

fn option_to_json(value: Option<bool>) -> Value {
    value.map(Value::Bool).unwrap_or(Value::Null)
}

fn option_number_to_json<T>(value: Option<T>) -> Value
where
    Number: From<T>,
{
    value
        .map(|number| Value::Number(Number::from(number)))
        .unwrap_or(Value::Null)
}

fn option_float_to_json<T: Into<f64>>(value: Option<T>) -> Value {
    value
        .and_then(|number| Number::from_f64(number.into()))
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn db_decode_error(error: sqlx::Error) -> RunnerError {
    RunnerError::DbQuery(format!("failed to decode PostgreSQL row: {error}"))
}

fn sql_has_returning_clause(sql: &str) -> bool {
    sql_without_literals_and_comments(sql)
        .split(|character: char| !character.is_ascii_alphabetic())
        .any(|token| token.eq_ignore_ascii_case("returning"))
}

fn prepare_named_sql(sql: &str) -> Result<PreparedSql, RunnerError> {
    let mut output = String::with_capacity(sql.len());
    let mut parameter_names = Vec::new();
    let mut chars = sql.char_indices().peekable();
    let mut bind_index = 1usize;

    while let Some((_, character)) = chars.next() {
        match character {
            '\'' => {
                output.push(character);
                copy_single_quoted_string(&mut chars, &mut output);
            }
            '"' => {
                output.push(character);
                copy_double_quoted_identifier(&mut chars, &mut output);
            }
            '-' if chars.peek().is_some_and(|(_, next)| *next == '-') => {
                output.push(character);
                if let Some((_, next)) = chars.next() {
                    output.push(next);
                }
                copy_line_comment(&mut chars, &mut output);
            }
            '/' if chars.peek().is_some_and(|(_, next)| *next == '*') => {
                output.push(character);
                if let Some((_, next)) = chars.next() {
                    output.push(next);
                }
                copy_block_comment(&mut chars, &mut output);
            }
            ':' if chars.peek().is_some_and(|(_, next)| is_parameter_start(*next)) => {
                let mut name = String::new();
                while let Some((_, next)) = chars.peek().copied() {
                    if !is_parameter_continue(next) {
                        break;
                    }
                    name.push(next);
                    chars.next();
                }
                output.push('$');
                output.push_str(&bind_index.to_string());
                bind_index += 1;
                parameter_names.push(name);
            }
            _ => output.push(character),
        }
    }

    Ok(PreparedSql {
        sql: output,
        parameter_names,
    })
}

fn is_parameter_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_parameter_continue(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn copy_single_quoted_string<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = (usize, char)>,
{
    while let Some((_, character)) = chars.next() {
        output.push(character);
        if character == '\'' {
            if chars.peek().is_some_and(|(_, next)| *next == '\'') {
                if let Some((_, escaped)) = chars.next() {
                    output.push(escaped);
                }
                continue;
            }
            break;
        }
    }
}

fn copy_double_quoted_identifier<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = (usize, char)>,
{
    while let Some((_, character)) = chars.next() {
        output.push(character);
        if character == '"' {
            if chars.peek().is_some_and(|(_, next)| *next == '"') {
                if let Some((_, escaped)) = chars.next() {
                    output.push(escaped);
                }
                continue;
            }
            break;
        }
    }
}

fn copy_line_comment<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = (usize, char)>,
{
    for (_, character) in chars.by_ref() {
        output.push(character);
        if character == '\n' {
            break;
        }
    }
}

fn copy_block_comment<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = (usize, char)>,
{
    let mut previous = '\0';
    for (_, character) in chars.by_ref() {
        output.push(character);
        if previous == '*' && character == '/' {
            break;
        }
        previous = character;
    }
}

fn sql_without_literals_and_comments(sql: &str) -> String {
    let mut output = String::with_capacity(sql.len());
    let mut chars = sql.char_indices().peekable();

    while let Some((_, character)) = chars.next() {
        match character {
            '\'' => copy_masked_single_quoted_string(&mut chars, &mut output),
            '"' => copy_masked_double_quoted_identifier(&mut chars, &mut output),
            '-' if chars.peek().is_some_and(|(_, next)| *next == '-') => {
                chars.next();
                copy_masked_line_comment(&mut chars, &mut output);
            }
            '/' if chars.peek().is_some_and(|(_, next)| *next == '*') => {
                chars.next();
                copy_masked_block_comment(&mut chars, &mut output);
            }
            _ => output.push(character),
        }
    }

    output
}

fn copy_masked_single_quoted_string<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = (usize, char)>,
{
    output.push(' ');
    while let Some((_, character)) = chars.next() {
        output.push(' ');
        if character == '\'' {
            if chars.peek().is_some_and(|(_, next)| *next == '\'') {
                chars.next();
                output.push(' ');
                continue;
            }
            break;
        }
    }
}

fn copy_masked_double_quoted_identifier<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = (usize, char)>,
{
    output.push(' ');
    while let Some((_, character)) = chars.next() {
        output.push(' ');
        if character == '"' {
            if chars.peek().is_some_and(|(_, next)| *next == '"') {
                chars.next();
                output.push(' ');
                continue;
            }
            break;
        }
    }
}

fn copy_masked_line_comment<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = (usize, char)>,
{
    output.push(' ');
    output.push(' ');
    for (_, character) in chars.by_ref() {
        output.push(if character == '\n' { '\n' } else { ' ' });
        if character == '\n' {
            break;
        }
    }
}

fn copy_masked_block_comment<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = (usize, char)>,
{
    output.push(' ');
    output.push(' ');
    let mut previous = '\0';
    for (_, character) in chars.by_ref() {
        output.push(' ');
        if previous == '*' && character == '/' {
            break;
        }
        previous = character;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_named_sql_and_ignores_literals_and_comments() {
        let prepared = prepare_named_sql(
            "select :order_no, ':ignored', \"schema:ignored\" -- :comment\n/* :block */ and :warehouse_id",
        )
        .expect("SQL should prepare");

        assert_eq!(
            prepared.sql,
            "select $1, ':ignored', \"schema:ignored\" -- :comment\n/* :block */ and $2"
        );
        assert_eq!(prepared.parameter_names, vec!["order_no", "warehouse_id"]);
    }

    #[test]
    fn converts_connection_keys_to_whitelisted_env_names() {
        assert_eq!(connection_key_to_env_name("default"), "SES_FLOW_DB_DEFAULT_URL");
        assert_eq!(
            connection_key_to_env_name("tenant-a.orders"),
            "SES_FLOW_DB_TENANT_A_ORDERS_URL"
        );
    }

    #[test]
    fn detects_returning_outside_literals_and_comments() {
        assert!(sql_has_returning_clause(
            "insert into orders(id) values(:id) returning id"
        ));
        assert!(!sql_has_returning_clause(
            "insert into orders(note) values('returning') -- returning id"
        ));
    }

    #[test]
    fn validates_mode_keywords() {
        assert!(validate_sql_mode("select 1", DbQueryMode::Read).is_ok());
        assert!(validate_sql_mode("with rows as (select 1) select * from rows", DbQueryMode::Read).is_ok());
        assert!(validate_sql_mode("insert into t values (1)", DbQueryMode::Write).is_ok());
        assert!(validate_sql_mode("delete from t", DbQueryMode::Read).is_err());
        assert!(validate_sql_mode("select 1", DbQueryMode::Write).is_err());
    }
}
