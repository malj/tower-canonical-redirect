use super::header::X_FORWARDED_HOST;
use super::header::X_FORWARDED_PROTO;
use http::HeaderName;
use http::HeaderValue;
use http::Request;
use http::Uri;
use http::header::FORWARDED;
use http::header::HOST;
use http::header::ToStrError;
use http::uri::Authority;
use http::uri::InvalidUri;
use http::uri::Scheme;
use std::error::Error;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::str::FromStr;

pub trait HttpExt: Sized
{
	fn from_request<B>(
		req: &Request<B>,
		custom_headers: impl IntoIterator<Item = HeaderName>,
	) -> Result<Self, HeaderError>;
}

impl FromHeader for Scheme
{
	type Headers = [HeaderName; 1];

	const DEFAULT_HEADERS: [HeaderName; 1] = [X_FORWARDED_PROTO];
	const FORWARD_KEY: &str = "proto";

	fn parse_uri(uri: &Uri) -> Option<Self>
	{
		uri.scheme().cloned()
	}
}

impl FromHeader for Authority
{
	type Headers = [HeaderName; 2];

	const DEFAULT_HEADERS: [HeaderName; 2] = [X_FORWARDED_HOST, HOST];
	const FORWARD_KEY: &str = "host";

	fn parse_uri(req: &Uri) -> Option<Self>
	{
		req.authority().cloned()
	}
}

impl<T: FromHeader> HttpExt for T
{
	fn from_request<B>(
		req: &Request<B>,
		custom_headers: impl IntoIterator<Item = HeaderName>,
	) -> Result<T, HeaderError>
	{
		custom_headers
			.into_iter()
			.find_map(|name| {
				req.headers()
					.get(&name)
					.map(|value| Self::parse_plain_header(&name, value))
			})
			.or_else(|| {
				req.headers()
					.get(FORWARDED)
					.map(|value| Self::parse_forwarded_header(&FORWARDED, value))
			})
			.or_else(|| {
				Self::DEFAULT_HEADERS.into_iter().find_map(|name| {
					req.headers()
						.get(&name)
						.map(|value| Self::parse_plain_header(&name, value))
				})
			})
			.transpose()?
			.or_else(|| Self::parse_uri(req.uri()))
			.ok_or_else(|| HeaderError::NotFound)
	}
}

trait FromHeader: FromStr<Err = InvalidUri>
{
	type Headers: IntoIterator<Item = HeaderName>;

	const DEFAULT_HEADERS: Self::Headers;
	const FORWARD_KEY: &str;

	fn parse_uri(uri: &Uri) -> Option<Self>;

	fn parse_plain_header(
		header_name: &HeaderName,
		header_value: &HeaderValue,
	) -> Result<Self, HeaderError>
	{
		header_value
			.to_str()
			.map_err(|err| HeaderError::InvalidEncoding(header_name.clone(), err))
			.and_then(|value| {
				value
					.parse()
					.map_err(|err| HeaderError::InvalidValue(header_name.clone(), err))
			})
	}

	fn parse_forwarded_header(
		header_name: &HeaderName,
		header_value: &HeaderValue,
	) -> Result<Self, HeaderError>
	{
		header_value
			.to_str()
			.map_err(|err| HeaderError::InvalidEncoding(header_name.clone(), err))?
			.split(',')
			.next()
			.and_then(|directives| {
				directives.split(';').find_map(|directive| {
					directive
						.split_once('=')
						.filter(|(key, _)| key.trim().eq_ignore_ascii_case(Self::FORWARD_KEY))?
						.1
						.trim()
						.trim_matches('"')
						.parse()
						.map_err(|err| HeaderError::InvalidValue(header_name.clone(), err))
						.into()
				})
			})
			.unwrap_or_else(|| Err(HeaderError::NotFound))
	}
}

#[derive(Debug)]
pub enum HeaderError
{
	InvalidEncoding(HeaderName, ToStrError),
	InvalidValue(HeaderName, InvalidUri),
	NotFound,
}

impl Error for HeaderError {}

