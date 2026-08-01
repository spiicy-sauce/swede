//! Structural checks on the table projections.

use swede_syntax::{parse, File};

fn recipe(name: &str) -> swede_syntax::Recipe {
    let path = format!(
        "{}/../../fixtures/valid/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    let src = std::fs::read_to_string(&path).unwrap();
    match parse(&src).file {
        Some(File::Recipe(r)) => r,
        _ => panic!("expected recipe"),
    }
}

#[test]
fn flow_table_has_structure() {
    let out = swede_render::flow_table(&recipe("miso_chicken_and_rice.swede"));
    assert!(out.contains("| marinade | mix |"));
    assert!(out.contains("whites, greens")); // split
    assert!(out.contains("finish by: assembled")); // link on preheat
    assert!(out.contains("!oven 350F")); // equipment + temp
    assert!(out.contains("~35-45m")); // passive range timer
    assert!(out.contains("eat right away")); // doneness annotation
}

#[test]
fn flow_table_resolves_basis() {
    let out = swede_render::flow_table(&recipe("fifty_fifty_loaf.swede"));
    // 50% of 800g flour = 400g; 72% = 576g
    assert!(out.contains("(400g)"), "{out}");
    assert!(out.contains("(576g)"), "{out}");
}

#[test]
fn grid_table_is_deterministic() {
    let r = recipe("miso_chicken_and_rice.swede");
    let a = swede_render::grid_table(&r);
    let b = swede_render::grid_table(&r);
    assert_eq!(a, b);
    assert!(a.contains("preheat")); // equipment-prep header strip
    assert!(a.contains("scallions")); // fork band leaf
}
