use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use language_identifier::{identify, identify_lines};

const USAGE: &str = "\
Usage:
  language-identifier [--pretty] <text>
  language-identifier [--pretty] -

  --pretty    Output indented JSON
  -           Read one input per line from stdin and identify them as an array
";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }

    let mut pretty = false;
    args.retain(|a| {
        if a == "--pretty" {
            pretty = true;
            false
        } else {
            true
        }
    });

    if args.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }

    let result = if args.len() == 1 && args[0] == "-" {
        let stdin = io::stdin();
        let lines: Vec<String> = stdin
            .lock()
            .lines()
            .filter_map(Result::ok)
            .collect();
        identify_lines(&lines)
    } else {
        let text = args.join(" ");
        identify(&text)
    };

    let serialized = if pretty {
        serde_json::to_string_pretty(&result)
    } else {
        serde_json::to_string(&result)
    };

    match serialized {
        Ok(s) => {
            let mut stdout = io::stdout().lock();
            let _ = writeln!(stdout, "{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Serialization error: {e}");
            ExitCode::FAILURE
        }
    }
}
