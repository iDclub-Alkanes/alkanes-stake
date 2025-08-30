use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;

use alkanes_runtime::{
    declare_alkane, message::MessageDispatch, runtime::AlkaneResponder, storage::StoragePointer,
    token::Token,
};

use alkanes_support::{
    cellpack::Cellpack,
    id::AlkaneId,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::CallResponse,
};

use anyhow::{Result, anyhow};
use std::sync::Arc;

const COLLECTION_SYMBOL: &str = "SLP";

#[derive(Default)]
pub struct StakingVault(());

impl AlkaneResponder for StakingVault {}

#[derive(MessageDispatch)]
enum StakingVaultMessage {
    #[opcode(0)]
    Initialize { index: u128 },

    #[opcode(51)]
    Unstake,

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

    #[opcode(999)]
    #[returns(Vec<u8>)]
    GetNftIndex,

    #[opcode(1000)]
    #[returns(Vec<u8>)]
    GetData,

    #[opcode(1001)]
    #[returns(String)]
    GetContentType,

    #[opcode(1002)]
    #[returns(String)]
    GetAttributes,
}

impl Token for StakingVault {
    fn name(&self) -> String {
        let collection_name = self.get_collection_name();
        format!("{} #{}", collection_name, self.index())
    }

    fn symbol(&self) -> String {
        format!("{} #{}", COLLECTION_SYMBOL, self.index())
    }
}

impl StakingVault {

    fn initialize(&self, index: u128) -> Result<CallResponse> {
        self.observe_initialization()?;

        let context = self.context()?;
        self.set_collection_alkane_id(&context.caller);
        self.set_index(index);

        let mut response = CallResponse::forward(&AlkaneTransferParcel::default());
        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself.clone(),
            value: 1u128,
        });
        Ok(response)
    }

    fn unstake(&self) -> Result<CallResponse> {
        self.only_owner()?;
        let context = self.context()?;
        if context.incoming_alkanes.0.len() != 1 {
            return Err(anyhow!("Include multiple alkanes"));
        }

        let mut response = CallResponse::forward(&AlkaneTransferParcel::default());
        let collection_id = self.collection_ref();
        let cellpack = Cellpack {
            target: collection_id,
            inputs: vec![51, self.index()],
        };

        let call_response = self.call(&cellpack, &AlkaneTransferParcel::default(), self.fuel())?;
        call_response.alkanes.0.iter().for_each(|alkane| {
            response.alkanes.0.push(*alkane);
        });

        let staking_token_id = AlkaneId::try_from(call_response.data[0..32].to_vec())?;
        response.alkanes.0.push(AlkaneTransfer {
            id: staking_token_id,
            value: self.balance(&context.myself, &staking_token_id),
        });

        Ok(response)
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
        response.data = (&1u128.to_le_bytes()).to_vec();
        Ok(response)
    }

    fn get_collection_identifier(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let collection = self.collection_ref();
        response.data = format!("{}:{}", collection.block, collection.tx).into_bytes();
        Ok(response)
    }

    fn get_nft_index(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = self.index().to_le_bytes().to_vec();
        Ok(response)
    }

    fn get_data(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let collection_id = self.collection_ref();
        let cellpack = Cellpack {
            target: collection_id,
            inputs: vec![1000, self.index()],
        };

        let call_response =
            self.staticcall(&cellpack, &AlkaneTransferParcel::default(), self.fuel())?;
        response.data = call_response.data;
        Ok(response)
    }

    fn get_content_type(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = String::from("image/png").into_bytes().to_vec();
        Ok(response)
    }

    fn get_attributes(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        let collection_id = self.collection_ref();

        let cellpack = Cellpack {
            target: collection_id,
            inputs: vec![1002],
        };

        let call_response =
            self.staticcall(&cellpack, &AlkaneTransferParcel::default(), self.fuel())?;
        response.data = call_response.data;
        Ok(response)
    }

    fn only_owner(&self) -> Result<()> {
        let context = self.context()?;

        if context.incoming_alkanes.0.len() != 1 {
            return Err(anyhow!(
                "did not authenticate with only the authentication token"
            ));
        }

        let transfer = context.incoming_alkanes.0[0].clone();
        if transfer.id != context.myself.clone() {
            return Err(anyhow!("supplied alkane is not authentication token"));
        }

        if transfer.value < 1 {
            return Err(anyhow!(
                "less than 1 unit of authentication token supplied to authenticate"
            ));
        }

        Ok(())
    }

    fn set_collection_alkane_id(&self, id: &AlkaneId) {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(&id.block.to_le_bytes());
        bytes.extend_from_slice(&id.tx.to_le_bytes());
        self.collection_alkane_id_pointer().set(Arc::new(bytes));
    }

    fn collection_alkane_id_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/collection-alkane-id")
    }

    fn collection_ref(&self) -> AlkaneId {
        let data = self.collection_alkane_id_pointer().get();
        if data.len() == 0 {
            panic!("Collection reference not found");
        }

        let bytes = data.as_ref();
        AlkaneId {
            block: u128::from_le_bytes(bytes[0..16].try_into().unwrap()),
            tx: u128::from_le_bytes(bytes[16..32].try_into().unwrap()),
        }
    }

    fn get_collection_name(&self) -> String {
        let collection_id = self.collection_ref();
        let cellpack = Cellpack {
            target: collection_id,
            inputs: vec![99],  // opcode 99 for GetName
        };

        match self.staticcall(&cellpack, &AlkaneTransferParcel::default(), self.fuel()) {
            Ok(call_response) => {
                String::from_utf8(call_response.data).unwrap_or_else(|_| "Unknown".to_string())
            }
            Err(_) => "Unknown".to_string()
        }
    }

    fn index_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/index")
    }

    fn index(&self) -> u128 {
        self.index_pointer().get_value::<u128>()
    }

    fn set_index(&self, index: u128) {
        self.index_pointer().set_value::<u128>(index);
    }
}

declare_alkane! {
  impl AlkaneResponder for StakingVault {
    type Message = StakingVaultMessage;
  }
}
