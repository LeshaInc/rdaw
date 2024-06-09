use std::future::Future;

use futures_lite::future::block_on;
use futures_lite::FutureExt;
use rdaw_api::track::TrackId;
use rdaw_api::{BackendProtocol, Result};
use rdaw_rpc::transport::{self, LocalClientTransport};
use rdaw_rpc::Client;
use slotmap::KeyData;

use crate::Backend;

pub fn run_test<Fn, Fut>(f: Fn) -> Result<()>
where
    Fn: FnOnce(Client<BackendProtocol, LocalClientTransport<BackendProtocol>>) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    let (client_transport, server_transport) = transport::local(None);

    let client = Client::new(client_transport);
    let handle_client = client.clone().handle();

    let mut backend = Backend::new(server_transport);
    let handle_backend = backend.handle();

    let run_test = f(client);

    block_on(run_test.or(handle_client).or(handle_backend))
}

pub fn invalid_track_id() -> TrackId {
    TrackId::from(KeyData::from_ffi(u64::MAX))
}
