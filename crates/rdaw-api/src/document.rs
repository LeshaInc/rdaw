use crate::{BackendProtocol, Result};

slotmap::new_key_type! {
    pub struct DocumentId;
}

#[rdaw_rpc::operations(protocol = BackendProtocol)]
pub trait DocumentOperations {
    async fn create_document(&self) -> Result<DocumentId>;
}
