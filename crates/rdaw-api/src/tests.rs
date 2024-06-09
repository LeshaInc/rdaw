use futures::executor::LocalPool;
use futures::task::SpawnExt;
use futures::FutureExt;
use rdaw_rpc::transport::{self, ServerTransport};
use rdaw_rpc::{handler, operations, protocol, Client, ClientMessage};

use crate::{Error, Result};

#[operations(protocol = TestProtocol)]
trait FooOperations {
    async fn get_foo(&self) -> Result<i32>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[protocol(operations(FooOperations), error = Error)]
struct TestProtocol;

struct TestBackend;

#[handler(protocol = TestProtocol, operations = FooOperations)]
impl TestBackend {
    #[handler]
    fn get_foo(&self) -> Result<i32> {
        Ok(1)
    }
}

impl TestBackend {
    async fn handle_message<T: ServerTransport<TestProtocol>>(
        &mut self,
        transport: T,
        msg: ClientMessage<TestProtocol>,
    ) -> Result<()> {
        match msg {
            ClientMessage::Request { id, payload } => match payload {
                TestRequest::Foo(req) => self.handle_foo_request(transport, id, req).await,
            },
            ClientMessage::CloseStream { .. } => todo!(),
        }
    }

    async fn handle<T: ServerTransport<TestProtocol>>(&mut self, transport: T) -> Result<()> {
        loop {
            match transport.recv().await {
                Ok(msg) => self.handle_message(transport.clone(), msg).await?,
                Err(Error::Disconnected) => return Ok(()),
                Err(e) => return Err(e),
            }
        }
    }
}

#[test]
fn local_client() -> Result<()> {
    let mut executor = LocalPool::new();
    let spawner = executor.spawner();

    let (client_transport, server_transport) = transport::local(None);

    let client = Client::<TestProtocol, _>::new(client_transport);
    let mut server = TestBackend;

    spawner
        .spawn(client.clone().handle().map(|v| v.unwrap()))
        .unwrap();

    spawner
        .spawn(async move { server.handle(server_transport).await.unwrap() })
        .unwrap();

    executor.run_until(async move {
        let foo = client.get_foo().await?;
        assert_eq!(foo, 1);

        Ok(())
    })
}
