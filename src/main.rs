mod pen;
use color_eyre::eyre::Result;
use pen::*;
use stardust_xr_fusion::client::Client;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let (client, event_loop) = Client::connect_with_async_loop().await?;

    let pen = Pen::new(&client, PenSettings::default())?;
    let _wrapped_root = client.wrap_root(pen)?;

    tokio::select! {
        _ = tokio::signal::ctrl_c() => (),
        e = event_loop => e??,
    }
    Ok(())
}
