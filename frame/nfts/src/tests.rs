// This file is part of Substrate.

// Copyright (C) 2019-2022 Parity Technologies (UK) Ltd.
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

//! Tests for Nfts pallet.

use crate::{mock::*, Event, *};
use enumflags2::BitFlags;
use frame_support::{
	assert_noop, assert_ok,
	dispatch::Dispatchable,
	traits::{Currency, Get},
};
use pallet_balances::Error as BalancesError;
use sp_std::prelude::*;

fn items() -> Vec<(u64, u32, u32)> {
	let mut r: Vec<_> = Account::<Test>::iter().map(|x| x.0).collect();
	r.sort();
	let mut s: Vec<_> = Item::<Test>::iter().map(|x| (x.2.owner, x.0, x.1)).collect();
	s.sort();
	assert_eq!(r, s);
	for collection in Item::<Test>::iter()
		.map(|x| x.0)
		.scan(None, |s, item| {
			if s.map_or(false, |last| last == item) {
				*s = Some(item);
				Some(None)
			} else {
				Some(Some(item))
			}
		})
		.flatten()
	{
		let details = Collection::<Test>::get(collection).unwrap();
		let items = Item::<Test>::iter_prefix(collection).count() as u32;
		assert_eq!(details.items, items);
	}
	r
}

fn collections() -> Vec<(u64, u32)> {
	let mut r: Vec<_> = CollectionAccount::<Test>::iter().map(|x| (x.0, x.1)).collect();
	r.sort();
	let mut s: Vec<_> = Collection::<Test>::iter().map(|x| (x.1.owner, x.0)).collect();
	s.sort();
	assert_eq!(r, s);
	r
}

macro_rules! bvec {
	($( $x:tt )*) => {
		vec![$( $x )*].try_into().unwrap()
	}
}

fn attributes(collection: u32) -> Vec<(Option<u32>, Vec<u8>, Vec<u8>)> {
	let mut s: Vec<_> = Attribute::<Test>::iter_prefix((collection,))
		.map(|(k, v)| (k.0, k.1.into(), v.0.into()))
		.collect();
	s.sort();
	s
}

fn approvals(collection_id: u32, item_id: u32) -> Vec<(u64, Option<u64>)> {
	let item = Item::<Test>::get(collection_id, item_id).unwrap();
	let s: Vec<_> = item.approvals.into_iter().collect();
	s
}

fn events() -> Vec<Event<Test>> {
	let result = System::events()
		.into_iter()
		.map(|r| r.event)
		.filter_map(|e| if let mock::RuntimeEvent::Nfts(inner) = e { Some(inner) } else { None })
		.collect::<Vec<_>>();

	System::reset_events();

	result
}

fn default_collection_config() -> CollectionConfigFor<Test> {
	CollectionConfig { settings: CollectionSetting::FreeHolding.into(), ..Default::default() }
}

fn default_item_config() -> ItemConfig {
	ItemConfig { settings: ItemSettings::default() }
}

fn make_collection_config(settings: BitFlags<CollectionSetting>) -> CollectionConfigFor<Test> {
	CollectionConfig { settings: settings.into(), ..Default::default() }
}

#[test]
fn basic_setup_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(items(), vec![]);
	});
}

#[test]
fn basic_minting_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
		assert_eq!(collections(), vec![(1, 0)]);
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 42, None));
		assert_eq!(items(), vec![(1, 0, 42)]);

		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 2, default_collection_config()));
		assert_eq!(collections(), vec![(1, 0), (2, 1)]);
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(2), 1, 69, 1, default_item_config()));
		assert_eq!(items(), vec![(1, 0, 42), (1, 1, 69)]);
	});
}

#[test]
fn lifecycle_should_work() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, 100);
		assert_ok!(Nfts::create(RuntimeOrigin::signed(1), 1, default_collection_config()));
		assert_eq!(Balances::reserved_balance(&1), 2);
		assert_eq!(collections(), vec![(1, 0)]);
		assert_ok!(Nfts::set_collection_metadata(RuntimeOrigin::signed(1), 0, bvec![0, 0]));
		assert_eq!(Balances::reserved_balance(&1), 5);
		assert!(CollectionMetadataOf::<Test>::contains_key(0));

		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 0, 42, 10, default_item_config()));
		assert_eq!(Balances::reserved_balance(&1), 6);
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 0, 69, 20, default_item_config()));
		assert_eq!(Balances::reserved_balance(&1), 7);
		assert_eq!(items(), vec![(10, 0, 42), (20, 0, 69)]);
		assert_eq!(Collection::<Test>::get(0).unwrap().items, 2);
		assert_eq!(Collection::<Test>::get(0).unwrap().item_metadatas, 0);

		assert_ok!(Nfts::set_metadata(RuntimeOrigin::signed(1), 0, 42, bvec![42, 42]));
		assert_eq!(Balances::reserved_balance(&1), 10);
		assert!(ItemMetadataOf::<Test>::contains_key(0, 42));
		assert_ok!(Nfts::set_metadata(RuntimeOrigin::signed(1), 0, 69, bvec![69, 69]));
		assert_eq!(Balances::reserved_balance(&1), 13);
		assert!(ItemMetadataOf::<Test>::contains_key(0, 69));

		let w = Collection::<Test>::get(0).unwrap().destroy_witness();
		assert_eq!(w.items, 2);
		assert_eq!(w.item_metadatas, 2);
		assert_ok!(Nfts::destroy(RuntimeOrigin::signed(1), 0, w));
		assert_eq!(Balances::reserved_balance(&1), 0);

		assert!(!Collection::<Test>::contains_key(0));
		assert!(!CollectionConfigOf::<Test>::contains_key(0));
		assert!(!Item::<Test>::contains_key(0, 42));
		assert!(!Item::<Test>::contains_key(0, 69));
		assert!(!CollectionMetadataOf::<Test>::contains_key(0));
		assert!(!ItemMetadataOf::<Test>::contains_key(0, 42));
		assert!(!ItemMetadataOf::<Test>::contains_key(0, 69));
		assert_eq!(collections(), vec![]);
		assert_eq!(items(), vec![]);
	});
}

#[test]
fn destroy_with_bad_witness_should_not_work() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, 100);
		assert_ok!(Nfts::create(RuntimeOrigin::signed(1), 1, default_collection_config()));

		let w = Collection::<Test>::get(0).unwrap().destroy_witness();
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 42, None));
		assert_noop!(Nfts::destroy(RuntimeOrigin::signed(1), 0, w), Error::<Test>::BadWitness);
	});
}

#[test]
fn mint_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 42, None));
		assert_eq!(Nfts::owner(0, 42).unwrap(), 1);
		assert_eq!(collections(), vec![(1, 0)]);
		assert_eq!(items(), vec![(1, 0, 42)]);

		// validate minting start and end settings
		assert_ok!(Nfts::update_mint_settings(
			RuntimeOrigin::signed(1),
			0,
			MintSettings { start_block: Some(2), end_block: Some(3), ..Default::default() }
		));

		System::set_block_number(1);
		assert_noop!(
			Nfts::mint(RuntimeOrigin::signed(1), 0, 43, None),
			Error::<Test>::MintNotStated
		);
		System::set_block_number(4);
		assert_noop!(Nfts::mint(RuntimeOrigin::signed(1), 0, 43, None), Error::<Test>::MintEnded);

		// validate price
		assert_ok!(Nfts::update_mint_settings(
			RuntimeOrigin::signed(1),
			0,
			MintSettings { mint_type: MintType::Public, price: Some(1), ..Default::default() }
		));
		Balances::make_free_balance_be(&2, 100);
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(2), 0, 43, None));
		assert_eq!(Balances::total_balance(&2), 99);

		// validate types
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
		assert_ok!(Nfts::update_mint_settings(
			RuntimeOrigin::signed(1),
			1,
			MintSettings { mint_type: MintType::HolderOf(0), ..Default::default() }
		));
		assert_noop!(Nfts::mint(RuntimeOrigin::signed(3), 1, 42, None), Error::<Test>::BadWitness);
		assert_noop!(Nfts::mint(RuntimeOrigin::signed(2), 1, 42, None), Error::<Test>::BadWitness);
		assert_noop!(
			Nfts::mint(RuntimeOrigin::signed(2), 1, 42, Some(MintWitness { owner_of_item: 42 })),
			Error::<Test>::BadWitness
		);
		assert_ok!(Nfts::mint(
			RuntimeOrigin::signed(2),
			1,
			42,
			Some(MintWitness { owner_of_item: 43 })
		));
	});
}

