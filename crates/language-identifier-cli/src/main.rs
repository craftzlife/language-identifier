use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use language_identifier::{identify_lines_with, identify_with, IdentifyOptions};

#[cfg(feature = "ml-fasttext")]
use language_identifier::FastTextClassifier;

#[cfg(all(target_os = "macos", feature = "llm-apple-foundation"))]
use language_identifier::AppleFoundationResolver;

const USAGE: &str = "\
Usage:
  language-identifier [--pretty] [--ml-fasttext <path>] [--llm-apple-foundation] <text>
  language-identifier [--pretty] [--ml-fasttext <path>] [--llm-apple-foundation] -

  --pretty                  Output indented JSON
  --ml-fasttext <path>      Enable Layer 9 using the lid.176.bin model at <path>
                            (requires --features ml-fasttext at build time)
  --llm-apple-foundation    Enable Layer 10 using Apple's on-device
                            FoundationModels system model (macOS 26+,
                            requires --features llm-apple-foundation at
                            build time)
  -                         Read one input per line from stdin and identify
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

    let mut llm_apple = false;
    args.retain(|a| {
        if a == "--llm-apple-foundation" {
            llm_apple = true;
            false
        } else {
            true
        }
    });

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

    #[cfg(all(target_os = "macos", feature = "llm-apple-foundation"))]
    let resolver = if llm_apple {
        match AppleFoundationResolver::new() {
            Ok(r) => Some(r),
            Err(e) => {
                eprintln!("--llm-apple-foundation: {e}");
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };

    #[cfg(not(all(target_os = "macos", feature = "llm-apple-foundation")))]
    if llm_apple {
        eprintln!(
            "--llm-apple-foundation requires building with \
             `--features llm-apple-foundation` on macOS 26+"
        );
        return ExitCode::from(2);
    }

    let opts = IdentifyOptions {
        #[cfg(feature = "ml-fasttext")]
        ml_classifier: classifier
            .as_ref()
            .map(|c| c as &dyn language_identifier::MlClassifier),
        #[cfg(not(feature = "ml-fasttext"))]
        ml_classifier: None,
        #[cfg(all(target_os = "macos", feature = "llm-apple-foundation"))]
        llm_resolver: resolver
            .as_ref()
            .map(|r| r as &dyn language_identifier::LlmResolver),
        #[cfg(not(all(target_os = "macos", feature = "llm-apple-foundation")))]
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
