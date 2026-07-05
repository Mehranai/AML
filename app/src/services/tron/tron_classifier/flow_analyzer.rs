use crate::services::tron::aml::flow_engine::compute_net_flows;

use crate::services::tron::aml::types::SimpleTransfer;

use super::types::{ContractCategory, ProtocolInfo};

pub fn analyze_flows(transfers: &[SimpleTransfer]) -> Option<ProtocolInfo> {
    let flows = compute_net_flows(transfers);

    for (_address, token_map) in flows {
        let mut negative = 0;
        let mut positive = 0;

        for (_token, delta) in token_map {
            if delta < 0 {
                negative += 1;
            }

            if delta > 0 {
                positive += 1;
            }
        }

        //
        // probable swap
        //
        if negative >= 1 && positive >= 1 {
            return Some(ProtocolInfo {
                protocol: "FlowBasedDex",
                category: ContractCategory::Dex,
                confidence: 0.60,
            });
        }
    }

    None
}
