use bytes::Bytes;
use socket2::{SockAddr, Socket};

use std::rc::Rc;

use crate::http;
use crate::{server::IO, HiisiError, Result};

pub fn serve_admin<T>(io: &mut IO<T>, sock: Rc<Socket>, addr: SockAddr) {
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

fn on_recv<T>(io: &mut IO<T>, sock: Rc<Socket>, buf: &[u8], n: usize) {
    if n == 0 {
        log::trace!("Client closed connection");
        io.close(sock);
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
    io.send(sock, resp, n, on_send);
}

fn on_send<T>(io: &mut IO<T>, sock: Rc<Socket>, _n: usize) {
    io.recv(sock, on_recv)
}

fn execute_request<T>(io: &mut IO<T>, buf: &[u8]) -> Result<Bytes> {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req = httparse::Request::new(&mut headers);
    let _ = req.parse(buf).unwrap().unwrap();
    match parse_route(req.path.unwrap()) {
        Some(Route::CreateNamespace(name)) => {
            let ctx = io.context();
            ctx.manager.create_database(&name)?;
            Ok("".into())
        }
        _ => Err(HiisiError::ProtocolError("Invalid path".to_owned()).into()),
    }
}

enum Route {
    // The `/v1/namespaces/:name/create` route.
    CreateNamespace(String),
}

fn parse_route(path: &str) -> Option<Route> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 4 {
        return None;
    }
    if parts[1] != "v1" {
        return None;
    }
    if parts[2] != "namespaces" {
        return None;
    }
    if parts[4] != "create" {
        return None;
    }
    Some(Route::CreateNamespace(parts[3].to_owned()))
}
