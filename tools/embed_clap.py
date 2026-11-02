#!/usr/bin/env python3
"""CLAP embeddings for the library, and for the words people search with.

`dsp-v1` describes a spectrum; CLAP describes what a record sounds like to
someone who has heard music before. This script is the offline half: it reads the
library, runs the audio encoder once per track, and writes vectors into the same
`embed` table the server already knows how to read. The server never links ONNX.

The mel front-end is transformers' own numpy path, called directly — the class
that wraps it refuses to load without PyTorch, which is a guard, not a
dependency of the maths.

    tools/venv/bin/python tools/embed_clap.py --limit 20
    tools/venv/bin/python tools/embed_clap.py --phrases tools/phrases.txt
"""

from __future__ import annotations

import argparse
import json
import os
import sqlite3
import subprocess
import sys

import numpy as np
import onnxruntime as ort

HERE = os.path.dirname(os.path.abspath(__file__))
MODELS = os.path.join(HERE, "models")
SR = 48_000
CHUNK = 10 * SR  # CLAP is trained on ten-second windows
FRAMES = 1001  # 1 + 480000 / 480

sys.path.insert(0, os.path.join(HERE, "venv", "lib"))


def mel_front_end():
    """The exact filters and spectrogram CLAP was trained with."""
    from transformers.audio_utils import (  # noqa: E402
        mel_filter_bank,
        spectrogram,
        window_function,
    )

    filters = mel_filter_bank(
        num_frequency_bins=513,
        num_mel_filters=64,
        min_frequency=50,
        max_frequency=14_000,
        sampling_rate=SR,
        norm="slaney",
        mel_scale="slaney",
    )
    window = window_function(1024, "hann")

    def compute(waveform: np.ndarray) -> np.ndarray:
        mel = spectrogram(
            waveform,
            window,
            frame_length=1024,
            hop_length=480,
            power=2.0,
            mel_filters=filters,
            log_mel="dB",
        )
        return mel.T[:FRAMES]  # (frames, mels), the layout the export wants

    return compute


def decode(path: str) -> np.ndarray:
    out = subprocess.run(
        [
            "ffmpeg", "-v", "error", "-i", path,
            "-ac", "1", "-ar", str(SR), "-f", "f32le", "-",
        ],
        capture_output=True,
        check=True,
    )
    return np.frombuffer(out.stdout, dtype=np.float32)


def windows(pcm: np.ndarray) -> list[np.ndarray]:
    """Three windows per track, at the start, the middle and the end.

    One window is a coin flip: a quiet intro and a loud chorus are different
    songs to the encoder. Three is the cheapest thing that describes the whole.
    """
    if len(pcm) < CHUNK:
        reps = int(np.ceil(CHUNK / max(len(pcm), 1)))
        pcm = np.tile(pcm, reps)
    starts = [0, max(0, (len(pcm) - CHUNK) // 2), max(0, len(pcm) - CHUNK)]
    return [pcm[s : s + CHUNK] for s in starts]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--db", default=os.path.expanduser("~/.local/share/bulba-music/bulba.db"))
    ap.add_argument("--model", default="clap-v1")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--offset", type=int, default=0)
    ap.add_argument("--phrases", default="")
    args = ap.parse_args()

    audio = ort.InferenceSession(
        os.path.join(MODELS, "audio_model_quantized.onnx"),
        providers=["CPUExecutionProvider"],
    )
    text = ort.InferenceSession(
        os.path.join(MODELS, "text_model_quantized.onnx"),
        providers=["CPUExecutionProvider"],
    )
    compute_mel = mel_front_end()
    conn = sqlite3.connect(args.db)
    conn.execute("PRAGMA journal_mode=WAL")

    if args.phrases:
        from tokenizers import Tokenizer

        tok = Tokenizer.from_file(os.path.join(MODELS, "tokenizer.json"))
        tok.enable_truncation(max_length=77)
        # No padding: the exported text tower has no attention mask, so padding
        # tokens are attended and every phrase collapses to the same vector
        # (measured: cosine 0.99 between "jazz" and "heavy metal"). One phrase at
        # a time keeps each sequence honest.
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS phrase_vec (phrase TEXT PRIMARY KEY, vec BLOB NOT NULL)"
        )
        lines = [l.strip() for l in open(args.phrases, encoding="utf-8") if l.strip()]
        vecs = []
        for line in lines:
            ids = np.array([tok.encode(line).ids], dtype=np.int64)
            v = text.run(None, {"input_ids": ids})[0][0]
            vecs.append(v / (np.linalg.norm(v) + 1e-9))
        vecs = np.stack(vecs)
        for phrase, vec in zip(lines, vecs):
            conn.execute(
                "INSERT OR REPLACE INTO phrase_vec(phrase, vec) VALUES(?, ?)",
                (phrase, vec.astype(np.float32).tobytes()),
            )
        conn.commit()
        print(f"phrases: {len(lines)}")
        return 0

    rows = conn.execute(
        """SELECT t.id, m.path
           FROM track t
           JOIN track_master tm ON tm.track_id = t.id
           JOIN master m ON m.id = tm.master_id
           WHERE m.id = (
             SELECT m2.id FROM track_master tm2 JOIN master m2 ON m2.id = tm2.master_id
             WHERE tm2.track_id = t.id ORDER BY (m2.codec='flac') DESC, m2.bytes DESC LIMIT 1)
           ORDER BY t.id LIMIT ? OFFSET ?""",
        (-1 if args.limit == 0 else args.limit, args.offset),
    ).fetchall()

    done = 0
    for track_id, path in rows:
        try:
            pcm = decode(path)
            feats = np.stack([compute_mel(w) for w in windows(pcm)])[:, None, :, :]
            emb = audio.run(None, {"input_features": feats.astype(np.float32)})[0]
            vec = emb.mean(axis=0)
            vec = vec / (np.linalg.norm(vec) + 1e-9)
            conn.execute(
                "INSERT OR REPLACE INTO embed(model, track_id, dim, vec) VALUES(?,?,?,?)",
                (args.model, track_id, vec.shape[0], vec.astype(np.float32).tobytes()),
            )
            conn.commit()
            done += 1
            if done % 20 == 0:
                print(f"{done}/{len(rows)}", flush=True)
        except Exception as exc:  # noqa: BLE001 — one bad file must not stop a library
            print(f"skip {track_id} {path}: {exc}", file=sys.stderr)
    print(f"embedded {done} tracks as {args.model}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
