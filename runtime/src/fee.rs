use sp_std::num::NonZeroI128;
use sp_runtime::{Perquintill, Fixed128, traits::{Convert, Saturating}};
use frame_support::{traits::Get, weights::Weight};
use kulupu_primitives::CENTS;
use crate::{Balance, MaximumBlockWeight, ExtrinsicBaseWeight};

pub struct WeightToFee;
impl Convert<Weight, Balance> for WeightToFee {
	fn convert(x: Weight) -> Balance {
		// Weight of 10_000_000 (smallest non-zero weight) is mapped to 1/10 CENT:
		Balance::from(x).saturating_mul(CENTS / 10) / Balance::from(ExtrinsicBaseWeight::get())
	}
}

/// Update the given multiplier based on the following formula
///
///   diff = (previous_block_weight - target_weight)/max_weight
///   v = 0.00004
///   next_weight = weight * (1 + (v * diff) + (v * diff)^2 / 2)
///
/// Where `target_weight` must be given as the `Get` implementation of the `T` generic type.
/// https://research.web3.foundation/en/latest/polkadot/Token%20Economics/#relay-chain-transaction-fees
pub struct TargetedFeeAdjustment<T, R>(sp_std::marker::PhantomData<(T, R)>);

impl<T: Get<Perquintill>, R: system::Trait> Convert<Fixed128, Fixed128> for TargetedFeeAdjustment<T, R> {
	fn convert(multiplier: Fixed128) -> Fixed128 {
		let max_weight = MaximumBlockWeight::get();
		let block_weight = <system::Module<R>>::all_extrinsics_weight().total().min(max_weight);
		let target_weight = (T::get() * max_weight) as u128;
		let block_weight = block_weight as u128;

		// determines if the first_term is positive
		let positive = block_weight >= target_weight;
		let diff_abs = block_weight.max(target_weight) - block_weight.min(target_weight);
		// safe, diff_abs cannot exceed u64 and it can always be computed safely even with the lossy
		// `Fixed128::from_rational`.
		let diff = Fixed128::from_rational(
			diff_abs as i128,
			NonZeroI128::new(max_weight.max(1) as i128).unwrap(),
		);
		let diff_squared = diff.saturating_mul(diff);

		// 0.00004 = 4/100_000 = 40_000/10^9
		let v = Fixed128::from_rational(4, NonZeroI128::new(100_000).unwrap());
		// 0.00004^2 = 16/10^10 Taking the future /2 into account... 8/10^10
		let v_squared_2 = Fixed128::from_rational(8, NonZeroI128::new(10_000_000_000).unwrap());

		let first_term = v.saturating_mul(diff);
		let second_term = v_squared_2.saturating_mul(diff_squared);

		if positive {
			// Note: this is merely bounded by how big the multiplier and the inner value can go,
			// not by any economical reasoning.
			let excess = first_term.saturating_add(second_term);
			multiplier.saturating_add(excess)
		} else {
			// Defensive-only: first_term > second_term. Safe subtraction.
			let negative = first_term.saturating_sub(second_term);
			multiplier.saturating_sub(negative)
				// despite the fact that apply_to saturates weight (final fee cannot go below 0)
				// it is crucially important to stop here and don't further reduce the weight fee
				// multiplier. While at -1, it means that the network is so un-congested that all
				// transactions have no weight fee. We stop here and only increase if the network
				// became more busy.
				.max(Fixed128::from_natural(-1))
		}
	}
}

#[cfg(test)]
mod tests {
	use sp_runtime::traits::Convert;
	use super::{WeightToFee, MaximumBlockWeight, ExtrinsicBaseWeight};
	use kulupu_primitives::{CENTS, DOLLARS};

	#[test]
	// This function tests that the fee for `MaximumBlockWeight` of weight is correct
	fn full_block_fee_is_correct() {
		// A full block should cost 16 DOLLARS
		assert_eq!(WeightToFee::convert(MaximumBlockWeight::get()), 16 * DOLLARS)
	}

	#[test]
	// This function tests that the fee for `ExtrinsicBaseWeight` of weight is correct
	fn extrinsic_base_fee_is_correct() {
		// `ExtrinsicBaseWeight` should cost 1/10 of a CENT
		assert_eq!(WeightToFee::convert(ExtrinsicBaseWeight::get()), CENTS / 10)
	}
}
