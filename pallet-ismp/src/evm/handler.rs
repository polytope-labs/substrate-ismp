//! Module Handler for EVM contracts
use crate::{
    evm::abi::{ContractData as SolContractData, OnAcceptCall, PostRequest},
    primitives::ModuleId,
    Config,
};
use alloy_primitives::U256;
use alloy_sol_types::{SolCall, SolType};
use core::marker::PhantomData;
use frame_support::traits::Get;
use ismp_rs::{
    contracts::Gas,
    error::Error,
    module::IsmpModule,
    router::{Post, Request, Response},
};
use pallet_evm::GasWeightMapping;
use sp_core::H160;

/// Handler host address
/// Contracts should only allow ismp module callbacks to be executed by this address
pub const EVM_HOST_ADDRESS: H160 = H160::zero();
/// EVM contract handler
pub struct EvmContractHandler<T: Config + pallet_evm::Config>(PhantomData<T>);

impl<T: Config + pallet_evm::Config> IsmpModule for EvmContractHandler<T> {
    fn on_accept(&self, request: Post) -> Result<Gas, Error> {
        let target_contract = parse_contract_id(&request.to)?;
        let contract_data = SolContractData::decode(&request.data, true)
            .map_err(|_| Error::ImplementationSpecific("Failed to decode input".to_string()))?;
        let gas_limit = contract_data.gasLimit;
        let post = PostRequest {
            source: request.source.to_string().as_bytes().to_vec(),
            dest: request.dest.to_string().as_bytes().to_vec(),
            nonce: u64_to_u256(request.nonce)?,
            timeoutTimestamp: u64_to_u256(request.timeout_timestamp)?,
            from: request.from,
            to: request.to,
            data: request.data,
        };
        let call_data = OnAcceptCall { request: post }.encode();
        match <<T as pallet_evm::Config>::Runner as pallet_evm::Runner<T>>::call(
            EVM_HOST_ADDRESS,
            target_contract,
            call_data,
            Default::default(),
            gas_limit,
            None,
            None,
            None,
            Default::default(),
            true,
            true,
            None,
            None,
            <T as pallet_evm::Config>::config(),
        ) {
            Ok(info) => {
                Ok(Gas {
                    // used gas would be at most equal to the gas limit as such we could convert
                    // U256 to U64 without panic
                    gas_used: Some(info.used_gas.standard.low_u64()),
                    gas_limit: Some(gas_limit),
                })
            }
            Err(error) => {
                // We still return ok so we can compensate for used gas only
                Ok(Gas {
                    gas_used: Some(<T as pallet_evm::Config>::GasWeightMapping::weight_to_gas(
                        error.weight,
                    )),
                    gas_limit: Some(gas_limit),
                })
            }
        }
    }

    fn on_response(&self, response: Response) -> Result<Gas, Error> {
        let target_contract = parse_contract_id(&response.destination_module())?;
        match response {
            Response::Post(post) => {}
            Response::Get(get) => {}
        }
        todo!()
    }

    fn on_timeout(&self, request: Request) -> Result<Gas, Error> {
        let target_contract = parse_contract_id(&request.source_module())?;
        todo!()
    }
}

/// Parse contract id from raw bytes
pub fn parse_contract_id(bytes: &[u8]) -> Result<H160, Error> {
    let module_id =
        ModuleId::from_bytes(bytes).map_err(|e| Error::ImplementationSpecific(e.to_string()))?;
    match module_id {
        ModuleId::Evm(id) => Ok(id),
        _ => Err(Error::ImplementationSpecific("Expected Evm contract id".to_string())),
    }
}

/// Convert u64 to U256
fn u64_to_u256(value: u64) -> Result<U256, Error> {
    U256::try_from(value)
        .map_err(|_| Error::ImplementationSpecific("Failed to convert u64 to u256".to_string()))
}
