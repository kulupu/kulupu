// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Kulupu.
//
// Copyright (c) 2019-2020 Wei Tang.
//
// Kulupu is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Kulupu is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Kulupu. If not, see <http://www.gnu.org/licenses/>.

//! The Kulupu runtime. This can be compiled with `#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit="256"]

mod fee;
mod weights;

extern crate system as frame_system;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use sp_std::{collections::btree_map::BTreeMap, cmp::{min, max}, prelude::*, cmp};
use codec::{Encode, Decode};
use sp_core::{OpaqueMetadata, u32_trait::{_1, _2, _4, _5}};
use sp_runtime::{
	ApplyExtrinsicResult, Percent, ModuleId, generic, create_runtime_str, MultiSignature,
	RuntimeDebug, Perquintill, transaction_validity::{TransactionValidity, TransactionSource},
	FixedPointNumber,
};
use sp_runtime::traits::{
	BlakeTwo256, Block as BlockT,
	Verify, IdentifyAccount, Convert, ConvertInto,
};
use sp_api::impl_runtime_apis;
use sp_version::RuntimeVersion;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use kulupu_primitives::{DOLLARS, CENTS, MILLICENTS, MICROCENTS, HOURS, DAYS, BLOCK_TIME, deposit};
use transaction_payment::{TargetedFeeAdjustment, Multiplier};
use system::{limits, EnsureRoot};
use static_assertions::const_assert;
use crate::fee::WeightToFee;
use contracts::weights::WeightInfo;

// A few exports that help ease life for downstream crates.
pub use sp_runtime::{Permill, Perbill};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use frame_support::{
	StorageValue, StorageMap, construct_runtime, parameter_types,
	traits::{Currency, Randomness, LockIdentifier, OnUnbalanced, InstanceFilter},
	weights::{
		Weight, RuntimeDbWeight, DispatchClass,
		constants::{
			WEIGHT_PER_SECOND, BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight
		},
	},
};
pub use timestamp::Call as TimestampCall;
pub use balances::Call as BalancesCall;

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The type for looking up accounts.
pub type AccountIndex = u32;

/// Balance of an account.
pub type Balance = u128;

/// Index of a transaction in the chain.
pub type Index = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// Digest item type.
pub type DigestItem = generic::DigestItem<Hash>;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core datastructures.
pub mod opaque {
	use super::*;

	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;
}

/// This runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("kulupu"),
	impl_name: create_runtime_str!("kulupu"),
	authoring_version: 5,
	spec_version: 19,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 10,
};

/// The version infromation used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}

/// We assume that an on-initialize consumes 2.5% of the weight on average, hence a single extrinsic
/// will not be allowed to consume more than `AvailableBlockRatio - 2.5%`.
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_perthousand(25);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2 seconds of compute with a 6 second average block time.
pub const MAXIMUM_BLOCK_WEIGHT: Weight = 2 * WEIGHT_PER_SECOND;

const_assert!(NORMAL_DISPATCH_RATIO.deconstruct() >= AVERAGE_ON_INITIALIZE_RATIO.deconstruct());

parameter_types! {
	pub const BlockHashCount: BlockNumber = 250;
	pub BlockLength: limits::BlockLength =
		limits::BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	/// Block weights base values and limits.
	pub BlockWeights: limits::BlockWeights = limits::BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have an extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT,
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
	pub const Version: RuntimeVersion = VERSION;
	pub const DbWeight: RuntimeDbWeight = frame_support::weights::constants::RocksDbWeight::get();
	pub const SS58Prefix: u8 = 16;
}

impl system::Config for Runtime {
	type BaseCallFilter = ();
	type BlockWeights = BlockWeights;
	type BlockLength = BlockLength;
	type Origin = Origin;
	type Call = Call;
	type Index = Index;
	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = Indices;
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = DbWeight;
	type Version = Version;
	type PalletInfo = PalletInfo;
	type AccountData = balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) *
		BlockWeights::get().max_block;
	pub const MaxScheduledPerBlock: u32 = 50;
}

impl scheduler::Config for Runtime {
	type Event = Event;
	type Origin = Origin;
	type Call = Call;
	type MaximumWeight = MaximumSchedulerWeight;
	type PalletsOrigin = OriginCaller;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = ();
}

