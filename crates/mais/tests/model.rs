use mais::model::*;

#[test]
fn usage_adds_up_and_counts_context_apart_from_output() {
    let a = Usage { input: 2, cache_read: 1000, cache_write: 50, output: 10 };
    let b = Usage { input: 1, cache_read: 900, cache_write: 0, output: 30 };
    assert_eq!(a.context(), 1052);
    assert_eq!(a.total(), 1062);
    assert_eq!([a, b].into_iter().sum::<Usage>(), Usage { input: 3, cache_read: 1900, cache_write: 50, output: 40 });
    assert_eq!(a.max(b), Usage { input: 2, cache_read: 1000, cache_write: 50, output: 30 });
}

#[test]
fn an_unknown_figure_is_not_zero() {
    let f: Figure<u64> = Figure::unknown();
    assert_eq!(serde_json::to_value(f).unwrap(), serde_json::json!({ "value": null, "provenance": "unknown" }));
}