#[test]
fn transfer_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 0, 42, 2, default_item_config()));

		assert_ok!(Nfts::transfer(RuntimeOrigin::signed(2), 0, 42, 3));
		assert_eq!(items(), vec![(3, 0, 42)]);
		assert_noop!(
			Nfts::transfer(RuntimeOrigin::signed(2), 0, 42, 4),
			Error::<Test>::NoPermission
		);

		assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(3), 0, 42, 2, None));
		assert_ok!(Nfts::transfer(RuntimeOrigin::signed(2), 0, 42, 4));

		// validate we can't transfer non-transferable items
		let collection_id = 1;
		assert_ok!(Nfts::force_create(
			RuntimeOrigin::root(),
			1,
			make_collection_config(
				CollectionSetting::NonTransferableItems | CollectionSetting::FreeHolding
			)
		));

		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 1, 1, 42, default_item_config()));

		assert_noop!(
			Nfts::transfer(RuntimeOrigin::signed(1), collection_id, 42, 3,),
			Error::<Test>::ItemsNotTransferable
		);
	});
}

#[test]
fn freezing_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 42, None));
		assert_ok!(Nfts::freeze(RuntimeOrigin::signed(1), 0, 42));
		assert_noop!(Nfts::transfer(RuntimeOrigin::signed(1), 0, 42, 2), Error::<Test>::ItemLocked);

		assert_ok!(Nfts::thaw(RuntimeOrigin::signed(1), 0, 42));
		assert_ok!(Nfts::lock_collection(
			RuntimeOrigin::signed(1),
			0,
			CollectionSettings(CollectionSetting::NonTransferableItems.into())
		));
		assert_noop!(
			Nfts::transfer(RuntimeOrigin::signed(1), 0, 42, 2),
			Error::<Test>::ItemsNotTransferable
		);

		assert_ok!(Nfts::force_collection_status(
			RuntimeOrigin::root(),
			0,
			1,
			1,
			1,
			1,
			CollectionConfig::default(),
		));
		assert_ok!(Nfts::transfer(RuntimeOrigin::signed(1), 0, 42, 2));
	});
}

#[test]
fn origin_guards_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 42, None));

		Balances::make_free_balance_be(&2, 100);
		assert_ok!(Nfts::set_accept_ownership(RuntimeOrigin::signed(2), Some(0)));
		assert_noop!(
			Nfts::transfer_ownership(RuntimeOrigin::signed(2), 0, 2),
			Error::<Test>::NoPermission
		);
		assert_noop!(
			Nfts::set_team(RuntimeOrigin::signed(2), 0, 2, 2, 2),
			Error::<Test>::NoPermission
		);
		assert_noop!(Nfts::freeze(RuntimeOrigin::signed(2), 0, 42), Error::<Test>::NoPermission);
		assert_noop!(Nfts::thaw(RuntimeOrigin::signed(2), 0, 42), Error::<Test>::NoPermission);
		assert_noop!(
			Nfts::mint(RuntimeOrigin::signed(2), 0, 69, None),
			Error::<Test>::NoPermission
		);
		assert_noop!(
			Nfts::burn(RuntimeOrigin::signed(2), 0, 42, None),
			Error::<Test>::NoPermission
		);
		let w = Collection::<Test>::get(0).unwrap().destroy_witness();
		assert_noop!(Nfts::destroy(RuntimeOrigin::signed(2), 0, w), Error::<Test>::NoPermission);
	});
}

#[test]
fn transfer_owner_should_work() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, 100);
		Balances::make_free_balance_be(&2, 100);
		Balances::make_free_balance_be(&3, 100);
		assert_ok!(Nfts::create(RuntimeOrigin::signed(1), 1, default_collection_config()));
		assert_eq!(collections(), vec![(1, 0)]);
		assert_noop!(
			Nfts::transfer_ownership(RuntimeOrigin::signed(1), 0, 2),
			Error::<Test>::Unaccepted
		);
		assert_ok!(Nfts::set_accept_ownership(RuntimeOrigin::signed(2), Some(0)));
		assert_ok!(Nfts::transfer_ownership(RuntimeOrigin::signed(1), 0, 2));

		assert_eq!(collections(), vec![(2, 0)]);
		assert_eq!(Balances::total_balance(&1), 98);
		assert_eq!(Balances::total_balance(&2), 102);
		assert_eq!(Balances::reserved_balance(&1), 0);
		assert_eq!(Balances::reserved_balance(&2), 2);

		assert_ok!(Nfts::set_accept_ownership(RuntimeOrigin::signed(1), Some(0)));
		assert_noop!(
			Nfts::transfer_ownership(RuntimeOrigin::signed(1), 0, 1),
			Error::<Test>::NoPermission
		);

		// Mint and set metadata now and make sure that deposit gets transferred back.
		assert_ok!(Nfts::set_collection_metadata(RuntimeOrigin::signed(2), 0, bvec![0u8; 20]));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 42, None));
		assert_ok!(Nfts::set_metadata(RuntimeOrigin::signed(2), 0, 42, bvec![0u8; 20]));
		assert_ok!(Nfts::set_accept_ownership(RuntimeOrigin::signed(3), Some(0)));
		assert_ok!(Nfts::transfer_ownership(RuntimeOrigin::signed(2), 0, 3));
		assert_eq!(collections(), vec![(3, 0)]);
		assert_eq!(Balances::total_balance(&2), 57);
		assert_eq!(Balances::total_balance(&3), 145);
		assert_eq!(Balances::reserved_balance(&2), 0);
		assert_eq!(Balances::reserved_balance(&3), 45);

		// 2's acceptence from before is reset when it became owner, so it cannot be transfered
		// without a fresh acceptance.
		assert_noop!(
			Nfts::transfer_ownership(RuntimeOrigin::signed(3), 0, 2),
			Error::<Test>::Unaccepted
		);
	});
}

#[test]
fn set_team_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
		assert_ok!(Nfts::set_team(RuntimeOrigin::signed(1), 0, 2, 3, 4));

		assert_ok!(Nfts::mint(RuntimeOrigin::signed(2), 0, 42, None));
		assert_ok!(Nfts::freeze(RuntimeOrigin::signed(4), 0, 42));
		assert_ok!(Nfts::thaw(RuntimeOrigin::signed(4), 0, 42));
		assert_ok!(Nfts::transfer(RuntimeOrigin::signed(3), 0, 42, 3));
		assert_ok!(Nfts::burn(RuntimeOrigin::signed(3), 0, 42, None));
	});
}

