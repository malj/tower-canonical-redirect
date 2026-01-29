mod http;
mod layer;
mod service;

pub use layer::CanonicalRedirectLayer;
pub use layer::builder::CanonicalRedirectLayerBuildError;
pub use layer::builder::CanonicalRedirectLayerBuilder;
pub use service::CanonicalRedirect;
