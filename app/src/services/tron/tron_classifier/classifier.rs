use crate::services::tron::aml::types::SimpleTransfer;

use super::confidence::boost_confidence;

use super::flow_analyzer::analyze_flows;

use super::method_decoder::detect_method;

use super::protocol_detector::detect_protocol;

use super::types::{ClassificationInput, ClassificationResult, ContractCategory};

pub fn classify(input: &ClassificationInput, transfers: &[SimpleTransfer]) -> ClassificationResult {
    //
    // protocol detection
    //
    if let Some(protocol_info) = detect_protocol(&input.contract_address) {
        return ClassificationResult {
            protocol: protocol_info.protocol.to_string(),

            category: protocol_info.category,

            confidence: boost_confidence(protocol_info.confidence, true, false, false),

            detection_source: "known_protocol".to_string(),

            method_id: None,
        };
    }

    //
    // method decoding
    //
    if let Some(ref data) = input.method_data
        && let Some((method_id, protocol_info)) = detect_method(data)
    {
        return ClassificationResult {
            protocol: protocol_info.protocol.to_string(),

            category: protocol_info.category,

            confidence: boost_confidence(protocol_info.confidence, false, true, false),

            detection_source: "method_signature".to_string(),

            method_id: Some(method_id),
        };
    }

    //
    // flow analysis
    //
    if let Some(protocol_info) = analyze_flows(transfers) {
        return ClassificationResult {
            protocol: protocol_info.protocol.to_string(),

            category: protocol_info.category,

            confidence: boost_confidence(protocol_info.confidence, false, false, true),

            detection_source: "flow_analysis".to_string(),

            method_id: None,
        };
    }

    ClassificationResult {
        protocol: "Unknown".to_string(),

        category: ContractCategory::Unknown,

        confidence: 0.0,

        detection_source: "none".to_string(),

        method_id: None,
    }
}
