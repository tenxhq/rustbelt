use anyhow::Result;
use libruskel::Ruskel;
pub async fn generate_skeleton(
    target: &str,
    features: &[String],
    all_features: bool,
    no_default_features: bool,
    private: bool,
) -> Result<String> {
    let ruskel = Ruskel::new(target);

    // Apply feature flags if provided
    let mut ruskel = if no_default_features {
        ruskel.with_no_default_features(true)
    } else {
        ruskel
    };

    if all_features {
        ruskel = ruskel.with_all_features(true);
    } else if !features.is_empty() {
        ruskel = ruskel.with_features(features.to_vec());
    }

    // Generate the skeleton
    let skeleton = ruskel
        .render(private, false, false)
        .map_err(|e| anyhow::anyhow!("Ruskel error: {e}"))?;
    Ok(skeleton)
}
