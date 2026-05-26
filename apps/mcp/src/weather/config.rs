use nestrs_config::env_var;

#[derive(Debug, Clone)]
pub struct WeatherConfig {
    pub base_url: String,
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
