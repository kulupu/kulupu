use sp_runtime::Perbill;
use frame_support::weights::{WeightToFeePolynomial, WeightToFeeCoefficient, WeightToFeeCoefficients};
use kulupu_primitives::CENTS;
use smallvec::smallvec;
use crate::{Balance, ExtrinsicBaseWeight};

pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
	type Balance = Balance;
	fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
		// in Kulupu, extrinsic base weight (smallest non-zero weight) is mapped to 1/10 CENT:
		let p = CENTS;
		let q = 10 * Balance::from(ExtrinsicBaseWeight::get());
		smallvec![WeightToFeeCoefficient {
			degree: 1,
			negative: false,
			coeff_frac: Perbill::from_rational_approximation(p % q, q),
			coeff_integer: p / q,
		}]
	}
}

#[cfg(test)]
mod tests {
	use frame_support::weights::WeightToFeePolynomial;
	use super::{WeightToFee, ExtrinsicBaseWeight};
	use crate::MaximumBlockWeight;
	use kulupu_primitives::{CENTS, DOLLARS};

	#[test]
	// This function tests that the fee for `MaximumBlockWeight` of weight is correct
	fn full_block_fee_is_correct() {
		// A full block should cost 16 DOLLARS
		assert_eq!(WeightToFee::calc(&MaximumBlockWeight::get()), 16 * DOLLARS)
	}

	#[test]
	// This function tests that the fee for `ExtrinsicBaseWeight` of weight is correct
	fn extrinsic_base_fee_is_correct() {
		// `ExtrinsicBaseWeight` should cost 1/10 of a CENT
		assert_eq!(WeightToFee::calc(&ExtrinsicBaseWeight::get()), CENTS / 10)
	}
}