parameter_types! {
	// One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
	pub const DepositBase: Balance = deposit(1, 88);
	// Additional storage item size of 32 bytes.
	pub const DepositFactor: Balance = deposit(0, 32);
	pub const MaxSignatories: u16 = 100;
}

impl multisig::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type Currency = Balances;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = MaxSignatories;
	type WeightInfo = ();
}

impl utility::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type WeightInfo = ();
}

parameter_types! {
	pub const IndexDeposit: Balance = 1 * DOLLARS;
}

impl indices::Config for Runtime {
	/// The type for recording indexing into the account enumeration.
	type AccountIndex = AccountIndex;
	/// Index deposit.
	type Deposit = IndexDeposit;
	/// Currency of the indices.
	type Currency = Balances;
	/// The ubiquitous event type.
	type Event = Event;
	/// Weight info for indices.
	type WeightInfo = ();
}

parameter_types! {
	pub const MinimumPeriod: u64 = 1000;
}

impl timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Difficulty;
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_types! {
	pub const ExistentialDeposit: u128 = 10 * MICROCENTS;
	pub const MaxLocks: u32 = 50;
}

impl balances::Config for Runtime {
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type MaxLocks = MaxLocks;
	type WeightInfo = ();
}

type NegativeImbalance = <Balances as Currency<AccountId>>::NegativeImbalance;

pub struct DealWithFees;
impl OnUnbalanced<NegativeImbalance> for DealWithFees {
	fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item=NegativeImbalance>) {
		if let Some(fees) = fees_then_tips.next() {
			// Burn base fees.
			drop(fees);
			if let Some(tips) = fees_then_tips.next() {
				// Pay tips to miners.
				Author::on_unbalanced(tips);
			}
		}
	}
}

parameter_types! {
	pub const TransactionByteFee: Balance = 10 * MILLICENTS;
	/// The portion of the `AvailableBlockRatio` that we adjust the fees with. Blocks filled less
	/// than this will decrease the weight and more will increase.
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	/// The adjustment variable of the runtime. Higher values will cause `TargetBlockFullness` to
	/// change the fees more rapidly.
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(3, 100_000);
	/// Minimum amount of the multiplier. This value cannot be too low. A test case should ensure
	/// that combined with `AdjustmentVariable`, we can recover from the minimum.
	/// See `multiplier_can_grow_from_zero`.
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000_000u128);
}

impl transaction_payment::Config for Runtime {
	type OnChargeTransaction = transaction_payment::CurrencyAdapter<Balances, DealWithFees>;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = WeightToFee;
	type FeeMultiplierUpdate = TargetedFeeAdjustment<Self, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier>;
}

parameter_types! {
	pub const TargetBlockTime: u64 = BLOCK_TIME;
}

impl difficulty::Config for Runtime {
	type TargetBlockTime = TargetBlockTime;
}

impl eras::Config for Runtime { }

pub struct GenerateRewardLocks;

impl rewards::GenerateRewardLocks<Runtime> for GenerateRewardLocks {
	fn generate_reward_locks(
		current_block: BlockNumber,
		total_reward: Balance,
		lock_parameters: Option<rewards::LockParameters>,
	) -> BTreeMap<BlockNumber, Balance> {
		let mut locks = BTreeMap::new();
		let locked_reward = total_reward.saturating_sub(1 * DOLLARS);

		if locked_reward > 0 {
			let total_lock_period: BlockNumber;
			let divide: BlockNumber;

			if let Some(lock_parameters) = lock_parameters {
				total_lock_period = u32::from(lock_parameters.period) * DAYS;
				divide = u32::from(lock_parameters.divide);
			} else {
				total_lock_period = 100 * DAYS;
				divide = 10;
			}
			for i in 0..divide {
				let one_locked_reward = locked_reward / divide as u128;

				let estimate_block_number = current_block.saturating_add((i + 1) * (total_lock_period / divide));
				let actual_block_number = estimate_block_number / DAYS * DAYS;

				locks.insert(actual_block_number, one_locked_reward);
			}
		}

		locks
	}

	fn max_locks(lock_bounds: rewards::LockBounds) -> u32 {
		// Max locks when a miner mines at least one block every day till the lock period of
		// the first mined block ends.
		cmp::max(100, u32::from(lock_bounds.period_max))
	}
}

