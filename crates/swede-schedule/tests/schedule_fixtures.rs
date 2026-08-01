//! Scheduler acceptance checks (SPEC §11 Stage 2/3).

use std::path::PathBuf;
use swede_syntax::{parse, File};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(format!(
        "{}/../../fixtures/valid",
        env!("CARGO_MANIFEST_DIR")
    ))
}

fn recipe(name: &str) -> swede_syntax::Recipe {
    let src = std::fs::read_to_string(fixtures_dir().join(name)).unwrap();
    match parse(&src).file {
        Some(File::Recipe(r)) => r,
        _ => panic!("expected recipe"),
    }
}

#[test]
fn preheat_is_back_placed() {
    // Stage 2: `hot` (preheat) should start after T=0, back-placed to finish
    // by `assembled`, rather than starting at 0.
    let plan = swede_schedule::schedule_recipe(&recipe("miso_chicken_and_rice.swede"));
    let hot = plan
        .lanes
        .iter()
        .flat_map(|l| l.tasks.iter())
        .find(|t| t.name == "hot")
        .expect("hot task");
    assert!(
        hot.start > 0.0,
        "preheat should be back-placed, started at {}",
        hot.start
    );
    assert!(plan.notes.iter().any(|n| n.contains("back-placed")));
}

#[test]
fn menu_aligns_and_proposes_coresidency() {
    // Stage 3: cross-recipe schedule proposes walnuts inside the chicken oven.
    let src = std::fs::read_to_string(fixtures_dir().join("tuesday.menu.swede")).unwrap();
    let menu = match parse(&src).file {
        Some(File::Menu(m)) => m,
        _ => panic!("expected menu"),
    };
    let plan = swede_schedule::schedule_menu(&menu, &fixtures_dir()).expect("schedule menu");
    assert_eq!(plan.lanes.len(), 2);
    assert!(
        plan.notes
            .iter()
            .any(|n| n.contains("co-residency") && n.contains("oven")),
        "expected oven co-residency note, got {:?}",
        plan.notes
    );
    // both recipes end by serve time (T=0)
    let max_finish = plan
        .lanes
        .iter()
        .flat_map(|l| l.tasks.iter())
        .map(|t| t.finish)
        .fold(f64::MIN, f64::max);
    assert!(
        max_finish <= 0.01,
        "everything should finish by serve time, got {max_finish}"
    );
}