#[test]
fn set_collection_metadata_should_work() {
	new_test_ext().execute_with(|| {
		// Cannot add metadata to unknown item
		assert_noop!(
			Nfts::set_collection_metadata(RuntimeOrigin::signed(1), 0, bvec![0u8; 20]),
			Error::<Test>::NoConfig,
		);
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, CollectionConfig::default()));
		// Cannot add metadata to unowned item
		assert_noop!(
			Nfts::set_collection_metadata(RuntimeOrigin::signed(2), 0, bvec![0u8; 20]),
			Error::<Test>::NoPermission,
		);

		// Successfully add metadata and take deposit
		Balances::make_free_balance_be(&1, 30);
		assert_ok!(Nfts::set_collection_metadata(RuntimeOrigin::signed(1), 0, bvec![0u8; 20]));
		assert_eq!(Balances::free_balance(&1), 9);
		assert!(CollectionMetadataOf::<Test>::contains_key(0));

		// Force origin works, too.
		assert_ok!(Nfts::set_collection_metadata(RuntimeOrigin::root(), 0, bvec![0u8; 18]));

		// Update deposit
		assert_ok!(Nfts::set_collection_metadata(RuntimeOrigin::signed(1), 0, bvec![0u8; 15]));
		assert_eq!(Balances::free_balance(&1), 14);
		assert_ok!(Nfts::set_collection_metadata(RuntimeOrigin::signed(1), 0, bvec![0u8; 25]));
		assert_eq!(Balances::free_balance(&1), 4);

		// Cannot over-reserve
		assert_noop!(
			Nfts::set_collection_metadata(RuntimeOrigin::signed(1), 0, bvec![0u8; 40]),
			BalancesError::<Test, _>::InsufficientBalance,
		);

		// Can't set or clear metadata once frozen
		assert_ok!(Nfts::set_collection_metadata(RuntimeOrigin::signed(1), 0, bvec![0u8; 15]));
		assert_ok!(Nfts::lock_collection(
			RuntimeOrigin::signed(1),
			0,
			CollectionSettings(CollectionSetting::LockedMetadata.into())
		));
		assert_noop!(
			Nfts::set_collection_metadata(RuntimeOrigin::signed(1), 0, bvec![0u8; 15]),
			Error::<Test, _>::LockedCollectionMetadata,
		);
		assert_noop!(
			Nfts::clear_collection_metadata(RuntimeOrigin::signed(1), 0),
			Error::<Test>::LockedCollectionMetadata
		);

		// Clear Metadata
		assert_ok!(Nfts::set_collection_metadata(RuntimeOrigin::root(), 0, bvec![0u8; 15]));
		assert_noop!(
			Nfts::clear_collection_metadata(RuntimeOrigin::signed(2), 0),
			Error::<Test>::NoPermission
		);
		assert_noop!(
			Nfts::clear_collection_metadata(RuntimeOrigin::signed(1), 1),
			Error::<Test>::UnknownCollection
		);
		assert_noop!(
			Nfts::clear_collection_metadata(RuntimeOrigin::signed(1), 0),
			Error::<Test>::LockedCollectionMetadata
		);
		assert_ok!(Nfts::clear_collection_metadata(RuntimeOrigin::root(), 0));
		assert!(!CollectionMetadataOf::<Test>::contains_key(0));
	});
}

#[test]
fn set_item_metadata_should_work() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, 30);

		// Cannot add metadata to unknown item
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, CollectionConfig::default()));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 42, None));
		// Cannot add metadata to unowned item
		assert_noop!(
			Nfts::set_metadata(RuntimeOrigin::signed(2), 0, 42, bvec![0u8; 20]),
			Error::<Test>::NoPermission,
		);

		// Successfully add metadata and take deposit
		assert_ok!(Nfts::set_metadata(RuntimeOrigin::signed(1), 0, 42, bvec![0u8; 20]));
		assert_eq!(Balances::free_balance(&1), 8);
		assert!(ItemMetadataOf::<Test>::contains_key(0, 42));

		// Force origin works, too.
		assert_ok!(Nfts::set_metadata(RuntimeOrigin::root(), 0, 42, bvec![0u8; 18]));

		// Update deposit
		assert_ok!(Nfts::set_metadata(RuntimeOrigin::signed(1), 0, 42, bvec![0u8; 15]));
		assert_eq!(Balances::free_balance(&1), 13);
		assert_ok!(Nfts::set_metadata(RuntimeOrigin::signed(1), 0, 42, bvec![0u8; 25]));
		assert_eq!(Balances::free_balance(&1), 3);

		// Cannot over-reserve
		assert_noop!(
			Nfts::set_metadata(RuntimeOrigin::signed(1), 0, 42, bvec![0u8; 40]),
			BalancesError::<Test, _>::InsufficientBalance,
		);

		// Can't set or clear metadata once frozen
		assert_ok!(Nfts::set_metadata(RuntimeOrigin::signed(1), 0, 42, bvec![0u8; 15]));
		assert_ok!(Nfts::lock_item(RuntimeOrigin::signed(1), 0, 42, true, false));
		assert_noop!(
			Nfts::set_metadata(RuntimeOrigin::signed(1), 0, 42, bvec![0u8; 15]),
			Error::<Test, _>::LockedItemMetadata,
		);
		assert_noop!(
			Nfts::clear_metadata(RuntimeOrigin::signed(1), 0, 42),
			Error::<Test>::LockedItemMetadata,
		);

		// Clear Metadata
		assert_ok!(Nfts::set_metadata(RuntimeOrigin::root(), 0, 42, bvec![0u8; 15]));
		assert_noop!(
			Nfts::clear_metadata(RuntimeOrigin::signed(2), 0, 42),
			Error::<Test>::NoPermission,
		);
		assert_noop!(
			Nfts::clear_metadata(RuntimeOrigin::signed(1), 1, 42),
			Error::<Test>::UnknownCollection,
		);
		assert_ok!(Nfts::clear_metadata(RuntimeOrigin::root(), 0, 42));
		assert!(!ItemMetadataOf::<Test>::contains_key(0, 42));
	});
}

#[test]
fn set_attribute_should_work() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, 100);

		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, CollectionConfig::default()));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 0, None));

		assert_ok!(Nfts::set_attribute(RuntimeOrigin::signed(1), 0, None, bvec![0], bvec![0]));
		assert_ok!(Nfts::set_attribute(RuntimeOrigin::signed(1), 0, Some(0), bvec![0], bvec![0]));
		assert_ok!(Nfts::set_attribute(RuntimeOrigin::signed(1), 0, Some(0), bvec![1], bvec![0]));
		assert_eq!(
			attributes(0),
			vec![
				(None, bvec![0], bvec![0]),
				(Some(0), bvec![0], bvec![0]),
				(Some(0), bvec![1], bvec![0]),
			]
		);
		assert_eq!(Balances::reserved_balance(1), 10);

		assert_ok!(Nfts::set_attribute(RuntimeOrigin::signed(1), 0, None, bvec![0], bvec![0; 10]));
		assert_eq!(
			attributes(0),
			vec![
				(None, bvec![0], bvec![0; 10]),
				(Some(0), bvec![0], bvec![0]),
				(Some(0), bvec![1], bvec![0]),
			]
		);
		assert_eq!(Balances::reserved_balance(1), 19);

		assert_ok!(Nfts::clear_attribute(RuntimeOrigin::signed(1), 0, Some(0), bvec![1]));
		assert_eq!(
			attributes(0),
			vec![(None, bvec![0], bvec![0; 10]), (Some(0), bvec![0], bvec![0]),]
		);
		assert_eq!(Balances::reserved_balance(1), 16);

		let w = Collection::<Test>::get(0).unwrap().destroy_witness();
		assert_ok!(Nfts::destroy(RuntimeOrigin::signed(1), 0, w));
		assert_eq!(attributes(0), vec![]);
		assert_eq!(Balances::reserved_balance(1), 0);
	});
}

