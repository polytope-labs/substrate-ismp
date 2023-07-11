//! IsmpDispatcher precompiles for pallet-evm

use crate::{
    dispatcher::Dispatcher,
    evm::abi::{
        DispatchGet as SolDispatchGet, DispatchPost as SolDispatchPost,
        PostResponse as SolPostResponse,
    },
    Config,
};
use alloc::str::FromStr;
use alloy_sol_types::SolType;
use core::marker::PhantomData;
use fp_evm::{
    ExitError, ExitSucceed, Precompile, PrecompileFailure, PrecompileHandle, PrecompileOutput,
    PrecompileResult,
};
use frame_support::weights::Weight;
use ismp_rs::{
    host::StateMachine,
    router::{DispatchGet, DispatchPost, DispatchRequest, IsmpDispatcher, Post, PostResponse},
};
use pallet_evm::GasWeightMapping;
use sp_core::{H256, U256};

/// Ismp Request Dispatcher precompile for evm contracts
pub struct IsmpPostDispatcher<T> {
    _marker: PhantomData<T>,
}

impl<T> Precompile for IsmpPostDispatcher<T>
where
    T: Config + pallet_evm::Config,
    <T as frame_system::Config>::Hash: From<H256>,
{
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        let input = handle.input();
        let context = handle.context();

        // todo:  benchmark dispatcher and use weight info here
        let weight = Weight::zero();

        let cost = T::GasWeightMapping::weight_to_gas(weight);

        let dispatcher = Dispatcher::<T>::default();
        let post_dispatch =
            SolDispatchPost::decode(input, true).map_err(|e| PrecompileFailure::Error {
                exit_status: ExitError::Other(format!("Failed to decode input: {:?}", e).into()),
            })?;
        let post_dispatch = DispatchPost {
            dest: parse_state_machine(post_dispatch.dest)?,
            from: context.caller.0.to_vec(),
            to: post_dispatch.to,
            timeout_timestamp: u256_to_u64(post_dispatch.timeoutTimestamp)?,
            data: post_dispatch.data,
        };
        handle.record_cost(cost)?;
        match dispatcher.dispatch_request(DispatchRequest::Post(post_dispatch)) {
            Ok(_) => Ok(PrecompileOutput { exit_status: ExitSucceed::Stopped, output: vec![] }),
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(format!("dispatch execution failed: {:?}", e).into()),
            }),
        }
    }
}

/// Ismp Request Dispatcher precompile for evm contracts
pub struct IsmpGetDispatcher<T> {
    _marker: PhantomData<T>,
}

impl<T> Precompile for IsmpGetDispatcher<T>
where
    T: Config + pallet_evm::Config,
    <T as frame_system::Config>::Hash: From<H256>,
{
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        let input = handle.input();
        let context = handle.context();

        // todo:  benchmark dispatcher and use weight here
        let weight = Weight::zero();

        let cost = T::GasWeightMapping::weight_to_gas(weight);

        let dispatcher = Dispatcher::<T>::default();

        let get_dispatch =
            SolDispatchGet::decode(input, true).map_err(|e| PrecompileFailure::Error {
                exit_status: ExitError::Other(format!("Failed to decode input: {:?}", e).into()),
            })?;
        let get_dispatch = DispatchGet {
            dest: parse_state_machine(get_dispatch.dest)?,
            from: context.caller.0.to_vec(),
            keys: get_dispatch.keys,
            height: u256_to_u64(get_dispatch.height)?,
            timeout_timestamp: u256_to_u64(get_dispatch.timeoutTimestamp)?,
        };
        handle.record_cost(cost)?;
        match dispatcher.dispatch_request(DispatchRequest::Get(get_dispatch)) {
            Ok(_) => Ok(PrecompileOutput { exit_status: ExitSucceed::Stopped, output: vec![] }),
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(format!("dispatch execution failed: {:?}", e).into()),
            }),
        }
    }
}

/// Ismp Response Dispatcher precompile for evm contracts
pub struct IsmpResponseDispatcher<T> {
    _marker: PhantomData<T>,
}

impl<T> Precompile for IsmpResponseDispatcher<T>
where
    T: Config + pallet_evm::Config,
    <T as frame_system::Config>::Hash: From<H256>,
{
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        let input = handle.input();

        // todo:  benchmark dispatcher and use weight here
        let weight = Weight::zero();

        let cost = T::GasWeightMapping::weight_to_gas(weight);

        let dispatcher = Dispatcher::<T>::default();
        let post_response =
            SolPostResponse::decode(input, true).map_err(|e| PrecompileFailure::Error {
                exit_status: ExitError::Other(format!("Failed to decode input: {:?}", e).into()),
            })?;
        let post_response = PostResponse {
            post: Post {
                source: parse_state_machine(post_response.request.source)?,
                dest: parse_state_machine(post_response.request.dest)?,
                nonce: u256_to_u64(post_response.request.nonce)?,
                from: post_response.request.from,
                to: post_response.request.to,
                timeout_timestamp: u256_to_u64(post_response.request.timeoutTimestamp)?,
                data: post_response.request.data,
            },
            response: post_response.response,
        };
        handle.record_cost(cost)?;

        match dispatcher.dispatch_response(post_response) {
            Ok(_) => Ok(PrecompileOutput { exit_status: ExitSucceed::Stopped, output: vec![] }),
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(format!("dispatch execution failed: {:?}", e).into()),
            }),
        }
    }
}

/// Convert u256 to u64 with overflow check
fn u256_to_u64(value: alloy_primitives::U256) -> Result<u64, PrecompileFailure> {
    let value = U256::from_big_endian(value.to_be_bytes::<32>().as_slice());
    for i in &value.0[1..] {
        if *i != 0u64 {
            return Err(PrecompileFailure::Error {
                exit_status: ExitError::Other("Integer Overflow".into()),
            })
        }
    }
    Ok(value.as_u64())
}

/// Parse state machine from utf8 bytes
fn parse_state_machine(bytes: Vec<u8>) -> Result<StateMachine, PrecompileFailure> {
    StateMachine::from_str(&String::from_utf8(bytes).unwrap_or_default()).map_err(|e| {
        PrecompileFailure::Error {
            exit_status: ExitError::Other(format!("Failed to destination chain: {:?}", e).into()),
        }
    })
}
