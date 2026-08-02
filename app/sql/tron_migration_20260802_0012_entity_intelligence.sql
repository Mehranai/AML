CREATE TABLE IF NOT EXISTS tron_db.intelligence_sources
(
    chain LowCardinality(String),
    source_id String,
    source_name String,
    source_type LowCardinality(String),
    trust_tier LowCardinality(String),
    reference_url String DEFAULT '',
    license String DEFAULT '',
    is_active UInt8 DEFAULT 1,
    created_by String,
    created_at_unix_ms UInt64,
    inserted_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (chain, source_id);

CREATE TABLE IF NOT EXISTS tron_db.intelligence_reviews
(
    review_id String,
    chain LowCardinality(String),
    subject_type LowCardinality(String),
    subject_id String,
    decision LowCardinality(String),
    reviewer String,
    reason String DEFAULT '',
    evidence_refs Array(String),
    created_at_unix_ms UInt64,
    inserted_at DateTime64(3) DEFAULT now64(3),
    INDEX idx_review_subject (subject_type, subject_id) TYPE bloom_filter(0.01) GRANULARITY 4
)
ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (chain, subject_type, subject_id, review_id);

ALTER TABLE tron_db.entity_labels
    ADD COLUMN IF NOT EXISTS address_role LowCardinality(String) DEFAULT 'UNKNOWN' AFTER entity_type;

ALTER TABLE tron_db.entity_labels
    ADD COLUMN IF NOT EXISTS risk_percent UInt8 DEFAULT 0 AFTER confidence;

ALTER TABLE tron_db.entity_labels
    ADD COLUMN IF NOT EXISTS source_record_id String DEFAULT '' AFTER source;

ALTER TABLE tron_db.entity_labels
    ADD COLUMN IF NOT EXISTS supersedes_label_id String DEFAULT '' AFTER source_record_id;

ALTER TABLE tron_db.entity_labels
    ADD COLUMN IF NOT EXISTS submitted_by String DEFAULT '' AFTER supersedes_label_id;

CREATE TABLE IF NOT EXISTS tron_db.address_cluster_claims
(
    claim_id String,
    chain LowCardinality(String),
    address String,
    cluster_id String,
    cluster_type LowCardinality(String),
    address_role LowCardinality(String),
    claim_method LowCardinality(String),
    confidence Float32,
    source String,
    source_record_id String DEFAULT '',
    evidence_tx_hashes Array(String),
    evidence_addresses Array(String),
    evidence_json String,
    supersedes_claim_id String DEFAULT '',
    review_status LowCardinality(String) DEFAULT 'PENDING',
    created_by String,
    created_at_unix_ms UInt64,
    inserted_at DateTime64(3) DEFAULT now64(3),
    INDEX idx_cluster_claim_id claim_id TYPE bloom_filter(0.01) GRANULARITY 4,
    INDEX idx_cluster_claim_cluster cluster_id TYPE bloom_filter(0.01) GRANULARITY 4
)
ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (chain, address, cluster_id, claim_id);

CREATE TABLE IF NOT EXISTS tron_db.address_cluster_memberships
(
    chain LowCardinality(String),
    address String,
    cluster_id String,
    cluster_type LowCardinality(String),
    address_role LowCardinality(String),
    confidence Float32,
    source_claim_id String,
    review_id String,
    cluster_version UInt32,
    is_active UInt8,
    created_at_unix_ms UInt64,
    inserted_at DateTime64(3) DEFAULT now64(3),
    INDEX idx_membership_cluster cluster_id TYPE bloom_filter(0.01) GRANULARITY 4
)
ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (chain, address, cluster_id);

CREATE TABLE IF NOT EXISTS tron_db.cluster_versions
(
    chain LowCardinality(String),
    cluster_id String,
    version UInt32,
    cluster_type LowCardinality(String),
    display_name String,
    change_type LowCardinality(String),
    change_reason String DEFAULT '',
    source_claim_ids Array(String),
    active_member_count UInt64,
    created_by String,
    created_at_unix_ms UInt64,
    inserted_at DateTime64(3) DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (chain, cluster_id, version);

ALTER TABLE tron_db.exchange_addresses
    ADD COLUMN IF NOT EXISTS is_active UInt8 DEFAULT 1 AFTER last_seen_block;

ALTER TABLE tron_db.exposure_seeds
    ADD COLUMN IF NOT EXISTS source_label_id String DEFAULT '' AFTER source;

ALTER TABLE tron_db.exposure_seeds
    ADD COLUMN IF NOT EXISTS is_active UInt8 DEFAULT 1 AFTER source_label_id;

INSERT INTO tron_db.intelligence_sources
(
    chain,
    source_id,
    source_name,
    source_type,
    trust_tier,
    reference_url,
    license,
    is_active,
    created_by,
    created_at_unix_ms
)
VALUES
(
    'tron',
    'tron_builtin_verified_seeds',
    'TRON verified bootstrap seeds',
    'INTERNAL_BOOTSTRAP',
    'VERIFIED',
    '',
    'internal',
    1,
    'schema_migration',
    toUInt64(toUnixTimestamp64Milli(now64(3)))
),
(
    'tron',
    'tron_structural_heuristics_v1',
    'TRON structural clustering v1',
    'HEURISTIC',
    'UNVERIFIED',
    '',
    'internal',
    1,
    'schema_migration',
    toUInt64(toUnixTimestamp64Milli(now64(3)))
);

INSERT INTO tron_db.entity_labels
(
    label_id,
    chain,
    address,
    entity_id,
    entity_name,
    entity_type,
    address_role,
    confidence,
    risk_percent,
    source,
    source_record_id,
    supersedes_label_id,
    submitted_by,
    case_id,
    evidence_refs,
    review_status,
    created_at_unix_ms
)
VALUES
(
    'tron_bootstrap_binance_hot_wallet',
    'tron',
    'TAUN6FwrnwwmaEqYcckffC7wYmbaS6cBiX',
    'exchange:binance',
    'Binance',
    'centralized_exchange',
    'HOT',
    1.0,
    0,
    'tron_builtin_verified_seeds',
    'bootstrap:binance:hot:1',
    '',
    'schema_migration',
    '',
    ['internal:verified-bootstrap-seed'],
    'APPROVED',
    toUInt64(toUnixTimestamp64Milli(now64(3)))
),
(
    'tron_bootstrap_okx_hot_wallet',
    'tron',
    'TU2TmqauSEiRf16CyFgzHV2BVxBejY9iyR',
    'exchange:okx',
    'OKX',
    'centralized_exchange',
    'HOT',
    1.0,
    0,
    'tron_builtin_verified_seeds',
    'bootstrap:okx:hot:1',
    '',
    'schema_migration',
    '',
    ['internal:verified-bootstrap-seed'],
    'APPROVED',
    toUInt64(toUnixTimestamp64Milli(now64(3)))
);

INSERT INTO tron_db.intelligence_reviews
(
    review_id,
    chain,
    subject_type,
    subject_id,
    decision,
    reviewer,
    reason,
    evidence_refs,
    created_at_unix_ms
)
VALUES
(
    'tron_bootstrap_review_binance',
    'tron',
    'ENTITY_LABEL',
    'tron_bootstrap_binance_hot_wallet',
    'APPROVED',
    'schema_migration',
    'Verified bootstrap service address',
    ['internal:verified-bootstrap-seed'],
    toUInt64(toUnixTimestamp64Milli(now64(3)))
),
(
    'tron_bootstrap_review_okx',
    'tron',
    'ENTITY_LABEL',
    'tron_bootstrap_okx_hot_wallet',
    'APPROVED',
    'schema_migration',
    'Verified bootstrap service address',
    ['internal:verified-bootstrap-seed'],
    toUInt64(toUnixTimestamp64Milli(now64(3)))
);

INSERT INTO tron_db.address_entity
(
    address,
    entity_id,
    entity_name,
    entity_type,
    confidence,
    source,
    is_active
)
VALUES
(
    'TAUN6FwrnwwmaEqYcckffC7wYmbaS6cBiX',
    'exchange:binance',
    'Binance',
    'centralized_exchange',
    1.0,
    'tron_bootstrap_binance_hot_wallet',
    1
),
(
    'TU2TmqauSEiRf16CyFgzHV2BVxBejY9iyR',
    'exchange:okx',
    'OKX',
    'centralized_exchange',
    1.0,
    'tron_bootstrap_okx_hot_wallet',
    1
);

INSERT INTO tron_db.exchange_addresses
(
    address,
    entity_id,
    exchange_name,
    address_role,
    confidence,
    detection_source,
    first_seen_block,
    last_seen_block,
    is_active
)
VALUES
(
    'TAUN6FwrnwwmaEqYcckffC7wYmbaS6cBiX',
    'exchange:binance',
    'Binance',
    'HOT',
    1.0,
    'entity_label:tron_bootstrap_binance_hot_wallet',
    0,
    0,
    1
),
(
    'TU2TmqauSEiRf16CyFgzHV2BVxBejY9iyR',
    'exchange:okx',
    'OKX',
    'HOT',
    1.0,
    'entity_label:tron_bootstrap_okx_hot_wallet',
    0,
    0,
    1
);