parameter_types! {
	pub DonationDestination: AccountId = Treasury::account_id();
	pub const LockBounds: rewards::LockBounds = rewards::LockBounds {period_max: 500, period_min: 20,
																	divide_max: 50, divide_min: 2};
}

impl rewards::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type DonationDestination = DonationDestination;
	type GenerateRewardLocks = GenerateRewardLocks;
	type WeightInfo = crate::weights::rewards::WeightInfo<Self>;
	type LockParametersBounds = LockBounds;
}

pub struct Author;
impl OnUnbalanced<NegativeImbalance> for Author {
	fn on_nonzero_unbalanced(amount: NegativeImbalance) {
		if let Some(author) = Rewards::author() {
			Balances::resolve_creating(&author, amount);
		} else {
			drop(amount);
		}
	}
}

parameter_types! {
	pub const LaunchPeriod: BlockNumber = 7 * DAYS;
	pub const VotingPeriod: BlockNumber = 7 * DAYS;
	pub const FastTrackVotingPeriod: BlockNumber = 1 * DAYS;
	pub const MinimumDeposit: Balance = 100 * DOLLARS;
	pub const EnactmentPeriod: BlockNumber = 8 * DAYS;
	pub const CooloffPeriod: BlockNumber = 7 * DAYS;
	// One cent: $10,000 / MB
	pub const PreimageByteDeposit: Balance = 10 * MILLICENTS;
	pub const InstantAllowed: bool = false;
	pub const MaxVotes: u32 = 100;
	pub const MaxProposals: u32 = 100;
}

impl democracy::Config for Runtime {
	type Proposal = Call;
	type Event = Event;
	type Currency = Balances;
	type EnactmentPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type MinimumDeposit = MinimumDeposit;
	/// A straight majority of the council can decide what their next motion is.
	type ExternalOrigin = system::EnsureOneOf<AccountId,
		collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>,
		system::EnsureRoot<AccountId>,
	>;
	/// A super-majority can have the next scheduled referendum be a straight
	/// majority-carries vote.
	type ExternalMajorityOrigin = system::EnsureOneOf<AccountId,
		collective::EnsureProportionMoreThan<_4, _5, AccountId, CouncilCollective>,
		system::EnsureRoot<AccountId>,
	>;
	/// A unanimous council can have the next scheduled referendum be a straight
	/// default-carries (NTB) vote.
	type ExternalDefaultOrigin = system::EnsureOneOf<AccountId,
		collective::EnsureProportionAtLeast<_1, _1, AccountId, CouncilCollective>,
		system::EnsureRoot<AccountId>,
	>;
	/// Full of the technical committee can have an
	/// ExternalMajority/ExternalDefault vote be tabled immediately and with a
	/// shorter voting/enactment period.
	type FastTrackOrigin = system::EnsureOneOf<AccountId,
		collective::EnsureProportionAtLeast<_1, _1, AccountId, TechnicalCollective>,
		system::EnsureRoot<AccountId>,
	>;
	type InstantOrigin = system::EnsureNever<AccountId>;
	type InstantAllowed = InstantAllowed;
	type FastTrackVotingPeriod = FastTrackVotingPeriod;
	/// To cancel a proposal which has been passed, all of the council must
	/// agree to it.
	type CancellationOrigin = system::EnsureOneOf<AccountId,
		collective::EnsureProportionAtLeast<_1, _1, AccountId, CouncilCollective>,
		system::EnsureRoot<AccountId>,
	>;
	type OperationalPreimageOrigin = collective::EnsureMember<AccountId, CouncilCollective>;
	/// To cancel a proposal before it has been passed, the technical committee must be unanimous or
	/// Root must agree.
	type CancelProposalOrigin = system::EnsureOneOf<AccountId,
		collective::EnsureProportionAtLeast<_1, _1, AccountId, TechnicalCollective>,
		EnsureRoot<AccountId>,
	>;
	type BlacklistOrigin = EnsureRoot<AccountId>;
	/// Any single technical committee member may veto a coming council
	/// proposal, however they can only do it once and it lasts only for the
	/// cooloff period.
	type VetoOrigin = collective::EnsureMember<AccountId, TechnicalCollective>;
	type CooloffPeriod = CooloffPeriod;
	type PreimageByteDeposit = PreimageByteDeposit;
	type Slash = Treasury;
	type Scheduler = Scheduler;
	type MaxVotes = MaxVotes;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = ();
	type MaxProposals = MaxProposals;
}

