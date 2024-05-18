macro_rules! define_dispatch_ops {
    (
        pub enum $EnumName:ident;

        impl $TypeName:ident {
            pub fn $fn_name:ident;
        }

        impl $TraitName:ident for $HandleName:ident;

        $($Name:ident => $method:ident($($arg:ident: $ArgTy:ty),* $(,)?) -> $RetTy:ty;)*
    ) => {
        pub enum $EnumName {
            $(
                $Name {
                    $($arg: $ArgTy,)*
                    sender: oneshot::Sender<$RetTy>,
                }
            ),*
        }

        impl $TypeName {
            pub async fn $fn_name(&mut self, op: $EnumName) {
                match op {
                    $(
                        $EnumName::$Name { $($arg,)* sender } => {
                            let res = self.$method($($arg),*).await;
                            let _ = sender.send(res);
                        }
                    ),*
                }
            }
        }

        impl $TraitName for $HandleName {
            $(
                #[allow(refining_impl_trait)]
                async fn $method(&self, $($arg: $ArgTy,)*) -> $RetTy {
                    let (sender, receiver) = oneshot::channel();
                    self.sender
                        .send($EnumName::$Name { $($arg,)* sender}.into())
                        .await
                        .map_err(|_| rdaw_api::Error::Disconnected)?;
                    receiver.await
                        .map_err(|_| rdaw_api::Error::Disconnected)?
                }
            )*
        }
    };
}

pub(crate) use define_dispatch_ops;
