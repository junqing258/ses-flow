use std::env;
use std::fmt::{self as stdfmt, Write as _};
use std::sync::Once;

use chrono::Local;
use tracing::Level;
use tracing::field::Field;
use tracing::{Event, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::field::Visit;
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields, Writer};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;

static TRACING_INIT: Once = Once::new();

// 持有 guard 防止后台写入线程提前退出，调用方需保持其生命周期到进程结束
pub fn init_tracing() -> Option<WorkerGuard> {
    let mut guard = None;

    TRACING_INIT.call_once(|| {
        let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter()));
        let use_json = env::var("LOG_FORMAT")
            .map(|v| v.eq_ignore_ascii_case("json"))
            .unwrap_or(false);

        tracing_log::LogTracer::init().ok();

        let registry = tracing_subscriber::registry().with(env_filter);

        let file_writer = env::var("LOG_FILE_DIR")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .and_then(|log_dir| {
                std::fs::create_dir_all(&log_dir).ok()?;
                let prefix = env::var("LOG_FILE_PREFIX").unwrap_or_else(|_| "runner.log".to_string());
                let file_appender = tracing_appender::rolling::daily(&log_dir, &prefix);
                let (non_blocking, worker_guard) = tracing_appender::non_blocking(file_appender);
                Some((non_blocking, worker_guard))
            });

        match file_writer {
            Some((non_blocking, worker_guard)) => {
                if use_json {
                    registry
                        .with(fmt::layer().event_format(BracketedEventFormatter))
                        .with(fmt::layer().json().with_ansi(false).with_writer(non_blocking))
                        .try_init()
                        .ok();
                } else {
                    registry
                        .with(fmt::layer().event_format(BracketedEventFormatter))
                        .with(fmt::layer().with_ansi(false).with_writer(non_blocking))
                        .try_init()
                        .ok();
                }
                guard = Some(worker_guard);
            }
            None => {
                if use_json {
                    registry.with(fmt::layer().json()).try_init().ok();
                } else {
                    registry
                        .with(fmt::layer().event_format(BracketedEventFormatter))
                        .try_init()
                        .ok();
                }
            }
        }
    });

    guard
}

fn default_filter() -> String {
    env::var("BACKEND_LOG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| env::var("RUNNER_LOG").ok().filter(|value| !value.trim().is_empty()))
        .or_else(|| env::var("RUST_LOG").ok().filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(|| "backend=info,runner=info".to_string())
}

struct BracketedEventFormatter;

impl<S, N> FormatEvent<S, N> for BracketedEventFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(&self, _ctx: &FmtContext<'_, S, N>, mut writer: Writer<'_>, event: &Event<'_>) -> stdfmt::Result {
        let meta = event.metadata();
        let timestamp = format_timestamp(writer.has_ansi_escapes());
        let level = format_level(meta.level(), writer.has_ansi_escapes());

        write!(writer, "{} {} [{}]", timestamp, level, meta.target())?;

        let mut visitor = LogVisitor::default();
        event.record(&mut visitor);

        if !visitor.fields.is_empty() {
            write!(writer, " ({})", visitor.fields)?;
        }

        if let Some(message) = visitor.message {
            write!(writer, " {} ", message)?;
        }

        writeln!(writer)
    }
}

fn format_level(level: &Level, use_ansi: bool) -> String {
    let label = format!("{:<5}", level);
    if !use_ansi {
        return label;
    }

    let color = match *level {
        Level::TRACE => "\x1b[35m",
        Level::DEBUG => "\x1b[34m",
        Level::INFO => "\x1b[32m",
        Level::WARN => "\x1b[33m",
        Level::ERROR => "\x1b[31m",
    };

    format!("{color}{label}\x1b[0m")
}

fn format_timestamp(use_ansi: bool) -> String {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();
    if !use_ansi {
        return timestamp;
    }

    format!("\x1b[90m{timestamp}\x1b[0m")
}

#[derive(Default)]
struct LogVisitor {
    fields: String,
    message: Option<String>,
}

impl LogVisitor {
    fn push_field(&mut self, field: &Field, value: &dyn stdfmt::Debug) {
        if !self.fields.is_empty() {
            self.fields.push(' ');
        }

        let _ = write!(&mut self.fields, "{}={:?}", field.name(), value);
    }
}

impl Visit for LogVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.push_field(field, &value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn stdfmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{:?}", value));
        } else {
            self.push_field(field, value);
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.push_field(field, &value);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.push_field(field, &value);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.push_field(field, &value);
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.push_field(field, &value);
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.push_field(field, &value.to_string());
    }
}
