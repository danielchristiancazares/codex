use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;

use pretty_assertions::assert_eq;

use super::write_json;

#[test]
fn failed_publish_removes_temporary_credentials() {
    let home = tempfile::tempdir().expect("temporary credential home");
    let path = home.path().join("auth.json");
    fs::create_dir(&path).expect("block credential destination with a directory");

    write_json(&path, &serde_json::json!({"token": "secret"}))
        .expect_err("a directory cannot be replaced with credentials");

    let entries = fs::read_dir(home.path())
        .expect("read credential home")
        .map(|entry| entry.expect("directory entry").file_name())
        .collect::<Vec<_>>();
    assert_eq!(entries, vec![OsString::from("auth.json")]);
}

#[test]
fn serialization_failure_leaves_no_credential_files() {
    let home = tempfile::tempdir().expect("temporary credential home");
    let path = home.path().join("private").join("auth.json");
    let invalid_json = BTreeMap::from([((1, 2), "secret")]);

    write_json(&path, &invalid_json).expect_err("JSON object keys must be strings");

    assert_eq!(
        fs::read_dir(home.path())
            .expect("read credential home")
            .count(),
        0
    );
}