#[test]
fn set_attribute_should_respect_lock() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, 100);

		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, CollectionConfig::default()));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 0, None));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 1, None));

		assert_ok!(Nfts::set_attribute(RuntimeOrigin::signed(1), 0, None, bvec![0], bvec![0]));
		assert_ok!(Nfts::set_attribute(RuntimeOrigin::signed(1), 0, Some(0), bvec![0], bvec![0]));
		assert_ok!(Nfts::set_attribute(RuntimeOrigin::signed(1), 0, Some(1), bvec![0], bvec![0]));
		assert_eq!(
			attributes(0),
			vec![
				(None, bvec![0], bvec![0]),
				(Some(0), bvec![0], bvec![0]),
				(Some(1), bvec![0], bvec![0]),
			]
		);
		assert_eq!(Balances::reserved_balance(1), 11);

		assert_ok!(Nfts::set_collection_metadata(RuntimeOrigin::signed(1), 0, bvec![]));
		assert_ok!(Nfts::lock_collection(
			RuntimeOrigin::signed(1),
			0,
			CollectionSettings(CollectionSetting::LockedAttributes.into())
		));

		let e = Error::<Test>::LockedCollectionAttributes;
		assert_noop!(Nfts::set_attribute(RuntimeOrigin::signed(1), 0, None, bvec![0], bvec![0]), e);
		assert_ok!(Nfts::set_attribute(RuntimeOrigin::signed(1), 0, Some(0), bvec![0], bvec![1]));

		assert_ok!(Nfts::lock_item(RuntimeOrigin::signed(1), 0, 0, false, true));
		let e = Error::<Test>::LockedItemAttributes;
		assert_noop!(
			Nfts::set_attribute(RuntimeOrigin::signed(1), 0, Some(0), bvec![0], bvec![1]),
			e
		);
		assert_ok!(Nfts::set_attribute(RuntimeOrigin::signed(1), 0, Some(1), bvec![0], bvec![1]));
	});
}

#[test]
fn preserve_config_for_frozen_items() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, 100);

		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, CollectionConfig::default()));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 0, None));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 1, None));

		// if the item is not locked/frozen then the config gets deleted on item burn
		assert_ok!(Nfts::burn(RuntimeOrigin::signed(1), 0, 1, Some(1)));
		assert!(!ItemConfigOf::<Test>::contains_key(0, 1));

		// lock the item and ensure the config stays unchanged
		assert_ok!(Nfts::lock_item(RuntimeOrigin::signed(1), 0, 0, true, true));

		let expect_config = ItemConfig {
			settings: ItemSettings(ItemSetting::LockedAttributes | ItemSetting::LockedMetadata),
		};
		let config = ItemConfigOf::<Test>::get(0, 0).unwrap();
		assert_eq!(config, expect_config);

		assert_ok!(Nfts::burn(RuntimeOrigin::signed(1), 0, 0, Some(1)));
		let config = ItemConfigOf::<Test>::get(0, 0).unwrap();
		assert_eq!(config, expect_config);

		// can't mint with the different config
		assert_noop!(
			Nfts::force_mint(RuntimeOrigin::signed(1), 0, 0, 1, default_item_config()),
			Error::<Test>::InconsistentItemConfig
		);

		assert_ok!(Nfts::update_mint_settings(
			RuntimeOrigin::signed(1),
			0,
			MintSettings {
				default_item_settings: ItemSettings(
					ItemSetting::LockedAttributes | ItemSetting::LockedMetadata
				),
				..Default::default()
			}
		));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 0, None));
	});
}

#[test]
fn force_collection_status_should_work() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, 100);

		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, CollectionConfig::default()));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 42, None));
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 0, 69, 2, default_item_config()));
		assert_ok!(Nfts::set_collection_metadata(RuntimeOrigin::signed(1), 0, bvec![0; 20]));
		assert_ok!(Nfts::set_metadata(RuntimeOrigin::signed(1), 0, 42, bvec![0; 20]));
		assert_ok!(Nfts::set_metadata(RuntimeOrigin::signed(1), 0, 69, bvec![0; 20]));
		assert_eq!(Balances::reserved_balance(1), 65);

		// force item status to be free holding
		assert_ok!(Nfts::force_collection_status(
			RuntimeOrigin::root(),
			0,
			1,
			1,
			1,
			1,
			make_collection_config(CollectionSetting::FreeHolding.into()),
		));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 0, 142, None));
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 0, 169, 2, default_item_config()));
		assert_ok!(Nfts::set_metadata(RuntimeOrigin::signed(1), 0, 142, bvec![0; 20]));
		assert_ok!(Nfts::set_metadata(RuntimeOrigin::signed(1), 0, 169, bvec![0; 20]));
		assert_eq!(Balances::reserved_balance(1), 65);

		assert_ok!(Nfts::redeposit(RuntimeOrigin::signed(1), 0, bvec![0, 42, 50, 69, 100]));
		assert_eq!(Balances::reserved_balance(1), 63);

		assert_ok!(Nfts::set_metadata(RuntimeOrigin::signed(1), 0, 42, bvec![0; 20]));
		assert_eq!(Balances::reserved_balance(1), 42);

		assert_ok!(Nfts::set_metadata(RuntimeOrigin::signed(1), 0, 69, bvec![0; 20]));
		assert_eq!(Balances::reserved_balance(1), 21);

		assert_ok!(Nfts::set_collection_metadata(RuntimeOrigin::signed(1), 0, bvec![0; 20]));
		assert_eq!(Balances::reserved_balance(1), 0);
	});
}

#[test]
fn burn_works() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, 100);
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, CollectionConfig::default()));
		assert_ok!(Nfts::set_team(RuntimeOrigin::signed(1), 0, 2, 3, 4));

		assert_noop!(
			Nfts::burn(RuntimeOrigin::signed(5), 0, 42, Some(5)),
			Error::<Test>::UnknownCollection
		);

		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(2), 0, 42, 5, default_item_config()));
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(2), 0, 69, 5, default_item_config()));
		assert_eq!(Balances::reserved_balance(1), 2);

		assert_noop!(
			Nfts::burn(RuntimeOrigin::signed(0), 0, 42, None),
			Error::<Test>::NoPermission
		);
		assert_noop!(
			Nfts::burn(RuntimeOrigin::signed(5), 0, 42, Some(6)),
			Error::<Test>::WrongOwner
		);

		assert_ok!(Nfts::burn(RuntimeOrigin::signed(5), 0, 42, Some(5)));
		assert_ok!(Nfts::burn(RuntimeOrigin::signed(3), 0, 69, Some(5)));
		assert_eq!(Balances::reserved_balance(1), 0);
	});
}

#[test]
fn approval_lifecycle_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 0, 42, 2, default_item_config()));
		assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(2), 0, 42, 3, None));
		assert_ok!(Nfts::transfer(RuntimeOrigin::signed(3), 0, 42, 4));
		assert_noop!(
			Nfts::transfer(RuntimeOrigin::signed(3), 0, 42, 3),
			Error::<Test>::NoPermission
		);
		assert!(Item::<Test>::get(0, 42).unwrap().approvals.is_empty());

		assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(4), 0, 42, 2, None));
		assert_ok!(Nfts::transfer(RuntimeOrigin::signed(2), 0, 42, 2));

		// ensure we can't buy an item when the collection has a NonTransferableItems flag
		let collection_id = 1;
		assert_ok!(Nfts::force_create(
			RuntimeOrigin::root(),
			1,
			make_collection_config(
				CollectionSetting::NonTransferableItems | CollectionSetting::FreeHolding
			)
		));

		assert_ok!(Nfts::mint(RuntimeOrigin::signed(1), 1, collection_id, None));

		assert_noop!(
			Nfts::approve_transfer(RuntimeOrigin::signed(1), collection_id, 1, 2, None),
			Error::<Test>::ItemsNotTransferable
		);
	});
}

