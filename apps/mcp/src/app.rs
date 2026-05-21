use nestrs_core::module;
use nestrs_health::HealthModule;
use nestrs_server_timing::ServerTiming;
use nestrs_telemetry::OtelHttp;

use crate::weather::WeatherModule;

#[module(
    imports = [WeatherModule, HealthModule],
    providers = [ServerTiming, OtelHttp],
)]
pub struct AppModule;
