use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, Registry};

use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{LogFormat, TelemetryConfig};
use crate::error::TelemetryError;

/// Set once [`Telemetry::init`] succeeds. [`TelemetryModule`](crate::TelemetryModule)
/// reads it to fail fast rather than register no-op telemetry providers when an
/// app forgot to initialise.
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Whether [`Telemetry::init`] / [`Telemetry::init_with`] has run successfully.
pub(crate) fn initialized() -> bool {
    INITIALIZED.load(Ordering::Relaxed)
}

fn mark_initialized() {
    INITIALIZED.store(true, Ordering::Relaxed);
}

/// Active telemetry instance. Returned by [`Telemetry::init`] and dropped at
/// the end of `main` — Drop synchronously flushes pending traces, metrics and
/// logs so trailing telemetry isn't lost on shutdown.
///
/// Keep the binding alive for the whole program: `let _telemetry =
/// Telemetry::init("api")?;`.
pub struct Telemetry {
    #[cfg(feature = "otlp")]
    tracer_provider: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
    #[cfg(feature = "otlp")]
    meter_provider: Option<opentelemetry_sdk::metrics::SdkMeterProvider>,
    #[cfg(feature = "otlp")]
    logger_provider: Option<opentelemetry_sdk::logs::SdkLoggerProvider>,
}

impl Telemetry {
    /// Shortcut: reads `NESTRS_*` env vars (see [`TelemetryConfig`]) and
    /// wires the console layer plus the OTel tracer. The batch exporters
    /// for traces/metrics/logs are added only when an OTLP endpoint is set;
    /// the tracer itself is installed unconditionally so `trace_id` values
    /// and `traceparent` propagation work out of the box.
    pub fn init(service_name: impl Into<String>) -> Result<Self, TelemetryError> {
        Self::init_with(TelemetryConfig::from_env(service_name))
    }

    /// Console-only tracing for tests: installs a fmt subscriber (once) and marks
    /// telemetry initialised, so an app importing
    /// [`TelemetryModule`](crate::TelemetryModule) boots without the OTLP exporter
    /// stack. **Idempotent** — safe to call from every test; the first call wins
    /// and later calls are no-ops (and a subscriber already set by another harness
    /// is tolerated). Returns no guard: a test does not flush exporters. The log
    /// level honours `RUST_LOG`, defaulting to `warn` to keep test output quiet.
    pub fn init_for_tests() {
        if initialized() {
            return;
        }
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
        let _ = Registry::default()
            .with(filter)
            .with(console_layer(LogFormat::Text))
            .try_init();
        mark_initialized();
    }

    pub fn init_with(config: TelemetryConfig) -> Result<Self, TelemetryError> {
        let filter =
            EnvFilter::try_new(&config.log_filter).unwrap_or_else(|_| EnvFilter::new("info"));
        let fmt_layer = console_layer(config.log_format);

        #[cfg(feature = "otlp")]
        {
            let exporters = crate::otlp::build(&config)?;
            let otel_layer = tracing_opentelemetry::layer().with_tracer(exporters.tracer);

            // The OTel log appender is only worth wiring when an exporter is
            // present; without one it would just drop events while paying the
            // per-event bridge cost.
            let appender_layer = exporters.logger_provider.as_ref().map(|lp| {
                let f = EnvFilter::try_new(&config.log_filter)
                    .unwrap_or_else(|_| EnvFilter::new("info"));
                opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(lp)
                    .with_filter(f)
            });

            Registry::default()
                .with(filter)
                .with(fmt_layer)
                .with(otel_layer)
                .with(appender_layer)
                .try_init()
                .map_err(|e| TelemetryError::Init(e.to_string()))?;
            mark_initialized();

            tracing::info!(
                service = %config.service_name,
                endpoint = config.otlp_endpoint.as_deref().unwrap_or("<none>"),
                sample_ratio = config.trace_sample_ratio,
                log_format = ?config.log_format,
                otlp_export = exporters.meter_provider.is_some(),
                "telemetry initialised"
            );

            Ok(Telemetry {
                tracer_provider: Some(exporters.tracer_provider),
                meter_provider: exporters.meter_provider,
                logger_provider: exporters.logger_provider,
            })
        }

        #[cfg(not(feature = "otlp"))]
        {
            Registry::default()
                .with(filter)
                .with(fmt_layer)
                .try_init()
                .map_err(|e| TelemetryError::Init(e.to_string()))?;
            mark_initialized();
            tracing::info!(
                service = %config.service_name,
                log_format = ?config.log_format,
                "telemetry initialised (console only)"
            );
            Ok(Telemetry {})
        }
    }
}

/// Boxed so the `text` and `json` layers — which have distinct concrete
/// types — can flow through the same `Registry` chain. The OTel span
/// itself is *not* rendered (`FmtSpan::NONE`, the default); only the
/// explicit access-log event emitted by `OtelHttp` shows up in the console.
fn console_layer<S>(format: LogFormat) -> Box<dyn Layer<S> + Send + Sync + 'static>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    match format {
        LogFormat::Text => tracing_subscriber::fmt::layer().boxed(),
        LogFormat::Json => tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(false)
            .with_span_list(false)
            .boxed(),
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        #[cfg(feature = "otlp")]
        {
            if let Some(p) = self.tracer_provider.take() {
                let _ = p.shutdown();
            }
            if let Some(p) = self.meter_provider.take() {
                let _ = p.shutdown();
            }
            if let Some(p) = self.logger_provider.take() {
                let _ = p.shutdown();
            }
        }
    }
}
