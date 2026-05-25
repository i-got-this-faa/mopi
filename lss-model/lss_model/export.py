import logging
from pathlib import Path

import torch
import onnx
import onnxruntime

from lss_model.config import ModelConfig
from lss_model.model import LSSEmbedModel

logger = logging.getLogger(__name__)


def export_to_onnx(model: LSSEmbedModel, config: ModelConfig) -> str:
    model.eval()
    output_dir = Path(config.output_dir) / config.model_name
    output_dir.mkdir(parents=True, exist_ok=True)
    onnx_path = str(output_dir / "model.onnx")

    dummy_input_ids = torch.randint(0, config.vocab_size - 1, (1, config.max_seq_length))
    dummy_attention_mask = torch.ones(1, config.max_seq_length, dtype=torch.long)
    dummy_token_type_ids = torch.zeros(1, config.max_seq_length, dtype=torch.long)

    torch.onnx.export(
        model,
        (dummy_input_ids, dummy_attention_mask, dummy_token_type_ids),
        onnx_path,
        input_names=["input_ids", "attention_mask", "token_type_ids"],
        output_names=["sentence_embedding"],
        dynamic_axes={
            "input_ids": {0: "batch_size", 1: "sequence_len"},
            "attention_mask": {0: "batch_size", 1: "sequence_len"},
            "token_type_ids": {0: "batch_size", 1: "sequence_len"},
            "sentence_embedding": {0: "batch_size"},
        },
        opset_version=config.onnx_opset,
    )

    onnx_model = onnx.load(onnx_path)
    onnx.checker.check_model(onnx_model)
    logger.info("ONNX model exported to %s", onnx_path)

    return onnx_path


def verify_onnx(onnx_path: str, config: ModelConfig) -> None:
    session = onnxruntime.InferenceSession(onnx_path)

    max_len = config.max_seq_length
    dummy_input_ids = torch.randint(0, 100, (1, max_len), dtype=torch.int64).numpy()
    dummy_attention_mask = torch.ones(1, max_len, dtype=torch.int64).numpy()
    dummy_token_type_ids = torch.zeros(1, max_len, dtype=torch.int64).numpy()

    outputs = session.run(
        ["sentence_embedding"],
        {
            "input_ids": dummy_input_ids,
            "attention_mask": dummy_attention_mask,
            "token_type_ids": dummy_token_type_ids,
        },
    )

    logger.info("ONNX inference OK, output shape: %s", outputs[0].shape)


def export(config: ModelConfig) -> str:
    model = LSSEmbedModel(config).eval()
    onnx_path = export_to_onnx(model, config)
    verify_onnx(onnx_path, config)
    return onnx_path
