//! Compare no-change borrowing against an always-materializing reference path.
use std::{borrow::Cow, hint::black_box, time::Instant};
use textforge::tokens::{InstructionMethods, transforms::unorm::Unorm};
use unicode_normalization::UnicodeNormalization;

#[test]
#[ignore = "informational release benchmark"]
fn normalization_fast_path() {
    let token = Unorm::new("nfc").unwrap();
    for (label, text) in [
        ("ascii", "plain text ".repeat(8192)),
        ("normalized", "café 🍌 ".repeat(8192)),
        ("decomposed", "cafe\u{301} 🍌 ".repeat(8192)),
    ] {
        let expected: String = text.nfc().collect();
        let started = Instant::now();
        for _ in 0..64 {
            let result = token
                .transform(Cow::Borrowed(black_box(&text)), None)
                .unwrap();
            assert_eq!(result, expected);
            black_box(result);
        }
        let fast_path = started.elapsed();
        let started = Instant::now();
        for _ in 0..64 {
            let result: String = black_box(text.as_str()).nfc().collect();
            assert_eq!(result, expected);
            black_box(result);
        }
        eprintln!(
            "{label}: {} bytes x 64, checked={fast_path:?}, always_collect={:?}",
            text.len(),
            started.elapsed()
        );
    }
}
