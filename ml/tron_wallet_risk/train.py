
from __future__ import annotations

import argparse
import csv
import json
import math
import random
import time
import uuid
from dataclasses import dataclass
from pathlib import Path

import torch
from torch import nn
from torch.utils.data import DataLoader, TensorDataset


FEATURE_SCHEMA_VERSION = "tron_wallet_behavior_features_v2"

FEATURE_NAMES = [
    "total_transfers_log",
    "unique_transactions_log",
    "incoming_transfers_log",
    "outgoing_transfers_log",
    "unique_senders_log",
    "unique_receivers_log",
    "fan_in_score",
    "fan_out_score",
    "flow_imbalance_score",
    "burst_score",
    "swap_ratio",
    "bridge_ratio",
    "exchange_interaction_ratio",
    "contract_call_ratio",
    "counterparty_concentration",
    "token_diversity_score",
    "exposure_score",
    "exposure_source_count_score",
    "exposure_path_count_score",
    "exposure_min_hop_score",
    "identity_confidence",
    "exchange_service_wallet_score",
    "truncated_sample_score",
    "data_volume_score",
]


@dataclass
class Dataset:
    addresses: list[str]
    features: torch.Tensor
    labels: torch.Tensor


class WalletRiskMlp(nn.Module):
    def __init__(self, input_width: int, hidden_widths: list[int]) -> None:
        super().__init__()
        self.hidden_layers = nn.ModuleList()
        previous_width = input_width
        for width in hidden_widths:
            self.hidden_layers.append(nn.Linear(previous_width, width))
            previous_width = width
        self.output_layer = nn.Linear(previous_width, 1)

    def forward(self, inputs: torch.Tensor) -> torch.Tensor:
        values = inputs
        for layer in self.hidden_layers:
            values = torch.relu(layer(values))
        return self.output_layer(values).squeeze(-1)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Train a PyTorch MLP for TRON wallet AML risk scoring."
    )
    parser.add_argument("--input", required=True, help="Training CSV path.")
    parser.add_argument(
        "--output-dir",
        default="ml/tron_wallet_risk/artifacts/latest",
        help="Directory for exported artifact files.",
    )
    parser.add_argument("--model-id", default="tron_wallet_pytorch_mlp_v1")
    parser.add_argument("--model-version", default="v1")
    parser.add_argument("--dataset-id", default="manual_csv_v1")
    parser.add_argument(
        "--label-policy",
        default="label_1_laundering_label_0_benign",
        help="Human readable label policy saved with the model.",
    )
    parser.add_argument(
        "--hidden-widths",
        default="32,16",
        help="Comma-separated hidden layer widths.",
    )
    parser.add_argument("--epochs", type=int, default=200)
    parser.add_argument("--batch-size", type=int, default=64)
    parser.add_argument("--learning-rate", type=float, default=1e-3)
    parser.add_argument("--weight-decay", type=float, default=1e-4)
    parser.add_argument("--validation-ratio", type=float, default=0.2)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument(
        "--activate",
        action="store_true",
        help="Write model registry SQL with status ACTIVE instead of CANDIDATE.",
    )
    return parser.parse_args()


def parse_hidden_widths(value: str) -> list[int]:
    widths = [int(item.strip()) for item in value.split(",") if item.strip()]
    if not widths:
        raise ValueError("at least one hidden width is required")
    if any(width <= 0 for width in widths):
        raise ValueError("hidden widths must be positive")
    return widths


def parse_label(value: str) -> float:
    normalized = value.strip().lower()
    if normalized in {"1", "true", "illicit", "laundering", "ml", "suspicious"}:
        return 1.0
    if normalized in {"0", "-1", "false", "benign", "clean", "normal"}:
        return 0.0
    raise ValueError(f"unsupported label value: {value!r}")


