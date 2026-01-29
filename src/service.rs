use crate::http::HeaderError;
use crate::http::HttpExt;
use crate::http::uri::Origin;
use futures::future;
use http::HeaderName;
use http::Request;
use http::Response;
use http::StatusCode;
use http::Uri;
use http::header::LOCATION;
use http::uri::Authority;
use http::uri::Scheme;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use tower::Service;

#[derive(Clone, Debug)]
pub struct CanonicalRedirect<S>
{
	pub(crate) inner: S,
	pub(crate) canonical_origin: Origin,
	pub(crate) temporary_origins: Arc<[Origin]>,
	pub(crate) proto_headers: Arc<[HeaderName]>,
	pub(crate) host_headers: Arc<[HeaderName]>,
}

impl<S, I, O> Service<Request<I>> for CanonicalRedirect<S>
where
	S: Service<Request<I>, Response = Response<O>>,
	O: Default,
{
	type Response = S::Response;
	type Error = S::Error;
	type Future = future::Either<S::Future, future::Ready<Result<Self::Response, Self::Error>>>;

	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>
	{
		self.inner.poll_ready(cx)
	}

	fn call(&mut self, req: Request<I>) -> Self::Future
	{
		let Some(result) = self.parse(&req).transpose()
		else
		{
			return future::Either::Left(self.inner.call(req));
		};

		let res = if let Ok(res) = result
		{
			res.map(|_| O::default())
		}
		else
		{
			Response::builder()
				.status(StatusCode::BAD_REQUEST)
				.body(O::default())
				.expect("infallible")
		};

		future::Either::Right(future::ready(Ok(res)))
	}
}

impl<S> CanonicalRedirect<S>
{
	fn parse<B>(&self, req: &Request<B>) -> Result<Option<Response<()>>, HeaderError>
	{
		let origin = Origin {
			scheme: Scheme::from_request(req, self.proto_headers.iter().cloned())?,
			authority: Authority::from_request(req, self.host_headers.iter().cloned())?,
		};
		if origin == self.canonical_origin
		{
			return Ok(None);
		}

		let mut redirect_uri_builder = Uri::builder()
			.scheme(self.canonical_origin.scheme.clone())
			.authority(self.canonical_origin.authority.clone());
		if let Some(path_and_query) = req.uri().path_and_query()
		{
			redirect_uri_builder = redirect_uri_builder.path_and_query(path_and_query.clone());
		}

		let redirect_uri = redirect_uri_builder.build().expect("infallible");
		let redirect_status = if self.temporary_origins.contains(&origin)
		{
			StatusCode::TEMPORARY_REDIRECT
		}
		else
		{
			StatusCode::PERMANENT_REDIRECT
		};
		let redirect_res = Response::builder()
			.status(redirect_status)
			.header(LOCATION, redirect_uri.to_string())
			.body(())
			.expect("infallible");

		Ok(Some(redirect_res))
	}
}

#[cfg(test)]
mod test
{
	use super::CanonicalRedirect;
	use crate::http::header::X_FORWARDED_HOST;
	use crate::http::header::X_FORWARDED_PROTO;
	use crate::http::uri::Origin;
	use http::HeaderName;
	use http::Request;
	use http::StatusCode;
	use http::header::FORWARDED;
	use http::header::HOST;
	use http::header::LOCATION;
	use test_case::test_case;
	use test_case::test_matrix;

	pub const MOCK_PROTO: HeaderName = HeaderName::from_static("mock-proto");
	pub const MOCK_HOST: HeaderName = HeaderName::from_static("mock-host");

	fn mock_service(
		uri: &'static str,
		temporary_origins: impl IntoIterator<Item = &'static str>,
	) -> CanonicalRedirect<()>
	{
		CanonicalRedirect::<()> {
			inner: (),
			canonical_origin: uri.parse().unwrap(),
			temporary_origins: temporary_origins
				.into_iter()
				.map(str::parse)
				.collect::<Result<Vec<_>, _>>()
				.unwrap()
				.into(),
			proto_headers: [MOCK_PROTO].into(),
			host_headers: [MOCK_HOST].into(),
		}
	}

	#[test]
	fn request_without_headers_errors()
	{
		let req = Request::builder().body(()).unwrap();
		let res = mock_service("https://example.com", []).parse(&req);

		assert!(res.is_err());
	}

	#[test_case(MOCK_PROTO)]
	#[test_case(X_FORWARDED_PROTO)]
	fn request_with_proto_header_only_errors(proto: HeaderName)
	{
		let req = Request::builder().header(proto, "https").body(()).unwrap();
		let res = mock_service("https://example.com", []).parse(&req);

		assert!(res.is_err());
	}

	#[test_case(MOCK_HOST)]
	#[test_case(X_FORWARDED_HOST)]
	#[test_case(HOST)]
	fn request_with_host_header_only_errors(host: HeaderName)
	{
		let req = Request::builder()
			.header(host, "example.com")
			.body(())
			.unwrap();
		let res = mock_service("https://example.com", []).parse(&req);

		assert!(res.is_err());
	}

