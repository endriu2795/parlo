"""Fetch only an explicitly requested CTranslate2 model; keep cache for resume."""
from faster_whisper.utils import download_model
from acceleration import ui_text


def ensure_faster_model(root, mid):
    target = root / 'models' / f'whisper-{mid}'
    if all((target / name).is_file() for name in ['model.bin', 'config.json', 'tokenizer.json']):
        return target
    print(ui_text(f'PARLO_STAGE:Download Whisper {mid}… Il download può essere ripreso riaprendo PARLO.', f'PARLO_STAGE:Downloading Whisper {mid}… Reopen PARLO to resume the download.'), flush=True)
    download_model(mid, output_dir=str(target), cache_dir=str(root / 'downloads/huggingface'))
    return target
