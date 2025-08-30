use alkanes_runtime::{
    declare_alkane, message::MessageDispatch, runtime::AlkaneResponder, storage::StoragePointer,
    token::Token,
};
use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;

use alkanes_support::{
    cellpack::Cellpack,
    id::AlkaneId,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::CallResponse,
};

use anyhow::{Result, anyhow};
use std::sync::Arc;

const COLLECTION_SYMBOL: &str = "SLP";
static COLLECTION_IMAGE: &[u8] = include_bytes!("assets/vault.png");

#[derive(Default)]
pub struct StakingPool(());

impl AlkaneResponder for StakingPool {}

#[derive(MessageDispatch)]
enum StakingPoolMessage {
    #[opcode(0)]
    Initialize {
        start_block: u128,
        end_block: u128,
        vault_template_id: u128,
        reward_token_id: AlkaneId,
        staking_token_id: AlkaneId,
        max_total_stake: u128,
    },

    #[opcode(50)]
    Stake,

    #[opcode(51)]
    Unstake,

    #[opcode(80)]
    Withdraw,

    #[opcode(99)]
    #[returns(String)]
    GetName,

    #[opcode(100)]
    #[returns(String)]
    GetSymbol,

    #[opcode(101)]
    #[returns(u128)]
    GetTotalSupply,

    #[opcode(998)]
    #[returns(String)]
    GetCollectionIdentifier,

    #[opcode(1000)]
    #[returns(Vec<u8>)]
    GetData { index: u128 },

    #[opcode(1002)]
    #[returns(String)]
    GetAttributes,
}

impl Token for StakingPool {
    fn name(&self) -> String {
        self.get_collection_name()
    }

    fn symbol(&self) -> String {
        String::from(COLLECTION_SYMBOL)
    }
}

