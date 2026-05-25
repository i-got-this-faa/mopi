import torch
import torch.nn as nn
import torch.nn.functional as F

from lss_model.config import ModelConfig


class LSSEmbedModel(nn.Module):
    def __init__(self, config: ModelConfig) -> None:
        super().__init__()
        self.config = config

        self.embeddings = Embeddings(config)
        self.encoder = TransformerEncoder(config)
        self.pooler = Pooler(config)

    def forward(
        self,
        input_ids: torch.Tensor,
        attention_mask: torch.Tensor,
        token_type_ids: torch.Tensor | None = None,
    ) -> torch.Tensor:
        hidden = self.embeddings(input_ids, token_type_ids)
        hidden = self.encoder(hidden, attention_mask)
        pooled = self.pooler(hidden, attention_mask)
        return pooled


class Embeddings(nn.Module):
    def __init__(self, config: ModelConfig) -> None:
        super().__init__()
        self.token_embed = nn.Embedding(config.vocab_size, config.hidden_size)
        self.pos_embed = nn.Embedding(config.max_position_embeddings, config.hidden_size)
        self.token_type_embed = nn.Embedding(2, config.hidden_size)
        self.layer_norm = nn.LayerNorm(config.hidden_size, eps=config.layer_norm_eps)
        self.dropout = nn.Dropout(config.hidden_dropout_prob)

    def forward(
        self,
        input_ids: torch.Tensor,
        token_type_ids: torch.Tensor | None = None,
    ) -> torch.Tensor:
        seq_len = input_ids.size(-1)
        pos_ids = torch.arange(seq_len, device=input_ids.device).unsqueeze(0)

        if token_type_ids is None:
            token_type_ids = torch.zeros_like(input_ids)

        x = self.token_embed(input_ids)
        x = x + self.pos_embed(pos_ids)
        x = x + self.token_type_embed(token_type_ids)
        x = self.layer_norm(x)
        x = self.dropout(x)
        return x


class TransformerEncoder(nn.Module):
    def __init__(self, config: ModelConfig) -> None:
        super().__init__()
        self.layers = nn.ModuleList([TransformerLayer(config) for _ in range(config.num_hidden_layers)])

    def forward(self, hidden: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
        for layer in self.layers:
            hidden = layer(hidden, attention_mask)
        return hidden


class TransformerLayer(nn.Module):
    def __init__(self, config: ModelConfig) -> None:
        super().__init__()
        self.attention = SelfAttention(config)
        self.attention_norm = nn.LayerNorm(config.hidden_size, eps=config.layer_norm_eps)
        self.ffn = FFN(config)
        self.ffn_norm = nn.LayerNorm(config.hidden_size, eps=config.layer_norm_eps)
        self.dropout = nn.Dropout(config.hidden_dropout_prob)

    def forward(self, hidden: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
        x = self.attention(hidden, attention_mask)
        x = self.dropout(x)
        hidden = self.attention_norm(hidden + x)

        x = self.ffn(hidden)
        x = self.dropout(x)
        hidden = self.ffn_norm(hidden + x)
        return hidden


class SelfAttention(nn.Module):
    def __init__(self, config: ModelConfig) -> None:
        super().__init__()
        self.num_heads = config.num_attention_heads
        self.head_dim = config.hidden_size // config.num_attention_heads
        self.scale = self.head_dim ** -0.5

        self.qkv = nn.Linear(config.hidden_size, config.hidden_size * 3)
        self.proj = nn.Linear(config.hidden_size, config.hidden_size)

    def forward(self, hidden: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
        B, L, D = hidden.shape

        qkv = self.qkv(hidden).reshape(B, L, 3, self.num_heads, self.head_dim)
        q, k, v = qkv.unbind(2)
        q, k, v = q.transpose(1, 2), k.transpose(1, 2), v.transpose(1, 2)

        attn = (q @ k.transpose(-2, -1)) * self.scale
        attn = attn + attention_mask[:, None, None, :].to(attn.dtype)
        attn = F.softmax(attn, dim=-1)

        out = (attn @ v).transpose(1, 2).reshape(B, L, D)
        out = self.proj(out)
        return out


class FFN(nn.Module):
    def __init__(self, config: ModelConfig) -> None:
        super().__init__()
        self.fc1 = nn.Linear(config.hidden_size, config.intermediate_size)
        self.fc2 = nn.Linear(config.intermediate_size, config.hidden_size)
        self.gelu = nn.GELU(approximate="tanh")

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        x = self.fc1(x)
        x = self.gelu(x)
        x = self.fc2(x)
        return x


class Pooler(nn.Module):
    def __init__(self, config: ModelConfig) -> None:
        super().__init__()
        self.config = config

    def forward(self, hidden: torch.Tensor, attention_mask: torch.Tensor) -> torch.Tensor:
        pooled = hidden[:, 0]
        if self.config.normalize:
            pooled = F.normalize(pooled, p=2, dim=-1)
        return pooled
