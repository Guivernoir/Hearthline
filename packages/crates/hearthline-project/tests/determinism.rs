use std::fs;
use std::path::{Path, PathBuf};

use hearthline_project::ProjectCompiler;
use walkdir::WalkDir;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn copied_repository(reverse: bool, crlf: bool) -> tempfile::TempDir {
    let source = repository_root();
    let target = tempfile::tempdir().expect("temporary deterministic project");
    for relative in ["project/config", "project/control"] {
        copy_tree(
            &source.join(relative),
            &target.path().join(relative),
            reverse,
            crlf,
        );
    }
    target
}

fn copy_tree(source: &Path, destination: &Path, reverse: bool, crlf: bool) {
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
        if crlf {
            let source = fs::read_to_string(path).expect("text model source");
            fs::write(target, source.replace('\n', "\r\n")).expect("write CRLF source");
        } else {
            fs::copy(path, target).expect("copy source");
        }
    }
}

#[test]
fn source_creation_and_traversal_order_do_not_change_lock_or_project_digest() {
    let repository = repository_root();
    let forward = copied_repository(false, false);
    let reverse = copied_repository(true, false);
    let windows_newlines = copied_repository(false, true);
    let first = ProjectCompiler::new(forward.path().join("project/config"))
        .compile("determinism test")
        .expect("forward project");
    let second = ProjectCompiler::new(reverse.path().join("project/config"))
        .compile("determinism test")
        .expect("reverse project");
    let third = ProjectCompiler::new(windows_newlines.path().join("project/config"))
        .compile("determinism test")
        .expect("CRLF project");

    assert_eq!(first.digest(), second.digest());
    assert_eq!(first.model_lock(), second.model_lock());
    assert_eq!(first.digest(), third.digest());
    assert_eq!(
        first.model_lock().first_difference(third.model_lock()),
        None
    );

    for relative in [
        "project/config/model.lock.json",
        "packages/web/src/generated/appliance-configs.json",
        "packages/web/src/generated/process-view.json",
    ] {
        let source = fs::read_to_string(repository.join(relative)).expect("locked project source");
        let destination = windows_newlines.path().join(relative);
        fs::create_dir_all(destination.parent().expect("locked source parent"))
            .expect("locked source directory");
        fs::write(destination, source.replace('\n', "\r\n")).expect("write locked CRLF source");
    }
    let locked = ProjectCompiler::new(windows_newlines.path().join("project/config"))
        .compile_locked()
        .expect("CRLF locked project");
    assert_eq!(first.digest(), locked.digest());
}
