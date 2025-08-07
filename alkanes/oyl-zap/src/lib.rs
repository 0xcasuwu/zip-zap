use alkanes_runtime::{declare_alkane, runtime::AlkaneResponder, message::MessageDispatch};
use metashrew_support::compat::to_arraybuffer_layout;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use alkanes_support::response::CallResponse;


#[derive(Default)]
pub struct OylZap(());

impl AlkaneResponder for OylZap {}

#[derive(MessageDispatch)]
enum OylZapMessage {
    #[opcode(0)]
    Initialize {
        dummy_id: AlkaneId,
    },
}

impl OylZap {
    fn initialize(&self, _dummy_id: AlkaneId) -> Result<CallResponse> {
        Ok(CallResponse::default())
    }
}

declare_alkane! {
    impl AlkaneResponder for OylZap {
        type Message = OylZapMessage;
    }
}
