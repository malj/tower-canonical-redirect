mod extensions;

pub use extensions::HeaderError;
pub use extensions::HttpExt;

pub mod header
{
	use http::HeaderName;

	pub const X_FORWARDED_PROTO: HeaderName = HeaderName::from_static("x-forwarded-proto");
	pub const X_FORWARDED_HOST: HeaderName = HeaderName::from_static("x-forwarded-host");
}

pub mod uri
{
	use http::uri::Authority;
	use http::uri::InvalidUri;
	use http::uri::Scheme;
	use std::str::FromStr;

	#[derive(Clone, Debug, PartialEq, Eq)]
	pub struct Origin
	{
		pub scheme: Scheme,
		pub authority: Authority,
	}

	impl FromStr for Origin
	{
		type Err = InvalidUri;

		fn from_str(s: &str) -> Result<Self, Self::Err>
		{
			let (scheme, authority) = s.split_once("://").unwrap_or_default();
			Ok(Self {
				scheme: scheme.parse()?,
				authority: authority.parse()?,
			})
		}
	}

	#[cfg(test)]
	mod test
	{
		use super::Origin;

		#[test]
		fn origin_with_protocol_and_host_is_parsable()
		{
			let origin = "http://example.com".parse::<Origin>();

			assert!(origin.is_ok());

			let Origin { scheme, authority } = origin.unwrap();

			assert_eq!(scheme.as_str(), "http");
			assert_eq!(authority.as_str(), "example.com");
		}

		#[test]
		fn origin_must_include_protocol()
		{
			let origin = "example.com".parse::<Origin>();

			assert!(origin.is_err());
		}

		#[test]
		fn origin_must_include_host()
		{
			let origin = "http://".parse::<Origin>();

			assert!(origin.is_err())
		}
	}
}
