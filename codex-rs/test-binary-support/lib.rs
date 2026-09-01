use std::path::Path;

use codex_arg0::Arg0DispatchPaths;
use codex_arg0::Arg0PathEntryGuard;
use codex_arg0::arg0_dispatch;
use tempfile::TempDir;

pub struct TestBinaryDispatchGuard {
    arg0: Arg0PathEntryGuard,
    _codex_home: TempDir,
    _previous_codex_home: Option<std::ffi::OsString>,
}

impl TestBinaryDispatchGuard {
    pub fn paths(&self) -> &Arg0DispatchPaths {
        self.arg0.paths()
    }
}

pub enum TestBinaryDispatchMode {
    DispatchArg0Only,
    Skip,
    InstallAliases,
}

pub fn configure_test_binary_dispatch<F>(
    codex_home_prefix: &str,
    classify: F,
) -> Option<TestBinaryDispatchGuard>
where
    F: FnOnce(&str, Option<&str>) -> TestBinaryDispatchMode,
{
    let mut args = std::env::args_os();
    let argv0 = args.next().unwrap_or_default();
    let exe_name = Path::new(&argv0)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let argv1 = args.next();
    match classify(exe_name, argv1.as_deref().and_then(|arg| arg.to_str())) {
        TestBinaryDispatchMode::DispatchArg0Only => {
            let _ = arg0_dispatch();
            None
        }
        TestBinaryDispatchMode::Skip => None,
        TestBinaryDispatchMode::InstallAliases => {
            let current_dir = match std::env::current_dir() {
                Ok(current_dir) => current_dir,
                Err(error) => panic!("failed to resolve test working directory: {error}"),
            };
            // Cargo places integration-test binaries in target/<profile>/deps. Keep each
            // test CODEX_HOME in the adjacent profile directory so nextest's per-test
            // homes stay within Cargo's disposable output tree. Other runners retain
            // the current working directory as their parent.
            let temp_parent = std::env::current_exe()
                .ok()
                .and_then(|current_exe| {
                    let deps_dir = current_exe.parent()?;
                    if !deps_dir.ends_with("deps") {
                        return None;
                    }
                    deps_dir.parent().map(Path::to_path_buf)
                })
                .unwrap_or(current_dir);
            let codex_home = match tempfile::Builder::new()
                .prefix(codex_home_prefix)
                .tempdir_in(temp_parent)
            {
                Ok(codex_home) => codex_home,
                Err(error) => panic!("failed to create test CODEX_HOME: {error}"),
            };
            let previous_codex_home = std::env::var_os("CODEX_HOME");
            // Safety: this runs from a test ctor before test threads begin.
            unsafe {
                std::env::set_var("CODEX_HOME", codex_home.path());
            }

            let arg0 = match arg0_dispatch() {
                Some(arg0) => arg0,
                None => panic!("failed to configure arg0 dispatch aliases for test binary"),
            };
            match previous_codex_home.as_ref() {
                Some(value) => unsafe {
                    std::env::set_var("CODEX_HOME", value);
                },
                None => unsafe {
                    std::env::remove_var("CODEX_HOME");
                },
            }

            Some(TestBinaryDispatchGuard {
                arg0,
                _codex_home: codex_home,
                _previous_codex_home: previous_codex_home,
            })
        }
    }
}
