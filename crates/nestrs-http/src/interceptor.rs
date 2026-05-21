use std::sync::Arc;

use nestrs_middleware::Interceptor;

/// Discovery metadata attached by the `#[interceptor]` macro. The
/// [`crate::HttpTransport`] walks these at boot via
/// [`nestrs_core::DiscoveryService::meta`] and folds them around the
/// assembled route, *innermost* to *outermost* in registration order
/// (the last interceptor declared across the module tree wraps the rest).
pub struct HttpInterceptorMeta {
    interceptor: Arc<dyn Interceptor>,
}

impl HttpInterceptorMeta {
    pub fn new(interceptor: Arc<dyn Interceptor>) -> Self {
        Self { interceptor }
    }

    pub fn interceptor(&self) -> Arc<dyn Interceptor> {
        self.interceptor.clone()
    }
}
