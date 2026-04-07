use uor_xiv::ipld::WorkspaceRoot;
use uor_xiv::store::MemoryStore;
use uor_xiv::workspace::{
    fork_workspace, load_workspace, merge_workspaces, put_entry, save_workspace, MergeStrategy,
};

#[test]
fn memory_roundtrip_fork_merge() {
    let s = MemoryStore::default();
    let w = WorkspaceRoot::empty();
    let r0 = save_workspace(&s, &w).unwrap();

    let w0 = load_workspace(&s, &r0).unwrap();
    let w1 = put_entry(w0.clone(), "a".into(), "cid-a".into());
    let r1 = save_workspace(&s, &w1).unwrap();

    let forked = fork_workspace(&w1, &r1);
    let r_fork = save_workspace(&s, &forked).unwrap();
    let wf = load_workspace(&s, &r_fork).unwrap();
    assert_eq!(wf.parents, vec![r1.clone()]);
    assert_eq!(wf.entries.get("a").map(String::as_str), Some("cid-a"));

    let mut other = WorkspaceRoot::empty();
    other.entries.insert("a".into(), "cid-a".into());
    other.entries.insert("b".into(), "cid-b".into());
    let r_other = save_workspace(&s, &other).unwrap();

    let merged = merge_workspaces(&w1, &r1, &other, &r_other, MergeStrategy::Strict).unwrap();
    assert_eq!(merged.entries.get("b").map(String::as_str), Some("cid-b"));

    let conflict_left = put_entry(WorkspaceRoot::empty(), "x".into(), "left".into());
    let r_left = save_workspace(&s, &conflict_left).unwrap();
    let wl = load_workspace(&s, &r_left).unwrap();

    let conflict_right = put_entry(WorkspaceRoot::empty(), "x".into(), "right".into());
    let r_right = save_workspace(&s, &conflict_right).unwrap();
    let wr = load_workspace(&s, &r_right).unwrap();

    assert!(merge_workspaces(&wl, &r_left, &wr, &r_right, MergeStrategy::Strict).is_err());
    let m_ours = merge_workspaces(&wl, &r_left, &wr, &r_right, MergeStrategy::Ours).unwrap();
    assert_eq!(m_ours.entries.get("x").map(String::as_str), Some("left"));
    let m_theirs = merge_workspaces(&wl, &r_left, &wr, &r_right, MergeStrategy::Theirs).unwrap();
    assert_eq!(m_theirs.entries.get("x").map(String::as_str), Some("right"));
}
