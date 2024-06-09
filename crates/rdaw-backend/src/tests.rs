use std::future::Future;

use futures::executor::LocalPool;
use futures::task::SpawnExt;
use futures::FutureExt;
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
    let mut executor = LocalPool::new();
    let spawner = executor.spawner();

    let (client_transport, server_transport) = transport::local(None);

    let client = Client::new(client_transport);
    let mut backend = Backend::new(server_transport);

    spawner
        .spawn(client.clone().handle().map(|v| v.unwrap()))
        .unwrap();

    spawner
        .spawn(async move { backend.handle().await.unwrap() })
        .unwrap();

    executor.run_until(f(client))
}

pub fn invalid_track_id() -> TrackId {
    TrackId::from(KeyData::from_ffi(u64::MAX))
}
