//! Parse the real spec fixtures and assert key AST structure.

use swede_syntax::*;

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/../../fixtures/valid/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn recipe(name: &str) -> Recipe {
    let r = parse(&fixture(name));
    assert!(r.errors.is_empty(), "{name} syntax errors: {:?}", r.errors);
    match r.file {
        Some(File::Recipe(rec)) => rec,
        other => panic!("{name}: expected recipe, got {other:?}"),
    }
}

#[test]
fn chicken_structure() {
    let r = recipe("miso_chicken_and_rice.swede");
    assert_eq!(r.name, "miso_chicken_and_rice");
    // meta: serves, tags, source (source URL kept verbatim, `//` intact)
    assert_eq!(r.meta.len(), 3);
    let source = r.meta.iter().find(|m| m.key == "source").unwrap();
    assert!(
        source.value.contains("https://smittenkitchen.com/2026/02/"),
        "{}",
        source.value
    );

    // the split binding: whites, greens = slice(scallions[2])
    let split = r.bindings.iter().find(|b| b.outputs.len() == 2).unwrap();
    let names: Vec<_> = split.outputs.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, vec!["whites", "greens"]);

    // the preheat with a forward &by link and an equipment-state node.
    // `!oven {350F}` sits in arg position here (no `;`), so it is an equip arg.
    let hot = r
        .bindings
        .iter()
        .find(|b| b.outputs[0].name == "hot")
        .unwrap();
    assert!(matches!(&hot.value, Expr::Call(c)
        if c.args.iter().any(|a| matches!(a, Arg::Equip(e) if e.name == "oven" && e.temp.is_some()))));
    let has_by = hot
        .annotations
        .iter()
        .any(|a| matches!(a, Annotation::Link(Link::By { .. })));
    assert!(has_by, "hot should carry a &by link");

    // baked: passive range timer + [covered] flag + doneness string
    let baked = r
        .bindings
        .iter()
        .find(|b| b.outputs[0].name == "baked")
        .unwrap();
    assert!(baked
        .annotations
        .iter()
        .any(|a| matches!(a, Annotation::Timer(Timer::Passive(_)))));
    assert!(baked
        .annotations
        .iter()
        .any(|a| matches!(a, Annotation::Flag { name, .. } if name == "covered")));
    assert!(baked
        .annotations
        .iter()
        .any(|a| matches!(a, Annotation::Doc { .. })));
}

#[test]
fn salad_structure() {
    let r = recipe("snow_pea_salad.swede");
    // cooled = cool(toasted) ~?
    let cooled = r
        .bindings
        .iter()
        .find(|b| b.outputs[0].name == "cooled")
        .unwrap();
    assert!(cooled
        .annotations
        .iter()
        .any(|a| matches!(a, Annotation::Timer(Timer::Unbounded { .. }))));
    // plate carries &lag(0)
    let plate = r.bindings.last().unwrap();
    assert!(plate
        .annotations
        .iter()
        .any(|a| matches!(a, Annotation::Link(Link::Lag { .. }))));
    // nested call: ribbons = slice(pat_dry(drain(soaked)))
    let ribbons = r
        .bindings
        .iter()
        .find(|b| b.outputs[0].name == "ribbons")
        .unwrap();
    assert!(matches!(&ribbons.value, Expr::Call(c) if matches!(&c.args[0], Arg::Call(_))));
}

#[test]
fn loaf_basis() {
    let r = recipe("fifty_fifty_loaf.swede");
    assert_eq!(r.bases.len(), 1);
    assert_eq!(r.bases[0].name, "flour");
    // poolish = <+bread/poolish>[20%* flour]
    let poolish = r
        .bindings
        .iter()
        .find(|b| b.outputs[0].name == "poolish")
        .unwrap();
    match &poolish.value {
        Expr::SubRef(s) => {
            assert_eq!(s.path, vec!["bread", "poolish"]);
            assert!(s.amount.is_some());
        }
        other => panic!("expected sub_ref, got {other:?}"),
    }
}

#[test]
fn menu_structure() {
    let src = fixture("tuesday.menu.swede");
    let r = parse(&src);
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    match r.file {
        Some(File::Menu(m)) => {
            assert_eq!(m.entries.len(), 2);
            let salad = &m.entries[1];
            // salad = snow_pea_salad &by(chicken.plate)
            match &salad.links[0] {
                Link::By { target, .. } => {
                    assert_eq!(target.name, "chicken");
                    assert_eq!(target.member.as_deref(), Some("plate"));
                }
                other => panic!("expected &by, got {other:?}"),
            }
        }
        other => panic!("expected menu, got {other:?}"),
    }
}