def read_training_csv(path: Path) -> Dataset:
    addresses: list[str] = []
    feature_rows: list[list[float]] = []
    labels: list[float] = []

    with path.open("r", newline="", encoding="utf-8") as handle:
        reader = csv.DictReader(handle)
        if reader.fieldnames is None:
            raise ValueError("training CSV has no header row")

        missing = [feature for feature in FEATURE_NAMES if feature not in reader.fieldnames]
        if missing:
            raise ValueError(f"training CSV is missing feature columns: {missing}")
        if "label" not in reader.fieldnames:
            raise ValueError("training CSV is missing required label column")

        for row_number, row in enumerate(reader, start=2):
            try:
                feature_rows.append(
                    [
                        float(row.get(feature, "").strip() or "0")
                        for feature in FEATURE_NAMES
                    ]
                )
                labels.append(parse_label(row["label"]))
                addresses.append(row.get("address", f"row_{row_number}"))
            except Exception as exc:
                raise ValueError(f"invalid training row {row_number}: {exc}") from exc

    if len(labels) < 4:
        raise ValueError("at least four labeled rows are required")
    if len(set(labels)) != 2:
        raise ValueError("training data must include both label 1 and label 0")

    return Dataset(
        addresses=addresses,
        features=torch.tensor(feature_rows, dtype=torch.float32),
        labels=torch.tensor(labels, dtype=torch.float32),
    )


def split_dataset(dataset: Dataset, validation_ratio: float, seed: int) -> tuple[Dataset, Dataset]:
    validation_ratio = min(max(validation_ratio, 0.05), 0.5)
    indices = list(range(len(dataset.labels)))
    random.Random(seed).shuffle(indices)

    validation_count = max(1, int(round(len(indices) * validation_ratio)))
    validation_indices = indices[:validation_count]
    train_indices = indices[validation_count:]

    if not train_indices:
        raise ValueError("not enough rows after validation split")

    return take_rows(dataset, train_indices), take_rows(dataset, validation_indices)


def take_rows(dataset: Dataset, indices: list[int]) -> Dataset:
    tensor_indices = torch.tensor(indices, dtype=torch.long)
    return Dataset(
        addresses=[dataset.addresses[index] for index in indices],
        features=dataset.features.index_select(0, tensor_indices),
        labels=dataset.labels.index_select(0, tensor_indices),
    )


