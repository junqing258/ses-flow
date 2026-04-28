use std::env;
use std::error::Error;
use std::fmt::{self as stdfmt, Write as _};
use std::sync::Once;
use std::time::Duration;

use chrono::Local;
use http::HeaderMap;
use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::propagation::Extractor;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::{BatchLogProcessor, SdkLogger, SdkLoggerProvider};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::Level;
use tracing::field::Field;
use tracing::{Event, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::field::Visit;
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields, Writer};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;

static TRACING_INIT: Once = Once::new();

pub struct TelemetryGuard {
    _log_guard: Option<WorkerGuard>,
    logger_provider: Option<SdkLoggerProvider>,
    tracer_provider: Option<SdkTracerProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.logger_provider.take() {
            let _ = provider.shutdown();
        }
        if let Some(provider) = self.tracer_provider.take() {
            let _ = provider.shutdown();
        }
    }
}

// 持有 guard 防止后台写入线程提前退出，调用方需保持其生命周期到进程结束
pub fn init_tracing() -> Option<TelemetryGuard> {
    init_tracing_with_service_name("ses-flow-backend")
}

pub fn init_tracing_with_service_name(default_service_name: &'static str) -> Option<TelemetryGuard> {
    let mut guard = None;

    TRACING_INIT.call_once(|| {
        let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter()));
        let use_json = env::var("LOG_FORMAT")
            .map(|v| v.eq_ignore_ascii_case("json"))
            .unwrap_or(false);

        tracing_log::LogTracer::init().ok();

        let registry = tracing_subscriber::registry().with(env_filter);
        let tracer_provider = init_otel_tracer_provider(default_service_name);
        let logger_provider = init_otel_logger_provider(default_service_name);

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
                    if let Some(provider) = tracer_provider.as_ref() {
                        registry
                            .with(fmt::layer().event_format(BracketedEventFormatter))
                            .with(fmt::layer().json().with_ansi(false).with_writer(non_blocking))
                            .with(otel_layer(provider))
                            .with(logger_provider.as_ref().map(log_layer))
                            .try_init()
                            .ok();
                    } else {
                        registry
                            .with(fmt::layer().event_format(BracketedEventFormatter))
                            .with(fmt::layer().json().with_ansi(false).with_writer(non_blocking))
                            .with(logger_provider.as_ref().map(log_layer))
                            .try_init()
                            .ok();
                    }
                } else {
                    if let Some(provider) = tracer_provider.as_ref() {
                        registry
                            .with(fmt::layer().event_format(BracketedEventFormatter))
                            .with(fmt::layer().with_ansi(false).with_writer(non_blocking))
                            .with(otel_layer(provider))
                            .with(logger_provider.as_ref().map(log_layer))
                            .try_init()
                            .ok();
                    } else {
                        registry
                            .with(fmt::layer().event_format(BracketedEventFormatter))
                            .with(fmt::layer().with_ansi(false).with_writer(non_blocking))
                            .with(logger_provider.as_ref().map(log_layer))
                            .try_init()
                            .ok();
                    }
                }
                guard = Some(TelemetryGuard {
                    _log_guard: Some(worker_guard),
                    logger_provider,
                    tracer_provider,
                });
            }
            None => {
                if use_json {
                    if let Some(provider) = tracer_provider.as_ref() {
                        registry
                            .with(fmt::layer().json())
                            .with(otel_layer(provider))
                            .with(logger_provider.as_ref().map(log_layer))
                            .try_init()
                            .ok();
                    } else {
                        registry
                            .with(fmt::layer().json())
                            .with(logger_provider.as_ref().map(log_layer))
                            .try_init()
                            .ok();
                    }
                } else {
                    if let Some(provider) = tracer_provider.as_ref() {
                        registry
                            .with(fmt::layer().event_format(BracketedEventFormatter))
                            .with(otel_layer(provider))
                            .with(logger_provider.as_ref().map(log_layer))
                            .try_init()
                            .ok();
                    } else {
                        registry
                            .with(fmt::layer().event_format(BracketedEventFormatter))
                            .with(logger_provider.as_ref().map(log_layer))
                            .try_init()
                            .ok();
                    }
                }
                if tracer_provider.is_some() || logger_provider.is_some() {
                    guard = Some(TelemetryGuard {
                        _log_guard: None,
                        logger_provider,
                        tracer_provider,
                    });
                }
            }
        }
    });

    guard
}

pub fn set_span_parent_from_headers(span: &tracing::Span, headers: &HeaderMap) {
    let parent_context = global::get_text_map_propagator(|propagator| propagator.extract(&HeaderExtractor(headers)));
    let _ = span.set_parent(parent_context);
}

struct HeaderExtractor<'a>(&'a HeaderMap);

impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|name| name.as_str()).collect()
    }
}

fn otel_layer<S>(
    provider: &SdkTracerProvider,
) -> tracing_opentelemetry::OpenTelemetryLayer<S, opentelemetry_sdk::trace::Tracer>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let tracer = provider.tracer("ses-flow");
    tracing_opentelemetry::layer().with_tracer(tracer)
}

fn log_layer(provider: &SdkLoggerProvider) -> OpenTelemetryTracingBridge<SdkLoggerProvider, SdkLogger> {
    OpenTelemetryTracingBridge::new(provider)
}

fn init_otel_tracer_provider(default_service_name: &'static str) -> Option<SdkTracerProvider> {
    if !env_bool("OTEL_ENABLED") {
        return None;
    }

    global::set_text_map_propagator(TraceContextPropagator::new());

    match build_otel_tracer_provider(default_service_name) {
        Ok(provider) => {
            global::set_tracer_provider(provider.clone());
            Some(provider)
        }
        Err(error) => {
            eprintln!("failed to initialize OpenTelemetry exporter: {error}");
            None
        }
    }
}

