use crate::http::uri::Origin;
use crate::layer::builder::CanonicalRedirectLayerBuilder;
use crate::service::CanonicalRedirect;
use http::HeaderName;
use http::uri::InvalidUri;
use std::sync::Arc;
use tower::Layer;

pub mod builder;

#[derive(Clone, Debug)]
pub struct CanonicalRedirectLayer
{
	canonical_origin: Origin,
	temporary_origins: Arc<[Origin]>,
	proto_headers: Arc<[HeaderName]>,
	host_headers: Arc<[HeaderName]>,
}

impl<S> Layer<S> for CanonicalRedirectLayer
{
	type Service = CanonicalRedirect<S>;

	fn layer(&self, inner: S) -> Self::Service
	{
		Self::Service {
			inner,
			canonical_origin: self.canonical_origin.clone(),
			temporary_origins: self.temporary_origins.clone(),
			proto_headers: self.proto_headers.clone(),
			host_headers: self.host_headers.clone(),
		}
	}
}

impl CanonicalRedirectLayer
{
	pub fn new(origin: impl AsRef<str>) -> Result<Self, InvalidUri>
	{
		Ok(Self {
			canonical_origin: origin.as_ref().parse()?,
			temporary_origins: Default::default(),
			proto_headers: Default::default(),
			host_headers: Default::default(),
		})
	}

	pub fn builder<'a, S>(origin: &'a S) -> CanonicalRedirectLayerBuilder<'a>
	where
		S: AsRef<str> + ?Sized,
	{
		CanonicalRedirectLayerBuilder::new(origin)
	}
}
