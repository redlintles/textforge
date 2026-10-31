# Unicode normalization conformance data

- Source: https://www.unicode.org/Public/18.0.0/ucd/NormalizationTest.txt
- Version: Unicode 18.0.0, matching unicode-normalization 0.1.26's tables.
- Downloaded: 2026-10-31; original data and copyright headers retained.
- SHA-256 (original bytes): `25a50d816764b04abfb4a646d3eb2b2a803284c3873d9a06757b94fe4513dde3`
- Unicode data license: [LICENSE.txt](LICENSE.txt), downloaded from
  https://www.unicode.org/license.txt. This fixture's license applies to its data;
  the TextForge project license is unchanged.

`tests/unicode_normalization_conformance.rs` checks all five columns against all
four forms, checks idempotence, and checks identity for scalar values absent from
Part 1. Version and row-count guards detect stale or truncated fixtures. The file
is kept offline so tests do not depend on network availability. Keep the header,
license, version guard and this digest current when refreshing the corpus.