#[test]
fn cancel_approval_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 0, 42, 2, default_item_config()));

		assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(2), 0, 42, 3, None));
		assert_noop!(
			Nfts::cancel_approval(RuntimeOrigin::signed(2), 1, 42, 3),
			Error::<Test>::UnknownCollection
		);
		assert_noop!(
			Nfts::cancel_approval(RuntimeOrigin::signed(2), 0, 43, 3),
			Error::<Test>::UnknownCollection
		);
		assert_noop!(
			Nfts::cancel_approval(RuntimeOrigin::signed(3), 0, 42, 3),
			Error::<Test>::NoPermission
		);
		assert_noop!(
			Nfts::cancel_approval(RuntimeOrigin::signed(2), 0, 42, 4),
			Error::<Test>::NotDelegate
		);

		assert_ok!(Nfts::cancel_approval(RuntimeOrigin::signed(2), 0, 42, 3));
		assert_noop!(
			Nfts::cancel_approval(RuntimeOrigin::signed(2), 0, 42, 3),
			Error::<Test>::NotDelegate
		);

		let current_block = 1;
		System::set_block_number(current_block);
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 0, 69, 2, default_item_config()));
		// approval expires after 2 blocks.
		assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(2), 0, 42, 3, Some(2)));
		assert_noop!(
			Nfts::cancel_approval(RuntimeOrigin::signed(5), 0, 42, 3),
			Error::<Test>::NoPermission
		);

		System::set_block_number(current_block + 3);
		// 5 can cancel the approval since the deadline has passed.
		assert_ok!(Nfts::cancel_approval(RuntimeOrigin::signed(5), 0, 42, 3));
		assert_eq!(approvals(0, 69), vec![]);
	});
}

#[test]
fn approving_multiple_accounts_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 0, 42, 2, default_item_config()));

		let current_block = 1;
		System::set_block_number(current_block);
		assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(2), 0, 42, 3, None));
		assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(2), 0, 42, 4, None));
		assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(2), 0, 42, 5, Some(2)));
		assert_eq!(approvals(0, 42), vec![(3, None), (4, None), (5, Some(current_block + 2))]);

		assert_ok!(Nfts::transfer(RuntimeOrigin::signed(4), 0, 42, 6));
		assert_noop!(
			Nfts::transfer(RuntimeOrigin::signed(3), 0, 42, 7),
			Error::<Test>::NoPermission
		);
		assert_noop!(
			Nfts::transfer(RuntimeOrigin::signed(5), 0, 42, 8),
			Error::<Test>::NoPermission
		);
	});
}

#[test]
fn approvals_limit_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 0, 42, 2, default_item_config()));

		for i in 3..13 {
			assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(2), 0, 42, i, None));
		}
		// the limit is 10
		assert_noop!(
			Nfts::approve_transfer(RuntimeOrigin::signed(2), 0, 42, 14, None),
			Error::<Test>::ReachedApprovalLimit
		);
	});
}

#[test]
fn approval_deadline_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(0);
		assert!(System::block_number().is_zero());

		assert_ok!(Nfts::force_create(
			RuntimeOrigin::root(),
			1,
			make_collection_config(CollectionSetting::FreeHolding.into())
		));
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 0, 42, 2, default_item_config()));

		// the approval expires after the 2nd block.
		assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(2), 0, 42, 3, Some(2)));

		System::set_block_number(3);
		assert_noop!(
			Nfts::transfer(RuntimeOrigin::signed(3), 0, 42, 4),
			Error::<Test>::ApprovalExpired
		);
		System::set_block_number(1);
		assert_ok!(Nfts::transfer(RuntimeOrigin::signed(3), 0, 42, 4));

		assert_eq!(System::block_number(), 1);
		// make a new approval with a deadline after 4 blocks, so it will expire after the 5th
		// block.
		assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(4), 0, 42, 6, Some(4)));
		// this should still work.
		System::set_block_number(5);
		assert_ok!(Nfts::transfer(RuntimeOrigin::signed(6), 0, 42, 5));
	});
}

#[test]
fn cancel_approval_works_with_admin() {
	new_test_ext().execute_with(|| {
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 0, 42, 2, default_item_config()));

		assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(2), 0, 42, 3, None));
		assert_noop!(
			Nfts::cancel_approval(RuntimeOrigin::signed(1), 1, 42, 1),
			Error::<Test>::UnknownCollection
		);
		assert_noop!(
			Nfts::cancel_approval(RuntimeOrigin::signed(1), 0, 43, 1),
			Error::<Test>::UnknownCollection
		);
		assert_noop!(
			Nfts::cancel_approval(RuntimeOrigin::signed(1), 0, 42, 4),
			Error::<Test>::NotDelegate
		);

		assert_ok!(Nfts::cancel_approval(RuntimeOrigin::signed(1), 0, 42, 3));
		assert_noop!(
			Nfts::cancel_approval(RuntimeOrigin::signed(1), 0, 42, 1),
			Error::<Test>::NotDelegate
		);
	});
}

#[test]
fn cancel_approval_works_with_force() {
	new_test_ext().execute_with(|| {
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 0, 42, 2, default_item_config()));

		assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(2), 0, 42, 3, None));
		assert_noop!(
			Nfts::cancel_approval(RuntimeOrigin::root(), 1, 42, 1),
			Error::<Test>::UnknownCollection
		);
		assert_noop!(
			Nfts::cancel_approval(RuntimeOrigin::root(), 0, 43, 1),
			Error::<Test>::UnknownCollection
		);
		assert_noop!(
			Nfts::cancel_approval(RuntimeOrigin::root(), 0, 42, 4),
			Error::<Test>::NotDelegate
		);

		assert_ok!(Nfts::cancel_approval(RuntimeOrigin::root(), 0, 42, 3));
		assert_noop!(
			Nfts::cancel_approval(RuntimeOrigin::root(), 0, 42, 1),
			Error::<Test>::NotDelegate
		);
	});
}

#[test]
fn clear_all_transfer_approvals_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
		assert_ok!(Nfts::force_mint(RuntimeOrigin::signed(1), 0, 42, 2, default_item_config()));

		assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(2), 0, 42, 3, None));
		assert_ok!(Nfts::approve_transfer(RuntimeOrigin::signed(2), 0, 42, 4, None));

		assert_noop!(
			Nfts::clear_all_transfer_approvals(RuntimeOrigin::signed(3), 0, 42),
			Error::<Test>::NoPermission
		);

		assert_ok!(Nfts::clear_all_transfer_approvals(RuntimeOrigin::signed(2), 0, 42));

		assert!(events().contains(&Event::<Test>::AllApprovalsCancelled {
			collection: 0,
			item: 42,
			owner: 2,
		}));
		assert_eq!(approvals(0, 42), vec![]);

		assert_noop!(
			Nfts::transfer(RuntimeOrigin::signed(3), 0, 42, 5),
			Error::<Test>::NoPermission
		);
		assert_noop!(
			Nfts::transfer(RuntimeOrigin::signed(4), 0, 42, 5),
			Error::<Test>::NoPermission
		);
	});
}

#[test]
fn max_supply_should_work() {
	new_test_ext().execute_with(|| {
		let collection_id = 0;
		let user_id = 1;
		let max_supply = 1;

		// validate set_collection_max_supply
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), user_id, default_collection_config()));
		assert_eq!(CollectionConfigOf::<Test>::get(collection_id).unwrap().max_supply, None);

		assert_ok!(Nfts::set_collection_max_supply(
			RuntimeOrigin::signed(user_id),
			collection_id,
			max_supply
		));
		assert_eq!(
			CollectionConfigOf::<Test>::get(collection_id).unwrap().max_supply,
			Some(max_supply)
		);

		assert!(events().contains(&Event::<Test>::CollectionMaxSupplySet {
			collection: collection_id,
			max_supply,
		}));

		assert_ok!(Nfts::set_collection_max_supply(
			RuntimeOrigin::signed(user_id),
			collection_id,
			max_supply + 1
		));
		assert_ok!(Nfts::lock_collection(
			RuntimeOrigin::signed(user_id),
			collection_id,
			CollectionSettings(CollectionSetting::LockedMaxSupply.into())
		));
		assert_noop!(
			Nfts::set_collection_max_supply(
				RuntimeOrigin::signed(user_id),
				collection_id,
				max_supply + 2
			),
			Error::<Test>::MaxSupplyLocked
		);

		// validate we can't mint more to max supply
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(user_id), collection_id, 0, None));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(user_id), collection_id, 1, None));
		assert_noop!(
			Nfts::mint(RuntimeOrigin::signed(user_id), collection_id, 2, None),
			Error::<Test>::MaxSupplyReached
		);
	});
}

