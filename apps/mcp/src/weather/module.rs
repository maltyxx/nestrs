use nestrs_core::module;

use crate::weather::controller::WeatherController;
use crate::weather::service::{OpenMeteoClient, WeatherProvider};

#[module(providers = [OpenMeteoClient as dyn WeatherProvider, WeatherController])]
pub struct WeatherModule;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weather::config::WeatherConfig;
    use nestrs_core::{Container, Module};
    use std::sync::Arc;

    #[test]
    fn registers_open_meteo_as_default_provider() {
        // `OpenMeteoClient` injects the shared client and config that the app
        // seeds before modules register, so the test seeds them too.
        let container = WeatherModule::register(
            Container::builder()
                .provide(reqwest::Client::new())
                .provide(WeatherConfig::default()),
        )
        .build();
        let provider: Option<Arc<dyn WeatherProvider>> = container.get_dyn();
        assert!(provider.is_some());
    }
}
