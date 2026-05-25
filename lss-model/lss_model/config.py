from dataclasses import dataclass, field


@dataclass
class ModelConfig:
    vocab_size: int = 30522
    hidden_size: int = 384
    num_hidden_layers: int = 6
    num_attention_heads: int = 6
    intermediate_size: int = 1536
    max_position_embeddings: int = 512
    hidden_dropout_prob: float = 0.1
    attention_probs_dropout_prob: float = 0.1
    layer_norm_eps: float = 1e-12
    pad_token_id: int = 0
    initializer_range: float = 0.02

    pooler: str = "cls"
    normalize: bool = True

    # training
    batch_size: int = 64
    learning_rate: float = 5e-4
    num_epochs: int = 5
    warmup_ratio: float = 0.1
    max_seq_length: int = 128
    gradient_accumulation_steps: int = 1
    fp16: bool = True

    # data
    train_file: str = ""
    eval_file: str = ""
    dataset_name: str = "sentence-transformers/all-nli"

    # export
    onnx_opset: int = 18

    model_name: str = "lss-embedding-model"
    output_dir: str = "output"

    def __post_init__(self) -> None:
        if self.hidden_size % self.num_attention_heads != 0:
            raise ValueError(
                f"hidden_size ({self.hidden_size}) must be divisible by "
                f"num_attention_heads ({self.num_attention_heads})"
            )
