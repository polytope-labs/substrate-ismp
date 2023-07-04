// Copyright (C) 2023 Polytope Labs.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
#![deny(missing_docs)]

//! ISMP Parachain Consensus Inherent Provider
//!
//! This exports the inherent provider for including ISMP parachain consensus updates as block
//! inherents.

use anyhow::anyhow;
use codec::Encode;
use cumulus_primitives_core::{relay_chain::BlockId, PersistedValidationData};
use cumulus_relay_chain_interface::{PHash, RelayChainInterface};
use ismp::{
    consensus::{StateMachineHeight, StateMachineId},
    host::StateMachine,
    messaging::{ConsensusMessage, Message, Proof, ResponseMessage},
    router::{Get, Request},
};
use ismp_parachain::consensus::{self, parachain_header_storage_key, ParachainConsensusProof};
use ismp_parachain_runtime_api::IsmpParachainApi;
use ismp_primitives::LeafIndexQuery;
use ismp_runtime_api::IsmpRuntimeApi;
use pallet_ismp::events::Event;
use primitive_types::H256;
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

/// Implements [`InherentDataProvider`] for providing ISMP updates as inherents.
pub struct IsmpInherentProvider(Option<Vec<Message>>);

impl IsmpInherentProvider {
    /// Create the [`ConsensusMessage`] at the given `relay_parent`. Will be [`None`] if no para ids
    /// have been confguired.
    pub async fn create<C, B>(
        client: Arc<C>,
        relay_parent: PHash,
        relay_chain_interface: &impl RelayChainInterface,
        validation_data: PersistedValidationData,
    ) -> Result<IsmpInherentProvider, anyhow::Error>
    where
        C: sp_api::ProvideRuntimeApi<B> + sp_blockchain::HeaderBackend<B>,
        C::Api: IsmpParachainApi<B> + IsmpRuntimeApi<B, H256>,
        B: BlockT,
    {
        let mut messages = vec![];
        let head = client.info().best_hash;
        let para_ids = client.runtime_api().para_ids(head)?;

        // insert para headers we care about
        if !para_ids.is_empty() {
            let keys = para_ids.iter().map(|id| parachain_header_storage_key(*id).0).collect();
            let storage_proof = relay_chain_interface
                .prove_read(relay_parent, &keys)
                .await?
                .into_iter_nodes()
                .collect();

            let consensus_proof = ParachainConsensusProof {
                para_ids,
                relay_height: validation_data.relay_parent_number,
                storage_proof,
            };
            let message = ConsensusMessage {
                consensus_client_id: consensus::PARACHAIN_CONSENSUS_ID,
                consensus_proof: consensus_proof.encode(),
            };

            messages.push(Message::Consensus(message));
        }

        // relay chain state machine id is 0. todo: make it a constant
        let relay_chain = match client.runtime_api().host_state_machine(head)? {
            StateMachine::Polkadot(_) => StateMachine::Polkadot(0),
            StateMachine::Kusama(_) => StateMachine::Kusama(0),
            s => Err(anyhow!("Invalid host state machine, expected Polkadot/Kusama found {s:?}"))?,
        };

        // check the events in the last block
        let query = client
            .runtime_api()
            .block_events(head)?
            .into_iter()
            .filter_map(|event| match event {
                Event::Request { dest_chain, source_chain, request_nonce: nonce }
                    if dest_chain == relay_chain =>
                {
                    Some(LeafIndexQuery { source_chain, dest_chain, nonce })
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        let requests: Vec<Get> = client
            .runtime_api()
            .get_request_leaf_indices(head, query)
            .and_then(|indices| client.runtime_api().get_requests(head, indices))?
            .into_iter()
            .filter_map(|req| match req {
                Request::Get(get) => Some(get),
                _ => None,
            })
            .collect();

        // todo: batch requests with the same height

        // for every request, read the keys in the relay chain storage.
        for request in requests {
            match client.runtime_api().relay_chain_state_root(head, request.height as u32)? {
                Some(_) => {}
                // ignore unkown heights, they'll timeout naturally.
                None => continue,
            };

            // doesn't exist yet
            let hash = relay_chain_interface.header(BlockId::Number(request.heigh)).await?.hash();

            let proof = relay_chain_interface
                .prove_read(hash, &request.keys)
                .await?
                .into_iter_nodes()
                .collect::<Vec<_>>();

            let proof = Proof {
                height: StateMachineHeight {
                    id: StateMachineId {
                        state_id: relay_chain,
                        consensus_client: consensus::PARACHAIN_CONSENSUS_ID,
                    },
                    height: request.height,
                },
                proof: proof.encode(),
            };

            messages.push(Message::Response(ResponseMessage::Get {
                requests: vec![Request::Get(request)],
                proof,
            }));
        }

        if messages.is_empty() {
            return Ok(IsmpInherentProvider(None))
        }

        Ok(IsmpInherentProvider(Some(messages)))
    }
}

#[async_trait::async_trait]
impl sp_inherents::InherentDataProvider for IsmpInherentProvider {
    async fn provide_inherent_data(
        &self,
        inherent_data: &mut sp_inherents::InherentData,
    ) -> Result<(), sp_inherents::Error> {
        if let Some(ref message) = self.0 {
            inherent_data.put_data(ismp_parachain::INHERENT_IDENTIFIER, message)?;
        }

        Ok(())
    }

    async fn try_handle_error(
        &self,
        _: &sp_inherents::InherentIdentifier,
        _: &[u8],
    ) -> Option<Result<(), sp_inherents::Error>> {
        None
    }
}
