use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn disabled_proxy_builds_as_an_inert_handle() -> anyhow::Result<()> {
    let state = build_config_state(
        NetworkProxyConfig::default(),
        NetworkProxyConstraints::default(),
    )?;
    let proxy = NetworkProxy::builder()
        .state(Arc::new(NetworkProxyState::with_reloader(
            state,
            Arc::new(TestReloader),
        )))
        .build()
        .await?;

    assert_eq!(proxy.http_addr(), "127.0.0.1:0".parse()?);
    assert_eq!(proxy.socks_addr(), "127.0.0.1:0".parse()?);
    proxy.run().await?.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn enabled_proxy_fails_closed() -> anyhow::Result<()> {
    let state = build_config_state(
        NetworkProxyConfig {
            enabled: true,
            ..NetworkProxyConfig::default()
        },
        NetworkProxyConstraints::default(),
    )?;
    let error = NetworkProxy::builder()
        .state(Arc::new(NetworkProxyState::with_reloader(
            state,
            Arc::new(TestReloader),
        )))
        .build()
        .await
        .expect_err("enabled managed proxy must fail closed");

    assert_eq!(error.to_string(), DISABLED_MESSAGE);
    Ok(())
}

struct TestReloader;

impl ConfigReloader for TestReloader {
    fn source_label(&self) -> String {
        "test".to_string()
    }

    fn maybe_reload(&self) -> ConfigReloaderFuture<'_, Option<ConfigState>> {
        Box::pin(async { Ok(None) })
    }

    fn reload_now(&self) -> ConfigReloaderFuture<'_, ConfigState> {
        Box::pin(async { anyhow::bail!("unused") })
    }
}
