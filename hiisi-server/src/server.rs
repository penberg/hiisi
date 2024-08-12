use anyhow::Result;
use bytes::Bytes;
use socket2::{SockAddr, Socket};

use std::rc::Rc;

use crate::executor::{self, Request};
use crate::http;
use crate::ResourceManager;
use crate::{proto, HiisiError};

pub type IO<T> = crate::io::IO<Context<T>>;

pub struct Context<T> {
    pub manager: Rc<ResourceManager>,
    pub user_data: T,
}

impl<T> Context<T> {
    pub fn new(manager: Rc<ResourceManager>, user_data: T) -> Self {
        Self { manager, user_data }
    }
}

pub fn serve<T>(io: &mut IO<T>, sock: Rc<Socket>, addr: SockAddr) {
    io.accept(sock, addr, on_accept);
}

fn on_accept<T>(
    io: &mut IO<T>,
    server_sock: Rc<Socket>,
    server_addr: SockAddr,
    conn_sock: Rc<Socket>,
    sock_addr: SockAddr,
) {
    log::trace!("Server accepted connection from {:?}", sock_addr);
    conn_sock.set_nodelay(true).unwrap();
    io.accept(server_sock, server_addr, on_accept);
    io.recv(conn_sock, on_recv);
}

fn execute_request<T>(io: &mut IO<T>, buf: &[u8]) -> Result<Bytes> {
    let ctx = io.context();
    let req = parse_request(&buf)?;
    let resp = executor::execute_client_req(ctx.manager.clone(), req)?;
    Ok(proto::format_msg(&resp)?)
}

fn on_recv<T>(io: &mut IO<T>, sock: Rc<Socket>, buf: &[u8], n: usize) {
    if n == 0 {
        log::trace!("Client closed connection");
        io.close(sock);
        return;
    }
    // When receiving POST request with chunked encoding,
    // the end marker consist of those bytes - [13, 10, 48, 13, 10, 13, 10]
    // and we don't recv them from the socket in one go.
    if n == 7 && is_complete_chunked_encoding_mark(buf) {
        return;
    }
    let resp = match execute_request(io, &buf[..n]) {
        Ok(resp) => http::format_response(resp, http::StatusCode::OK),
        Err(x) => http::format_response(
            format!("{}", x).into(),
            http::StatusCode::INTERNAL_SERVER_ERROR,
        ),
    };

    let n = resp.len();
    io.send(sock, resp.into(), n, on_send);
}

fn is_complete_chunked_encoding_mark(buf: &[u8]) -> bool {
    buf == b"\r\n0\r\n\r\n" 
}

enum Route {
    // The `/v2/pipeline` route.
    Pipeline,
}

fn parse_request(buf: &[u8]) -> Result<Request> {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req = httparse::Request::new(&mut headers);
    let body_off = req.parse(buf).unwrap().unwrap();
    let database = parse_database(&mut req)?;
    match parse_route(req.path.unwrap()) {
        Some(Route::Pipeline) => {
            let req = proto::parse_client_req(&buf[body_off..])?;
            Ok(Request {
                database: database.to_owned(),
                req,
            })
        }
        None => Err(HiisiError::ProtocolError("Invalid path".to_owned()).into()),
    }
}

const DEFAULT_DATABASE: &'static str = "default";

fn parse_database(req: &mut httparse::Request) -> Result<String> {
    let mut host: Option<&str> = None;
    for header in req.headers.iter() {
        if header.name == "Host" {
            host = Some(std::str::from_utf8(header.value)?);
            break;
        }
    }
    match host {
        Some(host) => {
            let parts: Vec<&str> = host.split('.').collect();
            if parts.len() > 1 {
                Ok(parts[0].to_owned())
            } else {
                Err(HiisiError::ProtocolError("Invalid host".to_owned()).into())
            }
        }
        None => Ok(DEFAULT_DATABASE.into()),
    }
}

fn parse_route(path: &str) -> Option<Route> {
    match path {
        "/v2/pipeline" => Some(Route::Pipeline),
        _ => None,
    }
}

fn on_send<T>(io: &mut IO<T>, sock: Rc<Socket>, _n: usize) {
    io.recv(sock, on_recv)
}