#[test]
fn mint_settings_should_work() {
	new_test_ext().execute_with(|| {
		let collection_id = 0;
		let user_id = 1;
		let item_id = 0;

		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), user_id, default_collection_config()));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(user_id), collection_id, item_id, None));
		assert_eq!(
			ItemConfigOf::<Test>::get(collection_id, item_id).unwrap().settings,
			ItemSettings::empty()
		);

		let collection_id = 1;
		assert_ok!(Nfts::force_create(
			RuntimeOrigin::root(),
			user_id,
			CollectionConfig {
				mint_settings: MintSettings {
					default_item_settings: ItemSettings(
						ItemSetting::NonTransferable | ItemSetting::LockedMetadata
					),
					..Default::default()
				},
				..default_collection_config()
			}
		));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(user_id), collection_id, item_id, None));
		assert_eq!(
			ItemConfigOf::<Test>::get(collection_id, item_id).unwrap().settings,
			ItemSettings(ItemSetting::NonTransferable | ItemSetting::LockedMetadata)
		);
	});
}

#[test]
fn set_price_should_work() {
	new_test_ext().execute_with(|| {
		let user_id = 1;
		let collection_id = 0;
		let item_1 = 1;
		let item_2 = 2;

		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), user_id, default_collection_config()));

		assert_ok!(Nfts::mint(RuntimeOrigin::signed(user_id), collection_id, item_1, None));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(user_id), collection_id, item_2, None));

		assert_ok!(Nfts::set_price(
			RuntimeOrigin::signed(user_id),
			collection_id,
			item_1,
			Some(1),
			None,
		));

		assert_ok!(Nfts::set_price(
			RuntimeOrigin::signed(user_id),
			collection_id,
			item_2,
			Some(2),
			Some(3)
		));

		let item = ItemPriceOf::<Test>::get(collection_id, item_1).unwrap();
		assert_eq!(item.0, 1);
		assert_eq!(item.1, None);

		let item = ItemPriceOf::<Test>::get(collection_id, item_2).unwrap();
		assert_eq!(item.0, 2);
		assert_eq!(item.1, Some(3));

		assert!(events().contains(&Event::<Test>::ItemPriceSet {
			collection: collection_id,
			item: item_1,
			price: 1,
			whitelisted_buyer: None,
		}));

		// validate we can unset the price
		assert_ok!(Nfts::set_price(
			RuntimeOrigin::signed(user_id),
			collection_id,
			item_2,
			None,
			None
		));
		assert!(events().contains(&Event::<Test>::ItemPriceRemoved {
			collection: collection_id,
			item: item_2
		}));
		assert!(!ItemPriceOf::<Test>::contains_key(collection_id, item_2));

		// ensure we can't set price when the items are non-transferable
		let collection_id = 1;
		assert_ok!(Nfts::force_create(
			RuntimeOrigin::root(),
			user_id,
			make_collection_config(
				CollectionSetting::NonTransferableItems | CollectionSetting::FreeHolding
			)
		));

		assert_ok!(Nfts::mint(RuntimeOrigin::signed(user_id), collection_id, item_1, None));

		assert_noop!(
			Nfts::set_price(RuntimeOrigin::signed(user_id), collection_id, item_1, Some(2), None),
			Error::<Test>::ItemsNotTransferable
		);
	});
}

#[test]
fn buy_item_should_work() {
	new_test_ext().execute_with(|| {
		let user_1 = 1;
		let user_2 = 2;
		let user_3 = 3;
		let collection_id = 0;
		let item_1 = 1;
		let item_2 = 2;
		let item_3 = 3;
		let price_1 = 20;
		let price_2 = 30;
		let initial_balance = 100;

		Balances::make_free_balance_be(&user_1, initial_balance);
		Balances::make_free_balance_be(&user_2, initial_balance);
		Balances::make_free_balance_be(&user_3, initial_balance);

		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), user_1, default_collection_config()));

		assert_ok!(Nfts::mint(RuntimeOrigin::signed(user_1), collection_id, item_1, None));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(user_1), collection_id, item_2, None));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(user_1), collection_id, item_3, None));

		assert_ok!(Nfts::set_price(
			RuntimeOrigin::signed(user_1),
			collection_id,
			item_1,
			Some(price_1),
			None,
		));

		assert_ok!(Nfts::set_price(
			RuntimeOrigin::signed(user_1),
			collection_id,
			item_2,
			Some(price_2),
			Some(user_3),
		));

		// can't buy for less
		assert_noop!(
			Nfts::buy_item(RuntimeOrigin::signed(user_2), collection_id, item_1, 1),
			Error::<Test>::BidTooLow
		);

		// pass the higher price to validate it will still deduct correctly
		assert_ok!(Nfts::buy_item(
			RuntimeOrigin::signed(user_2),
			collection_id,
			item_1,
			price_1 + 1,
		));

		// validate the new owner & balances
		let item = Item::<Test>::get(collection_id, item_1).unwrap();
		assert_eq!(item.owner, user_2);
		assert_eq!(Balances::total_balance(&user_1), initial_balance + price_1);
		assert_eq!(Balances::total_balance(&user_2), initial_balance - price_1);

		// can't buy from yourself
		assert_noop!(
			Nfts::buy_item(RuntimeOrigin::signed(user_1), collection_id, item_2, price_2),
			Error::<Test>::NoPermission
		);

		// can't buy when the item is listed for a specific buyer
		assert_noop!(
			Nfts::buy_item(RuntimeOrigin::signed(user_2), collection_id, item_2, price_2),
			Error::<Test>::NoPermission
		);

		// can buy when I'm a whitelisted buyer
		assert_ok!(Nfts::buy_item(RuntimeOrigin::signed(user_3), collection_id, item_2, price_2));

		assert!(events().contains(&Event::<Test>::ItemBought {
			collection: collection_id,
			item: item_2,
			price: price_2,
			seller: user_1,
			buyer: user_3,
		}));

		// ensure we reset the buyer field
		assert!(!ItemPriceOf::<Test>::contains_key(collection_id, item_2));

		// can't buy when item is not for sale
		assert_noop!(
			Nfts::buy_item(RuntimeOrigin::signed(user_2), collection_id, item_3, price_2),
			Error::<Test>::NotForSale
		);

		// ensure we can't buy an item when the collection or an item are frozen
		{
			assert_ok!(Nfts::set_price(
				RuntimeOrigin::signed(user_1),
				collection_id,
				item_3,
				Some(price_1),
				None,
			));

			// lock the collection
			assert_ok!(Nfts::lock_collection(
				RuntimeOrigin::signed(user_1),
				collection_id,
				CollectionSettings(CollectionSetting::NonTransferableItems.into())
			));

			let buy_item_call = mock::RuntimeCall::Nfts(crate::Call::<Test>::buy_item {
				collection: collection_id,
				item: item_3,
				bid_price: price_1,
			});
			assert_noop!(
				buy_item_call.dispatch(RuntimeOrigin::signed(user_2)),
				Error::<Test>::ItemsNotTransferable
			);

			// un-freeze the collection
			assert_ok!(Nfts::force_collection_status(
				RuntimeOrigin::root(),
				collection_id,
				user_1,
				user_1,
				user_1,
				user_1,
				CollectionConfig::default(),
			));

			// freeze the item
			assert_ok!(Nfts::freeze(RuntimeOrigin::signed(user_1), collection_id, item_3));

			let buy_item_call = mock::RuntimeCall::Nfts(crate::Call::<Test>::buy_item {
				collection: collection_id,
				item: item_3,
				bid_price: price_1,
			});
			assert_noop!(
				buy_item_call.dispatch(RuntimeOrigin::signed(user_2)),
				Error::<Test>::ItemLocked
			);
		}
	});
}