fn init_otel_logger_provider(default_service_name: &'static str) -> Option<SdkLoggerProvider> {
    if !env_bool_with_fallback("OTEL_LOGS_ENABLED", env_bool("OTEL_ENABLED")) {
        return None;
    }

    match build_otel_logger_provider(default_service_name) {
        Ok(provider) => Some(provider),
        Err(error) => {
            eprintln!("failed to initialize OpenTelemetry log exporter: {error}");
            None
        }
    }
}

fn build_otel_tracer_provider(
    default_service_name: &'static str,
) -> Result<SdkTracerProvider, Box<dyn Error + Send + Sync>> {
    let protocol = env::var("OTEL_EXPORTER_OTLP_PROTOCOL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "http/protobuf".to_string());
    let endpoint = resolve_otlp_traces_endpoint(&protocol);

    let exporter = if is_grpc_protocol(&protocol) {
        SpanExporter::builder().with_tonic().with_endpoint(endpoint).build()?
    } else {
        SpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .with_protocol(Protocol::HttpBinary)
            .with_timeout(Duration::from_secs(5))
            .build()?
    };

    Ok(SdkTracerProvider::builder()
        .with_resource(otel_resource(default_service_name))
        .with_batch_exporter(exporter)
        .build())
}

fn build_otel_logger_provider(
    default_service_name: &'static str,
) -> Result<SdkLoggerProvider, Box<dyn Error + Send + Sync>> {
    let protocol = env::var("OTEL_EXPORTER_OTLP_PROTOCOL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "http/protobuf".to_string());
    let endpoint = resolve_otlp_logs_endpoint(&protocol);

    let exporter = if is_grpc_protocol(&protocol) {
        LogExporter::builder().with_tonic().with_endpoint(endpoint).build()?
    } else {
        LogExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .with_protocol(Protocol::HttpBinary)
            .with_timeout(Duration::from_secs(5))
            .build()?
    };

    Ok(SdkLoggerProvider::builder()
        .with_resource(otel_resource(default_service_name))
        .with_log_processor(BatchLogProcessor::builder(exporter).build())
        .build())
}

fn otel_resource(default_service_name: &'static str) -> Resource {
    let service_name = env::var("OTEL_SERVICE_NAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default_service_name.to_string());
    let mut attributes = Vec::new();

    if let Ok(namespace) = env::var("OTEL_SERVICE_NAMESPACE") {
        if !namespace.trim().is_empty() {
            attributes.push(KeyValue::new("service.namespace", namespace));
        }
    }

    if let Ok(environment) = env::var("OTEL_DEPLOYMENT_ENVIRONMENT") {
        if !environment.trim().is_empty() {
            attributes.push(KeyValue::new("deployment.environment", environment));
        }
    }

    if let Ok(resource_attributes) = env::var("OTEL_RESOURCE_ATTRIBUTES") {
        attributes.extend(parse_resource_attributes(&resource_attributes));
    }

    Resource::builder()
        .with_service_name(service_name)
        .with_attributes(attributes)
        .build()
}

fn parse_resource_attributes(value: &str) -> Vec<KeyValue> {
    value
        .split(',')
        .filter_map(|entry| {
            let (key, value) = entry.split_once('=')?;
            let key = key.trim();
            let value = value.trim();
            (!key.is_empty() && !value.is_empty()).then(|| KeyValue::new(key.to_string(), value.to_string()))
        })
        .collect()
}

fn resolve_otlp_traces_endpoint(protocol: &str) -> String {
    if let Ok(endpoint) = env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT") {
        if !endpoint.trim().is_empty() {
            return endpoint;
        }
    }

    let endpoint = env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            if is_grpc_protocol(protocol) {
                "http://192.168.110.45:4317".to_string()
            } else {
                "http://192.168.110.45:4318".to_string()
            }
        });

    if is_grpc_protocol(protocol) {
        endpoint
    } else {
        normalize_http_traces_endpoint(&endpoint)
    }
}

fn resolve_otlp_logs_endpoint(protocol: &str) -> String {
    if let Ok(endpoint) = env::var("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT") {
        if !endpoint.trim().is_empty() {
            return endpoint;
        }
    }

    let endpoint = env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            if is_grpc_protocol(protocol) {
                "http://192.168.110.45:4317".to_string()
            } else {
                "http://192.168.110.45:4318".to_string()
            }
        });

    if is_grpc_protocol(protocol) {
        endpoint
    } else {
        normalize_http_signal_endpoint(&endpoint, "logs")
    }
}

fn normalize_http_traces_endpoint(endpoint: &str) -> String {
    normalize_http_signal_endpoint(endpoint, "traces")
}

fn normalize_http_signal_endpoint(endpoint: &str, signal: &str) -> String {
    let endpoint = endpoint.trim().trim_end_matches('/');
    let suffix = format!("/v1/{signal}");
    if endpoint.ends_with(&suffix) {
        endpoint.to_string()
    } else {
        format!("{endpoint}{suffix}")
    }
}

fn is_grpc_protocol(protocol: &str) -> bool {
    matches!(protocol.trim().to_ascii_lowercase().as_str(), "grpc" | "grpc/protobuf")
}

fn env_bool(key: &str) -> bool {
    env::var(key)
        .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn env_bool_with_fallback(key: &str, fallback: bool) -> bool {
    env::var(key)
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            if value.is_empty() {
                fallback
            } else {
                matches!(value.as_str(), "1" | "true" | "yes" | "on")
            }
        })
        .unwrap_or(fallback)
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
