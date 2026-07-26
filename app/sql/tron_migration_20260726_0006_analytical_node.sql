CREATE TABLE IF NOT EXISTS tron_db.analysis_subjects
(
    chain String,
    subject_type String,
    subject_id String,
    address String,
    entity_id String,
    latest_snapshot_id String,
    latest_status String,
    latest_risk_level String,
    latest_risk_probability Float32,
    latest_confidence Float32,
    latest_data_cutoff_block UInt64,
    latest_data_cutoff_unix_ms UInt64,
    created_at_unix_ms UInt64,
    updated_at_unix_ms UInt64
)
ENGINE = ReplacingMergeTree(updated_at_unix_ms)
ORDER BY (chain, subject_type, subject_id);

CREATE TABLE IF NOT EXISTS tron_db.wallet_analysis_snapshots
(
    snapshot_id String,
    chain String,
    address String,
    entity_id String,
    analysis_version String,
    analysis_status String,
    risk_level String,
    risk_probability Float32,
    risk_percent UInt8,
    confidence Float32,
    wallet_type String,
    fingerprint_label String,
    graph_depth UInt8,
    graph_node_count UInt32,
    graph_edge_count UInt32,
    exchange_interaction_count UInt32,
    holdings_asset_count UInt64,
    holdings_metadata_gap_count UInt32,
    observed_transfers UInt64,
    incoming_transfers UInt64,
    outgoing_transfers UInt64,
    exposure_score Float32,
    exposure_source_count UInt32,
    exposure_path_count UInt64,
    exposure_min_hop_distance UInt8,
    data_cutoff_block UInt64,
    data_cutoff_unix_ms UInt64,
    source_tables Array(String),
    model_id String,
    model_version String,
    feature_schema_version String,
    snapshot_json String,
    warnings Array(String),
    evidence_refs Array(String),
    created_at_unix_ms UInt64
)
ENGINE = MergeTree
ORDER BY (chain, address, created_at_unix_ms, snapshot_id);

CREATE TABLE IF NOT EXISTS tron_db.wallet_analysis_evidence
(
    evidence_id String,
    snapshot_id String,
    chain String,
    address String,
    evidence_type String,
    evidence_key String,
    evidence_value String,
    severity String,
    related_tx_hash String,
    related_address String,
    created_at_unix_ms UInt64
)
ENGINE = MergeTree
ORDER BY (chain, address, snapshot_id, evidence_type, evidence_id);

CREATE TABLE IF NOT EXISTS tron_db.analysis_jobs
(
    job_id String,
    chain String,
    subject_type String,
    subject_id String,
    requested_by String,
    status String,
    parameters_json String,
    snapshot_id String,
    error_message String,
    requested_at_unix_ms UInt64,
    started_at_unix_ms UInt64,
    completed_at_unix_ms UInt64,
    updated_at_unix_ms UInt64
)
ENGINE = ReplacingMergeTree(updated_at_unix_ms)
ORDER BY (chain, subject_type, subject_id, job_id);
