use std::future::Future;
use std::sync::Arc;

use floem::reactive::use_context;
use rdaw_api::{Backend, Error, Result};
use rdaw_ui::task::spawn;

pub fn get_backend() -> Arc<dyn Backend> {
    use_context().expect("no backend in scope")
}

#[cold]
pub fn handle_error(error: Error) {
    tracing::error!(?error);
}

pub fn call<Fac, Fut, Cb, Res>(fac: Fac, callback: Cb)
where
    Fac: (FnOnce(Arc<dyn Backend>) -> Fut) + 'static,
    Fut: Future<Output = Result<Res>> + Send + 'static,
    Cb: FnOnce(Res) + 'static,
    Res: Send + 'static,
{
    let backend = get_backend();
    spawn(fac(backend), |res| match res {
        Ok(v) => callback(v),
        Err(e) => handle_error(e),
    })
}