impl Display for HeaderError
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result
	{
		match &self
		{
			HeaderError::InvalidEncoding(header_name, error) =>
			{
				write!(f, "Header \"{header_name}\" invalid encoding: {error}")
			}
			HeaderError::InvalidValue(header_name, error) =>
			{
				write!(f, "Header \"{header_name}\" invalid value: {error}")
			}
			HeaderError::NotFound => write!(f, "Header not found"),
		}
	}
}

#[cfg(test)]
mod test
{
	mod mock_request
	{
		use http::Request;
		use http::Uri;
		use http::header::FORWARDED;

		pub fn direct(uri: &'static str) -> Request<()>
		{
			let uri = Uri::from_static(uri);
			Request::builder().uri(uri).body(()).unwrap()
		}

		pub fn forwarded(uri: &'static str) -> Request<()>
		{
			forwarded_multihop([uri])
		}

		pub fn forwarded_multihop(uris: impl IntoIterator<Item = &'static str>) -> Request<()>
		{
			let mut uris = uris.into_iter().map(Uri::from_static).peekable();
			let path_and_query = uris.peek().and_then(Uri::path_and_query).cloned().unwrap();
			let value = uris
				.map(|uri| {
					format!(
						"proto={}; host={}",
						uri.scheme().unwrap().as_str(),
						uri.authority().unwrap().as_str()
					)
				})
				.collect::<Vec<_>>()
				.join(", ");
			Request::builder()
				.header(FORWARDED, value)
				.uri(path_and_query)
				.body(())
				.unwrap()
		}
	}

	mod scheme
	{
		use super::super::HttpExt;
		use super::mock_request;
		use crate::http::header::X_FORWARDED_PROTO;
		use http::HeaderName;
		use http::Request;
		use http::header::FORWARDED;
		use http::uri::Scheme;
		use test_case::test_case;
		use test_case::test_matrix;

		pub const MOCK_PROTO: HeaderName = HeaderName::from_static("mock-proto");

		mod mock_proto_request
		{
			use super::MOCK_PROTO;
			use crate::http::header::X_FORWARDED_PROTO;
			use http::HeaderName;
			use http::Request;
			use http::Uri;

			pub fn x_forwarded(uri: &'static str) -> Request<()>
			{
				new(uri, X_FORWARDED_PROTO)
			}

			pub fn custom(uri: &'static str) -> Request<()>
			{
				new(uri, MOCK_PROTO)
			}

			fn new(uri: &'static str, header: HeaderName) -> Request<()>
			{
				let uri = Uri::from_static(uri);
				Request::builder()
					.header(header, uri.scheme().unwrap().as_str())
					.body(())
					.unwrap()
			}
		}

		#[test_matrix(
			[
				mock_proto_request::custom,
				mock_request::forwarded,
				mock_proto_request::x_forwarded,
				mock_request::direct,
			],
			[
				("http://example.com", "http"),
				("https://example.com", "https")
			]
		)]
		fn extracted(
			req_factory: fn(&'static str) -> Request<()>,
			(url, expected): (&'static str, &'static str),
		)
		{
			let req = req_factory(url);
			let scheme = Scheme::from_request(&req, [MOCK_PROTO]).unwrap();

			assert_eq!(scheme.as_str(), expected);
		}

		#[test_case(["http://example.com", "https://example.com"], "http")]
		#[test_case(["https://example.com", "http://example.com"], "https")]
		fn multihop_first_extracted(urls: [&'static str; 2], expected: &'static str)
		{
			let req = mock_request::forwarded_multihop(urls);
			let scheme = Scheme::from_request(&req, []).unwrap();

			assert_eq!(scheme.as_str(), expected);
		}

		#[test_case(Some("https"), Some("proto=ssh"), Some("ftp"), "https"; "Custom 1st")]
		#[test_case(None, Some("proto=ssh"), Some("ftp"), "ssh"; "Forwarded 2nd")]
		#[test_case(None, None, Some("ftp"), "ftp"; "X-Forwarded 3d")]
		#[test_case(None, None, None, "http"; "URI 4th")]
		fn extraction_priority(
			custom: Option<&str>,
			forwarded: Option<&str>,
			x_forwarded: Option<&str>,
			expected: &str,
		)
		{
			let mut builder = Request::builder().uri("http://localhost");

			if let Some(value) = custom
			{
				builder = builder.header(MOCK_PROTO, value);
			}
			if let Some(value) = forwarded
			{
				builder = builder.header(FORWARDED, value);
			}
			if let Some(value) = x_forwarded
			{
				builder = builder.header(X_FORWARDED_PROTO, value);
			}

			let req = builder.body(()).unwrap();
			let scheme = Scheme::from_request(&req, [MOCK_PROTO]).unwrap();

			assert_eq!(scheme.as_str(), expected);
		}
	}

	mod authority
	{
		use super::super::HttpExt;
		use super::mock_request;
		use crate::http::header::X_FORWARDED_HOST;
		use http::HeaderName;
		use http::Request;
		use http::header::FORWARDED;
		use http::header::HOST;
		use http::uri::Authority;
		use test_case::test_case;
		use test_case::test_matrix;

		pub const MOCK_HOST: HeaderName = HeaderName::from_static("mock-host");

		mod mock_host_request
		{
			use super::MOCK_HOST;
			use crate::http::header::X_FORWARDED_HOST;
			use http::HeaderName;
			use http::Request;
			use http::Uri;
			use http::header::HOST;

			pub fn x_forwarded(uri: &'static str) -> Request<()>
			{
				new(uri, X_FORWARDED_HOST)
			}

			pub fn custom(uri: &'static str) -> Request<()>
			{
				new(uri, MOCK_HOST)
			}

			pub fn standard(uri: &'static str) -> Request<()>
			{
				new(uri, HOST)
			}

			fn new(uri: &'static str, header: HeaderName) -> Request<()>
			{
				let uri = Uri::from_static(uri);
				Request::builder()
					.header(header, uri.authority().unwrap().as_str())
					.body(())
					.unwrap()
			}
		}

		#[test_matrix(
			[
				mock_host_request::custom,
				mock_request::forwarded,
				mock_host_request::x_forwarded,
				mock_host_request::standard,
				mock_request::direct,
			],
			[
				("http://example.com", "example.com"),
				("http://www.example.com", "www.example.com"),
				("http://example.com:80", "example.com:80"),
				("http://username:password@example.com", "username:password@example.com"),
			]
		)]
		fn extracted(
			req_factory: fn(&'static str) -> Request<()>,
			(url, expected): (&'static str, &'static str),
		)
		{
			let req = req_factory(url);
			let authority = Authority::from_request(&req, [MOCK_HOST]).unwrap();

			assert_eq!(authority.as_str(), expected);
		}

		#[test_case(["http://example.com", "http://www.example.com"], "example.com")]
		#[test_case(["http://www.example.com", "http://example.com"], "www.example.com")]
		fn multihop_first_extracted(urls: [&'static str; 2], expected: &'static str)
		{
			let req = mock_request::forwarded_multihop(urls);
			let authority = Authority::from_request(&req, []).unwrap();

			assert_eq!(authority.as_str(), expected);
		}

		#[test_case(Some("a"), Some("host=b"), Some("c"), Some("d"), "a"; "Custom 1st")]
		#[test_case(None, Some("host=b"), Some("c"), Some("d"), "b"; "Forwarded 2nd")]
		#[test_case(None, None, Some("c"), Some("d"), "c"; "X-Forwarded 3rd")]
		#[test_case(None, None, None, Some("d"), "d"; "Host 4th")]
		#[test_case(None, None, None, None, "e"; "URI 5th")]
		fn extraction_priority(
			custom: Option<&str>,
			forwarded: Option<&str>,
			x_forwarded: Option<&str>,
			host: Option<&str>,
			expected: &str,
		)
		{
			let mut builder = Request::builder().uri("http://e");

			if let Some(value) = custom
			{
				builder = builder.header(MOCK_HOST, value);
			}
			if let Some(value) = forwarded
			{
				builder = builder.header(FORWARDED, value);
			}
			if let Some(value) = x_forwarded
			{
				builder = builder.header(X_FORWARDED_HOST, value);
			}
			if let Some(value) = host
			{
				builder = builder.header(HOST, value);
			}

			let req = builder.body(()).unwrap();
			let authority = Authority::from_request(&req, [MOCK_HOST]).unwrap();

			assert_eq!(authority.as_str(), expected);
		}
	}
}
