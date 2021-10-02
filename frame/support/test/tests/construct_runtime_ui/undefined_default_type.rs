use frame_support::construct_runtime;
use sp_runtime::{generic, traits::BlakeTwo256};
use sp_core::sr25519;

#[frame_support::pallet]
mod pallet {
    use frame_support::pallet_prelude::*;
	#[pallet::config]
	pub trait Config: frame_system::Config {
        type Balance: Parameter;
    }

	#[pallet::pallet]
	pub struct Pallet<T>(_);
}

pub type Signature = sr25519::Signature;
pub type BlockNumber = u64;
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<u32, Call, Signature, ()>;

#[frame_support::use_default_config_for(Balance)]
impl pallet::Config for Runtime {}

construct_runtime! {
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: system::{Pallet, Call, Storage, Config, Event<T>},
		Pallet: pallet::{Pallet},
	}
}

fn main() {}