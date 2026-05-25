import argparse
import logging

from lss_model.config import ModelConfig
from lss_model.train import train, save_model
from lss_model.export import export_to_onnx, verify_onnx
from lss_model.evaluate import load_model, evaluate

logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")


def train_main() -> None:
    parser = argparse.ArgumentParser("lss-train")
    parser.add_argument("--epochs", type=int, default=3)
    parser.add_argument("--batch-size", type=int, default=32)
    parser.add_argument("--lr", type=float, default=2e-5)
    parser.add_argument("--max-len", type=int, default=128)
    parser.add_argument("--output-dir", default="output")
    args = parser.parse_args()

    config = ModelConfig(
        num_epochs=args.epochs,
        batch_size=args.batch_size,
        learning_rate=args.lr,
        max_seq_length=args.max_len,
        output_dir=args.output_dir,
    )
    model = train(config)
    save_model(model, config)


def export_main() -> None:
    parser = argparse.ArgumentParser("lss-export")
    parser.add_argument("--checkpoint", required=True)
    parser.add_argument("--output-dir", default="output")
    args = parser.parse_args()

    config = ModelConfig(output_dir=args.output_dir)
    model = load_model(config, args.checkpoint)
    onnx_path = export_to_onnx(model, config)
    verify_onnx(onnx_path)
    print(f"Exported to {onnx_path}")
