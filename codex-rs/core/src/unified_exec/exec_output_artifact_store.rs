use std::path::Path;

use codex_exec_output_artifacts::ArtifactStore;
use codex_exec_output_artifacts::ArtifactStoreConfig;

pub(crate) fn initialize_exec_output_artifact_store(
    thread_extension_data: &codex_extension_api::ExtensionData,
    codex_home: &Path,
    thread_id: impl ToString,
) {
    match ArtifactStore::open(
        codex_home.join("exec-output-artifacts"),
        thread_id.to_string(),
        ArtifactStoreConfig::default(),
    ) {
        Ok(store) => {
            thread_extension_data.insert(store.clone());
            tokio::spawn(async move {
                let cleanup = tokio::task::spawn_blocking(move || store.cleanup_expired()).await;
                match cleanup {
                    Ok(Ok(_)) => {}
                    Ok(Err(err)) => {
                        tracing::warn!(
                            error = %err,
                            "failed to clean expired exec-output artifacts"
                        );
                    }
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            "exec-output artifact cleanup worker stopped unexpectedly"
                        );
                    }
                }
            });
        }
        Err(err) => {
            tracing::warn!(
                error = %err,
                "exec-output artifact storage is unavailable"
            );
        }
    }
}
