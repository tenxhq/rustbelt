use anyhow::Result;
use libruskel::Ruskel;
pub async fn generate_skeleton(
    target: &str,
    features: &[String],
    all_features: bool,
    no_default_features: bool,
    private: bool,
) -> Result<String> {
    let ruskel = Ruskel::new();

    // Generate the skeleton
    let skeleton = ruskel
        .render(
            target,
            no_default_features,
            all_features,
            features.to_vec(),
            private,
        )
        .map_err(|e| anyhow::anyhow!("Ruskel error: {e}"))?;
    Ok(skeleton)
}
