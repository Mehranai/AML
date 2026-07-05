use crate::services::tron::aml::types::{FlowMap, NetFlow, SimpleTransfer};

pub fn compute_net_flows(transfers: &[SimpleTransfer]) -> FlowMap {
    let mut flows = FlowMap::new();

    for t in transfers {
        let amount = i128::try_from(t.amount).unwrap_or(i128::MAX);

        //
        // sender loses
        //
        flows
            .entry(t.from.clone())
            .or_default()
            .entry(t.token.clone())
            .and_modify(|v| *v -= amount)
            .or_insert(-amount);

        //
        // receiver gains
        //
        flows
            .entry(t.to.clone())
            .or_default()
            .entry(t.token.clone())
            .and_modify(|v| *v += amount)
            .or_insert(amount);
    }

    flows
}

pub fn flatten_flows(flow_map: &FlowMap) -> Vec<NetFlow> {
    let mut out = Vec::new();

    for (address, token_map) in flow_map {
        for (token, delta) in token_map {
            out.push(NetFlow {
                address: address.clone(),
                token: token.clone(),
                delta: *delta,
            });
        }
    }

    out
}