parameter_types! {
	pub const CouncilMotionDuration: BlockNumber = 3 * DAYS;
	pub const CouncilMaxProposals: u32 = 100;
	pub const CouncilMaxMembers: u32 = 100;
}

type CouncilCollective = collective::Instance1;
impl collective::Config<CouncilCollective> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = CouncilMotionDuration;
	type MaxProposals = CouncilMaxProposals;
	type MaxMembers = CouncilMaxMembers;
	type DefaultVote = collective::MoreThanMajorityThenPrimeDefaultVote;
	type WeightInfo = ();
}

/// Converter for currencies to votes.
pub struct CurrencyToVoteHandler<R>(sp_std::marker::PhantomData<R>);

impl<R> CurrencyToVoteHandler<R>
where
	R: balances::Config,
	R::Balance: Into<u128>,
{
	fn factor() -> u128 {
		let issuance: u128 = <balances::Module<R>>::total_issuance().into();
		(issuance / u64::max_value() as u128).max(1)
	}
}

impl<R> Convert<u128, u64> for CurrencyToVoteHandler<R>
where
	R: balances::Config,
	R::Balance: Into<u128>,
{
	fn convert(x: u128) -> u64 { (x / Self::factor()) as u64 }
}

impl<R> Convert<u128, u128> for CurrencyToVoteHandler<R>
where
	R: balances::Config,
	R::Balance: Into<u128>,
{
	fn convert(x: u128) -> u128 { x * Self::factor() }
}

parameter_types! {
	pub const CandidacyBond: Balance = 1 * DOLLARS;
	// 1 storage item created, key size is 32 bytes, value size is 16+16.
	pub const VotingBondBase: Balance = deposit(1, 64);
	// additional data per vote is 32 bytes (account id).
	pub const VotingBondFactor: Balance = deposit(0, 32);
	/// Daily council elections.
	pub const TermDuration: BlockNumber = 24 * HOURS;
	pub const ElectionsPhragmenModuleId: LockIdentifier = *b"phrelect";
}

pub enum DesiredMembers { }
impl frame_support::traits::Get<u32> for DesiredMembers {
	fn get() -> u32 {
		let var = variables::U32s::get(b"runtime::elections_phragmen::desired_members".to_vec()).unwrap_or(7);
		max(min(var, 50), 7)
	}
}

pub enum DesiredRunnersUp { }
impl frame_support::traits::Get<u32> for DesiredRunnersUp {
	fn get() -> u32 {
		let var = variables::U32s::get(b"runtime::elections_phragmen::desired_runners_up".to_vec()).unwrap_or(30);
		max(min(var, 100), 7)
	}
}

impl elections_phragmen::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type ChangeMembers = Council;
	type InitializeMembers = Council;
	type CurrencyToVote = frame_support::traits::U128CurrencyToVote;
	type CandidacyBond = CandidacyBond;
	type VotingBondBase = VotingBondBase;
	type VotingBondFactor = VotingBondFactor;
	type DesiredMembers = DesiredMembers;
	type DesiredRunnersUp = DesiredRunnersUp;
	type LoserCandidate = Treasury;
	type KickedMember = Treasury;
	type TermDuration = TermDuration;
	type ModuleId = ElectionsPhragmenModuleId;
	type WeightInfo = ();
}

parameter_types! {
	pub const TechnicalMotionDuration: BlockNumber = 3 * DAYS;
	pub const TechnicalMaxProposals: u32 = 100;
	pub const TechnicalMaxMembers: u32 = 100;
}

type TechnicalCollective = collective::Instance2;
impl collective::Config<TechnicalCollective> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = TechnicalMotionDuration;
	type MaxProposals = TechnicalMaxProposals;
	type MaxMembers = TechnicalMaxMembers;
	type DefaultVote = collective::PrimeDefaultVote;
	type WeightInfo = ();
}

