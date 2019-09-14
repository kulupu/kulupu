//! Compact representation of `U256`

use primitives::{H256, U256};

/// Compact representation of `U256`
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Compact(u32);

impl From<u32> for Compact {
	fn from(u: u32) -> Self {
		Compact(u)
	}
}

impl From<Compact> for u32 {
	fn from(c: Compact) -> Self {
		c.0
	}
}

impl From<U256> for Compact {
	fn from(u: U256) -> Self {
		Compact::from_u256(u)
	}
}

impl From<Compact> for U256 {
	fn from(c: Compact) -> Self {
		// ignore overflows and negative values
		c.to_u256().unwrap_or_else(|x| x)
	}
}

impl From<u128> for Compact {
	fn from(u: u128) -> Self {
		let u = core::cmp::min(u, u32::max_value() as u128);
		Compact(u as u32)
	}
}

impl Into<u128> for Compact {
	fn into(self) -> u128 {
		self.0 as u128
	}
}

impl Compact {
	pub fn new(u: u32) -> Self {
		Compact(u)
	}

	pub fn max_value() -> Self {
		U256::max_value().into()
	}

	pub fn verify(&self, hash: H256) -> bool {
		let num = U256::from(&hash[..]);
		let max = match self.to_u256() {
			Ok(v) => v,
			Err(_) => return false,
		};

		num < max
	}

	/// Computes the target [0, T] that a blockhash must land in to be valid
	/// Returns value in error, if there is an overflow or its negative value
	pub fn to_u256(&self) -> Result<U256, U256> {
		let size = self.0 >> 24;
		let mut word = self.0 & 0x007fffff;

		let result = if size <= 3 {
			word >>= 8 * (3 - size as usize);
			word.into()
		} else {
			U256::from(word) << (8 * (size as usize - 3))
		};

		let is_negative = word != 0 && (self.0 & 0x00800000) != 0;
		let is_overflow = (word != 0 && size > 34) ||
				(word > 0xff && size > 33) ||
				(word > 0xffff && size > 32);

		if is_negative || is_overflow {
			Err(result)
		} else {
			Ok(result)
		}
	}

	pub fn from_u256(val: U256) -> Self {
		let mut size = (val.bits() + 7) / 8;
		let mut compact = if size <= 3 {
			(val.low_u64() << (8 * (3 - size))) as u32
		} else {
			let bn = val >> (8 * (size - 3));
			bn.low_u32()
		};

		if (compact & 0x00800000) != 0 {
			compact >>= 8;
			size += 1;
		}

		assert!((compact & !0x007fffff) == 0);
		assert!(size < 256);
		Compact(compact | (size << 24) as u32)
	}

	pub fn to_f64(&self) -> f64 {
		let mut shift = (self.0 >> 24) & 0xff;
		let mut diff = f64::from(0x0000ffffu32) / f64::from(self.0 & 0x00ffffffu32);
		while shift < 29 {
			diff *= f64::from(256);
			shift += 1;
		}
		while shift > 29 {
			diff /= f64::from(256.0);
			shift -= 1;
		}
		diff
	}
}
