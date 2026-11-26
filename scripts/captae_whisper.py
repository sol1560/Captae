#!/usr/bin/env python3
"""Captae 语音识别 sidecar。

用 faster-whisper（MIT）在本地把音频转成带时间轴的文字。
Captae 通过子进程调用它，逐行读取 JSON：
  {"type":"progress","value":0.42}
  {"type":"segment","start":1.2,"end":3.4,"text":"..."}
  {"type":"done","language":"en","duration":888.0}
  {"type":"error","message":"..."}

用法：captae_whisper.py AUDIO --model small [--language zh] [--device auto]
      captae_whisper.py --download --model small --models-dir DIR
      只下载模型，逐行输出 {"type":"download","done":字节,"total":字节}，最后 {"type":"done"}
"""
import argparse
import json
import os
import sys

# huggingface_hub 的 xet 后端是一阵一阵汇报进度的（几十秒不动再一下跳一大截），
# 界面的进度和剩余时间会乱跳；单个模型文件用普通 HTTP 下载速度差不多，进度是连续的
os.environ.setdefault("HF_HUB_DISABLE_XET", "1")


def emit(obj):
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
    sys.stdout.flush()


# faster_whisper.utils.download_model 用的文件列表（它把进度条关掉了，所以这里自己调 snapshot_download）
MODEL_FILES = ["config.json", "preprocessor_config.json", "model.bin", "tokenizer.json", "vocabulary.*"]


def download(model, models_dir):
    """下载模型并汇报字节进度。新版 huggingface_hub 走 xet 后端时，数据先进 ~/.cache/huggingface/xet，
    最后才出现在 models_dir 里，只能靠进度回调知道下了多少。"""
    import time
    from huggingface_hub import snapshot_download
    from faster_whisper.utils import _MODELS
    from tqdm.auto import tqdm as base

    bars = {}
    last = [0.0]

    def report(force=False):
        if force or time.time() - last[0] >= 0.3:
            last[0] = time.time()
            # 同一份数据可能同时有「整体字节」和「单个文件」两条进度（xet 后端），相加会算两遍；
            # 取总量最大的那条：xet 下是整体进度，普通下载时是 model.bin（其余文件只有几 MB）
            n, t = max(bars.values(), key=lambda b: b[1], default=(0, 0))
            emit({"type": "download", "done": n, "total": t})

    class Progress(base):
        def __init__(self, *a, **k):
            k["disable"] = False
            # 不往 stderr 画进度条。用 os.devnull：Windows 上没有 /dev/null，open 会报 FileNotFoundError，
            # 模型一个字节都下不了（2026-11-25 Devin 的 Windows 上实测，只留下 Hugging Face 的元数据）
            k["file"] = open(os.devnull, "w")
            super().__init__(*a, **k)

        def update(self, n=1):
            r = super().update(n)
            if self.unit == "B":
                bars[id(self)] = (self.n, self.total or 0)
                report()
            return r

    repo = model if "/" in model else _MODELS.get(model, model)
    snapshot_download(repo, allow_patterns=MODEL_FILES, cache_dir=models_dir, tqdm_class=Progress)
    report(force=True)


def av_compat():
    """faster-whisper 1.2.1 调 av.open(..., metadata_errors="ignore")，PyAV 19 删了这个参数，转文字一开始就 TypeError
    （2026-11-25 Devin 的 Windows 上实测：pip 装到了 av 19.0.2）。安装时已经限定 av<19，这里给已经装了 av 19 的环境兜底：
    去掉这个参数再调（faster-whisper 的新代码也是这么处理的，见它仓库 master 的 audio.py）"""
    try:
        import av
        if int(av.__version__.split(".")[0]) >= 19:
            orig = av.open

            def open_(*a, **k):
                k.pop("metadata_errors", None)
                return orig(*a, **k)

            av.open = open_
    except Exception:  # noqa: BLE001
        pass


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("audio", nargs="?")
    ap.add_argument("--download", action="store_true")
    ap.add_argument("--model", default="small")
    ap.add_argument("--language", default=None)
    ap.add_argument("--device", default="auto")
    ap.add_argument("--compute-type", default="default")
    ap.add_argument("--models-dir", default=None)
    a = ap.parse_args()
    try:
        from faster_whisper import WhisperModel
    except ImportError:
        emit({"type": "error", "code": "MISSING_FASTER_WHISPER",
              "message": "没有安装语音识别组件（faster-whisper），在设置里点「安装语音识别」"})
        return 2
    if a.download:
        try:
            download(a.model, a.models_dir)
            emit({"type": "done"})
            return 0
        except Exception as e:  # noqa: BLE001
            emit({"type": "error", "message": str(e)})
            return 1
    av_compat()
    try:
        model = WhisperModel(a.model, device=a.device, compute_type=a.compute_type,
                             download_root=a.models_dir)
        segments, info = model.transcribe(a.audio, language=a.language, vad_filter=True,
                                          beam_size=5)
        total = info.duration or 0
        for s in segments:
            emit({"type": "segment", "start": s.start, "end": s.end, "text": s.text.strip()})
            if total:
                emit({"type": "progress", "value": min(1.0, s.end / total)})
        emit({"type": "done", "language": info.language, "duration": total})
        return 0
    except Exception as e:  # noqa: BLE001
        emit({"type": "error", "message": str(e)})
        return 1


if __name__ == "__main__":
    sys.exit(main())
