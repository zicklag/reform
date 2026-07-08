"""Inspect Stanza model architectures to understand what would need porting."""
import torch
import os

model_dir = "stanza_resources/en"

models = {
    "tokenize": f"{model_dir}/tokenize/combined_nocharlm.pt",
    "pos": f"{model_dir}/pos/combined_charlm.pt",
    "depparse": f"{model_dir}/depparse/combined_charlm.pt",
    "lemma": f"{model_dir}/lemma/combined_nocharlm.pt",
    "pretrain": f"{model_dir}/pretrain/conll17.pt",
    "forward_charlm": f"{model_dir}/forward_charlm/1billion.pt",
    "backward_charlm": f"{model_dir}/backward_charlm/1billion.pt",
}

for name, path in models.items():
    print(f"\n{'='*60}")
    print(f"{name}: {path}")
    print(f"Size: {os.path.getsize(path) / 1024 / 1024:.1f} MB")
    try:
        checkpoint = torch.load(path, map_location="cpu", weights_only=False)
        if isinstance(checkpoint, dict):
            # Print top-level keys
            keys = list(checkpoint.keys())
            print(f"Top-level keys ({len(keys)}): {keys[:15]}")

            # If there's a config, print it
            if "config" in checkpoint:
                config = checkpoint["config"]
                if isinstance(config, dict):
                    print(f"Config keys: {list(config.keys())[:20]}")
                    for k in ["wordvec_dim", "hidden_dim", "num_layers", "arch",
                              "charlm", "bert_model", "num_classes", "label",
                              "vocab_size", "emb_dim", "input_size", "output_size"]:
                        if k in config:
                            print(f"  {k}: {config[k]}")

            # If there's a model state dict, print shapes
            if "model" in checkpoint:
                sd = checkpoint["model"]
                if isinstance(sd, dict):
                    print(f"State dict entries: {len(sd)}")
                    # Print first few parameter shapes
                    for i, (k, v) in enumerate(sd.items()):
                        if i < 10:
                            shape = v.shape if hasattr(v, 'shape') else type(v)
                            print(f"  {k}: {shape}")
            elif "state_dict" in checkpoint:
                sd = checkpoint["state_dict"]
                if isinstance(sd, dict):
                    print(f"State dict entries: {len(sd)}")
                    for i, (k, v) in enumerate(sd.items()):
                        if i < 10:
                            shape = v.shape if hasattr(v, 'shape') else type(v)
                            print(f"  {k}: {shape}")
        else:
            print(f"Type: {type(checkpoint)}")
    except Exception as e:
        print(f"Error: {e}")