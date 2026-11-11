"""Download only requested models. The shipping runtime is already assembled."""
from pathlib import Path
import argparse
import importlib.util
import json
import os
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'engine'))
from acceleration import ui_text
from model_download import ensure_faster_model


def load_accelerator():
    spec = importlib.util.spec_from_file_location('setup_accelerator', Path(__file__).with_name('setup-accelerator.py'))
    accelerator = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(accelerator)
    return accelerator


def setup(root, mid, live_model=None, preview_only=False, compute_mode='auto', live_compute_mode='auto'):
    root.mkdir(parents=True, exist_ok=True)
    if not preview_only:
        load_accelerator().setup(root, mid, compute_mode=compute_mode)
    if preview_only:
        if compute_mode == 'cpu': ensure_faster_model(root, mid)
        else: load_accelerator().setup(root, mid, compute_mode='auto', preview=True)
    elif live_model:
        if live_compute_mode == 'cpu': ensure_faster_model(root, live_model)
        else: load_accelerator().setup(root, live_model, compute_mode='auto', preview=True)
    print(ui_text('PARLO_STAGE:Trascrizione locale pronta.', 'PARLO_STAGE:Local transcription is ready.'), flush=True)
    print(json.dumps({'model': mid, 'preview': mid if preview_only else live_model}), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--model', default='turbo', choices=['turbo', 'tiny', 'small'])
    parser.add_argument('--live-model', choices=['turbo', 'tiny', 'small'])
    parser.add_argument('--preview-only', action='store_true')
    parser.add_argument('--compute-mode', choices=['auto', 'cpu'], default='auto')
    parser.add_argument('--live-compute-mode', choices=['auto', 'cpu'], default='auto')
    parser.add_argument('--engine', type=Path)
    args = parser.parse_args()
    root = (args.engine or Path(os.environ.get('CALLNOTES_ENGINE_DIR', Path(__file__).resolve().parents[1] / '.tools/engine'))).resolve()
    setup(root, args.model, args.live_model, args.preview_only, args.compute_mode, args.live_compute_mode)
