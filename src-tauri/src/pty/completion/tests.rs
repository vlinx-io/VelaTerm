use super::*;

fn state() -> State {
    let mut state = State::default();
    state.nonce = "test".into();
    state
}

#[test]
fn private_protocol_is_removed_at_every_chunk_boundary() {
    let bytes = b"before\x1b]6973;test;P\x07after\x1b[31mred";
    for split in 0..=bytes.len() {
        let mut filter = Filter::default();
        let mut state = state();
        let mut visible = filter.feed(&bytes[..split], &mut state);
        visible.extend(filter.feed(&bytes[split..], &mut state));
        assert_eq!(visible, b"beforeafter\x1b[31mred");
        assert!(state.snapshot.ready);
    }
}

#[test]
fn private_protocol_passes_through_without_integration() {
    // Windows and other unintegrated shells keep their bytes untouched, including the protocol prefix.
    let bytes = b"a\x1b]6973;test;P\x07b";
    let mut filter = Filter::default();
    assert_eq!(filter.feed(bytes, &mut State::default()), bytes);
}

#[test]
fn stale_candidates_are_not_accepted_after_another_clients_input() {
    let mut state = state();
    state.message(b"test;P");
    state.requested = Some((0, Instant::now()));
    state.message(b"test;A;1");
    state.message(b"test;C;676974;64657363");
    state.input("x");
    state.message(b"test;Z");
    assert!(state.snapshot.items.is_empty());
    assert!(!state.snapshot.pending);
}

#[test]
fn authenticated_candidate_and_busy_lifecycle() {
    let mut state = state();
    state.message(b"wrong;P");
    assert!(!state.snapshot.supported);
    state.message(b"test;P");
    state.requested = Some((0, Instant::now()));
    state.message(b"test;A;42");
    state.message(b"test;C;e4b8ade69687;");
    state.message(b"test;Z");
    assert_eq!(state.snapshot.items[0].label, "中文");
    assert_eq!(state.snapshot.revision, 42);
    state.input("\r");
    assert!(!state.snapshot.ready);
    assert!(state.snapshot.items.is_empty());
}

#[test]
fn native_token_controls_unicode_highlights_and_description_cleanup() {
    let mut state = state();
    state.message(b"test;P");
    state.requested = Some((0, Instant::now()));
    state.message(b"test;A;7");
    state.message(b"test;T;6368");
    state.message(b"test;C;636865636b6f7574;636865636b6f757420202d2d20537769746368206272616e6368");
    state.message(b"test;Z");
    assert_eq!(state.snapshot.query, "ch");
    assert_eq!(state.snapshot.items[0].matches, vec![[0, 2]]);
    assert_eq!(state.snapshot.items[0].description, "Switch branch");
    assert_eq!(state.snapshot.items[0].index, 0);
    assert_eq!(match_ranges("😀中文", "😀中"), vec![[0, 3]]);
    assert_eq!(match_ranges("terminal/", "src/ter"), vec![[0, 3]]);
    assert_eq!(match_ranges("terminal", "src/ter"), vec![[0, 3]]);
}

#[test]
fn relevance_ranking_preserves_native_selection_indices() {
    let mut state = state();
    state.message(b"test;P");
    state.requested = Some((0, Instant::now()));
    state.message(b"test;A;1");
    state.message(b"test;T;6769");
    for label in ["git-upload-pack", "giftool", "git", "gi", "agit", "gXi", "GIT"] {
        let hex: String = label.bytes().map(|byte| format!("{byte:02x}")).collect();
        state.message(format!("test;C;{hex};").as_bytes());
    }
    state.message(b"test;Z");
    let ranked: Vec<_> = state.snapshot.items.iter().map(|item| (item.label.as_str(), item.index)).collect();
    assert_eq!(ranked, vec![("gi", 3), ("git", 2), ("giftool", 1), ("git-upload-pack", 0), ("GIT", 6), ("agit", 4), ("gXi", 5)]);
}

#[test]
fn relevance_ranking_handles_paths_unicode_and_empty_tokens() {
    let mut items: Vec<_> = ["中文目录/", "中文/"].iter().enumerate().map(|(index, label)| Item {
        index, label: (*label).into(), description: String::new(), matches: Vec::new(),
    }).collect();
    rank_items(&mut items, "");
    assert_eq!(items[0].index, 0);
    rank_items(&mut items, "/tmp/中");
    assert_eq!(items[0].label, "中文/");
    assert_eq!(items[0].index, 1);
}

#[test]
fn shared_prefix_is_published_only_with_several_current_candidates() {
    let mut state = state();
    state.message(b"test;P");
    state.requested = Some((0, Instant::now()));
    state.message(b"test;A;1");
    state.message(b"test;C;446f63756d656e74732f;");
    state.message(b"test;C;446f776e6c6f6164732f;");
    state.message(b"test;E");
    state.message(b"test;Z");
    assert!(state.snapshot.extendable);
    state.input("x");
    assert!(!state.snapshot.extendable);
    // A shared prefix with a single candidate is just that candidate; accept it instead.
    state.requested = Some((state.snapshot.input_version, Instant::now()));
    state.message(b"test;A;2");
    state.message(b"test;C;446f63756d656e74732f;");
    state.message(b"test;E");
    state.message(b"test;Z");
    assert!(!state.snapshot.extendable);
}

#[test]
fn ampersand_keeps_the_line_ready_and_ctrl_z_closes_it() {
    let mut state = state();
    state.message(b"test;P");
    state.input("make && ");
    assert!(state.snapshot.ready);
    state.input("\u{1a}");
    assert!(!state.snapshot.ready);
}
