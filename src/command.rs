use crate::{commands::Args, Error};
use std::{future::Future, pin::Pin, sync::Arc};

type ResultFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send>>;

pub trait AsyncFn<A, T>: 'static {
    fn call(&self, args: A) -> ResultFuture<T, Error>;
}

impl<A, F, G, T> AsyncFn<A, T> for F
where
    F: Fn(A) -> G + 'static,
    G: Future<Output = Result<T, Error>> + Send + 'static,
{
    fn call(&self, args: A) -> ResultFuture<T, Error> {
        let fut = (self)(args);
        Box::pin(async move { fut.await })
    }
}

pub type Handler = dyn AsyncFn<Arc<Args>, ()> + Send + Sync;
pub type Auth = dyn AsyncFn<Arc<Args>, bool> + Send + Sync;

pub enum CommandKind {
    Base,
    Protected,
    Help,
}

pub struct Command {
    pub kind: CommandKind,
    pub auth: &'static Auth,
    pub handler: &'static Handler,
}

impl Command {
    pub fn new(handler: &'static Handler) -> Self {
        Self {
            kind: CommandKind::Base,
            auth: &|_| async { Ok(true) },
            handler,
        }
    }

    pub fn new_with_auth(handler: &'static Handler, auth: &'static Auth) -> Self {
        Self {
            kind: CommandKind::Protected,
            auth,
            handler,
        }
    }

    pub fn help() -> Self {
        Self {
            kind: CommandKind::Help,
            auth: &|_| async { Ok(true) },
            handler: &|_| async { Ok(()) },
        }
    }
}