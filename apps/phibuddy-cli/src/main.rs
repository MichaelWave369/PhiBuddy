//! PhiBuddy v0.2 CLI: local-only Ollama provider and three-agent collaboration.
//! No tool execution, background agents, shell access, or automatic GitHub writes.

use clap::{Parser, Subcommand};
use phibuddy_core::{
    agents::{Agent, Mode},
    engine::{CouncilDepth, Engine},
    evaluation::{evaluate_case, EvalOrder, EvalSuite},
    store::Store,
    Message, ModelProvider, Result,
};
use reqwest::{blocking::Client, Url};
use serde::Deserialize;
use serde_json::json;
use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

#[derive(Parser, Debug)]
#[command(
    name = "phibuddy",
    version,
    about = "PhiBuddy: Buddy, Homie & Dude, local-first AI collaborators"
)]
struct Cli {
    #[arg(
        long,
        global = true,
        help = "SQLite database file (defaults to your local user-data directory)"
    )]
    db: Option<PathBuf>,
    #[arg(long, global = true, default_value = "qwen3:4b")]
    model: String,
    #[arg(long, global = true, default_value = "http://127.0.0.1:11434")]
    endpoint: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Start interactive chat or resume an existing session.
    Chat {
        #[arg(long, default_value = "solo")]
        mode: String,
        #[arg(long, default_value = "buddy")]
        agent: String,
        #[arg(
            long,
            help = "Resume existing session ID; stored mode and agent take precedence"
        )]
        session: Option<String>,
        #[arg(
            long,
            default_value = "quick",
            help = "Council depth: quick (4 calls) or debate (7 calls)"
        )]
        depth: String,
        #[arg(
            long,
            help = "Print each actual contribution before the final synthesis"
        )]
        show_contributions: bool,
    },
    /// Show locally saved sessions (no Ollama connection required).
    Sessions,
    /// Show all messages including per-agent draft contributions.
    History { session: String },
    /// Inspect append-only events (including partial or failed Council sessions).
    Events {
        session: String,
        #[arg(long, help = "Print machine-readable JSONL for the session")]
        jsonl: bool,
    },
    /// Check that the configured Ollama service is reachable.
    Check,
    /// Run the same prompts through Solo and Council into a new local JSONL report.
    Eval {
        #[arg(long)]
        cases: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = "quick")]
        depth: String,
    },
    /// Explicit user-managed notes; never automatically extracted by an agent.
    Memory {
        #[command(subcommand)]
        action: MemoryCommand,
    },
}

