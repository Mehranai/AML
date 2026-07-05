use super::registry::KNOWN_PROTOCOLS;

use super::types::ProtocolInfo;

pub fn detect_protocol(address: &str) -> Option<ProtocolInfo> {
    KNOWN_PROTOCOLS.get(address).cloned()
}