impl membership::Config<membership::Instance1> for Runtime {
	type Event = Event;
	type AddOrigin = system::EnsureRoot<AccountId>;
	type RemoveOrigin = system::EnsureRoot<AccountId>;
	type SwapOrigin = system::EnsureRoot<AccountId>;
	type ResetOrigin = system::EnsureRoot<AccountId>;
	type PrimeOrigin = system::EnsureRoot<AccountId>;
	type MembershipInitialized = TechnicalCommittee;
	type MembershipChanged = TechnicalCommittee;
}

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 20 * DOLLARS;
	pub const SpendPeriod: BlockNumber = 6 * DAYS;
	pub const Burn: Permill = Permill::from_percent(1);
	pub const TreasuryModuleId: ModuleId = ModuleId(*b"py/trsry");

	pub const TipCountdown: BlockNumber = 1 * DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(20);
	pub const TipReportDepositBase: Balance = 1 * DOLLARS;
	pub const DataDepositPerByte: Balance = 1 * CENTS;
	pub const BountyDepositBase: Balance = 1 * DOLLARS;
	pub const BountyDepositPayoutDelay: BlockNumber = 8 * DAYS;
	pub const BountyUpdatePeriod: BlockNumber = 16 * DAYS;
	pub const MaximumReasonLength: u32 = 16384;
	pub const BountyCuratorDeposit: Permill = Permill::from_percent(50);
	pub const BountyValueMinimum: Balance = 10 * DOLLARS;
}

impl treasury::Config for Runtime {
	type Currency = Balances;
	type ApproveOrigin = system::EnsureOneOf<AccountId,
		collective::EnsureProportionMoreThan<_4, _5, AccountId, CouncilCollective>,
		system::EnsureRoot<AccountId>,
	>;
	type RejectOrigin = system::EnsureOneOf<AccountId,
		collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>,
		system::EnsureRoot<AccountId>,
	>;
	type Event = Event;
	type OnSlash = Treasury;
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type SpendPeriod = SpendPeriod;
	type SpendFunds = Bounties;
	type Burn = Burn;
	type BurnDestination = ();
	type ModuleId = TreasuryModuleId;
	type WeightInfo = ();
}

impl bounties::Config for Runtime {
	type Event = Event;
	type BountyDepositBase = BountyDepositBase;
	type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
	type BountyUpdatePeriod = BountyUpdatePeriod;
	type BountyCuratorDeposit = BountyCuratorDeposit;
	type BountyValueMinimum = BountyValueMinimum;
	type DataDepositPerByte = DataDepositPerByte;
	type MaximumReasonLength = MaximumReasonLength;
	type WeightInfo = ();
}

impl tips::Config for Runtime {
	type Event = Event;
	type DataDepositPerByte = DataDepositPerByte;
	type MaximumReasonLength = MaximumReasonLength;
	type Tippers = ElectionsPhragmen;
	type TipCountdown = TipCountdown;
	type TipFindersFee = TipFindersFee;
	type TipReportDepositBase = TipReportDepositBase;
	type WeightInfo = ();
}

parameter_types! {
	// Minimum 100 bytes/KSM deposited (1 CENT/byte)
	pub const BasicDeposit: Balance = 10 * DOLLARS;       // 258 bytes on-chain
	pub const FieldDeposit: Balance = 250 * CENTS;        // 66 bytes on-chain
	pub const SubAccountDeposit: Balance = 2 * DOLLARS;   // 53 bytes on-chain
	pub const MaxSubAccounts: u32 = 100;
	pub const MaxAdditionalFields: u32 = 100;
	pub const MaxRegistrars: u32 = 20;
}

impl identity::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type Slashed = Treasury;
	type BasicDeposit = BasicDeposit;
	type FieldDeposit = FieldDeposit;
	type SubAccountDeposit = SubAccountDeposit;
	type MaxSubAccounts = MaxSubAccounts;
	type MaxAdditionalFields = MaxAdditionalFields;
	type MaxRegistrars = MaxRegistrars;
	type RegistrarOrigin = system::EnsureRoot<AccountId>;
	type ForceOrigin = system::EnsureNever<AccountId>;
	type WeightInfo = ();
}

