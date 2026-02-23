#![cfg(feature = "cli")]

use std::env;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{ArgAction, Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::scaffold::validate::{
    ComponentName, ValidationError, ensure_path_available, normalize_version,
};
use crate::wizard::{self, WizardRequest};

#[derive(Subcommand, Debug, Clone)]
pub enum WizardCommand {
    /// Generate a component@0.6.0 template scaffold
    New(WizardNewArgs),
    /// Emit wizard QA spec for a mode
    Spec(WizardSpecArgs),
}

#[derive(Args, Debug, Clone)]
pub struct WizardNewArgs {
    /// Component name (kebab-or-snake case)
    #[arg(value_name = "name")]
    pub name: String,
    /// ABI version to target (template is fixed to 0.6.0 for now)
    #[arg(long = "abi-version", default_value = "0.6.0", value_name = "semver")]
    pub abi_version: String,
    /// QA mode to prefill when --answers is provided
    #[arg(long = "mode", value_enum, default_value = "default")]
    pub mode: WizardMode,
    /// Answers JSON to prefill QA setup
    #[arg(long = "answers", value_name = "answers.json")]
    pub answers: Option<PathBuf>,
    /// Output directory (template will be created under <out>/<name>)
    #[arg(long = "out", value_name = "dir")]
    pub out: Option<PathBuf>,
    /// Required capabilities to embed in describe payload (repeatable)
    #[arg(
        long = "required-capability",
        value_name = "capability",
        action = ArgAction::Append
    )]
    pub required_capabilities: Vec<String>,
    /// Provided capabilities to embed in describe payload (repeatable)
    #[arg(
        long = "provided-capability",
        value_name = "capability",
        action = ArgAction::Append
    )]
    pub provided_capabilities: Vec<String>,
    /// Print deterministic plan as JSON and do not write files
    #[arg(long = "plan-json", default_value_t = false)]
    pub plan_json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct WizardSpecArgs {
    /// QA mode
    #[arg(long = "mode", value_enum, default_value = "default")]
    pub mode: WizardMode,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardMode {
    Default,
    Setup,
    Update,
    Remove,
}

impl From<WizardMode> for wizard::WizardMode {
    fn from(value: WizardMode) -> Self {
        match value {
            WizardMode::Default => wizard::WizardMode::Default,
            WizardMode::Setup => wizard::WizardMode::Setup,
            WizardMode::Update => wizard::WizardMode::Update,
            WizardMode::Remove => wizard::WizardMode::Remove,
        }
    }
}

pub fn run(command: WizardCommand) -> Result<()> {
    match command {
        WizardCommand::New(args) => run_new(args),
        WizardCommand::Spec(args) => run_spec(args),
    }
}

fn run_new(args: WizardNewArgs) -> Result<()> {
    let name = ComponentName::parse(&args.name)?;
    let abi_version = normalize_version(&args.abi_version)?;
    let target = resolve_out_path(&name, args.out.as_deref())?;
    ensure_path_available(&target)?;

    let answers = match args.answers.as_ref() {
        Some(path) => Some(wizard::load_answers_payload(path)?),
        None => None,
    };

    let request = WizardRequest {
        name: name.into_string(),
        abi_version,
        mode: args.mode.into(),
        target: target.clone(),
        answers,
        required_capabilities: args.required_capabilities,
        provided_capabilities: args.provided_capabilities,
    };

    // Keep apply side-effect-free, then execute plan as a separate phase.
    let result = wizard::apply_scaffold(request, true)?;
    for warning in result.warnings {
        eprintln!("{warning}");
    }
    if args.plan_json {
        print_json(&result.plan)?;
        return Ok(());
    }
    wizard::execute_plan(&result.plan)?;

    println!("wizard: created {}", target.display());
    Ok(())
}

fn run_spec(args: WizardSpecArgs) -> Result<()> {
    let spec = wizard::spec_scaffold(args.mode.into());
    print_json(&spec)
}

fn resolve_out_path(
    name: &ComponentName,
    out: Option<&Path>,
) -> std::result::Result<PathBuf, ValidationError> {
    if let Some(out) = out {
        let base = if out.is_absolute() {
            out.to_path_buf()
        } else {
            env::current_dir()
                .map_err(ValidationError::WorkingDir)?
                .join(out)
        };
        Ok(base.join(name.as_str()))
    } else {
        crate::scaffold::validate::resolve_target_path(name, None)
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    let encoded = serde_json::to_string_pretty(value)?;
    println!("{encoded}");
    Ok(())
}
