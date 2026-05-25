import math
import logging
from pathlib import Path

import torch
import torch.nn as nn
from torch.utils.data import DataLoader
from torch.optim import AdamW
from transformers import AutoTokenizer, get_linear_schedule_with_warmup
from datasets import load_dataset

from lss_model.config import ModelConfig
from lss_model.model import LSSEmbedModel

logger = logging.getLogger(__name__)


def train(config: ModelConfig) -> LSSEmbedModel:
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    logger.info("device: %s", device)

    model = LSSEmbedModel(config).to(device)
    tokenizer = AutoTokenizer.from_pretrained("bert-base-uncased")

    if config.train_file:
        dataset = load_dataset("json", data_files=config.train_file, split="train")
    else:
        dataset = load_dataset(config.dataset_name, "pair", split="train")

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

    dataloader = DataLoader(dataset, batch_size=config.batch_size, shuffle=True)

    optimizer = AdamW(model.parameters(), lr=config.learning_rate)

    total_steps = math.ceil(len(dataloader) / config.gradient_accumulation_steps) * config.num_epochs
    warmup_steps = int(total_steps * config.warmup_ratio)
    scheduler = get_linear_schedule_with_warmup(optimizer, warmup_steps, total_steps)

    scaler = torch.amp.GradScaler("cuda") if config.fp16 and torch.cuda.is_available() else None

    model.train()
    global_step = 0
    for epoch in range(config.num_epochs):
        epoch_loss = 0.0
        for step, batch in enumerate(dataloader):
            batch = {k: v.to(device) for k, v in batch.items()}

            if scaler:
                with torch.amp.autocast("cuda"):
                    loss = _compute_loss(model, batch)
            else:
                loss = _compute_loss(model, batch)

            loss = loss / config.gradient_accumulation_steps
            if scaler:
                scaler.scale(loss).backward()
            else:
                loss.backward()

            epoch_loss += loss.item()

            if (step + 1) % config.gradient_accumulation_steps == 0:
                if scaler:
                    scaler.unscale_(optimizer)
                    torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
                    scaler.step(optimizer)
                    scaler.update()
                else:
                    torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
                    optimizer.step()
                scheduler.step()
                optimizer.zero_grad()
                global_step += 1

            if step % 100 == 0:
                logger.info(
                    "epoch %d step %d loss %.4f lr %.2e",
                    epoch, step, loss.item() * config.gradient_accumulation_steps,
                    scheduler.get_last_lr()[0],
                )

        avg_loss = epoch_loss / len(dataloader)
        logger.info("epoch %d average loss %.4f", epoch, avg_loss)

    return model


def _compute_loss(model: LSSEmbedModel, batch: dict[str, torch.Tensor]) -> torch.Tensor:
    embeddings = model(batch["input_ids"], batch["attention_mask"], batch.get("token_type_ids"))
    sim = embeddings @ embeddings.T
    labels = torch.arange(sim.size(0), device=sim.device)
    loss = nn.CrossEntropyLoss()(sim, labels)
    return loss


def save_model(model: LSSEmbedModel, config: ModelConfig) -> None:
    output_dir = Path(config.output_dir) / config.model_name
    output_dir.mkdir(parents=True, exist_ok=True)

    torch.save(model.state_dict(), output_dir / "pytorch_model.bin")
    logger.info("model saved to %s", output_dir)