parameter_types! {
	// One storage item; key size 32, value size 8; .
	pub const ProxyDepositBase: Balance = deposit(1, 8);
	// Additional storage item size of 33 bytes.
	pub const ProxyDepositFactor: Balance = deposit(0, 33);
	pub const MaxProxies: u16 = 32;
	pub const AnnouncementDepositBase: Balance = deposit(1, 8);
	pub const AnnouncementDepositFactor: Balance = deposit(0, 66);
	pub const MaxPending: u16 = 32;
}

/// The type used to represent the kinds of proxying allowed.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug)]
pub enum ProxyType {
	Any,
	NonTransfer,
	Governance,
	IdentityJudgement,
}
impl Default for ProxyType { fn default() -> Self { Self::Any } }
impl InstanceFilter<Call> for ProxyType {
	fn filter(&self, c: &Call) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::NonTransfer => matches!(c,
				Call::System(..) |
				Call::Timestamp(..) |
				Call::Indices(indices::Call::claim(..)) |
				Call::Indices(indices::Call::free(..)) |
				Call::Indices(indices::Call::freeze(..)) |
				// Specifically omitting Indices `transfer`, `force_transfer`
				// Specifically omitting the entire Balances pallet
				Call::Democracy(..) |
				Call::Council(..) |
				Call::TechnicalCommittee(..) |
				Call::ElectionsPhragmen(..) |
				Call::TechnicalMembership(..) |
				Call::Treasury(..) |
				Call::Utility(..) |
				Call::Identity(..) |
				Call::Vesting(vesting::Call::vest(..)) |
				Call::Vesting(vesting::Call::vest_other(..)) |
				// Specifically omitting Vesting `vested_transfer`, and `force_vested_transfer`
				Call::Scheduler(..) |
				Call::Proxy(..) |
				Call::Multisig(..)
			),
			ProxyType::Governance => matches!(c,
				Call::Democracy(..) | Call::Council(..) | Call::TechnicalCommittee(..)
					| Call::ElectionsPhragmen(..) | Call::Treasury(..) | Call::Utility(..)
			),
			ProxyType::IdentityJudgement => matches!(c,
				Call::Identity(identity::Call::provide_judgement(..))
				| Call::Utility(utility::Call::batch(..))
			)
		}
	}
	fn is_superset(&self, o: &Self) -> bool {
		match (self, o) {
			(x, y) if x == y => true,
			(ProxyType::Any, _) => true,
			(_, ProxyType::Any) => false,
			(ProxyType::NonTransfer, _) => true,
			_ => false,
		}
	}
}

impl proxy::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type Currency = Balances;
	type ProxyType = ProxyType;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type MaxProxies = MaxProxies;
	type WeightInfo = ();
	type MaxPending = MaxPending;
	type CallHasher = BlakeTwo256;
	type AnnouncementDepositBase = AnnouncementDepositBase;
	type AnnouncementDepositFactor = AnnouncementDepositFactor;
}

parameter_types! {
	pub const MinVestedTransfer: Balance = 10 * DOLLARS;
}

impl vesting::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type BlockNumberToBalance = ConvertInto;
	type MinVestedTransfer = MinVestedTransfer;
	type WeightInfo = ();
}

impl variables::Config for Runtime {
	type Event = Event;
}

pub struct PhragmenElectionDepositRuntimeUpgrade;
impl elections_phragmen::migrations_3_0_0::V2ToV3 for PhragmenElectionDepositRuntimeUpgrade {
	type AccountId = AccountId;
	type Balance = Balance;
	type Module = ElectionsPhragmen;
}
impl frame_support::traits::OnRuntimeUpgrade for PhragmenElectionDepositRuntimeUpgrade {
	fn on_runtime_upgrade() -> frame_support::weights::Weight {
		elections_phragmen::migrations_3_0_0::apply::<Self>(5 * CENTS, DOLLARS)
	}
}

parameter_types! {
	pub const PayloadLenLimit: u32 = 1024;
	pub const RemoveKeysLimit: u32 = 1024;
}

impl lockdrop::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type PayloadLenLimit = PayloadLenLimit;
	type RemoveKeysLimit = RemoveKeysLimit;
	type WeightInfo = crate::weights::lockdrop::WeightInfo<Self>;
}

