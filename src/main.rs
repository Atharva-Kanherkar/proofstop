use anyhow::{Result, ensure};
use clap::{Parser, Subcommand, ValueEnum};
use proofstop::{
    online, repair, run, sandbox,
    task::{Config, Task},
};
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(
    version,
    about = "ProofStop research controller: pinned tasks, isolated verification, and baseline accounting"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, ValueEnum)]
enum Suite {
    Acceptance,
    Evaluation,
    Both,
}
#[derive(Clone, ValueEnum)]
enum Evidence {
    Unclassified,
    HarnessValidation,
}

#[derive(Subcommand)]
enum Commands {
    /// Cheap direct answer, then at most one reasoning-enabled acceptance-feedback repair.
    Repair {
        #[arg(long, default_value = "tasks/merge-ranges")]
        task: PathBuf,
        #[arg(long, default_value = "configs/gpu-nonthinking.json")]
        direct_config: PathBuf,
        #[arg(long, default_value = "configs/gpu-thinking.json")]
        repair_config: PathBuf,
        #[arg(long, default_value = "http://127.0.0.1:8000/v1")]
        endpoint: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, value_enum, default_value = "unclassified")]
        evidence_class: Evidence,
    },
    /// Experimental Qwen3/vLLM token-prefix continuation with optional probes.
    Online {
        #[arg(long, default_value = "tasks/merge-ranges")]
        task: PathBuf,
        #[arg(long, default_value = "configs/gpu-thinking.json")]
        config: PathBuf,
        #[arg(long, default_value = "http://127.0.0.1:8000/v1")]
        endpoint: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 128)]
        segment_tokens: u32,
        #[arg(long, default_value_t = 1024)]
        probe_tokens: u32,
        #[arg(long, default_value_t = 4)]
        max_probes: u32,
        #[arg(long, value_enum, default_value = "unclassified")]
        evidence_class: Evidence,
    },
    /// Validate a pinned task and inspect the exact model-visible prompt.
    Inspect {
        #[arg(long, default_value = "tasks/merge-ranges")]
        task: PathBuf,
        #[arg(long)]
        show_prompt: bool,
    },
    /// Execute a replacement file inside a bounded Docker container.
    Verify {
        #[arg(long, default_value = "tasks/merge-ranges")]
        task: PathBuf,
        #[arg(long, default_value = "configs/local.json")]
        config: PathBuf,
        #[arg(long)]
        candidate: PathBuf,
        #[arg(long, value_enum, default_value = "both")]
        suite: Suite,
    },
    /// Run one baseline generation. This command does not implement early exit.
    Baseline {
        #[arg(long, default_value = "tasks/merge-ranges")]
        task: PathBuf,
        #[arg(long, default_value = "configs/local.json")]
        config: PathBuf,
        #[arg(long, default_value = "http://127.0.0.1:8000/v1")]
        endpoint: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, value_enum, default_value = "unclassified")]
        evidence_class: Evidence,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Repair {
            task,
            direct_config,
            repair_config,
            endpoint,
            model,
            output,
            evidence_class,
        } => {
            let task = Task::load(&task)?;
            let direct = Config::load(&direct_config)?;
            let thinking = Config::load(&repair_config)?;
            let evidence = match evidence_class {
                Evidence::Unclassified => "unclassified",
                Evidence::HarnessValidation => "harness_validation",
            };
            let summary = repair::run(
                &task, &direct, &thinking, &endpoint, &model, &output, evidence,
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            ensure!(
                summary.outcome == "passed",
                "repair run did not pass; inspect artifacts"
            );
        }
        Commands::Online {
            task,
            config,
            endpoint,
            model,
            output,
            segment_tokens,
            probe_tokens,
            max_probes,
            evidence_class,
        } => {
            let task = Task::load(&task)?;
            let config = Config::load(&config)?;
            let options = online::Options {
                segment_tokens,
                probe_tokens,
                max_probes,
            };
            let evidence = match evidence_class {
                Evidence::Unclassified => "unclassified",
                Evidence::HarnessValidation => "harness_validation",
            };
            let summary = online::run(
                &task, &config, &endpoint, &model, &output, &options, evidence,
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            ensure!(
                summary.outcome == "passed",
                "online run did not pass; inspect artifacts"
            );
        }
        Commands::Inspect { task, show_prompt } => {
            let task = Task::load(&task)?;
            println!(
                "{} / {}: pinned fixture validated",
                task.manifest.id, task.manifest.revision
            );
            if show_prompt {
                println!("{}", task.prompt()?);
            }
        }
        Commands::Verify {
            task,
            config,
            candidate,
            suite,
        } => {
            let task = Task::load(&task)?;
            let config = Config::load(&config)?;
            let code = fs::read_to_string(candidate)?;
            let suites = match suite {
                Suite::Acceptance => vec!["acceptance"],
                Suite::Evaluation => vec!["evaluation"],
                Suite::Both => vec!["acceptance", "evaluation"],
            };
            let mut passed = true;
            for suite in suites {
                let result = sandbox::verify_task(&code, &task, suite, &config).await?;
                passed &= matches!(result.verdict, sandbox::Verdict::Passed);
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &serde_json::json!({"suite":suite,"verification":result})
                    )?
                );
            }
            ensure!(passed, "candidate did not pass all selected checks");
        }
        Commands::Baseline {
            task,
            config,
            endpoint,
            model,
            output,
            evidence_class,
        } => {
            let task = Task::load(&task)?;
            let config = Config::load(&config)?;
            let evidence = match evidence_class {
                Evidence::Unclassified => "unclassified",
                Evidence::HarnessValidation => "harness_validation",
            };
            let summary =
                run::baseline(&task, &config, &endpoint, &model, &output, evidence).await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            ensure!(
                summary.outcome == "passed",
                "baseline did not pass; inspect run artifacts"
            );
        }
    }
    Ok(())
}
