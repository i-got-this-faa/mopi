import logging
from pathlib import Path

import torch
from torch.utils.data import DataLoader
from transformers import AutoTokenizer
from datasets import load_dataset

from lss_model.config import ModelConfig
from lss_model.model import LSSEmbedModel

logger = logging.getLogger(__name__)


@torch.no_grad()
def evaluate(model: LSSEmbedModel, config: ModelConfig) -> dict[str, float]:
    device = next(model.parameters()).device
    model.eval()
    tokenizer = AutoTokenizer.from_pretrained("bert-base-uncased")

    if config.eval_file:
        dataset = load_dataset("json", data_files=config.eval_file, split="train")
    else:
        dataset = load_dataset(config.dataset_name, "pair", split="train").select(range(500))

    def tokenize(batch):
        return tokenizer(
        batch["anchor"],
        batch["positive"],
            padding="max_length",
            truncation=True,
            max_length=config.max_seq_length,
            return_tensors="pt",
        )

    dataset = dataset.map(tokenize, batched=True, remove_columns=dataset.column_names)
    dataset.set_format(type="torch", columns=["input_ids", "attention_mask", "token_type_ids"])

    dataloader = DataLoader(dataset, batch_size=config.batch_size)

    total = 0
    correct = 0
    for batch in dataloader:
        batch = {k: v.to(device) for k, v in batch.items()}
        embeddings = model(batch["input_ids"], batch["attention_mask"], batch.get("token_type_ids"))
        sim = embeddings @ embeddings.T
        preds = sim.argmax(dim=-1)
        labels = torch.arange(sim.size(0), device=sim.device)
        correct += (preds == labels).sum().item()
        total += labels.size(0)

    accuracy = correct / total if total > 0 else 0.0
    logger.info("accuracy: %.4f (%d/%d)", accuracy, correct, total)
    return {"accuracy": accuracy}


def load_model(config: ModelConfig, checkpoint_path: str) -> LSSEmbedModel:
    model = LSSEmbedModel(config)
    state = torch.load(checkpoint_path, map_location="cpu", weights_only=True)
    model.load_state_dict(state)
    return model