#[test]
fn pay_tips_should_work() {
	new_test_ext().execute_with(|| {
		let user_1 = 1;
		let user_2 = 2;
		let user_3 = 3;
		let collection_id = 0;
		let item_id = 1;
		let tip = 2;
		let initial_balance = 100;

		Balances::make_free_balance_be(&user_1, initial_balance);
		Balances::make_free_balance_be(&user_2, initial_balance);
		Balances::make_free_balance_be(&user_3, initial_balance);

		assert_ok!(Nfts::pay_tips(
			RuntimeOrigin::signed(user_1),
			bvec![
				ItemTip { collection: collection_id, item: item_id, receiver: user_2, amount: tip },
				ItemTip { collection: collection_id, item: item_id, receiver: user_3, amount: tip },
			]
		));

		assert_eq!(Balances::total_balance(&user_1), initial_balance - tip * 2);
		assert_eq!(Balances::total_balance(&user_2), initial_balance + tip);
		assert_eq!(Balances::total_balance(&user_3), initial_balance + tip);

		let events = events();
		assert!(events.contains(&Event::<Test>::TipSent {
			collection: collection_id,
			item: item_id,
			sender: user_1,
			receiver: user_2,
			amount: tip,
		}));
		assert!(events.contains(&Event::<Test>::TipSent {
			collection: collection_id,
			item: item_id,
			sender: user_1,
			receiver: user_3,
			amount: tip,
		}));
	});
}

#[test]
fn create_cancel_swap_should_work() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		let user_id = 1;
		let collection_id = 0;
		let item_1 = 1;
		let item_2 = 2;
		let price = 1;
		let price_direction = PriceDirection::Receive;
		let price_with_direction = PriceWithDirection { amount: price, direction: price_direction };
		let duration = 2;
		let expect_deadline = 3;

		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), user_id, default_collection_config()));

		assert_ok!(Nfts::mint(RuntimeOrigin::signed(user_id), collection_id, item_1, None));
		assert_ok!(Nfts::mint(RuntimeOrigin::signed(user_id), collection_id, item_2, None));

		// validate desired item and the collection exists
		assert_noop!(
			Nfts::create_swap(
				RuntimeOrigin::signed(user_id),
				collection_id,
				item_1,
				collection_id,
				Some(item_2 + 1),
				Some(price_with_direction.clone()),
				duration,
			),
			Error::<Test>::UnknownItem
		);
		assert_noop!(
			Nfts::create_swap(
				RuntimeOrigin::signed(user_id),
				collection_id,
				item_1,
				collection_id + 1,
				None,
				Some(price_with_direction.clone()),
				duration,
			),
			Error::<Test>::UnknownCollection
		);

		let max_duration: u64 = <Test as Config>::MaxDeadlineDuration::get();
		assert_noop!(
			Nfts::create_swap(
				RuntimeOrigin::signed(user_id),
				collection_id,
				item_1,
				collection_id,
				Some(item_2),
				Some(price_with_direction.clone()),
				max_duration.saturating_add(1),
			),
			Error::<Test>::WrongDuration
		);

		assert_ok!(Nfts::create_swap(
			RuntimeOrigin::signed(user_id),
			collection_id,
			item_1,
			collection_id,
			Some(item_2),
			Some(price_with_direction.clone()),
			duration,
		));

		let swap = PendingSwapOf::<Test>::get(collection_id, item_1).unwrap();
		assert_eq!(swap.desired_collection, collection_id);
		assert_eq!(swap.desired_item, Some(item_2));
		assert_eq!(swap.price, Some(price_with_direction.clone()));
		assert_eq!(swap.deadline, expect_deadline);

		assert!(events().contains(&Event::<Test>::SwapCreated {
			offered_collection: collection_id,
			offered_item: item_1,
			desired_collection: collection_id,
			desired_item: Some(item_2),
			price: Some(price_with_direction.clone()),
			deadline: expect_deadline,
		}));

		// validate we can cancel the swap
		assert_ok!(Nfts::cancel_swap(RuntimeOrigin::signed(user_id), collection_id, item_1));
		assert!(events().contains(&Event::<Test>::SwapCancelled {
			offered_collection: collection_id,
			offered_item: item_1,
			desired_collection: collection_id,
			desired_item: Some(item_2),
			price: Some(price_with_direction.clone()),
			deadline: expect_deadline,
		}));
		assert!(!PendingSwapOf::<Test>::contains_key(collection_id, item_1));

		// validate anyone can cancel the expired swap
		assert_ok!(Nfts::create_swap(
			RuntimeOrigin::signed(user_id),
			collection_id,
			item_1,
			collection_id,
			Some(item_2),
			Some(price_with_direction.clone()),
			duration,
		));
		assert_noop!(
			Nfts::cancel_swap(RuntimeOrigin::signed(user_id + 1), collection_id, item_1),
			Error::<Test>::NoPermission
		);
		System::set_block_number(expect_deadline + 1);
		assert_ok!(Nfts::cancel_swap(RuntimeOrigin::signed(user_id + 1), collection_id, item_1));

		// validate optional desired_item param
		assert_ok!(Nfts::create_swap(
			RuntimeOrigin::signed(user_id),
			collection_id,
			item_1,
			collection_id,
			None,
			Some(price_with_direction),
			duration,
		));

		let swap = PendingSwapOf::<Test>::get(collection_id, item_1).unwrap();
		assert_eq!(swap.desired_item, None);
	});
}

