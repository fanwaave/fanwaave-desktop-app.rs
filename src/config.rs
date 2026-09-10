#![forbid(unsafe_code)]

use std::{collections::BTreeMap, path::Path};

use fanwaave_lib_core::fanwaave_config::{
    parse_fanwaave_config, resolve_fanwaave_config, ConfigValue,
};
use flags2env::BundledFlags2Env;

pub type EnvMap = BTreeMap<String, String>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopConfig {
    pub api_base: String,
}

impl DesktopConfig {
    pub fn from_process() -> Result<Self, String> {
        Self::from_sources(
            std::env::args().collect(),
            std::env::vars().collect(),
            Path::new(".cli-flags.toml"),
            include_str!("../.fanwaave-cfg.toml"),
        )
    }

    pub fn from_sources(
        argv: Vec<String>,
        ambient: EnvMap,
        cli_contract: &Path,
        fanwaave_config_text: &str,
    ) -> Result<Self, String> {
        let cli_contract = cli_contract
            .to_str()
            .ok_or_else(|| "flags-2-env contract path is not valid UTF-8".to_owned())?;
        let parser = BundledFlags2Env::new();
        parser
            .audit_config(Some(cli_contract))
            .map_err(|_| "flags-2-env configuration audit failed".to_owned())?;
        let parsed = parser
            .parse_structured(&argv, Some(cli_contract))
            .map_err(|_| "flags-2-env parse failed".to_owned())?;
        if !parsed.unknown_options.is_empty() {
            return Err("unknown command-line option".to_owned());
        }
        if !parsed.errors.is_empty() {
            return Err("invalid command-line value".to_owned());
        }
        if !parsed.command.is_empty() {
            return Err("desktop executable does not accept subcommands".to_owned());
        }

        // `provided_flags` is argv-only. Do not merge the parser's default-bearing
        // `flags` map over the real process environment: the domain contract owns
        // non-secret defaults and the precedence is argv > environment > default.
        let argv_overrides: EnvMap = parsed.provided_flags.into_iter().collect();
        let fanwaave = parse_fanwaave_config(fanwaave_config_text)
            .map_err(|_| "Fanwaave configuration parse failed".to_owned())?;
        let resolved = resolve_fanwaave_config(&fanwaave, &ambient, &argv_overrides)
            .map_err(|_| "Fanwaave configuration resolution failed".to_owned())?;

        let api_base = match resolved.binding("api_base_url").map(|binding| binding.value()) {
            Some(ConfigValue::Url(value)) if !value.trim().is_empty() => value.clone(),
            Some(_) => return Err("Fanwaave api_base_url binding has the wrong type".to_owned()),
            None => return Err("Fanwaave api_base_url binding is unresolved".to_owned()),
        };
        Ok(Self { api_base })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".cli-flags.toml")
    }

    const DOMAIN_CONFIG: &str = include_str!("../.fanwaave-cfg.toml");

    #[test]
    fn domain_default_is_used_when_argv_and_environment_are_silent() {
        let config = DesktopConfig::from_sources(
            vec!["fanwaave-desktop".into()],
            EnvMap::new(),
            &contract_path(),
            DOMAIN_CONFIG,
        )
        .expect("config resolves");
        assert_eq!(config.api_base, "http://127.0.0.1:8080");
    }

    #[test]
    fn environment_overrides_domain_default() {
        let config = DesktopConfig::from_sources(
            vec!["fanwaave-desktop".into()],
            EnvMap::from([(
                "FANWAAVE_API_BASE".into(),
                "https://ambient.example".into(),
            )]),
            &contract_path(),
            DOMAIN_CONFIG,
        )
        .expect("config resolves");
        assert_eq!(config.api_base, "https://ambient.example");
    }

    #[test]
    fn argv_overrides_environment_without_mutating_process_environment() {
        let before = std::env::var_os("FANWAAVE_API_BASE");
        let config = DesktopConfig::from_sources(
            vec![
                "fanwaave-desktop".into(),
                "--api-base=https://argv.example".into(),
            ],
            EnvMap::from([(
                "FANWAAVE_API_BASE".into(),
                "https://ambient.example".into(),
            )]),
            &contract_path(),
            DOMAIN_CONFIG,
        )
        .expect("config resolves");
        assert_eq!(config.api_base, "https://argv.example");
        assert_eq!(std::env::var_os("FANWAAVE_API_BASE"), before);
    }

    #[test]
    fn unknown_options_fail_closed_without_echoing_the_token() {
        let secret_like = "--unknown=DO_NOT_ECHO_SENTINEL";
        let error = DesktopConfig::from_sources(
            vec!["fanwaave-desktop".into(), secret_like.into()],
            EnvMap::new(),
            &contract_path(),
            DOMAIN_CONFIG,
        )
        .expect_err("unknown option must fail");
        assert!(!error.contains("DO_NOT_ECHO_SENTINEL"));
    }
}
