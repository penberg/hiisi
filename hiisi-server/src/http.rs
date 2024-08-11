use bytes::{Bytes, BytesMut};

pub use http::StatusCode;

pub fn format_response(body: Bytes, status: http::StatusCode) -> Bytes {
    let n = body.len();

    let response = http::Response::builder().status(status).body(body).unwrap();

    let mut response_bytes = BytesMut::new();
    response_bytes.extend_from_slice(
        format!(
            "HTTP/1.1 {} {}\r\n",
            response.status().as_u16(),
            response.status().canonical_reason().unwrap_or("")
        )
        .as_bytes(),
    );

    for (key, value) in response.headers() {
        response_bytes.extend_from_slice(
            format!("{}: {}\r\n", key.as_str(), value.to_str().unwrap()).as_bytes(),
        );
    }

    response_bytes.extend_from_slice(format!("Content-Length: {}\r\n\r\n", n).as_bytes());
    response_bytes.extend_from_slice(response.into_body().as_ref());

    response_bytes.into()
}
