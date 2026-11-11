"""Conservative detection of runaway ASR loops, independent of speaker labels."""
import re

_tokens = re.compile(r"\w+(?:['’]\w+)*", re.UNICODE)
REPETITION_WARNING = 'Ripetizione anomala ridotta: verifica questo passaggio. Il testo originale rimane consultabile.'
RETRY_WARNING = 'Passaggio rielaborato per ripetizioni o timestamp anomali: verifica il testo.'

def reduce_repetitions(text):
    """Keep short stutters and preserve the content following a detected loop.

    A single word needs 12 repetitions; longer phrases need >=4 repetitions
    and >=24 words. This is a reviewable presentation, never a reconstruction.
    """
    matches = list(_tokens.finditer(text))
    keys = [m.group().casefold() for m in matches]
    cuts, removed, index = [], 0, 0
    while index < len(keys):
        for size in range(1, min(12, (len(keys) - index) // 4) + 1):
            pattern, count = keys[index:index + size], 1
            while keys[index + count * size:index + (count + 1) * size] == pattern:
                count += 1
            if count >= (12 if size == 1 else 4) and count * size >= (12 if size == 1 else 24):
                keep = 3 if size == 1 else 2
                cuts.append((matches[index + keep * size].start(), matches[index + count * size - 1].end()))
                removed += (count - keep) * size
                index += count * size
                break
        else:
            index += 1
    for first, last in reversed(cuts):
        text = text[:first] + text[last:]
    return (re.sub(r'\s+', ' ', text).strip() if cuts else text), removed

def review_segments(segments):
    result = []
    for segment in segments:
        text, removed = reduce_repetitions(segment['text'])
        if removed:
            segment = {**segment, 'text': text,
                'original_text': segment.get('original_text') or segment['text'],
                'quality': REPETITION_WARNING}
        result.append(segment)
    return result

def suspicious_decode(words, duration):
    """Inspect raw timestamps before clamping can disguise an alignment loop."""
    import math
    if reduce_repetitions(''.join(w.word for w in words))[1]:
        return True
    if words:
        count = len(_tokens.findall(''.join(w.word for w in words)))
        span = words[-1].end_ts - words[0].start_ts
        if count >= 12 and span >= 0 and count > max(12, span * 9):
            return True
    previous, invalid = -1., 0
    for word in words:
        if not math.isfinite(word.start_ts) or not math.isfinite(word.end_ts):
            return True
        if word.start_ts < previous - .1 or word.end_ts < word.start_ts - .02 or word.end_ts > duration + .25:
            invalid += 1
        previous = word.start_ts
    return invalid >= 3
