use browsai_agent_tree::AgentRenderTree;
use browsai_state::{PageState, SnapshotError};
use url::Url;

#[test]
fn snapshot_pages_are_bounded_and_reject_stale_generations() {
    let state = PageState {
        url: Url::parse("https://example.test/").unwrap(),
        tree: AgentRenderTree::new_page("Example"),
        generation: 1,
    };
    let page = state.page(0, 1, Some(1), Some(1)).unwrap_err();
    assert_eq!(page, SnapshotError::Stale);
    let page = state.page(0, 1, Some(100), Some(0)).unwrap();
    assert_eq!(page.offset, 0);
    assert_eq!(page.cursor, 0);
    assert_eq!(page.limit, 1);
    assert!(!page.truncated);
    assert_eq!(page.next_cursor, None);
}