	#[test_case("proto=https")]
	#[test_case("host=example.com")]
	fn forwarded_request_with_partial_header_only_errors(header: &'static str)
	{
		let req = Request::builder()
			.header(FORWARDED, header)
			.body(())
			.unwrap();
		let res = mock_service("https://example.com", []).parse(&req);

		assert!(res.is_err());
	}

	#[test_matrix(
		[MOCK_PROTO, X_FORWARDED_PROTO],
		[MOCK_HOST, X_FORWARDED_HOST, HOST]
	)]
	fn match_does_not_redirect(proto: HeaderName, host: HeaderName)
	{
		let req = Request::builder()
			.header(proto, "https")
			.header(host, "example.com")
			.body(())
			.unwrap();
		let res = mock_service("https://example.com", []).parse(&req);

		assert!(res.is_ok());

		let res = res.unwrap();

		assert!(res.is_none());
	}

	#[test]
	fn forwarded_match_does_not_redirect()
	{
		let req = Request::builder()
			.header(FORWARDED, "proto=https;host=example.com")
			.body(())
			.unwrap();
		let res = mock_service("https://example.com", []).parse(&req);

		assert!(res.is_ok());

		let res = res.unwrap();

		assert!(res.is_none());
	}

	#[test]
	fn uri_match_does_not_redirect()
	{
		let req = Request::builder()
			.uri("https://example.com")
			.body(())
			.unwrap();
		let res = mock_service("https://example.com", []).parse(&req);

		assert!(res.is_ok());

		let res = res.unwrap();

		assert!(res.is_none());
	}

	#[test_matrix(
		[MOCK_PROTO, X_FORWARDED_PROTO],
		[MOCK_HOST, X_FORWARDED_HOST, HOST],
		[
			("http://example.com", "https://example.com"),
			("https://example.com", "http://example.com"),
			("http://www.example.com", "http://example.com"),
			("http://example.com", "http://www.example.com"),
			("http://example.com:8000", "http://example.com"),
			("http://example.com", "http://example.com:8000"),
			("http://other.com", "https://example.com"),
			("http://example.com", "https://other.com"),
			("http://username:password@example.com", "http://example.com"),
			("http://example.com", "http://username:password@example.com"),
		]
	)]
	fn mismatch_redirects(
		proto: HeaderName,
		host: HeaderName,
		(origin, redirect): (&'static str, &'static str),
	)
	{
		let Origin { scheme, authority } = origin.parse().unwrap();
		let req = Request::builder()
			.header(proto, scheme.as_str())
			.header(host, authority.as_str())
			.body(())
			.unwrap();
		let res = mock_service(redirect, []).parse(&req);

		assert!(res.is_ok());

		let res = res.unwrap();

		assert!(res.is_some());

		let res = res.unwrap();
		let loc = res.headers().get(LOCATION).unwrap().to_str().unwrap();

		assert_eq!(loc, format!("{redirect}/"));
	}

	#[test_case("http://example.com", "https://example.com")]
	#[test_case("https://example.com", "http://example.com")]
	#[test_case("http://www.example.com", "http://example.com")]
	#[test_case("http://example.com", "http://www.example.com")]
	#[test_case("http://example.com:8000", "http://example.com")]
	#[test_case("http://example.com", "http://example.com:8000")]
	#[test_case("http://other.com", "https://example.com")]
	#[test_case("http://example.com", "https://other.com")]
	#[test_case("http://username:password@example.com", "http://example.com")]
	#[test_case("http://example.com", "http://username:password@example.com")]
	fn forwarded_mismatch_redirects(origin: &'static str, redirect: &'static str)
	{
		let Origin { scheme, authority } = origin.parse().unwrap();
		let req = Request::builder()
			.header(FORWARDED, format!("proto={scheme};host={authority}"))
			.body(())
			.unwrap();
		let res = mock_service(redirect, []).parse(&req);

		assert!(res.is_ok());

		let res = res.unwrap();

		assert!(res.is_some());

		let res = res.unwrap();
		let loc = res.headers().get(LOCATION).unwrap().to_str().unwrap();

		assert_eq!(loc, format!("{redirect}/"));
	}

	#[test_case("http://example.com", "https://example.com")]
	#[test_case("https://example.com", "http://example.com")]
	#[test_case("http://www.example.com", "http://example.com")]
	#[test_case("http://example.com", "http://www.example.com")]
	#[test_case("http://example.com:8000", "http://example.com")]
	#[test_case("http://example.com", "http://example.com:8000")]
	#[test_case("http://other.com", "https://example.com")]
	#[test_case("http://example.com", "https://other.com")]
	#[test_case("http://username:password@example.com", "http://example.com")]
	#[test_case("http://example.com", "http://username:password@example.com")]
	fn uri_mismatch_redirects(origin: &'static str, redirect: &'static str)
	{
		let req = Request::builder().uri(origin).body(()).unwrap();
		let res = mock_service(redirect, []).parse(&req);

		assert!(res.is_ok());

		let res = res.unwrap();

		assert!(res.is_some());

		let res = res.unwrap();
		let loc = res.headers().get(LOCATION).unwrap().to_str().unwrap();

		assert_eq!(loc, format!("{redirect}/"));
	}

	#[test]
	fn temporary_redirect()
	{
		let req = Request::builder()
			.uri("http://temporary.example.com")
			.body(())
			.unwrap();
		let res = mock_service("http://example.com", ["http://temporary.example.com"])
			.parse(&req)
			.unwrap()
			.unwrap();

		assert_eq!(res.status(), StatusCode::TEMPORARY_REDIRECT);
	}

	#[test]
	fn permanent_redirect()
	{
		let req = Request::builder()
			.uri("http://permanent.example.com")
			.body(())
			.unwrap();
		let res = mock_service("http://example.com", [])
			.parse(&req)
			.unwrap()
			.unwrap();

		assert_eq!(res.status(), StatusCode::PERMANENT_REDIRECT);
	}

	#[test]
	fn redirect_preserves_path_and_query()
	{
		let req = Request::builder()
			.uri("http://www.example.com/path?query=1")
			.body(())
			.unwrap();
		let res = mock_service("https://example.com", [])
			.parse(&req)
			.unwrap()
			.unwrap();
		let loc = res.headers().get(LOCATION).unwrap().to_str().unwrap();

		assert_eq!(loc, "https://example.com/path?query=1");
	}
}
