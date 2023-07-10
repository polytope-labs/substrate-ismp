//! Module Handler for EVM contracts
use crate::Config;
use core::marker::PhantomData;
use ismp_rs::{
    contracts::Gas,
    error::Error,
    module::IsmpModule,
    router::{Post, Request, Response},
};

/// EVM contract handler
pub struct EvmContractHandler<T: Config + pallet_evm::Config>(PhantomData<T>);

impl<T: Config + pallet_evm::Config> IsmpModule for EvmContractHandler<T> {
    fn on_accept(&self, request: Post) -> Result<Gas, Error> {
        todo!()
    }

    fn on_response(&self, response: Response) -> Result<Gas, Error> {
        todo!()
    }

    fn on_timeout(&self, request: Request) -> Result<Gas, Error> {
        todo!()
    }
}
