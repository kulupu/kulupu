use kulupu_runtime::{
	AccountId, BalancesConfig, GenesisConfig,
	IndicesConfig, SystemConfig, DifficultyConfig, WASM_BINARY,
};
use kulupu_primitives::{U256, Difficulty};
use substrate_service;

// Note this is the URL for the telemetry server
//const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = substrate_service::ChainSpec<GenesisConfig>;

/// The chain specification option. This is expected to come in from the CLI and
/// is little more than one of a number of alternatives which can easily be converted
/// from a string (`--chain=...`) into a `ChainSpec`.
#[derive(Clone, Debug)]
pub enum Alternative {
	/// Whatever the current runtime is, with just Alice as an auth.
	Development,
	/// Whatever the current runtime is, with simple Alice/Bob auths.
	LocalTestnet,
	/// Kulupu
	Kulupu,
}

impl Alternative {
	/// Get an actual chain config from one of the alternatives.
	pub(crate) fn load(self) -> Result<ChainSpec, String> {
		Ok(match self {
			Alternative::Development => ChainSpec::from_genesis(
				"Development",
				"dev",
				|| testnet_genesis(
					vec![],
					true
				),
				vec![],
				None,
				None,
				None,
				None
			),
			Alternative::LocalTestnet => ChainSpec::from_genesis(
				"Local Testnet",
				"local_testnet",
				|| testnet_genesis(
					vec![],
					true
				),
				vec![],
				None,
				None,
				None,
				None
			),
			Alternative::Kulupu => ChainSpec::from_genesis(
				"Kulupu",
				"kulupu",
				|| mainnet_genesis(),
				vec![
					"/dns4/node.kulupu.network/tcp/30333/p2p/QmRpJhTrFUjPGKvvP9HDWC81ByK3WN2vvVPVKbn71BnMiB".to_string(),
				],
				None,
				Some("kul"),
				None,
				None
			),
		})
	}

	pub(crate) fn from(s: &str) -> Option<Self> {
		match s {
			"dev" => Some(Alternative::Development),
			"local" => Some(Alternative::LocalTestnet),
			"" | "kulupu" => Some(Alternative::Kulupu),
			_ => None,
		}
	}
}

fn testnet_genesis(
	endowed_accounts: Vec<AccountId>,
	_enable_println: bool) -> GenesisConfig {
	GenesisConfig {
		system: Some(SystemConfig {
			code: WASM_BINARY.to_vec(),
			changes_trie_config: Default::default(),
		}),
		indices: Some(IndicesConfig {
			ids: endowed_accounts.clone(),
		}),
		balances: Some(BalancesConfig {
			balances: endowed_accounts.iter().cloned().map(|k|(k, 1 << 60)).collect(),
			vesting: vec![],
		}),
		difficulty: Some(DifficultyConfig {
			initial_difficulty: Difficulty(U256::from(200)),
		}),
	}
}

fn mainnet_genesis() -> GenesisConfig {
	GenesisConfig {
		system: Some(SystemConfig {
			code: include_bytes!("../res/0-genesis/kulupu_runtime.compact.wasm").to_vec(),
			changes_trie_config: Default::default(),
		}),
		indices: Some(IndicesConfig {
			ids: vec![],
		}),
		balances: Some(BalancesConfig {
			balances: vec![],
			vesting: vec![],
		}),
		difficulty: Some(DifficultyConfig {
			initial_difficulty: Difficulty(U256::from(10000)),
		}),
	}
}