parameter_types! {
	pub const TombstoneDeposit: Balance = deposit(
		1,
		sp_std::mem::size_of::<contracts::ContractInfo<Runtime>>() as u32
	);
	pub const DepositPerContract: Balance = TombstoneDeposit::get();
	pub const DepositPerStorageByte: Balance = deposit(0, 1);
	pub const DepositPerStorageItem: Balance = deposit(1, 0);
	pub RentFraction: Perbill = Perbill::from_rational_approximation(1u32, 30 * DAYS);
	pub const SurchargeReward: Balance = 150 * MILLICENTS;
	pub const SignedClaimHandicap: u32 = 2;
	pub const MaxDepth: u32 = 32;
	pub const MaxValueSize: u32 = 16 * 1024;
	// The lazy deletion runs inside on_initialize.
	pub DeletionWeightLimit: Weight = AVERAGE_ON_INITIALIZE_RATIO *
		BlockWeights::get().max_block;
	// The weight needed for decoding the queue should be less or equal than a fifth
	// of the overall weight dedicated to the lazy deletion.
	pub DeletionQueueDepth: u32 = ((DeletionWeightLimit::get() / (
			<Runtime as contracts::Config>::WeightInfo::on_initialize_per_queue_item(1) -
			<Runtime as contracts::Config>::WeightInfo::on_initialize_per_queue_item(0)
		)) / 5) as u32;
	pub MaxCodeSize: u32 = 128 * 1024;
}

impl contracts::Config for Runtime {
	type Time = Timestamp;
	type Randomness = RandomnessCollectiveFlip;
	type Currency = Balances;
	type Event = Event;
	type RentPayment = ();
	type SignedClaimHandicap = SignedClaimHandicap;
	type TombstoneDeposit = TombstoneDeposit;
	type DepositPerContract = DepositPerContract;
	type DepositPerStorageByte = DepositPerStorageByte;
	type DepositPerStorageItem = DepositPerStorageItem;
	type RentFraction = RentFraction;
	type SurchargeReward = SurchargeReward;
	type MaxDepth = MaxDepth;
	type MaxValueSize = MaxValueSize;
	type WeightPrice = transaction_payment::Module<Self>;
	type WeightInfo = contracts::weights::SubstrateWeight<Self>;
	type ChainExtension = ();
	type DeletionQueueDepth = DeletionQueueDepth;
	type DeletionWeightLimit = DeletionWeightLimit;
	type MaxCodeSize = MaxCodeSize;
}

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		// Basic stuff.
		System: system::{Module, Call, Storage, Config, Event<T>} = 0,
		RandomnessCollectiveFlip: randomness_collective_flip::{Module, Call, Storage} = 17,
		Timestamp: timestamp::{Module, Call, Storage, Inherent} = 1,
		Indices: indices::{Module, Call, Storage, Config<T>, Event<T>} = 2,
		Balances: balances::{Module, Call, Storage, Config<T>, Event<T>} = 3,
		TransactionPayment: transaction_payment::{Module, Storage} = 18,

		// PoW consensus and era support.
		Difficulty: difficulty::{Module, Call, Storage, Config} = 19,
		Eras: eras::{Module, Call, Storage, Config<T>} = 20,
		Rewards: rewards::{Module, Call, Storage, Event<T>, Config<T>} = 4,

		// Governance.
		Democracy: democracy::{Module, Call, Storage, Config, Event<T>} = 5,
		Council: collective::<Instance1>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>} = 6,
		TechnicalCommittee: collective::<Instance2>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>} = 7,
		ElectionsPhragmen: elections_phragmen::{Module, Call, Storage, Event<T>, Config<T>} = 8,
		TechnicalMembership: membership::<Instance1>::{Module, Call, Storage, Event<T>, Config<T>} = 9,
		Treasury: treasury::{Module, Call, Storage, Event<T>, Config} = 10,
		Bounties: bounties::{Module, Call, Storage, Event<T>} = 22,
		Tips: tips::{Module, Call, Storage, Event<T>} = 23,

		Identity: identity::{Module, Call, Storage, Event<T>} = 11,
		Utility: utility::{Module, Call, Event} = 12,
		Scheduler: scheduler::{Module, Call, Storage, Event<T>} = 13,
		Multisig: multisig::{Module, Call, Storage, Event<T>} = 14,
		Proxy: proxy::{Module, Call, Storage, Event<T>} = 15,
		Vesting: vesting::{Module, Call, Storage, Event<T>, Config<T>} = 16,
		Variables: variables::{Module, Call, Storage, Event} = 21,
		Lockdrop: lockdrop::{Module, Call, Storage, Event<T>} = 24,
		Contracts: contracts::{Module, Call, Config<T>, Storage, Event<T>},
	}
);

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, AccountIndex>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	system::CheckSpecVersion<Runtime>,
	system::CheckTxVersion<Runtime>,
	system::CheckGenesis<Runtime>,
	system::CheckEra<Runtime>,
	system::CheckNonce<Runtime>,
	system::CheckWeight<Runtime>,
	transaction_payment::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	system::ChainContext<Runtime>,
	Runtime,
	AllModules,
	PhragmenElectionDepositRuntimeUpgrade,
