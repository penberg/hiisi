use bytes::{Bytes, BytesMut};
use polling::{Event, Events, Poller};

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

pub struct IO<C> {
    poller: Poller,
    events: Events,
    key_seq: usize,
    submissions: HashMap<usize, Completion<C>>,
    completions: VecDeque<Completion<C>>,
    context: C,
}

impl<C> IO<C> {
    pub fn new(context: C) -> Self {
        Self {
            poller: Poller::new().unwrap(),
            events: Events::new(),
            key_seq: 0,
            submissions: HashMap::new(),
            completions: VecDeque::new(),
            context,
        }
    }

    pub fn context(&self) -> &C {
        &self.context
    }

    pub fn run_once(&mut self) {
        log::debug!("Running IO loop");
        self.events.clear();
        let _ = self.poller.wait(
            &mut self.events,
            Some(std::time::Duration::from_micros(500)),
        );
        self.flush_submissions();
        self.flush_completions();
    }

    fn flush_submissions(&mut self) {
        log::debug!("Flushing submissions");
        for event in self.events.iter() {
            log::debug!("Event: {:?}", event.key);
            let c = self.submissions.remove(&event.key).unwrap();
            c.prepare();
            match &c {
                Completion::Accept { server_sock, .. } => {
                    self.poller.delete(server_sock).unwrap();
                }
                Completion::Recv { sock, .. } => {
                    self.poller.delete(sock).unwrap();
                }
                Completion::Send { sock, .. } => {
                    self.poller.delete(sock).unwrap();
                }
                _ => {
                    todo!();
                }
            }
            self.completions.push_back(c);
        }
    }

    fn flush_completions(&mut self) {
        log::debug!("Flushing completions");
        loop {
            let c = self.completions.pop_front();
            if let Some(c) = c {
                c.complete(self);
            } else {
                break;
            }
        }
    }

    pub fn accept(
        &mut self,
        server_sock: Rc<socket2::Socket>,
        server_addr: socket2::SockAddr,
        cb: AcceptCallback<C>,
    ) {
        log::debug!("Accepting connection on sockfd {:?}", server_sock);
        let c = Completion::Accept {
            server_sock,
            server_addr,
            cb,
        };
        let key = self.get_key();
        match &c {
            Completion::Accept { server_sock, .. } => unsafe {
                self.poller.add(server_sock, Event::readable(key)).unwrap();
            },
            _ => {
                todo!();
            }
        }
        self.enqueue(key, c);
    }

    pub fn close(&mut self, sock: Rc<socket2::Socket>) {
        log::debug!("Closing sockfd {:?}", sock);
        drop(sock);
    }

    pub fn recv(&mut self, sock: Rc<socket2::Socket>, cb: RecvCallback<C>) {
        log::debug!("Receiving on sockfd {:?}", sock);
        let c = Completion::Recv { sock, cb };
        let key = self.get_key();
        match &c {
            Completion::Recv { sock, .. } => unsafe {
                self.poller.add(sock, Event::readable(key)).unwrap();
            },
            _ => {
                todo!();
            }
        }
        self.enqueue(key, c);
    }

    pub fn send(&mut self, sock: Rc<socket2::Socket>, buf: Bytes, n: usize, cb: SendCallback<C>) {
        log::debug!("Sending on sockfd {:?}", sock);
        let c = Completion::Send { sock, buf, n, cb };
        let key = self.get_key();
        match &c {
            Completion::Send { sock, .. } => unsafe {
                self.poller.add(sock, Event::writable(key)).unwrap();
            },
            _ => {
                todo!();
            }
        }
        self.enqueue(key, c)
    }

    fn get_key(&mut self) -> usize {
        let ret = self.key_seq;
        self.key_seq += 1;
        ret
    }

    fn enqueue(&mut self, key: usize, c: Completion<C>) {
        self.submissions.insert(key, c);
    }
}

pub enum Completion<C> {
    Accept {
        server_sock: Rc<socket2::Socket>,
        server_addr: socket2::SockAddr,
        cb: AcceptCallback<C>,
    },
    Close,
    Recv {
        sock: Rc<socket2::Socket>,
        cb: RecvCallback<C>,
    },
    Send {
        sock: Rc<socket2::Socket>,
        buf: Bytes,
        n: usize,
        cb: SendCallback<C>,
    },
}

impl<C> std::fmt::Debug for Completion<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Completion::Accept { .. } => write!(f, "Accept"),
            Completion::Close => write!(f, "Close"),
            Completion::Recv { .. } => write!(f, "Recv"),
            Completion::Send { .. } => write!(f, "Send"),
        }
    }
}

impl<C> Completion<C> {
    fn prepare(&self) {
        match self {
            Completion::Accept { .. } => {}
            Completion::Close => {
                todo!();
            }
            Completion::Recv { .. } => {}
            Completion::Send { .. } => {}
        }
    }

    fn complete(self, io: &mut IO<C>) {
        match self {
            Completion::Accept {
                server_sock,
                server_addr,
                cb,
            } => {
                let (sock, sock_addr) = server_sock.accept().unwrap();
                cb(io, server_sock, server_addr, Rc::new(sock), sock_addr);
            }
            Completion::Close => {
                todo!();
            }
            Completion::Recv { sock, cb } => {
                let mut buf = BytesMut::with_capacity(4096);
                let uninit = buf.spare_capacity_mut();
                let n = sock.recv(uninit).unwrap();
                unsafe {
                    buf.set_len(n);
                }
                cb(io, sock, &buf[..], n);
            }
            Completion::Send { sock, buf, n, cb } => {
                let n = sock.send(&buf[..n]).unwrap();
                cb(io, sock, n);
            }
        }
    }
}

type ConnectCallback<C> = fn(&mut IO<C>, Rc<socket2::Socket>, socket2::SockAddr);

type AcceptCallback<C> =
    fn(&mut IO<C>, Rc<socket2::Socket>, socket2::SockAddr, Rc<socket2::Socket>, socket2::SockAddr);

type RecvCallback<C> = fn(&mut IO<C>, Rc<socket2::Socket>, &[u8], usize);

type SendCallback<C> = fn(&mut IO<C>, Rc<socket2::Socket>, usize);
