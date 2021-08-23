use super::*;
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;

benchmarks! {
	create_campaign {
		let campaign = [b't', b'e', b's', b't'];
	}: _(RawOrigin::Root, campaign, 20u32.into(), 30u32.into())

	conclude_campaign {
		let caller = whitelisted_caller();
		let campaign = [b't', b'e', b's', b't'];

		Campaigns::<T>::insert(campaign, CampaignInfo {
			end_block: 20u32.into(),
			min_lock_end_block: 30u32.into(),
			child_root: None,
		});

		frame_system::Module::<T>::set_block_number(40u32.into());
	}: _(RawOrigin::Signed(caller), campaign)

	remove_expired_campaign {
		let caller = whitelisted_caller();
		let campaign = [b't', b'e', b's', b't'];

		Campaigns::<T>::insert(campaign, CampaignInfo {
			end_block: 20u32.into(),
			min_lock_end_block: 30u32.into(),
			child_root: None,
		});

		frame_system::Module::<T>::set_block_number(40u32.into());
	}: _(RawOrigin::Signed(caller), campaign)

	lock {
		let caller = whitelisted_caller();
		let campaign = [b't', b'e', b's', b't'];

		Campaigns::<T>::insert(campaign, CampaignInfo {
			end_block: 20u32.into(),
			min_lock_end_block: 30u32.into(),
			child_root: None,
		});
	}: _(RawOrigin::Signed(caller), Default::default(), campaign, 40u32.into(), None)

	unlock {
		let caller = whitelisted_caller();
		let campaign = [b't', b'e', b's', b't'];

		Campaigns::<T>::insert(campaign, CampaignInfo {
			end_block: 20u32.into(),
			min_lock_end_block: 30u32.into(),
			child_root: None,
		});

		frame_system::Module::<T>::set_block_number(40u32.into());
	}: _(RawOrigin::Signed(caller), campaign)
}
