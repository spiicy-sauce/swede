//! Golden diagnostics over the fixture corpus (SPEC §11 Stage 0 acceptance).

use swede_semantics::{validate_source, Diagnostic};

fn read(dir: &str, name: &str) -> String {
    let path = format!(
        "{}/../../fixtures/{}/{}",
        env!("CARGO_MANIFEST_DIR"),
        dir,
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn codes(diags: &[Diagnostic]) -> Vec<String> {
    diags.iter().map(|d| d.code.clone()).collect()
}

#[test]
fn valid_fixtures_match_expected() {
    // (fixture, expected diagnostic codes in source order)
    let cases: &[(&str, &[&str])] = &[
        ("miso_chicken_and_rice.swede", &[]),
        ("snow_pea_salad.swede", &["W002"]), // cooled = cool(toasted) ~?
        ("fifty_fifty_loaf.swede", &[]),     // constituents sum to 100%, poolish excluded
        ("tuesday.menu.swede", &[]),
    ];
    for (fixture, expected) in cases {
        let diags = validate_source(&read("valid", fixture));
        assert_eq!(
            &codes(&diags),
            expected,
            "{fixture}: got {:?}",
            diags
                .iter()
                .map(|d| format!("{} {}", d.code, d.message))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn negative_fixtures_flag_expected_code() {
    let cases: &[(&str, &str)] = &[
        ("e001_unknown_ref.swede", "E001"),
        ("e002_reuse.swede", "E002"),
        ("e003_redeclare.swede", "E003"),
        ("e004_forward.swede", "E004"),
        ("e005_link_contradiction.swede", "E005"),
        ("w003_split_sum.swede", "W003"),
        ("w004_constituents.swede", "W004"),
    ];
    for (fixture, code) in cases {
        let diags = validate_source(&read("invalid", fixture));
        assert!(
            codes(&diags).iter().any(|c| c == code),
            "{fixture}: expected {code}, got {:?}",
            diags
                .iter()
                .map(|d| format!("{} {}", d.code, d.message))
                .collect::<Vec<_>>()
        );
    }
}
