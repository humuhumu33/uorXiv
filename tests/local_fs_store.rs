use tempfile::tempdir;
use uor_xiv::ContentStore;
use uor_xiv::ipld::WorkspaceRoot;
use uor_xiv::store::{LocalFsStore, LOCAL_BLOB_PREFIX, LOCAL_DAG_PREFIX};
use uor_xiv::workspace::{
    fork_workspace, load_workspace, merge_workspaces, put_entry, save_workspace, MergeStrategy,
};

#[test]
fn local_fs_persists_across_reopen() {
    let dir = tempdir().unwrap();
    let path = dir.path().to_path_buf();

    let s1 = LocalFsStore::open(&path).unwrap();
    let cid = s1.add_blob(b"doc v1").unwrap();
    assert!(cid.starts_with(LOCAL_BLOB_PREFIX));

    let s2 = LocalFsStore::open(&path).unwrap();
    assert_eq!(s2.cat_blob(&cid).unwrap(), b"doc v1");
}

#[test]
fn local_fs_workspace_fork_merge_flow() {
    let dir = tempdir().unwrap();
    let s = LocalFsStore::open(dir.path()).unwrap();

    let doc = s.add_blob(b"abstract...").unwrap();
    let w = put_entry(WorkspaceRoot::empty(), "paper".into(), doc);
    let r1 = save_workspace(&s, &w).unwrap();
    assert!(r1.starts_with(LOCAL_DAG_PREFIX));

    let forked = fork_workspace(&load_workspace(&s, &r1).unwrap(), &r1);
    let r_fork = save_workspace(&s, &forked).unwrap();
    assert_eq!(load_workspace(&s, &r_fork).unwrap().parents, vec![r1.clone()]);

    let code = s.add_blob(b"fn main()").unwrap();
    let w_other = put_entry(WorkspaceRoot::empty(), "code".into(), code);
    let r_other = save_workspace(&s, &w_other).unwrap();

    let merged = merge_workspaces(
        &load_workspace(&s, &r1).unwrap(),
        &r1,
        &load_workspace(&s, &r_other).unwrap(),
        &r_other,
        MergeStrategy::Strict,
    )
    .unwrap();
    let r_merged = save_workspace(&s, &merged).unwrap();

    let s2 = LocalFsStore::open(dir.path()).unwrap();
    let w_final = load_workspace(&s2, &r_merged).unwrap();
    assert!(w_final.entries.contains_key("paper"));
    assert!(w_final.entries.contains_key("code"));
}