#[test]
fn claim_swap_should_work() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		let user_1 = 1;
		let user_2 = 2;
		let collection_id = 0;
		let item_1 = 1;
		let item_2 = 2;
		let item_3 = 3;
		let item_4 = 4;
		let item_5 = 5;
		let price = 100;
		let price_direction = PriceDirection::Receive;
		let price_with_direction =
			PriceWithDirection { amount: price, direction: price_direction.clone() };
		let duration = 2;
		let initial_balance = 1000;
		let deadline = 1 + duration;

		Balances::make_free_balance_be(&user_1, initial_balance);
		Balances::make_free_balance_be(&user_2, initial_balance);

		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), user_1, default_collection_config()));

		assert_ok!(Nfts::mint(
			RuntimeOrigin::signed(user_1),
			collection_id,
			item_1,
			None,
		));
		assert_ok!(Nfts::force_mint(
			RuntimeOrigin::signed(user_1),
			collection_id,
			item_2,
			user_2,
			default_item_config(),
		));
		assert_ok!(Nfts::force_mint(
			RuntimeOrigin::signed(user_1),
			collection_id,
			item_3,
			user_2,
			default_item_config(),
		));
		assert_ok!(Nfts::mint(
			RuntimeOrigin::signed(user_1),
			collection_id,
			item_4,
			None,
		));
		assert_ok!(Nfts::force_mint(
			RuntimeOrigin::signed(user_1),
			collection_id,
			item_5,
			user_2,
			default_item_config(),
		));

		assert_ok!(Nfts::create_swap(
			RuntimeOrigin::signed(user_1),
			collection_id,
			item_1,
			collection_id,
			Some(item_2),
			Some(price_with_direction.clone()),
			duration,
		));

		// validate the deadline
		System::set_block_number(5);
		assert_noop!(
			Nfts::claim_swap(
				RuntimeOrigin::signed(user_2),
				collection_id,
				item_2,
				collection_id,
				item_1,
				Some(price_with_direction.clone()),
			),
			Error::<Test>::DeadlineExpired
		);
		System::set_block_number(1);

		// validate edge cases
		assert_noop!(
			Nfts::claim_swap(
				RuntimeOrigin::signed(user_2),
				collection_id,
				item_2,
				collection_id,
				item_4, // no swap was created for that asset
				Some(price_with_direction.clone()),
			),
			Error::<Test>::UnknownSwap
		);
		assert_noop!(
			Nfts::claim_swap(
				RuntimeOrigin::signed(user_2),
				collection_id,
				item_4, // not my item
				collection_id,
				item_1,
				Some(price_with_direction.clone()),
			),
			Error::<Test>::NoPermission
		);
		assert_noop!(
			Nfts::claim_swap(
				RuntimeOrigin::signed(user_2),
				collection_id,
				item_5, // my item, but not the one another part wants
				collection_id,
				item_1,
				Some(price_with_direction.clone()),
			),
			Error::<Test>::UnknownSwap
		);
		assert_noop!(
			Nfts::claim_swap(
				RuntimeOrigin::signed(user_2),
				collection_id,
				item_2,
				collection_id,
				item_1,
				Some(PriceWithDirection { amount: price + 1, direction: price_direction.clone() }), // wrong price
			),
			Error::<Test>::UnknownSwap
		);
		assert_noop!(
			Nfts::claim_swap(
				RuntimeOrigin::signed(user_2),
				collection_id,
				item_2,
				collection_id,
				item_1,
				Some(PriceWithDirection { amount: price, direction: PriceDirection::Send }), // wrong direction
			),
			Error::<Test>::UnknownSwap
		);

		assert_ok!(Nfts::claim_swap(
			RuntimeOrigin::signed(user_2),
			collection_id,
			item_2,
			collection_id,
			item_1,
			Some(price_with_direction.clone()),
		));

		// validate the new owner
		let item = Item::<Test>::get(collection_id, item_1).unwrap();
		assert_eq!(item.owner, user_2);
		let item = Item::<Test>::get(collection_id, item_2).unwrap();
		assert_eq!(item.owner, user_1);

		// validate the balances
		assert_eq!(Balances::total_balance(&user_1), initial_balance + price);
		assert_eq!(Balances::total_balance(&user_2), initial_balance - price);

		// ensure we reset the swap
		assert!(!PendingSwapOf::<Test>::contains_key(collection_id, item_1));

		// validate the event
		assert!(events().contains(&Event::<Test>::SwapClaimed {
			sent_collection: collection_id,
			sent_item: item_2,
			sent_item_owner: user_2,
			received_collection: collection_id,
			received_item: item_1,
			received_item_owner: user_1,
			price: Some(price_with_direction.clone()),
			deadline,
		}));

		// validate the optional desired_item param and another price direction
		let price_direction = PriceDirection::Send;
		let price_with_direction = PriceWithDirection { amount: price, direction: price_direction };
		Balances::make_free_balance_be(&user_1, initial_balance);
		Balances::make_free_balance_be(&user_2, initial_balance);

		assert_ok!(Nfts::create_swap(
			RuntimeOrigin::signed(user_1),
			collection_id,
			item_4,
			collection_id,
			None,
			Some(price_with_direction.clone()),
			duration,
		));
		assert_ok!(Nfts::claim_swap(
			RuntimeOrigin::signed(user_2),
			collection_id,
			item_1,
			collection_id,
			item_4,
			Some(price_with_direction),
		));
		let item = Item::<Test>::get(collection_id, item_1).unwrap();
		assert_eq!(item.owner, user_1);
		let item = Item::<Test>::get(collection_id, item_4).unwrap();
		assert_eq!(item.owner, user_2);

		assert_eq!(Balances::total_balance(&user_1), initial_balance - price);
		assert_eq!(Balances::total_balance(&user_2), initial_balance + price);
	});
}

#[test]
fn various_collection_settings() {
	new_test_ext().execute_with(|| {
		// when we set only one value it's required to call .into() on it
		let config = make_collection_config(CollectionSetting::NonTransferableItems.into());
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, config));

		let config = CollectionConfigOf::<Test>::get(0).unwrap();
		let stored_settings = config.settings.values();
		assert!(stored_settings.contains(CollectionSetting::NonTransferableItems));
		assert!(!stored_settings.contains(CollectionSetting::LockedMetadata));

		// no need to call .into() for multiple values
		let config = make_collection_config(
			CollectionSetting::LockedMetadata | CollectionSetting::NonTransferableItems,
		);
		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, config));

		let config = CollectionConfigOf::<Test>::get(1).unwrap();
		let stored_settings = config.settings.values();
		assert!(stored_settings.contains(CollectionSetting::NonTransferableItems));
		assert!(stored_settings.contains(CollectionSetting::LockedMetadata));

		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), 1, default_collection_config()));
	});
}

#[test]
fn collection_locking_should_work() {
	new_test_ext().execute_with(|| {
		let user_id = 1;
		let collection_id = 0;

		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), user_id, CollectionConfig::default()));

		// validate partial lock
		let lock_settings = CollectionSettings(
			CollectionSetting::NonTransferableItems | CollectionSetting::LockedAttributes,
		);
		assert_ok!(Nfts::lock_collection(
			RuntimeOrigin::signed(user_id),
			collection_id,
			lock_settings,
		));

		let stored_config = CollectionConfigOf::<Test>::get(collection_id).unwrap();
		assert_eq!(stored_config.settings, lock_settings);

		// validate full lock
		let all_settings_locked = CollectionSettings(
			CollectionSetting::NonTransferableItems |
				CollectionSetting::LockedMetadata |
				CollectionSetting::LockedAttributes,
		);
		assert_ok!(Nfts::lock_collection(
			RuntimeOrigin::signed(user_id),
			collection_id,
			CollectionSettings(CollectionSetting::LockedMetadata.into()),
		));

		let stored_config = CollectionConfigOf::<Test>::get(collection_id).unwrap();
		assert_eq!(stored_config.settings, all_settings_locked);
	});
}

#[test]
fn pallet_level_feature_flags_should_work() {
	new_test_ext().execute_with(|| {
		Features::set(&PalletFeatures(
			PalletFeature::NoTrading | PalletFeature::NoApprovals | PalletFeature::NoAttributes,
		));

		let user_id = 1;
		let collection_id = 0;
		let item_id = 1;

		assert_ok!(Nfts::force_create(RuntimeOrigin::root(), user_id, default_collection_config()));

		assert_ok!(Nfts::mint(RuntimeOrigin::signed(user_id), collection_id, item_id, None));

		// PalletFeature::NoTrading
		assert_noop!(
			Nfts::set_price(RuntimeOrigin::signed(user_id), collection_id, item_id, Some(1), None),
			Error::<Test>::MethodDisabled
		);
		assert_noop!(
			Nfts::buy_item(RuntimeOrigin::signed(user_id), collection_id, item_id, 1),
			Error::<Test>::MethodDisabled
		);

		// PalletFeature::NoApprovals
		assert_noop!(
			Nfts::approve_transfer(RuntimeOrigin::signed(user_id), collection_id, item_id, 2, None),
			Error::<Test>::MethodDisabled
		);

		// PalletFeature::NoAttributes
		assert_noop!(
			Nfts::set_attribute(
				RuntimeOrigin::signed(user_id),
				collection_id,
				None,
				bvec![0],
				bvec![0]
			),
			Error::<Test>::MethodDisabled
		);
	})
}