impl StakingPool {
    fn initialize(
        &self,
        start_block: u128,
        end_block: u128,
        vault_template_id: u128,
        reward_token_id: AlkaneId,
        staking_token_id: AlkaneId,
        max_total_stake: u128,
    ) -> Result<CallResponse> {
        self.observe_initialization()?;

        let context = self.context()?;
        if start_block == 0 || end_block == 0 || vault_template_id == 0 {
            let response = CallResponse::forward(&context.incoming_alkanes);
            return Ok(response)
        }

        self.set_reward_token_id(&reward_token_id);
        self.set_staking_token_id(&staking_token_id);
        self.set_vault_template_id(vault_template_id);
        self.set_max_total_stake(max_total_stake);
        self.start_height_pointer().set_value::<u64>(start_block as u64);
        self.end_height_pointer().set_value::<u64>(end_block as u64);
        
        // Get staking token name and concatenate with "Staking"
        let staking_token_name = self.get_staking_token_name()?;
        let collection_name = format!("{} Staking", staking_token_name);
        self.set_collection_name(&collection_name);
        
        let mut total_reward_amount = 0u128;
        let mut invalid_alkanes = AlkaneTransferParcel::default();
        for alkane in &context.incoming_alkanes.0 {
            if alkane.id == reward_token_id {
                total_reward_amount += alkane.value;
            } else {
                invalid_alkanes.0.push(alkane.clone());
            }
        }
        self.set_total_reward_amount(total_reward_amount);

        // Initialize total staking blocks and total staking amount to 0
        self.set_staking_count(0);
        self.set_total_stake_blocks(0);
        self.set_total_stake_amount(0);

        let mut response = CallResponse::forward(&invalid_alkanes);
        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself.clone(),
            value: 1u128,
        });
        Ok(response)
    }

    fn stake(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let staking_token_id = self.get_staking_token_id();

        // Find the incoming staking asset
        let mut total_amount = 0u128;
        let mut transfer = AlkaneTransferParcel::default();
        let mut invalid_alkanes = AlkaneTransferParcel::default();
        for alkane in &context.incoming_alkanes.0 {
            if alkane.id == staking_token_id {
                transfer.0.push(alkane.clone());
                total_amount += alkane.value;
            } else {
                invalid_alkanes.0.push(alkane.clone());
            }
        }

        // Validate staking parameters
        self.validate_staking_parameters(total_amount)?;

        // Set staking index, starting from 1
        let staking_index = self.get_next_staking_index();
        self.set_staking_count(staking_index);

        // Call vault contract to create staking asset
        let cellpack = Cellpack {
            target: AlkaneId { block: 5, tx: self.get_vault_template_id() },
            inputs: vec![0x0, staking_index],
        };
        let sequence = self.sequence();
        let sub_response = self.call(&cellpack, &transfer, self.fuel())
            .map_err(|e| anyhow!("Failed to create staking position: {}", e))?;
        let vault_alkane = AlkaneId { block: 2, tx: sequence };

        // Store staking data: staking block and staking amount
        let current_height = self.height() as u128;
        let end_height = self.get_end_height() as u128;
        
        // Calculate user's staking blocks (from staking start to mining end)
        let stake_blocks = end_height - current_height;
        
        // Store user's staking block height and staking blocks
        self.set_stake_block(&vault_alkane, current_height);
        self.set_stake_amount(&vault_alkane, total_amount);

        // Store user's staking blocks (for weight calculation)
        self.set_stake_blocks(&vault_alkane, stake_blocks);

        // Store total staking blocks (sum of all users' staking blocks)
        let total_stake_blocks = self.get_total_stake_blocks();
        self.set_total_stake_blocks(total_stake_blocks + stake_blocks);

        // Store total staking amount (sum of all users' staking amounts)
        let total_stake_amount = self.get_total_stake_amount();
        self.set_total_stake_amount(total_stake_amount + total_amount);
        
        let mut response = CallResponse::forward(&invalid_alkanes);
        if sub_response.alkanes.0.is_empty() {
            Err(anyhow!("Failed to create staking position"))
        } else {
            response.alkanes.0.push(sub_response.alkanes.0[0].clone());
            Ok(response)
        }
    }

    fn unstake(&self) -> Result<CallResponse> {
        let context = self.context()?;

        let user_stake_amount = self.get_stake_amount(&context.caller);
        let stake_block = self.get_stake_block(&context.caller);
        if stake_block == 0 || user_stake_amount == 0 {
            return Err(anyhow!("Caller is not a staker"));
        }

        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let end_height = self.get_end_height();
        let current_height = self.height();

        // Expired, normal reward extraction
        if current_height >= end_height {
            // Check if within 7-day (1008 blocks) claim period
            let claim_deadline = end_height + 1008;
            if current_height < claim_deadline {
                let reward_value = self.calc_reward(&context.caller);
                if reward_value > 0 {
                    response.alkanes.0.push(AlkaneTransfer {
                        id: self.get_reward_token_id(),
                        value: reward_value,
                    });

                    // Store user's claimed reward amount
                    self.set_user_claimed_reward(&context.caller, reward_value);
                }
            }

            // Reward data cannot be cleared, used for other stakers' reward calculation
        }
        // Not expired, early withdrawal without rewards
        else {
            // Deduct current staking amount from total, redistribute rewards to other stakers
            let user_stake_blocks = self.get_stake_blocks(&context.caller);
            let total_stake_blocks = self.get_total_stake_blocks();
            self.set_total_stake_blocks(total_stake_blocks.saturating_sub(user_stake_blocks));

            let total_stake_amount = self.get_total_stake_amount();
            self.set_total_stake_amount(total_stake_amount.saturating_sub(user_stake_amount));
        }

        response.data = self.get_staking_token_id().try_into()?;
        Ok(response)
    }

    fn withdraw(&self) -> Result<CallResponse> {
        self.only_owner()?;

        let end_height = self.get_end_height();
        let current_height = self.height();
        let claim_deadline = end_height + 1008;
        if current_height < claim_deadline {
            return Err(anyhow!("Hold on, the user is claiming rewards."));
        }

        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let reward_token = self.get_reward_token_id();
        response.alkanes.0.push(AlkaneTransfer {
            id: reward_token,
            value: self.balance(&context.myself, &reward_token)
        });

        Ok(response)
    }

    fn calc_reward(&self, caller: &AlkaneId) -> u128 {
        let user_stake_blocks = self.get_stake_blocks(caller);
        let user_stake_amount = self.get_stake_amount(caller);
        if user_stake_blocks == 0 || user_stake_amount == 0 {
            return 0;
        }

        // System total weight = total staking amount × total staking blocks
        let total_stake_amount = self.get_total_stake_amount();
        let total_stake_blocks = self.get_total_stake_blocks();
        let total_weight = total_stake_blocks * total_stake_amount;

        // Calculate user weight: staking amount × staking blocks
        let user_weight = user_stake_blocks * user_stake_amount;

        // Calculate user's deserved reward: distributed based on weight ratio
        // Reward = total reward pool × (user weight / total weight)
        let total_reward_amount = self.get_total_reward_amount();
        match user_weight.checked_mul(total_reward_amount) {
            Some(product) => product.checked_div(total_weight).unwrap_or_else(|| 0),
            None => 0,
        }
    }

    fn only_owner(&self) -> Result<()> {
        let context = self.context()?;

        if context.incoming_alkanes.0.len() != 1 {
            return Err(anyhow!("did not authenticate with only the collection token"));
        }

        let transfer = context.incoming_alkanes.0[0].clone();
        if transfer.id != context.myself.clone() {
            return Err(anyhow!("supplied alkane is not collection token"));
        }

        if transfer.value < 1 {
            return Err(anyhow!("less than 1 unit of collection token supplied to authenticate"));
        }

        Ok(())
    }

    fn validate_staking_parameters(&self, stake_amount: u128) -> Result<()> {
        if self.height() < self.get_start_height() {
            return Err(anyhow!("Staking has not started yet"));
        }

        // Reject staking when only 1 block away from deadline
        if self.height() > self.get_end_height() - 2 {
            return Err(anyhow!("Staking period has ended"));
        }

        let current_total_stake = self.get_total_stake_amount();
        if current_total_stake + stake_amount > self.get_max_total_stake() {
            return Err(anyhow!("Total staking amount exceeds maximum limit"));
        }

        Ok(())
    }

    fn reward_token_id_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/reward_token_id")
    }

    fn set_reward_token_id(&self, reward_token_id: &AlkaneId) {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&reward_token_id.block.to_le_bytes());
        bytes.extend_from_slice(&reward_token_id.tx.to_le_bytes());
        self.reward_token_id_pointer().set(Arc::new(bytes));
    }

    fn get_reward_token_id(&self) -> AlkaneId {
        let bytes = self.reward_token_id_pointer().get();
        AlkaneId {
            block: u128::from_le_bytes(bytes[0..16].try_into().unwrap()),
            tx: u128::from_le_bytes(bytes[16..32].try_into().unwrap()),
        }
    }

    fn staking_token_id_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/staking_token_id")
    }

    fn set_staking_token_id(&self, staking_token_id: &AlkaneId) {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&staking_token_id.block.to_le_bytes());
        bytes.extend_from_slice(&staking_token_id.tx.to_le_bytes());
        self.staking_token_id_pointer().set(Arc::new(bytes));
    }

    fn get_staking_token_id(&self) -> AlkaneId {
        let bytes = self.staking_token_id_pointer().get();
        AlkaneId {
            block: u128::from_le_bytes(bytes[0..16].try_into().unwrap()),
            tx: u128::from_le_bytes(bytes[16..32].try_into().unwrap()),
        }
    }

    fn collection_name_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/collection_name")
    }

    fn set_collection_name(&self, name: &str) {
        let name_bytes = name.as_bytes().to_vec();
        self.collection_name_pointer().set(Arc::new(name_bytes));
    }

    fn get_collection_name(&self) -> String {
        let name_bytes = self.collection_name_pointer().get();
        String::from_utf8(name_bytes.to_vec()).unwrap_or_else(|_| "Unknown SLP".to_string())
    }

    fn get_staking_token_name(&self) -> Result<String> {
        let cellpack = Cellpack {
            target: self.get_staking_token_id(),
            inputs: vec![99]
        };

        let call_response =
            self.staticcall(&cellpack, &AlkaneTransferParcel::default(), self.fuel())?;
        
        let name_bytes = call_response.data;
        String::from_utf8(name_bytes).map_err(|e| anyhow!("Failed to parse staking token name: {}", e))
    }

    fn total_stake_blocks_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/total_stake_blocks")
    }

    fn get_total_stake_blocks(&self) -> u128 {
        self.total_stake_blocks_pointer().get_value::<u128>()
    }

    fn set_total_stake_blocks(&self, blocks: u128) {
        self.total_stake_blocks_pointer().set_value::<u128>(blocks);
    }

    fn stake_block_pointer(&self, alkane_id: &AlkaneId) -> StoragePointer {
        StoragePointer::from_keyword(
            format!("/stake_block/{}:{}", alkane_id.block, alkane_id.tx).as_str(),
        )
    }

    fn get_stake_block(&self, alkane_id: &AlkaneId) -> u128 {
        self.stake_block_pointer(alkane_id).get_value::<u128>()
    }

    fn set_stake_block(&self, alkane_id: &AlkaneId, weight: u128) {
        self.stake_block_pointer(alkane_id).set_value::<u128>(weight);
    }

    fn stake_amount_pointer(&self, alkane_id: &AlkaneId) -> StoragePointer {
        StoragePointer::from_keyword(
            format!("/stake_amount/{}:{}", alkane_id.block, alkane_id.tx).as_str(),
        )
    }

    fn get_stake_amount(&self, alkane_id: &AlkaneId) -> u128 {
        self.stake_amount_pointer(alkane_id).get_value::<u128>()
    }

    fn set_stake_amount(&self, alkane_id: &AlkaneId, amount: u128) {
        self.stake_amount_pointer(alkane_id).set_value::<u128>(amount);
    }

    fn stake_blocks_pointer(&self, alkane_id: &AlkaneId) -> StoragePointer {
        StoragePointer::from_keyword(
            format!("/stake_blocks/{}:{}", alkane_id.block, alkane_id.tx).as_str(),
        )
    }

    fn get_stake_blocks(&self, alkane_id: &AlkaneId) -> u128 {
        self.stake_blocks_pointer(alkane_id).get_value::<u128>()
    }

    fn set_stake_blocks(&self, alkane_id: &AlkaneId, blocks: u128) {
        self.stake_blocks_pointer(alkane_id).set_value::<u128>(blocks);
    }

    fn get_next_staking_index(&self) -> u128 {
        self.get_staking_count().checked_add(1).unwrap_or(1)
    }

    fn staking_count_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/staking_count")
    }

    pub fn get_staking_count(&self) -> u128 {
        self.staking_count_pointer().get_value::<u128>()
    }

    fn set_staking_count(&self, count: u128) {
        self.staking_count_pointer().set_value::<u128>(count)
    }

    pub fn get_start_height(&self) -> u64 {
        self.start_height_pointer().get_value::<u64>()
    }

    pub fn start_height_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/start_height")
    }

    pub fn get_end_height(&self) -> u64 {
        self.end_height_pointer().get_value::<u64>()
    }

    pub fn end_height_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/end_height")
    }

    fn vault_template_id_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/vault_template_id")
    }

    fn set_vault_template_id(&self, vault_template_id: u128) {
        let mut p = self.vault_template_id_pointer();
        p.set_value::<u128>(vault_template_id);
    }

    fn get_vault_template_id(&self) -> u128 {
        self.vault_template_id_pointer().get_value::<u128>()
    }

    fn max_total_stake_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/max_total_stake")
    }

    fn set_max_total_stake(&self, max_total_stake: u128) {
        self.max_total_stake_pointer().set_value::<u128>(max_total_stake)
    }

    fn get_max_total_stake(&self) -> u128 {
        self.max_total_stake_pointer().get_value::<u128>()
    }

    fn total_stake_amount_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/total_stake_amount")
    }

    fn get_total_stake_amount(&self) -> u128 {
        self.total_stake_amount_pointer().get_value::<u128>()
    }

    fn set_total_stake_amount(&self, total_stake: u128) {
        self.total_stake_amount_pointer().set_value::<u128>(total_stake);
    }

    fn total_reward_amount_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/total_reward_amount")
    }

    fn get_total_reward_amount(&self) -> u128 {
        self.total_reward_amount_pointer().get_value::<u128>()
    }

    fn set_total_reward_amount(&self, amount: u128) {
        self.total_reward_amount_pointer().set_value::<u128>(amount);
    }

    fn user_claimed_reward_pointer(&self, alkane_id: &AlkaneId) -> StoragePointer {
        StoragePointer::from_keyword(
            format!("/user_claimed_reward/{}:{}", alkane_id.block, alkane_id.tx).as_str(),
        )
    }

    fn set_user_claimed_reward(&self, alkane_id: &AlkaneId, claimed_reward: u128) {
        self.user_claimed_reward_pointer(alkane_id).set_value::<u128>(claimed_reward);
    }

    fn get_user_claimed_reward(&self, alkane_id: &AlkaneId) -> u128 {
        self.user_claimed_reward_pointer(alkane_id).get_value::<u128>()
    }

    fn get_name(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = self.name().into_bytes();
        Ok(response)
    }

    fn get_symbol(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = self.symbol().into_bytes();
        Ok(response)
    }

    fn get_total_supply(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = self.get_staking_count().to_le_bytes().to_vec();
        Ok(response)
    }

    fn get_collection_identifier(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let identifier = format!("{}:{}", context.myself.block, context.myself.tx);
        response.data = identifier.into_bytes();
        Ok(response)
    }

    pub fn get_data(&self, _index: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = COLLECTION_IMAGE.to_vec();
        Ok(response)
    }

    pub fn get_attributes(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let end_height = self.get_end_height() as u128;

        // Query caller's staking information
        let stake_block = self.get_stake_block(&context.caller);
        let stake_amount = self.get_stake_amount(&context.caller);

        // If no staking info, return staking pool information
        if stake_block == 0 || stake_amount == 0 {
            let stake_alkane = self.get_staking_token_id();
            let reward_alkane = self.get_reward_token_id();
            let pool_info = format!(
                r#"{{"start_block":{},"end_block":{},"staking_token":"{}","reward_token":"{}","max_total_stake":"{}","total_stake_amount":"{}","total_reward_amount":"{}","claimable_reward_amount":"{}"}}"#,
                self.get_start_height(),
                end_height,
                format!("{}:{}", stake_alkane.block, stake_alkane.tx).as_str(),
                format!("{}:{}", reward_alkane.block, reward_alkane.tx).as_str(),
                self.get_max_total_stake(),
                self.get_total_stake_amount(),
                self.get_total_reward_amount(),
                self.balance(&context.myself, &reward_alkane)
            );
            response.data = pool_info.into_bytes();
            return Ok(response)
        }
        
        // Calculate total reward that can be mined
        let total_reward = self.calc_reward(&context.caller);
        
        // Calculate mined reward (based on user's staking blocks)
        let current_height = self.height() as u128;

        let mined_reward = if current_height < end_height {
            // Calculate mined blocks from staking start to current block
            let mined_blocks = if current_height > stake_block {
                current_height - stake_block
            } else {
                0
            };
            
            // Calculate user's total staking blocks (from staking start to mining end)
            let total_stake_blocks = end_height - stake_block;
            
            if mined_blocks > 0 && total_stake_blocks > 0 {
                (total_reward * mined_blocks) / total_stake_blocks
            } else {
                0
            }
        } else {
            // Mining ended, all rewards have been mined
            total_reward
        };
        
        // Get whether user has claimed rewards
        let claimed_reward = self.get_user_claimed_reward(&context.caller);
        
        let stake_info = format!(
            r#"{{"stake_block":{},"stake_amount":"{}","total_reward":"{}","mined_reward":"{}","claimed_reward":"{}"}}"#,
            stake_block, stake_amount, total_reward, mined_reward, claimed_reward
        );
        response.data = stake_info.into_bytes();
        Ok(response)
    }
}

declare_alkane! {
    impl AlkaneResponder for StakingPool {
        type Message = StakingPoolMessage;
    }
}
