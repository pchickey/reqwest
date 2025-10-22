use std::fmt;

use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use http_body_util::BodyExt;
use url::Url;

#[cfg(feature = "stream")]
use futures_util::stream::{self, StreamExt};

#[cfg(feature = "json")]
use serde::de::DeserializeOwned;

#[cfg(feature = "charset")]
use encoding_rs::{Encoding, UTF_8};
#[cfg(feature = "charset")]
use mime::Mime;

/// A Response to a submitted `Request`.
pub struct Response {
    http: wstd::http::Response<wstd::http::Body>,
    // Boxed to save space (11 words to 1 word), and it's not accessed
    // frequently internally.
    url: Box<Url>,
}

impl Response {
    pub(super) fn new(http: wstd::http::Response<wstd::http::Body>, url: Url) -> Response {
        Response {
            http,
            url: Box::new(url),
        }
    }

    /// Get the `StatusCode` of this `Response`.
    #[inline]
    pub fn status(&self) -> StatusCode {
        self.http.status()
    }

    /// Get the `Headers` of this `Response`.
    #[inline]
    pub fn headers(&self) -> &HeaderMap {
        self.http.headers()
    }

    /// Get a mutable reference to the `Headers` of this `Response`.
    #[inline]
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        self.http.headers_mut()
    }

    /// Get the content-length of this response, if known.
    ///
    /// Reasons it may not be known:
    ///
    /// - The server didn't send a `content-length` header.
    /// - The response is compressed and automatically decoded (thus changing
    ///   the actual decoded length).
    pub fn content_length(&self) -> Option<u64> {
        self.headers()
            .get(http::header::CONTENT_LENGTH)?
            .to_str()
            .ok()?
            .parse()
            .ok()
    }

    /// Get the final `Url` of this `Response`.
    #[inline]
    pub fn url(&self) -> &Url {
        &self.url
    }

    /* TODO this is nominally possible in wasi-http
    * It might not be possible to detect this in JS?
    /// Get the HTTP `Version` of this `Response`.
    #[inline]
    pub fn version(&self) -> Version {
        self.http.version()
    }
    */

    /// Try to deserialize the response body as JSON.
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub async fn json<T: DeserializeOwned>(self) -> crate::Result<T> {
        let full = self.bytes().await?;

        serde_json::from_slice(&full).map_err(crate::error::decode)
    }

    /// Get the full response text.
    ///
    /// This method decodes the response body with BOM sniffing
    /// and with malformed sequences replaced with the
    /// [`char::REPLACEMENT_CHARACTER`].
    /// Encoding is determined from the `charset` parameter of `Content-Type` header,
    /// and defaults to `utf-8` if not presented.
    ///
    /// Note that the BOM is stripped from the returned String.
    ///
    /// # Note
    ///
    /// If the `charset` feature is disabled the method will only attempt to decode the
    /// response as UTF-8, regardless of the given `Content-Type`
    ///
    /// # Example
    ///
    /// ```
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let content = reqwest::get("http://httpbin.org/range/26")
    ///     .await?
    ///     .text()
    ///     .await?;
    ///
    /// println!("text: {content:?}");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn text(mut self) -> crate::Result<String> {
        #[cfg(feature = "charset")]
        {
            self.text_with_charset("utf-8").await
        }

        #[cfg(not(feature = "charset"))]
        {
            let full = self.bytes().await?;
            let text = String::from_utf8_lossy(&full);
            Ok(text.into_owned())
        }
    }

    /// Get the full response text given a specific encoding.
    ///
    /// This method decodes the response body with BOM sniffing
    /// and with malformed sequences replaced with the [`char::REPLACEMENT_CHARACTER`].
    /// You can provide a default encoding for decoding the raw message, while the
    /// `charset` parameter of `Content-Type` header is still prioritized. For more information
    /// about the possible encoding name, please go to [`encoding_rs`] docs.
    ///
    /// Note that the BOM is stripped from the returned String.
    ///
    /// [`encoding_rs`]: https://docs.rs/encoding_rs/0.8/encoding_rs/#relationship-with-windows-code-pages
    ///
    /// # Optional
    ///
    /// This requires the optional `encoding_rs` feature enabled.
    ///
    /// # Example
    ///
    /// ```
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let content = reqwest::get("http://httpbin.org/range/26")
    ///     .await?
    ///     .text_with_charset("utf-8")
    ///     .await?;
    ///
    /// println!("text: {content:?}");
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "charset")]
    #[cfg_attr(docsrs, doc(cfg(feature = "charset")))]
    pub async fn text_with_charset(self, default_encoding: &str) -> crate::Result<String> {
        let content_type = self
            .headers()
            .get(crate::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<Mime>().ok());
        let encoding_name = content_type
            .as_ref()
            .and_then(|mime| mime.get_param("charset").map(|charset| charset.as_str()))
            .unwrap_or(default_encoding);
        let encoding = Encoding::for_label(encoding_name.as_bytes()).unwrap_or(UTF_8);

        let full = self.bytes().await?;

        let (text, _, _) = encoding.decode(&full);
        Ok(text.into_owned())
    }

    /// Get the full response body as `Bytes`.
    ///
    /// # Example
    ///
    /// ```
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let bytes = reqwest::get("http://httpbin.org/ip")
    ///     .await?
    ///     .bytes()
    ///     .await?;
    ///
    /// println!("bytes: {bytes:?}");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bytes(self) -> crate::Result<Bytes> {
        let mut body = self.http.into_body().into_boxed_body();
        let full = BodyExt::collect(body).await.map_err(crate::error::body)?;
        Ok(full.to_bytes())
    }

    /// Convert the response into a `Stream` of `Bytes` from the body.
    #[cfg(feature = "stream")]
    pub fn bytes_stream(self) -> impl futures_core::Stream<Item = crate::Result<Bytes>> {
        super::body::DataStream(self.res.into_body())
    }

    // util methods

    /// Turn a response into an error if the server returned an error.
    pub fn error_for_status(self) -> crate::Result<Self> {
        let status = self.status();
        if status.is_client_error() || status.is_server_error() {
            Err(crate::error::status_code(*self.url, status))
        } else {
            Ok(self)
        }
    }

    /// Turn a reference to a response into an error if the server returned an error.
    pub fn error_for_status_ref(&self) -> crate::Result<&Self> {
        let status = self.status();
        if status.is_client_error() || status.is_server_error() {
            Err(crate::error::status_code(*self.url.clone(), status))
        } else {
            Ok(self)
        }
    }
}

impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Response")
            //.field("url", self.url())
            .field("status", &self.status())
            .field("headers", self.headers())
            .finish()
    }
}

/// Converts any `impl Body` into a `impl Stream` of just its DATA frames.
#[cfg(any(feature = "stream", feature = "multipart",))]
pub(crate) struct DataStream<B>(pub(crate) B);

#[cfg(any(feature = "stream", feature = "multipart",))]
impl<B> futures_core::Stream for DataStream<B>
where
    B: HttpBody<Data = Bytes> + Unpin,
{
    type Item = Result<Bytes, B::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        loop {
            return match ready!(Pin::new(&mut self.0).poll_frame(cx)) {
                Some(Ok(frame)) => {
                    // skip non-data frames
                    if let Ok(buf) = frame.into_data() {
                        Poll::Ready(Some(Ok(buf)))
                    } else {
                        continue;
                    }
                }
                Some(Err(err)) => Poll::Ready(Some(Err(err))),
                None => Poll::Ready(None),
            };
        }
    }
}
