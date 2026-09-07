use std::fs;
use std::path::{Path, PathBuf};

use hearthline_project::ProjectCompiler;
use walkdir::WalkDir;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn copied_repository(reverse: bool) -> tempfile::TempDir {
    let source = repository_root();
    let target = tempfile::tempdir().expect("temporary deterministic project");
    for relative in ["project/config", "project/control"] {
        copy_tree(
            &source.join(relative),
            &target.path().join(relative),
            reverse,
        );
    }
    target
}

fn copy_tree(source: &Path, destination: &Path, reverse: bool) {
    let mut files = WalkDir::new(source)
        .into_iter()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some("model.lock.json"))
        .collect::<Vec<_>>();
    files.sort();
    if reverse {
        files.reverse();
    }
    for path in files {
        let relative = path.strip_prefix(source).expect("relative source");
        let target = destination.join(relative);
        fs::create_dir_all(target.parent().expect("target parent")).expect("target directory");
        fs::copy(path, target).expect("copy source");
    }
}

#[test]
fn source_creation_and_traversal_order_do_not_change_lock_or_project_digest() {
    let forward = copied_repository(false);
    let reverse = copied_repository(true);
    let first = ProjectCompiler::new(forward.path().join("project/config"))
        .compile("determinism test")
        .expect("forward project");
    let second = ProjectCompiler::new(reverse.path().join("project/config"))
        .compile("determinism test")
        .expect("reverse project");

    assert_eq!(first.digest(), second.digest());
    assert_eq!(first.model_lock(), second.model_lock());
}
