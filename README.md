# tower-canonical-redirect

A [`tower`] middleware to enforce canonical hosts in HTTP requests. Useful when you want to redirect website users from insecure `http` connections and/or `www` subdomains to a single canonical `https` domain.

The middleware uses framework-agnostic [`http`] and [`futures`] abstractions, making it compatible with other crates such as [`hyper`], [`axum`], [`tonic`], [`warp`], etc.

[`tower`]: https://docs.rs/tower
[`http`]: https://docs.rs/http
[`futures`]: https://docs.rs/futures
[`hyper`]: https://docs.rs/hyper
[`axum`]: https://docs.rs/axum
[`tonic`]: https://docs.rs/tonic
[`warp`]: https://docs.rs/warp

## Features

- Enables redirecting HTTP requests from any valid host to another, preserving path and query.
- Works behind reverse proxies by parsing [`Forwarded`], [`X-Forwarded-Proto`], [`X-Forwarded-Host`], or custom headers.
- Defaults to [`308 Permanent Redirect`], but can be configured to use [`307 Temporary Redirect`] for specific origins.

[`Forwarded`]: https://developer.mozilla.org/docs/Web/HTTP/Reference/Headers/Forwarded
[`X-Forwarded-Proto`]: https://developer.mozilla.org/docs/Web/HTTP/Reference/Headers/X-Forwarded-Proto
[`X-Forwarded-Host`]: https://developer.mozilla.org/docs/Web/HTTP/Reference/Headers/X-Forwarded-Host
[`307 Temporary Redirect`]: https://developer.mozilla.org/docs/Web/HTTP/Reference/Status/307
[`308 Permanent Redirect`]: https://developer.mozilla.org/docs/Web/HTTP/Reference/Status/308

## Example

Basic usage with [`axum`]:

```rust
use tower_canonical_redirect::CanonicalRedirectLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>
{
	let mut router = axum::Router::new()
		.route("/", axum::routing::get(async || "Hello world"));

	if let Ok(origin) = std::env::var("CANONICAL_ORIGIN")
	{
		let layer = CanonicalRedirectLayer::new(origin)?;
		router = router.layer(layer);
	}

	let listener = tokio::net::TcpListener::bind("127.0.0.1:8000").await?;
	axum::serve(listener, router).await?;

	Ok(())
}
```

Using the builder API:

```rust
let layer = CanonicalRedirectLayer::builder("https://example.com")
	.proto_header("X-Custom-Proto")
	.host_header("X-Custom-Host")
	.temporary_origin("http://www.example.com")
	.temporary_origin("https://www.example.com")
	.build()
	.unwrap();
```

## License

[MIT](LICENSE)