>;

impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			Runtime::metadata().into()
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}

		fn random_seed() -> <Block as BlockT>::Hash {
			RandomnessCollectiveFlip::random_seed()
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(_seed: Option<Vec<u8>>) -> Vec<u8> {
			Default::default()
		}

		fn decode_session_keys(
			_encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, sp_core::crypto::KeyTypeId)>> {
			None
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
	}

	impl sp_consensus_pow::TimestampApi<Block, u64> for Runtime {
		fn timestamp() -> u64 {
			timestamp::Module::<Runtime>::get()
		}
	}

	impl sp_consensus_pow::DifficultyApi<Block, kulupu_primitives::Difficulty> for Runtime {
		fn difficulty() -> kulupu_primitives::Difficulty {
			difficulty::Module::<Runtime>::difficulty()
		}
	}

	impl kulupu_primitives::AlgorithmApi<Block> for Runtime {
		fn identifier() -> [u8; 8] {
			kulupu_primitives::ALGORITHM_IDENTIFIER_V2
		}
	}

	impl pallet_contracts_rpc_runtime_api::ContractsApi<Block, AccountId, Balance, BlockNumber> for Runtime {
		fn call(
			origin: AccountId,
			dest: AccountId,
			value: Balance,
			gas_limit: u64,
			input_data: Vec<u8>,
		) -> pallet_contracts_primitives::ContractExecResult {
			Contracts::bare_call(origin, dest, value, gas_limit, input_data)
		}

		fn get_storage(
			address: AccountId,
			key: [u8; 32],
		) -> pallet_contracts_primitives::GetStorageResult {
			Contracts::get_storage(address, key)
		}

		fn rent_projection(
			address: AccountId,
		) -> pallet_contracts_primitives::RentProjectionResult<BlockNumber> {
			Contracts::rent_projection(address)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, add_benchmark, TrackedStorageKey};
			use frame_system_benchmarking::Module as SystemBench;

			impl frame_system_benchmarking::Config for Runtime {}

			let whitelist: Vec<TrackedStorageKey> = vec![
				// Block Number
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
				// Total Issuance
				hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
				// Execution Phase
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
				// Event Count
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
				// System Events
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
				// System Digest
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef799e7f93fc6a98f0874fd057f111c4d2d").to_vec().into(),
				// Treasury Account
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7b99d880ec681799c0cf30e8886371da95ecffd7b6c0f78751baa9d281e0bfa3a6d6f646c70792f74727372790000000000000000000000000000000000000000").to_vec().into(),
			];

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);

			add_benchmark!(params, batches, balances, Balances);
			add_benchmark!(params, batches, collective, Council);
			add_benchmark!(params, batches, democracy, Democracy);
			add_benchmark!(params, batches, identity, Identity);
			add_benchmark!(params, batches, indices, Indices);
			add_benchmark!(params, batches, multisig, Multisig);
			add_benchmark!(params, batches, proxy, Proxy);
			add_benchmark!(params, batches, scheduler, Scheduler);
			add_benchmark!(params, batches, system, SystemBench::<Runtime>);
			add_benchmark!(params, batches, timestamp, Timestamp);
			add_benchmark!(params, batches, treasury, Treasury);
			add_benchmark!(params, batches, utility, Utility);
			add_benchmark!(params, batches, vesting, Vesting);

			add_benchmark!(params, batches, rewards, Rewards);
			add_benchmark!(params, batches, lockdrop, Lockdrop);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}
}
