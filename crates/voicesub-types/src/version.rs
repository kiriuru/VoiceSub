/// Product version — single source until crate-only policy is enforced.
pub const PROJECT_VERSION: &str = "0.5.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_version_matches_voice_sub_line() {
        assert_eq!(PROJECT_VERSION, "0.5.0");
    }
}
