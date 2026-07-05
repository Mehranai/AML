DROP TABLE IF EXISTS tron_db.wallet_ai_risk_assessments;
DROP TABLE IF EXISTS tron_db.wallet_feature_snapshots;
DROP TABLE IF EXISTS tron_db.wallet_risk_assessments;

/*
label: 1 = Money Laundering, 0 = Clear
label_name: laundering, scam_cashout, exchange_hot_wallet, normal_user,...
typologies: layering, bridge_hopping, swap_obfuscation, fan_in_cashout, mule_wallet (ولت واسط), ...
source: منبع واقعی یا هوش مصنوعی تولید کرده؟
case_id: همه کیف پول های یه مورد پول شویی یه آیدی مشخص میگیرند
evidence_refs: یه لیستی از آیدی تراکنش هایی که شواهدی هستند برای مشکوک بودن بهش
*/
CREATE TABLE IF NOT EXISTS tron_db.wallet_ml_labels
(
    address String,
    label UInt8,
    label_name String,
    typologies Array(String),
    confidence Float32,
    source String,
    case_id String,
    evidence_refs Array(String),
    created_at_unix_ms UInt64,

    inserted_at DateTime DEFAULT now()
)
    ENGINE = ReplacingMergeTree(inserted_at)
    ORDER BY (
        address,
        label_name,
        case_id,
        created_at_unix_ms
    );

CREATE TABLE IF NOT EXISTS tron_db.wallet_ml_feature_snapshots
(
    snapshot_id String,

    address String,
    window_days UInt16,
    feature_schema_version String,
    feature_names Array(String),

    features_json String CODEC(ZSTD),
    evidence_refs Array(String),

    generated_at_unix_ms UInt64,

    inserted_at DateTime DEFAULT now()
)
    ENGINE = ReplacingMergeTree(inserted_at)
    PARTITION BY toYYYYMM(toDateTime(intDiv(generated_at_unix_ms, 1000)))
    ORDER BY (
        address,
        feature_schema_version,
        window_days,
        generated_at_unix_ms,
        snapshot_id
    );

CREATE TABLE IF NOT EXISTS tron_db.wallet_ml_training_runs
(
    training_run_id String,
    model_id String,
    model_version String,
    feature_schema_version String,
    training_dataset_id String,
    label_policy String,

    train_sample_count UInt64,
    validation_sample_count UInt64,
    positive_label_count UInt64,
    negative_label_count UInt64,

    metrics_json String CODEC(ZSTD),
    parameters_json String CODEC(ZSTD),
    artifact_uri String,
    artifact_json String CODEC(ZSTD),

    status String,
    started_at_unix_ms UInt64,
    completed_at_unix_ms UInt64,

    inserted_at DateTime DEFAULT now()
)
    ENGINE = ReplacingMergeTree(inserted_at)
    ORDER BY (
        model_id,
        model_version,
        training_run_id
    );

CREATE TABLE IF NOT EXISTS tron_db.wallet_ml_model_registry
(
    model_id String,
    model_version String,
    model_family String,
    feature_schema_version String,
    calibration_version String,

    artifact_json String CODEC(ZSTD),
    metrics_json String CODEC(ZSTD),
    training_run_id String,
    training_dataset_id String,
    label_policy String,
    model_quality_score Float32,

    status String,
    trained_at_unix_ms UInt64,
    activated_at_unix_ms UInt64,

    inserted_at DateTime DEFAULT now()
)
    ENGINE = ReplacingMergeTree(inserted_at)
    ORDER BY (
        status,
        feature_schema_version,
        model_version,
        model_id
    );

CREATE TABLE IF NOT EXISTS tron_db.wallet_ml_predictions
(
    prediction_id String,
    snapshot_id String,

    model_id String,
    model_version String,
    model_family String,
    calibration_version String,

    address String,
    window_days UInt16,

    risk_probability Float32,
    risk_percent UInt8,
    risk_level String,
    confidence Float32,

    feature_importance_json String CODEC(ZSTD),
    model_patterns_json String CODEC(ZSTD),
    evidence_refs Array(String),

    predicted_at_unix_ms UInt64,

    inserted_at DateTime DEFAULT now()
)
    ENGINE = ReplacingMergeTree(inserted_at)
    PARTITION BY toYYYYMM(toDateTime(intDiv(predicted_at_unix_ms, 1000)))
    ORDER BY (
        address,
        model_version,
        window_days,
        predicted_at_unix_ms,
        prediction_id
    );

ALTER TABLE tron_db.wallet_ml_labels
    ADD INDEX IF NOT EXISTS idx_wallet_ml_label_name (label_name)
    TYPE set(20)
    GRANULARITY 4;

ALTER TABLE tron_db.wallet_ml_feature_snapshots
    ADD INDEX IF NOT EXISTS idx_wallet_ml_feature_schema (feature_schema_version)
    TYPE set(20)
    GRANULARITY 4;

ALTER TABLE tron_db.wallet_ml_model_registry
    ADD INDEX IF NOT EXISTS idx_wallet_ml_model_status (status)
    TYPE set(10)
    GRANULARITY 4;

ALTER TABLE tron_db.wallet_ml_predictions
    ADD INDEX IF NOT EXISTS idx_wallet_ml_prediction_risk (risk_percent)
    TYPE minmax
    GRANULARITY 4;
