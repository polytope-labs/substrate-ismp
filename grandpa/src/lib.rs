pub mod consensus_message;
pub mod consensus;

use alloc::{vec, vec::Vec};
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use codec::alloc::collections::BTreeSet;
    use super::*;
    use cumulus_primitives_core::{ParaId, relay_chain};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use ismp::host::StateMachine;
    use ismp::messaging::{ConsensusMessage, Message};
    use primitive_types::H256;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The config trait
    #[pallet::config]
    pub trait Config:
    frame_system::Config
    {
        /// The overarching event type
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    /// Mapping of standalone chain consensus state id to 1 state machine.
    #[pallet::storage]
    #[pallet::getter(fn relay_chain_state)]
    pub type StandaloneChainConsensusState<T: Config> =
    StorageMap<_, Blake2_128Concat, Vec<u8>, StateMachine>;

    /// Mapping of relay chain consensus state id to set of para ids.
    #[pallet::storage]
    #[pallet::getter(fn relay_chain_state)]
    pub type RelayChainConsensusState<T: Config> =
    StorageMap<_, Blake2_128Concat, Vec<u8>, BTreeSet<ParaId>>;

    /// Events emitted by this pallet
    #[pallet::event]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {
        /// Standalone Consensus State Already Exists
        StandaloneConsensusStateAlreadyExists,
        /// Standalone Consensus Does not Exist
        StandaloneConsensusStateDontExists,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T>
        where
            <T as frame_system::Config>::Hash: From<H256>,
    {
        /// Add some new parachains to the list of parachains in the relay chain consensus state
        #[pallet::call_index(0)]
        #[pallet::weight(0)]
        pub fn add_parachains(origin: OriginFor<T>, consensus_state_id: Vec<u8>, para_ids: Vec<u32>) -> DispatchResult {
            ensure_root(origin)?;

            RelayChainConsensusState::<T>::mutate(&consensus_state_id, |state_set| {
                para_ids.iter().for_each(|para_id| {
                    if let Some(set) = state_set {
                        set.insert(*para_id);
                    } else {
                        let mut new_set = BTreeSet::new();
                        new_set.insert(*para_id);
                        *state_set = Some(new_set);
                    }
                });
            });

            Ok(())
        }

        /// Remove some parachains from the list of parachains in the relay chain consensus state
        #[pallet::call_index(1)]
        #[pallet::weight(0)]
        pub fn remove_parachains(origin: OriginFor<T>, para_ids: Vec<u32>) -> DispatchResult {
            ensure_root(origin)?;
            para_ids.iter().for_each(|para_id| {
                RelayChainConsensusState::<T>::mutate_exists(para_id, |state_set| {
                    if let Some(state_set) = state_set {
                        state_set.remove(para_id);
                        if state_set.is_empty() {
                            *state_set = None;
                        }
                    }
                });
            });

            Ok(())
        }

        /// Add state machine to standalone consensus state storage
        #[pallet::call_index(2)]
        #[pallet::weight(0)]
        pub fn set_state_machine(origin: OriginFor<T>, consensus_state_id: Vec<u8>, state_machine: StateMachine) -> DispatchResult {
            ensure_root(origin)?;

            StandaloneChainConsensusState::<T>::insert(&consensus_state_id, state_machine);

            Ok(())
        }
    }
}
