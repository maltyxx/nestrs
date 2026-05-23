//! Configuration for the weather surface: the upstream endpoint and the HTTP
//! client timeout. Read from `NESTRS_WEATHER__*` (see [`WeatherConfig::from_env`]),
//! seeded into the container in `main`, then consumed by both the
//! `reqwest::Client` factory (timeout) and [`OpenMeteoClient`](super::service)
//! (base URL) — the two halves of NestJS's `useFactory` + config pattern.

use nestrs_core::config::env_var;

#[derive(Debug, Clone)]
pub struct WeatherConfig {
    /// Open-Meteo forecast endpoint. Override with `NESTRS_WEATHER__BASE_URL`.
    pub base_url: String,
    /// Per-request timeout for the upstream call, in milliseconds. Override with
    /// `NESTRS_WEATHER__REQUEST_TIMEOUT_MS`.
    pub request_timeout_ms: u64,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.open-meteo.com/v1/forecast".into(),
            request_timeout_ms: 10_000,
        }
    }
}

impl WeatherConfig {
    /// Read `NESTRS_WEATHER__*`, falling back to the defaults. Empty values are
    /// treated as unset.
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Some(url) = env_var("NESTRS_WEATHER__BASE_URL") {
            cfg.base_url = url;
        }
        if let Some(ms) = env_var("NESTRS_WEATHER__REQUEST_TIMEOUT_MS").and_then(|s| s.parse().ok())
        {
            cfg.request_timeout_ms = ms;
        }
        cfg
    }
}
