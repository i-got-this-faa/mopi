import time
import logging
from pathlib import Path

import numpy as np
import torch
from datasets import load_dataset
from scipy.stats import spearmanr
from transformers import AutoModel, AutoTokenizer

from lss_model.config import ModelConfig
from lss_model.model import LSSEmbedModel

logger = logging.getLogger(__name__)

device = torch.device("cuda" if torch.cuda.is_available() else "cpu")


def load_minilm_v6() -> tuple[AutoModel, AutoTokenizer]:
    model_name = "sentence-transformers/all-MiniLM-L6-v2"
    tokenizer = AutoTokenizer.from_pretrained(model_name)
    model = AutoModel.from_pretrained(model_name).to(device).eval()
    return model, tokenizer


def load_lss(config: ModelConfig | None = None, checkpoint: str | None = None) -> tuple[LSSEmbedModel, AutoTokenizer]:
    if config is None:
        config = ModelConfig()
    model = LSSEmbedModel(config).to(device).eval()
    if checkpoint:
        state = torch.load(checkpoint, map_location=device, weights_only=True)
        model.load_state_dict(state)
    tokenizer = AutoTokenizer.from_pretrained("bert-base-uncased")
    return model, tokenizer


@torch.no_grad()
def encode_minilm(model: AutoModel, tokenizer: AutoTokenizer, texts: list[str], max_len: int = 128) -> np.ndarray:
    inputs = tokenizer(texts, padding=True, truncation=True, max_length=max_len, return_tensors="pt")
    inputs = {k: v.to(device) for k, v in inputs.items()}
    outputs = model(**inputs)
    emb = outputs.last_hidden_state[:, 0]
    emb = emb / emb.norm(p=2, dim=-1, keepdim=True)
    return emb.cpu().numpy()


@torch.no_grad()
def encode_lss(model: LSSEmbedModel, tokenizer: AutoTokenizer, texts: list[str], max_len: int = 128) -> np.ndarray:
    inputs = tokenizer(texts, padding=True, truncation=True, max_length=max_len, return_tensors="pt")
    inputs = {k: v.to(device) for k, v in inputs.items()}
    emb = model(inputs["input_ids"], inputs["attention_mask"], inputs.get("token_type_ids"))
    return emb.cpu().numpy()


def sts_spearman(model_fn, texts1: list[str], texts2: list[str], labels: list[float], batch_size: int = 64) -> float:
    all_sims = []
    for i in range(0, len(texts1), batch_size):
        t1 = texts1[i : i + batch_size]
        t2 = texts2[i : i + batch_size]
        e1 = model_fn(t1)
        e2 = model_fn(t2)
        sim = (e1 * e2).sum(axis=-1)
        all_sims.extend(sim.tolist())
    all_sims = np.array(all_sims)
    labels = np.array(labels)
    if np.std(all_sims) < 1e-8 or np.std(labels) < 1e-8:
        return 0.0
    return float(spearmanr(all_sims, labels).statistic)


def representation_similarity(
    model_a_fn,
    model_b_fn,
    texts: list[str],
    sample: int = 500,
) -> dict:
    idx = np.random.choice(len(texts), min(sample, len(texts)), replace=False)
    ea = model_a_fn([texts[i] for i in idx])
    eb = model_b_fn([texts[i] for i in idx])
    sims = (ea * eb).sum(axis=-1)
    return {
        "mean_cosine": float(np.mean(sims)),
        "std_cosine": float(np.std(sims)),
        "alignment": float(np.mean(np.abs(sims))),
    }


def benchmark_latency(model_fn, texts: list[str], n_warmup: int = 10, n_bench: int = 200) -> dict:
    for _ in range(n_warmup):
        model_fn(texts[:1])

    times = []
    for _ in range(n_bench):
        start = time.perf_counter()
        model_fn(texts[:1])
        times.append(time.perf_counter() - start)

    return {
        "mean_ms": float(np.mean(times) * 1000),
        "median_ms": float(np.median(times) * 1000),
        "p95_ms": float(np.percentile(times, 95) * 1000),
        "std_ms": float(np.std(times) * 1000),
    }


def model_size(model) -> int:
    return sum(p.numel() for p in model.parameters())


def compare(config: ModelConfig | None = None) -> dict:
    if config is None:
        config = ModelConfig()

    logger.info("loading MiniLM-L6-v2...")
    minilm, minilm_tok = load_minilm_v6()

    logger.info("loading LSS model...")
    lss_model, lss_tok = load_lss(config)

    logger.info("loading STS-B validation set...")
    sts = load_dataset("SetFit/stsb", split="validation")
    texts1 = list(sts["text1"])
    texts2 = list(sts["text2"])
    labels = list(sts["label"])

    minilm_encode = lambda t: encode_minilm(minilm, minilm_tok, t, config.max_seq_length)
    lss_encode = lambda t: encode_lss(lss_model, lss_tok, t, config.max_seq_length)

    logger.info("benchmarking MiniLM-L6-v2 STS...")
    minilm_sts = sts_spearman(minilm_encode, texts1, texts2, labels)

    logger.info("benchmarking LSS STS...")
    lss_sts = sts_spearman(lss_encode, texts1, texts2, labels)

    logger.info("measuring representational similarity...")
    all_texts = list(set(texts1 + texts2))
    rep_sim = representation_similarity(minilm_encode, lss_encode, all_texts)

    logger.info("benchmarking MiniLM-L6-v2 latency...")
    minilm_lat = benchmark_latency(minilm_encode, texts1)

    logger.info("benchmarking LSS latency...")
    lss_lat = benchmark_latency(lss_encode, texts1)

    results = {
        "model_size_params": {
            "minilm_v6": model_size(minilm),
            "lss": model_size(lss_model),
        },
        "sts_spearman": {
            "minilm_v6": minilm_sts,
            "lss": lss_sts,
        },
        "representational_similarity": rep_sim,
        "latency_ms_single": {
            "minilm_v6": minilm_lat,
            "lss": lss_lat,
        },
    }

    logger.info("=" * 50)
    logger.info("COMPARISON RESULTS")
    logger.info("=" * 50)
    logger.info("Model size:")
    logger.info("  MiniLM-L6-v2: %d params (%.1fM)", results["model_size_params"]["minilm_v6"], results["model_size_params"]["minilm_v6"] / 1e6)
    logger.info("  LSS:           %d params (%.1fM)", results["model_size_params"]["lss"], results["model_size_params"]["lss"] / 1e6)
    logger.info("")
    logger.info("STS Spearman correlation (higher = better):")
    logger.info("  MiniLM-L6-v2: %.4f", results["sts_spearman"]["minilm_v6"])
    logger.info("  LSS:           %.4f", results["sts_spearman"]["lss"])
    logger.info("")
    logger.info("Representational similarity (LSS vs MiniLM):")
    logger.info("  Mean cosine:  %.4f", results["representational_similarity"]["mean_cosine"])
    logger.info("  Alignment:    %.4f", results["representational_similarity"]["alignment"])
    logger.info("")
    logger.info("Single-query latency (lower = better):")
    logger.info("  MiniLM-L6-v2: mean=%.2fms  p95=%.2fms", results["latency_ms_single"]["minilm_v6"]["mean_ms"], results["latency_ms_single"]["minilm_v6"]["p95_ms"])
    logger.info("  LSS:           mean=%.2fms  p95=%.2fms", results["latency_ms_single"]["lss"]["mean_ms"], results["latency_ms_single"]["lss"]["p95_ms"])

    return results


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO, format="%(message)s")
    compare()
