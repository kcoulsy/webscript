mod client;
mod db;
mod db_runtime;
mod model_runtime;
mod schema;
mod schema_runtime;
mod debugbar;
mod dev;
mod diagnostic;
mod expr;
mod parser;
mod project;
mod render;
mod runtime;
mod server;
mod stmt;
mod validate;

use std::env;
use std::path::Path;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".to_string());

    match command.as_str() {
        "new" => {
            let name = args
                .next()
                .ok_or_else(|| "usage: web new <project-name>".to_string())?;
            project::create_project(Path::new(&name))
        }
        "routes" => {
            let root = env::current_dir().map_err(|error| error.to_string())?;
            match project::discover_routes(&root) {
                Ok(routes) => {
                    if routes.is_empty() {
                        println!("No routes found.");
                    } else {
                        for route in routes {
                            println!("GET   {:<24} {}", route.path, route.file.display());
                        }
                    }
                    Ok(())
                }
                Err(diagnostic) => {
                    diagnostic.report_stderr();
                    Err("routes failed".to_string())
                }
            }
        }
        "check" => {
            let root = env::current_dir().map_err(|error| error.to_string())?;
            let diagnostics = project::check_project(&root)?;
            if diagnostics.is_empty() {
                println!("Project OK");
                Ok(())
            } else {
                diagnostic::render_all(&diagnostics);
                Err("check failed".to_string())
            }
        }
        "db:generate" => {
            let root = env::current_dir().map_err(|error| error.to_string())?;
            let name = args.next();
            match db::generate(&root, name.as_deref()) {
                Ok(outcome) => {
                    if let Some(migration) = outcome.migration {
                        println!("Created {}", migration.display());
                    } else {
                        println!("No schema changes.");
                    }
                    Ok(())
                }
                Err(db::ModelLoadError::Diagnostic(diagnostic)) => {
                    diagnostic.report_stderr();
                    Err("db:generate failed".to_string())
                }
                Err(db::ModelLoadError::Io(error)) => Err(error),
            }
        }
        "db:migrate" => {
            let root = env::current_dir().map_err(|error| error.to_string())?;
            let outcome = db::migrate(&root)?;
            if outcome.applied.is_empty() {
                println!("No pending migrations.");
            } else {
                for migration in outcome.applied {
                    println!("Applied {migration}");
                }
            }
            Ok(())
        }
        "serve" => {
            let root = env::current_dir().map_err(|error| error.to_string())?;
            let options = ServeOptions::from_args(args.collect())?;
            let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
            runtime.block_on(server::serve(root, options.host, options.port))
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(format!(
            "unknown command `{other}`\n\nRun `web help` for usage."
        )),
    }
}

struct ServeOptions {
    host: String,
    port: u16,
}

impl ServeOptions {
    fn from_args(args: Vec<String>) -> Result<Self, String> {
        let mut host = "127.0.0.1".to_string();
        let mut port = 3000;
        let mut index = 0;

        while index < args.len() {
            match args[index].as_str() {
                "--host" => {
                    index += 1;
                    host = args
                        .get(index)
                        .ok_or_else(|| "--host requires a value".to_string())?
                        .clone();
                }
                "--port" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| "--port requires a value".to_string())?;
                    port = value
                        .parse()
                        .map_err(|_| format!("invalid port `{value}`"))?;
                }
                value => return Err(format!("unknown serve option `{value}`")),
            }
            index += 1;
        }

        Ok(Self { host, port })
    }
}

fn print_help() {
    println!(
        "WebScript MVP\n\nCommands:\n  web new <name>              Create a starter project\n  web serve [--port 3000]     Start the local dev server\n  web routes                  Print discovered routes\n  web check                   Parse and validate .web files\n  web db:generate [name]      Generate a SQL migration from app/models\n  web db:migrate              Apply pending SQL migrations\n"
    );
}
