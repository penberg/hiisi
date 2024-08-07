///! Main server module.
use bytes::Bytes;
use futures::Future;
use http_body_util::{BodyExt, Full};
use hyper::body::{Body, Incoming};
use hyper::{server::conn::http1, service::service_fn};
use hyper::{Method, Request, Response, StatusCode};
use log::trace;
use monoio::{io::IntoPollIo, net::TcpListener};
use std::net::{IpAddr, SocketAddr};
use std::rc::Rc;

use crate::manager::ResourceManager;
use crate::Result;
use crate::{executor, proto, InfernoError};

pub async fn serve(addr: IpAddr, port: u16) -> std::io::Result<()> {
    let manager = Rc::new(ResourceManager::new());
    let handler = move |req: Request<Incoming>| {
        let manager = manager.clone();
        http_handler(manager, req)
    };
    serve_http((addr, port), handler).await?;
    Ok(())
}

async fn serve_http<S, F, E, A>(addr: A, service: S) -> std::io::Result<()>
where
    S: Clone + Fn(Request<Incoming>) -> F + 'static,
    F: Future<Output = std::result::Result<Response<Full<Bytes>>, E>> + 'static,
    E: std::error::Error + 'static + Send + Sync,
    A: Into<SocketAddr>,
{
    let mut opts = monoio::net::ListenerOpts::new();
    opts.reuse_port = true;
    opts.reuse_addr = true;
    let listener = TcpListener::bind_with_config(addr.into(), &opts)?;
    loop {
        let service = service.clone();
        let (stream, _) = listener.accept().await?;
        trace!(
            "Accepted connection on thread {:?}",
            std::thread::current().id()
        );
        stream.set_nodelay(true)?;
        let stream_poll = monoio_compat::hyper::MonoioIo::new(stream.into_poll_io()?);
        monoio::spawn(async move {
            let conn = http1::Builder::new()
                .timer(monoio_compat::hyper::MonoioTimer)
                .serve_connection(stream_poll, service_fn(service));
            if let Err(err) = conn.await {
                log::debug!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn http_handler(
    manager: Rc<ResourceManager>,
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>> {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    match (method, path.as_str()) {
        (Method::GET, path) if path.ends_with("/v2") => {
            let resp = Response::builder()
                .status(StatusCode::OK)
                .body(Full::new(Bytes::from("")))
                .map_err(|e| InfernoError::from(e))?;
            Ok(resp)
        }
        (Method::POST, path) if path.ends_with("/v2/pipeline") => {
            let db_name = path.trim_end_matches("/v2/pipeline");
            let db_name = db_name.trim_start_matches('/');
            if db_name.is_empty() {
                return handle_not_found(req).await;
            }
            handle_pipeline_req(manager, req, db_name).await
        }
        _ => handle_not_found(req).await,
    }
}

async fn handle_pipeline_req(
    manager: Rc<ResourceManager>,
    req: Request<Incoming>,
    db_name: &str,
) -> Result<Response<Full<Bytes>>> {
    if let Some(upper) = req.body().size_hint().upper() {
        if upper > 1024 * 64 {
            let resp = Response::builder()
                .status(StatusCode::PAYLOAD_TOO_LARGE)
                .body(Full::new(Bytes::from("")))
                .map_err(|e| InfernoError::from(e))?;
            return Ok(resp);
        }
    }
    let req_body = req
        .collect()
        .await
        .map_err(|e: hyper::Error| InfernoError::from(e))?
        .to_bytes();
    let msg = proto::parse_client_req(&req_body)?;
    let baton = msg.baton.to_owned().unwrap_or(generate_baton());
    let resp = executor::execute_client_req(manager, msg, db_name, &baton).await?;
    let resp = proto::format_client_req(&resp)?;
    Ok(Response::new(Full::new(resp)))
}

fn generate_baton() -> String {
    // NOTE: This is different from the baton generation in libSQL server.
    uuid::Uuid::new_v4().to_string()
}

async fn handle_not_found(_req: Request<Incoming>) -> Result<Response<Full<Bytes>>> {
    let resp = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Full::new(Bytes::from("")))
        .map_err(|e| InfernoError::from(e))?;
    Ok(resp)
}

impl From<hyper::Error> for InfernoError {
    fn from(e: hyper::Error) -> Self {
        InfernoError::ProtocolError(e.to_string())
    }
}

impl From<hyper::http::Error> for InfernoError {
    fn from(e: hyper::http::Error) -> Self {
        InfernoError::ProtocolError(e.to_string())
    }
}
