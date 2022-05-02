// Copyright 2020 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use super::{IfEvent, Incoming, Provider};

use async_io_crate::Async;
use futures::future::{BoxFuture, FutureExt};
use std::io;
use std::net;
use std::task::{Context, Poll};

use libp2p_core::tid;

#[derive(Copy, Clone)]
pub enum Tcp {}

impl Provider for Tcp {
    type Stream = Async<net::TcpStream>;
    type Listener = Async<net::TcpListener>;
    type IfWatcher = if_watch::IfWatcher;

    fn if_watcher() -> BoxFuture<'static, io::Result<Self::IfWatcher>> {
        log::debug!("Tcp::Provider::if_watcher:+");
        let result = if_watch::IfWatcher::new().boxed();
        log::debug!("Tcp::Provider::if_watcher:-");
        result
    }

    fn new_listener(l: net::TcpListener) -> io::Result<Self::Listener> {
        log::debug!("Tcp::Provider::new_listener:+");
        let result = Async::new(l);
        log::debug!("Tcp::Provider::new_listener:-");
        result
    }

    fn new_stream(s: net::TcpStream) -> BoxFuture<'static, io::Result<Self::Stream>> {
        log::debug!("Tcp::Provider::new_stream:+");
        let result = async move {
            // Taken from [`Async::connect`].

            let stream = Async::new(s)?;

            // The stream becomes writable when connected.
            stream.writable().await?;

            // Check if there was an error while connecting.
            match stream.get_ref().take_error()? {
                None => Ok(stream),
                Some(err) => Err(err),
            }
        }
        .boxed();
        log::debug!("Tcp::Provider::new_stream:-");
        result
    }

    fn poll_accept(
        l: &mut Self::Listener,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<Incoming<Self::Stream>>> {
        log::debug!("Tcp::Provider::poll_accept:+ tid={} incomming", tid());
        let (stream, remote_addr) = loop {
            // This is the "higher level receiving" side of accepting a connection.
            // This doesn't happen when libp2p-lookup fails. I'm trying to get closer
            // to that point where it fails. Basically trying to determine if the lower
            // level code (epoll?) is never deliverying or this higher level code can't
            // accept it.
            log::debug!("Tcp::Provider::poll_accept: call poll_readable cx={:?}", cx);
            let prr = l.poll_readable(cx);
            log::debug!("Tcp::Provider::poll_accept: retf poll_readable prr={:?}", prr);
            match prr {
                Poll::Pending => {
                    let result = Poll::Pending;
                    log::debug!("Tcp::Provider::poll_accept:- result.is_ready(): {}", result.is_ready());
                    return result
                }
                Poll::Ready(Err(err)) => {
                    let err_string: String = err.to_string();
                    let result = Poll::Ready(Err(err));
                    log::debug!("Tcp::Provider::poll_accept:- result.is_ready(): {}, err: {}", result.is_ready(), err_string);
                    return result;
                }
                Poll::Ready(Ok(())) => {
                    log::debug!("Tcp::Provider::poll_accept: call accept()");
                    let accept_result= l.accept();
                    log::debug!("Tcp::Provider::poll_accept: retf accept()");
                    log::debug!("Tcp::Provider::poll_accept: call now_or_never()");
                    let now_or_never_result = accept_result.now_or_never();
                    log::debug!("Tcp::Provider::poll_accept: retf now_or_never() now_or_never_result={:?}", now_or_never_result);
                    match now_or_never_result {
                        Some(Err(e)) => {
                            let err_string: String = e.to_string();
                            let result = Poll::Ready(Err(e));
                            log::debug!("Tcp::Provider::poll_accept:- result.is_ready(): {}, err: {}", result.is_ready(), err_string);
                            return result;
                        }
                        Some(Ok(res)) => {
                            log::debug!("Tcp::Provider::poll_accept: accepted res: {:?}", res);
                            break res
                        }
                        None => {
                            log::debug!("Tcp::Provider::poll_accept: None, continue looping");
                            // Since it doesn't do any harm, account for false positives of
                            // `poll_readable` just in case, i.e. try again.
                        }
                    }
                },
            }
        };

        let local_addr = stream.get_ref().local_addr()?;

        let result = Poll::Ready(Ok(Incoming {
            stream,
            local_addr,
            remote_addr,
        }));
        log::debug!("Tcp::Provider::poll_accept:- incoming result.is_ready():, {} local_addr: {}, remote_addr: {}", result.is_ready(), local_addr, remote_addr);
        result
    }

    fn poll_interfaces(w: &mut Self::IfWatcher, cx: &mut Context<'_>) -> Poll<io::Result<IfEvent>> {
        log::debug!("Tcp::Provider::poll_interfaces:+");

        let result = w.poll_unpin(cx).map_ok(|e| match e {
            if_watch::IfEvent::Up(a) => {
                log::debug!("Tcp::Provider::poll_interfaces call IfEvent::Up {}", a);
                IfEvent::Up(a)
            }
            if_watch::IfEvent::Down(a) => {
                log::debug!("Tcp::Provider::poll_interfaces call IfEvent::Down {}", a);
                IfEvent::Down(a)
            }
        });

        log::debug!("Tcp::Provider::poll_interfaces:- result.is_ready(): {}", result.is_ready());
        result
    }
}
