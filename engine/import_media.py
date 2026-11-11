"""Decode a local audio/video file into the WAV format used by the transcript worker."""

import argparse
from pathlib import Path
import wave


def convert(source: Path, destination: Path) -> None:
    import av

    with av.open(str(source)) as media:
        stream = next((item for item in media.streams if item.type == "audio"), None)
        if stream is None:
            raise ValueError("Il file non contiene una traccia audio.")
        resampler = av.AudioResampler(format="s16", layout="mono", rate=16000)
        with wave.open(str(destination), "wb") as output:
            output.setnchannels(1)
            output.setsampwidth(2)
            output.setframerate(16000)
            for frame in media.decode(stream):
                for converted in resampler.resample(frame):
                    output.writeframesraw(converted.to_ndarray().tobytes())
            for converted in resampler.resample(None):
                output.writeframesraw(converted.to_ndarray().tobytes())
            if output.getnframes() == 0:
                raise ValueError("La traccia audio è vuota.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    convert(args.source, args.destination)
