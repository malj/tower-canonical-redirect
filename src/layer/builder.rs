use super::CanonicalRedirectLayer;
use http::header::InvalidHeaderName;
use http::uri::InvalidUri;
use std::error::Error;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

pub struct CanonicalRedirectLayerBuilder<'a>
{
	canonical_origin: &'a str,
	temporary_origins: Vec<&'a str>,
	proto_headers: Vec<&'a str>,
	host_headers: Vec<&'a str>,
}

impl<'a> CanonicalRedirectLayerBuilder<'a>
{
	pub fn new<S>(origin: &'a S) -> Self
	where
		S: AsRef<str> + ?Sized,
	{
		Self {
			canonical_origin: origin.as_ref(),
			temporary_origins: Vec::new(),
			proto_headers: Vec::new(),
			host_headers: Vec::new(),
		}
	}

	pub fn temporary_origin<S>(mut self, origin: &'a S) -> Self
	where
		S: AsRef<str> + ?Sized,
	{
		self.temporary_origins.push(origin.as_ref());
		self
	}

	pub fn proto_header<S>(mut self, header: &'a S) -> Self
	where
		S: AsRef<str> + ?Sized,
	{
		self.proto_headers.push(header.as_ref());
		self
	}

	pub fn host_header<S>(mut self, header: &'a S) -> Self
	where
		S: AsRef<str> + ?Sized,
	{
		self.host_headers.push(header.as_ref());
		self
	}

	pub fn build(self) -> Result<CanonicalRedirectLayer, CanonicalRedirectLayerBuildError>
	{
		Ok(CanonicalRedirectLayer {
			canonical_origin: self.canonical_origin.parse()?,
			temporary_origins: self
				.temporary_origins
				.into_iter()
				.map(str::parse)
				.collect::<Result<Vec<_>, _>>()?
				.into(),
			proto_headers: self
				.proto_headers
				.into_iter()
				.map(str::parse)
				.collect::<Result<Vec<_>, _>>()?
				.into(),
			host_headers: self
				.host_headers
				.into_iter()
				.map(str::parse)
				.collect::<Result<Vec<_>, _>>()?
				.into(),
		})
	}
}

#[derive(Debug)]
pub enum CanonicalRedirectLayerBuildError
{
	InvalidUri(InvalidUri),
	InvalidHeaderName(InvalidHeaderName),
}

impl Error for CanonicalRedirectLayerBuildError {}

impl Display for CanonicalRedirectLayerBuildError
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result
	{
		match &self
		{
			CanonicalRedirectLayerBuildError::InvalidUri(e) => e.fmt(f),
			CanonicalRedirectLayerBuildError::InvalidHeaderName(e) => e.fmt(f),
		}
	}
}

impl From<InvalidUri> for CanonicalRedirectLayerBuildError
{
	fn from(e: InvalidUri) -> Self
	{
		Self::InvalidUri(e)
	}
}

impl From<InvalidHeaderName> for CanonicalRedirectLayerBuildError
{
	fn from(e: InvalidHeaderName) -> Self
	{
		Self::InvalidHeaderName(e)
	}
}
