use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use language_identifier::{identify_lines_with, identify_with, IdentifyOptions};

#[cfg(feature = "ml-fasttext")]
use language_identifier::FastTextClassifier;

const USAGE: &str = "\
Usage:
  language-identifier [--pretty] [--ml-fasttext <path>] <text>
  language-identifier [--pretty] [--ml-fasttext <path>] -

  --pretty                Output indented JSON
  --ml-fasttext <path>    Enable Layer 9 using the lid.176.bin model at <path>
                          (requires --features ml-fasttext at build time)
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

    let ml_path = match take_flag_value(&mut args, "--ml-fasttext") {
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

    #[cfg(feature = "ml-fasttext")]
    let classifier = match ml_path.as_deref() {
        Some(p) => match FastTextClassifier::builder().model_path(p).build() {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("--ml-fasttext: {e}");
                return ExitCode::from(2);
            }
        },
        None => None,
    };

    #[cfg(not(feature = "ml-fasttext"))]
    if ml_path.is_some() {
        eprintln!("--ml-fasttext requires building with `--features ml-fasttext`");
        return ExitCode::from(2);
    }

    let opts = IdentifyOptions {
        #[cfg(feature = "ml-fasttext")]
        ml_classifier: classifier
            .as_ref()
            .map(|c| c as &dyn language_identifier::MlClassifier),
        #[cfg(not(feature = "ml-fasttext"))]
        ml_classifier: None,
        llm_resolver: None,
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
