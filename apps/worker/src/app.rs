use nestrs_core::module;
use nestrs_queue::{QueueModule, QueueOptions};

use crate::audio::AudioModule;

#[module(imports = [
    QueueModule::for_root(QueueOptions {
        url: std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".into()),
    }),
    AudioModule,
])]
pub struct AppModule;
