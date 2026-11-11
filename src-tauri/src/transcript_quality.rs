//! Reviewable display of old ASR loops. Never rewrites the archived source text.
use crate::model::Segment;

pub const WARNING: &str = "Ripetizione anomala ridotta: verifica questo passaggio. Il testo originale rimane consultabile.";

pub fn reduce(text: &str) -> (String, usize) {
    let mut tokens: Vec<(usize, usize, String)> = Vec::new();
    let mut start = None;
    for (index, character) in text.char_indices() {
        if character.is_alphanumeric()
            || character == '_'
            || ((character == '\'' || character == '’') && start.is_some())
        {
            start.get_or_insert(index);
        } else if let Some(first) = start.take() {
            tokens.push((first, index, text[first..index].to_lowercase()));
        }
    }
    if let Some(first) = start {
        tokens.push((first, text.len(), text[first..].to_lowercase()));
    }
    let mut cuts = vec![];
    let (mut index, mut removed) = (0, 0);
    while index < tokens.len() {
        let mut found = false;
        for size in 1..=12.min((tokens.len() - index) / 4) {
            let mut count = 1;
            while index + (count + 1) * size <= tokens.len()
                && (0..size).all(|j| tokens[index + j].2 == tokens[index + count * size + j].2)
            {
                count += 1;
            }
            if count >= if size == 1 { 12 } else { 4 }
                && count * size >= if size == 1 { 12 } else { 24 }
            {
                let keep = if size == 1 { 3 } else { 2 };
                cuts.push((
                    tokens[index + keep * size].0,
                    tokens[index + count * size - 1].1,
                ));
                removed += (count - keep) * size;
                index += count * size;
                found = true;
                break;
            }
        }
        if !found {
            index += 1;
        }
    }
    if cuts.is_empty() {
        return (text.into(), 0);
    }
    let mut result = text.to_owned();
    for (first, last) in cuts.into_iter().rev() {
        result.replace_range(first..last, "");
    }
    (
        result.split_whitespace().collect::<Vec<_>>().join(" "),
        removed,
    )
}
pub fn review(segments: Vec<Segment>) -> Vec<Segment> {
    segments
        .into_iter()
        .map(|mut segment| {
            let (text, removed) = reduce(&segment.text);
            if removed > 0 {
                if segment.original_text.is_none() {
                    segment.original_text = Some(segment.text.clone());
                }
                segment.text = text;
                segment.quality = Some(match segment.quality.take() {
                    Some(previous) => format!("{previous} {WARNING}"),
                    None => WARNING.into(),
                });
            }
            segment
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extreme_loops_keep_prefix_suffix_and_short_real_repetitions() {
        for token in ["ci", "dei", "sì"] {
            let original = format!(
                "Prima. {} Poi la decisione.",
                format!("{token} ").repeat(328)
            );
            let (clean, removed) = reduce(&original);
            assert_eq!(removed, 325);
            assert!(clean.starts_with("Prima."));
            assert!(clean.ends_with("Poi la decisione."));
            assert_eq!(clean.split_whitespace().filter(|w| *w == token).count(), 3);
        }
        let phrase = "si può costruire il timestamp ";
        let (clean, removed) = reduce(&format!("{} poi proseguiamo.", phrase.repeat(31)));
        assert_eq!(removed, 29 * 5);
        assert!(clean.ends_with("poi proseguiamo."));
        for text in [
            "sì sì sì",
            "dei dei dei dei",
            "test test test test",
            "si può costruire il timestamp, si può costruire il timestamp",
        ] {
            assert_eq!(reduce(text), (text.into(), 0));
        }
    }
}
