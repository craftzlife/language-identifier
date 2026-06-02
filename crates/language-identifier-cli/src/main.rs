use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use language_identifier::{identify_lines_with, identify_with, IdentifyOptions};

#[cfg(feature = "ml-openlid")]
use language_identifier::OpenLidClassifier;

const USAGE: &str = "\
Usage:
  language-identifier [--pretty] [--ml-openlid <path>] <text>
  language-identifier [--pretty] [--ml-openlid <path>] -

  --pretty                Output indented JSON
  --ml-openlid <path>     Enable Layer 9 using the openlid-v3.bin model at <path>
                          (requires --features ml-openlid at build time)
  -                       Read one input per line from stdin and identify
                          them as an array
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

    let ml_path = match take_flag_value(&mut args, "--ml-openlid") {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };

    if args.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }

    #[cfg(feature = "ml-openlid")]
    let classifier = match ml_path.as_deref() {
        Some(p) => match OpenLidClassifier::builder().model_path(p).build() {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("--ml-openlid: {e}");
                return ExitCode::from(2);
            }
        },
        None => None,
    };

    #[cfg(not(feature = "ml-openlid"))]
    if ml_path.is_some() {
        eprintln!("--ml-openlid requires building with `--features ml-openlid`");
        return ExitCode::from(2);
    }

    let opts = IdentifyOptions {
        #[cfg(feature = "ml-openlid")]
        ml_classifier: classifier
            .as_ref()
            .map(|c| c as &dyn language_identifier::MlClassifier),
        #[cfg(not(feature = "ml-openlid"))]
        ml_classifier: None,
    };

    let result = if args.len() == 1 && args[0] == "-" {
        let stdin = io::stdin();
        let lines: Vec<String> = stdin.lock().lines().map_while(Result::ok).collect();
        identify_lines_with(&lines, &opts)
    } else {
        let text = args.join(" ");
        identify_with(&text, &opts)
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

fn take_flag_value(args: &mut Vec<String>, flag: &str) -> Result<Option<String>, String> {
    let Some(pos) = args.iter().position(|a| a == flag) else {
        return Ok(None);
    };
    if pos + 1 >= args.len() {
        return Err(format!("{flag} requires a value"));
    }
    args.remove(pos);
    Ok(Some(args.remove(pos)))
}