def fit_standardizer(features: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
    means = features.mean(dim=0)
    stds = features.std(dim=0, unbiased=False)
    stds = torch.where(stds < 1e-6, torch.ones_like(stds), stds)
    return means, stds


def standardize(features: torch.Tensor, means: torch.Tensor, stds: torch.Tensor) -> torch.Tensor:
    return (features - means) / stds


def train_model(
    train: Dataset,
    validation: Dataset,
    hidden_widths: list[int],
    args: argparse.Namespace,
) -> tuple[WalletRiskMlp, dict[str, float], torch.Tensor, torch.Tensor]:
    torch.manual_seed(args.seed)
    means, stds = fit_standardizer(train.features)
    train_x = standardize(train.features, means, stds)
    validation_x = standardize(validation.features, means, stds)

    model = WalletRiskMlp(train_x.shape[1], hidden_widths)
    positives = train.labels.sum().item()
    negatives = float(len(train.labels)) - positives
    pos_weight = torch.tensor([max(negatives / max(positives, 1.0), 1.0)])
    loss_fn = nn.BCEWithLogitsLoss(pos_weight=pos_weight)
    optimizer = torch.optim.AdamW(
        model.parameters(),
        lr=args.learning_rate,
        weight_decay=args.weight_decay,
    )

    loader = DataLoader(
        TensorDataset(train_x, train.labels),
        batch_size=args.batch_size,
        shuffle=True,
    )

    for _ in range(args.epochs):
        model.train()
        for batch_x, batch_y in loader:
            optimizer.zero_grad(set_to_none=True)
            logits = model(batch_x)
            loss = loss_fn(logits, batch_y)
            loss.backward()
            optimizer.step()

    model.eval()
    with torch.no_grad():
        train_prob = torch.sigmoid(model(train_x))
        validation_prob = torch.sigmoid(model(validation_x))

    metrics = {
        **prefix_metrics("train", classification_metrics(train.labels, train_prob)),
        **prefix_metrics("validation", classification_metrics(validation.labels, validation_prob)),
    }
    return model, metrics, means, stds


def classification_metrics(labels: torch.Tensor, probabilities: torch.Tensor) -> dict[str, float]:
    y_true = [float(item) for item in labels.tolist()]
    y_prob = [float(item) for item in probabilities.tolist()]
    y_pred = [1.0 if item >= 0.5 else 0.0 for item in y_prob]

    tp = sum(1 for truth, pred in zip(y_true, y_pred) if truth == 1.0 and pred == 1.0)
    tn = sum(1 for truth, pred in zip(y_true, y_pred) if truth == 0.0 and pred == 0.0)
    fp = sum(1 for truth, pred in zip(y_true, y_pred) if truth == 0.0 and pred == 1.0)
    fn = sum(1 for truth, pred in zip(y_true, y_pred) if truth == 1.0 and pred == 0.0)

    precision = safe_div(tp, tp + fp)
    recall = safe_div(tp, tp + fn)
    f1 = safe_div(2.0 * precision * recall, precision + recall)
    accuracy = safe_div(tp + tn, len(y_true))
    brier = sum((prob - truth) ** 2 for truth, prob in zip(y_true, y_prob)) / len(y_true)

    return {
        "accuracy": accuracy,
        "precision": precision,
        "recall": recall,
        "f1": f1,
        "auc": roc_auc(y_true, y_prob),
        "brier": brier,
    }


def safe_div(numerator: float, denominator: float) -> float:
    return 0.0 if denominator == 0 else numerator / denominator


def roc_auc(labels: list[float], probabilities: list[float]) -> float:
    positives = sum(1 for label in labels if label == 1.0)
    negatives = len(labels) - positives
    if positives == 0 or negatives == 0:
        return 0.5

    pairs = sorted(zip(probabilities, labels), key=lambda item: item[0])
    rank_sum = 0.0
    for rank, (_, label) in enumerate(pairs, start=1):
        if label == 1.0:
            rank_sum += rank

    return (rank_sum - positives * (positives + 1) / 2.0) / (positives * negatives)


def prefix_metrics(prefix: str, metrics: dict[str, float]) -> dict[str, float]:
    return {f"{prefix}_{key}": value for key, value in metrics.items()}


def export_artifact(
    model: WalletRiskMlp,
    means: torch.Tensor,
    stds: torch.Tensor,
    args: argparse.Namespace,
) -> dict:
    hidden_layers = []
    for layer in model.hidden_layers:
        hidden_layers.append(
            {
                "activation": "relu",
                "weights": tensor_to_list(layer.weight.detach()),
                "bias": tensor_to_list(layer.bias.detach()),
            }
        )

    return {
        "model_type": "pytorch_mlp",
        "feature_schema_version": FEATURE_SCHEMA_VERSION,
        "feature_names": FEATURE_NAMES,
        "feature_means": tensor_to_list(means),
        "feature_stds": tensor_to_list(stds),
        "hidden_layers": hidden_layers,
        "output_weights": tensor_to_list(model.output_layer.weight.detach().squeeze(0)),
        "output_bias": float(model.output_layer.bias.detach().squeeze(0).item()),
        "calibration": {"method": "identity"},
        "explanation_top_k": 12,
        "training": {
            "framework": "pytorch",
            "model_id": args.model_id,
            "model_version": args.model_version,
            "feature_schema_version": FEATURE_SCHEMA_VERSION,
        },
    }


def tensor_to_list(tensor: torch.Tensor) -> list:
    return json.loads(json.dumps(tensor.cpu().tolist()))


def write_outputs(
    output_dir: Path,
    artifact: dict,
    metrics: dict[str, float],
    train: Dataset,
    validation: Dataset,
    args: argparse.Namespace,
) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    now_ms = int(time.time() * 1000)
    training_run_id = f"tron_wallet_train_{uuid.uuid4().hex}"
    status = "ACTIVE" if args.activate else "CANDIDATE"
    model_quality_score = float(metrics.get("validation_auc", 0.5))

    parameters = {
        "epochs": args.epochs,
        "batch_size": args.batch_size,
        "learning_rate": args.learning_rate,
        "weight_decay": args.weight_decay,
        "hidden_widths": parse_hidden_widths(args.hidden_widths),
        "validation_ratio": args.validation_ratio,
        "seed": args.seed,
    }

    artifact_path = output_dir / "model_artifact.json"
    metrics_path = output_dir / "metrics.json"
    feature_schema_path = output_dir / "feature_schema.json"
    register_sql_path = output_dir / "register_model.sql"

    artifact_json = json.dumps(artifact, indent=2, sort_keys=True)
    metrics_json = json.dumps(metrics, indent=2, sort_keys=True)
    parameters_json = json.dumps(parameters, indent=2, sort_keys=True)

    artifact_path.write_text(artifact_json + "\n", encoding="utf-8")
    metrics_path.write_text(metrics_json + "\n", encoding="utf-8")
    feature_schema_path.write_text(
        json.dumps(
            {
                "feature_schema_version": FEATURE_SCHEMA_VERSION,
                "feature_names": FEATURE_NAMES,
                "label_column": "label",
                "label_policy": args.label_policy,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )

    register_sql_path.write_text(
        build_register_sql(
            artifact_json=artifact_json,
            metrics_json=metrics_json,
            parameters_json=parameters_json,
            artifact_uri=str(artifact_path),
            training_run_id=training_run_id,
            status=status,
            train=train,
            validation=validation,
            model_quality_score=model_quality_score,
            now_ms=now_ms,
            args=args,
        )
        + "\n",
        encoding="utf-8",
    )

    print(f"wrote {artifact_path}")
    print(f"wrote {metrics_path}")
    print(f"wrote {feature_schema_path}")
    print(f"wrote {register_sql_path}")
    print(json.dumps(metrics, indent=2, sort_keys=True))


def build_register_sql(
    artifact_json: str,
    metrics_json: str,
    parameters_json: str,
    artifact_uri: str,
    training_run_id: str,
    status: str,
    train: Dataset,
    validation: Dataset,
    model_quality_score: float,
    now_ms: int,
    args: argparse.Namespace,
) -> str:
    positive_count = int(train.labels.sum().item() + validation.labels.sum().item())
    total_count = len(train.labels) + len(validation.labels)
    negative_count = total_count - positive_count

    return f"""
INSERT INTO tron_db.wallet_ml_training_runs
(
    training_run_id,
    model_id,
    model_version,
    feature_schema_version,
    training_dataset_id,
    label_policy,
    train_sample_count,
    validation_sample_count,
    positive_label_count,
    negative_label_count,
    metrics_json,
    parameters_json,
    artifact_uri,
    artifact_json,
    status,
    started_at_unix_ms,
    completed_at_unix_ms
)
VALUES
(
    {sql_string(training_run_id)},
    {sql_string(args.model_id)},
    {sql_string(args.model_version)},
    {sql_string(FEATURE_SCHEMA_VERSION)},
    {sql_string(args.dataset_id)},
    {sql_string(args.label_policy)},
    {len(train.labels)},
    {len(validation.labels)},
    {positive_count},
    {negative_count},
    {sql_string(metrics_json)},
    {sql_string(parameters_json)},
    {sql_string(artifact_uri)},
    {sql_string(artifact_json)},
    {sql_string(status)},
    {now_ms},
    {now_ms}
);

INSERT INTO tron_db.wallet_ml_model_registry
(
    model_id,
    model_version,
    model_family,
    feature_schema_version,
    calibration_version,
    artifact_json,
    metrics_json,
    training_run_id,
    training_dataset_id,
    label_policy,
    model_quality_score,
    status,
    trained_at_unix_ms,
    activated_at_unix_ms
)
VALUES
(
    {sql_string(args.model_id)},
    {sql_string(args.model_version)},
    'pytorch_mlp',
    {sql_string(FEATURE_SCHEMA_VERSION)},
    'identity',
    {sql_string(artifact_json)},
    {sql_string(metrics_json)},
    {sql_string(training_run_id)},
    {sql_string(args.dataset_id)},
    {sql_string(args.label_policy)},
    {model_quality_score},
    {sql_string(status)},
    {now_ms},
    {now_ms if status == "ACTIVE" else 0}
);
""".strip()


def sql_string(value: str) -> str:
    return "'" + value.replace("\\", "\\\\").replace("'", "''") + "'"


def main() -> None:
    args = parse_args()
    random.seed(args.seed)
    torch.manual_seed(args.seed)

    dataset = read_training_csv(Path(args.input))
    train, validation = split_dataset(dataset, args.validation_ratio, args.seed)
    model, metrics, means, stds = train_model(
        train,
        validation,
        parse_hidden_widths(args.hidden_widths),
        args,
    )
    artifact = export_artifact(model, means, stds, args)
    write_outputs(Path(args.output_dir), artifact, metrics, train, validation, args)


if __name__ == "__main__":
    main()