#[derive(Subcommand, Debug)]
enum MemoryCommand {
    /// Save an explicit note. Source labels are user-supplied, not verified references.
    Add {
        #[arg(long)]
        text: String,
        #[arg(long)]
        source_ref: Option<String>,
    },
    /// List recent notes (up to 200).
    List {
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Literal substring search. No embeddings or automatic model retrieval.
    Search {
        query: String,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Delete a note from the active SQLite table by ID.
    Forget { id: String },
}

struct Ollama {
    endpoint: Url,
    model: String,
    client: Client,
}

#[derive(Deserialize)]
struct OllamaReply {
    message: OllamaMessage,
}
#[derive(Deserialize)]
struct OllamaMessage {
    content: String,
}

impl Ollama {
    fn new(endpoint: &str, model: String) -> Result<Self> {
        let endpoint = Url::parse(endpoint)?;
        // Network access is deliberately limited to the same machine in v0.2.
        if endpoint.scheme() != "http"
            || !matches!(
                endpoint.host_str(),
                Some("localhost" | "127.0.0.1" | "::1" | "[::1]")
            )
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.path() != "/"
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(
                "v0.2 accepts only a local HTTP endpoint such as http://127.0.0.1:11434".into(),
            );
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(180))
            .build()?;
        Ok(Self {
            endpoint,
            model,
            client,
        })
    }

    fn check(&self) -> Result<()> {
        let url = self.endpoint.join("api/tags")?;
        let response = self.client.get(url).send()?.error_for_status()?;
        let tags: serde_json::Value = response.json()?;
        if tags.get("models").and_then(|m| m.as_array()).is_none() {
            return Err("Ollama returned an unexpected /api/tags response".into());
        }
        Ok(())
    }
}

impl ModelProvider for Ollama {
    fn complete(&self, messages: &[Message]) -> Result<String> {
        let input: Vec<_> = messages
            .iter()
            .map(|m| json!({"role": m.role, "content": m.content}))
            .collect();
        let reply: OllamaReply = self
            .client
            .post(self.endpoint.join("api/chat")?)
            .json(&json!({"model": self.model, "messages": input, "stream": false}))
            .send()?
            .error_for_status()?
            .json()?;
        if reply.message.content.trim().is_empty() {
            return Err("Ollama generated an empty response".into());
        }
        Ok(reply.message.content)
    }
}

fn default_db() -> PathBuf {
    if let Some(dir) = env::var_os("LOCALAPPDATA") {
        return PathBuf::from(dir).join("PhiBuddy").join("buddy.sqlite3");
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("phibuddy")
            .join("buddy.sqlite3");
    }
    PathBuf::from("phibuddy.sqlite3")
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Command::Eval {
        cases,
        output,
        depth,
    } = &cli.command
    {
        let suite: EvalSuite = serde_json::from_str(&fs::read_to_string(cases)?)?;
        suite.validate()?;
        let depth = CouncilDepth::from_str(depth)?;
        if cli.model.to_ascii_lowercase().contains("cloud") {
            return Err(
                "Council Lab requires a local Ollama model name; cloud tags are disabled".into(),
            );
        }
        // Evaluation never writes test conversations into the user's history DB.
        let provider = Ollama::new(&cli.endpoint, cli.model.clone())?;
        let mut lab = Engine::new(provider, Store::in_memory()?);
        // An existing report is never silently overwritten. If Ollama fails midway,
        // completed JSONL case records remain recoverable.
        let mut writer = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?;
        for (index, case) in suite.cases.iter().enumerate() {
            let order = if index % 2 == 0 {
                EvalOrder::SoloFirst
            } else {
                EvalOrder::CouncilFirst
            };
            let record = evaluate_case(&mut lab, case, depth, order)?;
            let row = json!({
                "format_version": 1,
                "model": &cli.model,
                "evaluation": record,
            });
            writeln!(writer, "{}", serde_json::to_string(&row)?)?;
            writer.flush()?;
            println!("Completed {}/{}: {}", index + 1, suite.cases.len(), case.id);
        }
        println!("Saved local comparison report: {}", output.display());
        return Ok(());
    }

    let provider = Ollama::new(&cli.endpoint, cli.model)?;
    let store = Store::open(cli.db.unwrap_or_else(default_db))?;
    let mut engine = Engine::new(provider, store);

    match cli.command {
        Command::Eval { .. } => unreachable!("eval handled before persistent store initialization"),
        Command::Memory { action } => match action {
            MemoryCommand::Add { text, source_ref } => {
                let entry = engine
                    .store()
                    .remember_manual(&text, source_ref.as_deref())?;
                println!("{}", serde_json::to_string_pretty(&entry)?);
            }
            MemoryCommand::List { limit } => {
                let notes = engine.store().list_memories(limit)?;
                println!("{}", serde_json::to_string_pretty(&notes)?);
            }
            MemoryCommand::Search { query, limit } => {
                let notes = engine.store().search_memories(&query, limit)?;
                println!("{}", serde_json::to_string_pretty(&notes)?);
            }
            MemoryCommand::Forget { id } => {
                let deleted = engine.store().forget_memory(&id)?;
                if deleted {
                    println!("Deleted memory note {} from the active database.", id);
                } else {
                    println!("No active memory note matches {}.", id);
                }
            }
        },
        Command::Check => {
            engine.provider().check()?;
            println!("Ollama is reachable on the local machine.");
        }
        Command::Sessions => {
            for s in engine.store().list_sessions()? {
                println!("{}  {}  {}  {}", s.id, s.mode, s.agent, s.created_at);
            }
        }
        Command::History { session } => {
            if engine.store().get_session(&session)?.is_none() {
                return Err("session ID not found".into());
            }
            for m in engine.store().messages(&session)? {
                let actor = m.actor.unwrap_or_else(|| "user".to_owned());
                println!("\n[{} / {}]\n{}", m.role, actor, m.content);
            }
        }
        Command::Events { session, jsonl } => {
            if engine.store().get_session(&session)?.is_none() {
                return Err("session ID not found".into());
            }
            for event in engine.store().events(&session)? {
                if jsonl {
                    println!("{}", serde_json::to_string(&event)?);
                } else {
                    println!(
                        "\n[{} / {} / {} / {}]\n{}",
                        event.sequence,
                        event.turn_id,
                        event.kind.as_str(),
                        event.actor.as_deref().unwrap_or("user"),
                        event.content
                    );
                }
            }
        }
        Command::Chat {
            mode,
            agent,
            session,
            depth,
            show_contributions,
        } => {
            let depth = CouncilDepth::from_str(&depth)?;
            let session = match session {
                Some(id) => engine
                    .store()
                    .get_session(&id)?
                    .ok_or("session ID not found")?,
                None => engine
                    .store()
                    .create_session(Mode::from_str(&mode)?, Agent::from_str(&agent)?)?,
            };
            println!(
                "PhiBuddy v{} | session={} | mode={} | agent={}",
                env!("CARGO_PKG_VERSION"),
                session.id,
                session.mode,
                session.agent
            );
            println!("Local inference only. Type :exit to quit, :help for commands.\n");
            loop {
                print!("you> ");
                io::stdout().flush()?;
                let mut line = String::new();
                if io::stdin().read_line(&mut line)? == 0 {
                    break;
                }
                let line = line.trim();
                match line {
                    ":exit" | ":quit" => break,
                    ":help" => {
                        println!(":exit | :quit | :help | :id | :paste (finish with :end)");
                        continue;
                    }
                    ":id" => {
                        println!("{}", session.id);
                        continue;
                    }
                    "" => continue,
                    _ => {}
                }
                let request = if line == ":paste" {
                    println!(
                        "Paste your multi-line request. Enter :end on its own line to submit."
                    );
                    let mut whole = String::new();
                    loop {
                        let mut extra = String::new();
                        if io::stdin().read_line(&mut extra)? == 0 || extra.trim() == ":end" {
                            break;
                        }
                        whole.push_str(&extra);
                        if whole.chars().count() > 20_000 {
                            println!("Message exceeds the 20,000-character limit; cancelled.");
                            whole.clear();
                            break;
                        }
                    }
                    whole
                } else {
                    line.to_owned()
                };
                if request.trim().is_empty() {
                    continue;
                }
                match engine.run_with_depth(&session, &request, depth) {
                    Ok(turn) => {
                        if show_contributions {
                            for c in &turn.contributions {
                                println!("\n[{} / {}]\n{}", c.agent, c.phase.as_str(), c.text);
                            }
                        }
                        println!("\ncrew> {}\n\n[turn id: {}]", turn.answer, turn.turn_id);
                    },
                    Err(err) => eprintln!("Response failed: {err}. No completed chat turn was saved; see the session event log for partial work."),
                }
            }
        }
    }
    Ok(())
}
