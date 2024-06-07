use std::sync::Arc;

use futures_lite::future::block_on;
use futures_lite::FutureExt;

use crate::transport::{self, ServerTransport};
use crate::{Client, ClientMessage, Error, Result};

#[rdaw_macros::api_operations(TestProtocol)]
trait FooOperations {
    async fn get_foo(&self) -> Result<i32>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rdaw_macros::api_protocol(FooOperations)]
struct TestProtocol;

struct TestBackend;

#[rdaw_macros::api_handler(TestProtocol, FooOperations)]
impl TestBackend {
    fn get_foo(&self) -> Result<i32> {
        Ok(1)
    }
}

impl TestBackend {
    async fn handle_message<T: ServerTransport<TestProtocol>>(
        &mut self,
        transport: Arc<T>,
        msg: ClientMessage<TestProtocol>,
    ) -> Result<()> {
        match msg {
            ClientMessage::Request { id, payload } => match payload {
                TestRequest::Foo(req) => self.handle_foo_request(transport, id, req).await,
            },
            ClientMessage::CloseStream { .. } => todo!(),
        }
    }

    async fn handle<T: ServerTransport<TestProtocol>>(&mut self, transport: Arc<T>) -> Result<()> {
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
    let (client_transport, server_transport) = transport::local(None);

    let client = Client::<TestProtocol, _>::new(client_transport);
    let mut server = TestBackend;

    let handle = client
        .clone()
        .handle()
        .or(server.handle(Arc::new(server_transport)));

    let test = async move {
        let foo = client.get_foo().await?;
        assert_eq!(foo, 1);

        Ok(())
    };

    block_on(test.or(handle))
}
