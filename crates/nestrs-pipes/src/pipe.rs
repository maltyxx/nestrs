/// A pipe runs at a surface's request boundary, between extraction and the
/// handler: it `transform`s an extracted value, returning the new value or a
/// [`PipeError`]. The two use cases mirror NestJS: **transformation** (reshape
/// the value) and **validation** (pass it through or reject).
///
/// This maps NestJS's `PipeTransform.transform(value, metadata)` minus the
/// `ArgumentMetadata` — in Rust the value's source (path/query/body) and target
/// type are already encoded by the extractor the pipe wraps.
///
/// Pipes are **stateless**: a pipe is a zero-sized marker named at a call site
/// (`Piped<ParseInt, _>`), never instantiated, so `transform` is an associated
/// function. (Stateful/DI-injected pipes would need a different binding and are
/// a future extension.)
pub trait Pipe {
    type In;
    type Out;
    fn transform(input: Self::In) -> Result<Self::Out, PipeError>;
}

/// Why a pipe rejected its input. A surface adapter renders it (the HTTP one as
/// a `400`). Carries a human `message` plus optional structured `details` (e.g.
/// the field-level errors from [`ValidationPipe`](crate::ValidationPipe)) so a
/// surface can render both.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct PipeError {
    message: String,
    details: Option<serde_json::Value>,
}

impl PipeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(message: impl Into<String>, details: serde_json::Value) -> Self {
        Self {
            message: message.into(),
            details: Some(details),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn details(&self) -> Option<&serde_json::Value> {
        self.details.as_ref()
    }

    /// Take ownership of the `details`, so a surface rendering the error can
    /// move them into a response body instead of cloning.
    pub fn into_details(self) -> Option<serde_json::Value> {
        self.details
    }
}
