use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Display, Formatter};
use std::fs;
use std::iter::Peekable;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Acquire;

#[derive(Debug)]
pub enum PreprocError {
    EmptyIdent,
    UnexpectedEOF,
    UnexpectedSymbol(char),
    TooManyColons,
    BadAssignmentOperator(char),
    CommandFailed,
    UndefinedMacro(String),
    BadMacroName,
}

impl Display for PreprocError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:?}", self)?;
        Ok(())
    }
}

impl std::error::Error for PreprocError {}

type Result<T> = std::result::Result<T, PreprocError>;

fn skip_blank(letters: &mut Peekable<impl Iterator<Item = char>>) {
    while let Some(letter) = letters.peek() {
        if !letter.is_whitespace() {
            break;
        };
        letters.next();
    }
}

fn suitable_ident(c: &char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '.' | '-')
}

fn get_ident(letters: &mut Peekable<impl Iterator<Item = char>>) -> Result<String> {
    let mut ident = String::new();

    while let Some(letter) = letters.peek() {
        if !suitable_ident(letter) {
            break;
        };
        ident.push(*letter);
        letters.next();
    }

    if ident.is_empty() {
        Err(PreprocError::EmptyIdent)
    } else {
        Ok(ident)
    }
}

fn take_till_eol(letters: &mut Peekable<impl Iterator<Item = char>>) -> String {
    let mut content = String::new();

    while let Some(letter) = letters.peek() {
        if matches!(letter, '\n' | '#') {
            if !content.ends_with('\\') { break; }
        };
        content.push(*letter);
        letters.next();
    }

    content
}

/// Searches for all the lines in makefile that resemble macro definition
/// and creates hashtable from macro names and bodies
fn generate_macro_table(
    source: &str,
) -> std::result::Result<HashMap<String, String>, PreprocError> {
    let macro_defs = source.lines().filter(|line| line.contains('=') && line.strip_prefix('\t').is_none());
    let mut macro_table = HashMap::<String, String>::new();

    for def in macro_defs {
        enum Operator {
            Equals,
            Colon,
            Colon2,
            Colon3,
            Bang,
            QuestionMark,
            Plus,
        }

        let mut text = def.chars().peekable();
        let line = take_till_eol(&mut text);
        if line.is_empty() || line.split_whitespace().next().is_none() { continue; }
        
        let mut text = line.chars().peekable();

        let mut macro_name = get_ident(&mut text)?;
        if macro_name == "export" {
            skip_blank(&mut text);
            macro_name = get_ident(&mut text)?;
        }
        skip_blank(&mut text);
        let Some(symbol) = text.next() else {
            Err(PreprocError::UnexpectedEOF)?
        };
        let operator = match symbol {
            '=' => Operator::Equals,
            ':' => {
                let mut count = 1;
                while let Some(':') = text.peek() {
                    count += 1;
                    text.next();
                }
                let Some('=') = text.next() else {
                    Err(PreprocError::BadAssignmentOperator(':'))?
                };

                match count {
                    1 => Operator::Colon,
                    2 => Operator::Colon2,
                    3 => Operator::Colon3,
                    _ => Err(PreprocError::TooManyColons)?,
                }
            }
            '!' => {
                let Some('=') = text.next() else {
                    Err(PreprocError::BadAssignmentOperator('!'))?
                };
                Operator::Bang
            }
            '?' => {
                let Some('=') = text.next() else {
                    Err(PreprocError::BadAssignmentOperator('?'))?
                };
                Operator::QuestionMark
            }
            '+' => {
                let Some('=') = text.next() else {
                    Err(PreprocError::BadAssignmentOperator('+'))?
                };
                Operator::Plus
            }
            c => Err(dbg!(PreprocError::UnexpectedSymbol(c)))?,
        };
        skip_blank(&mut text);
        let mut macro_body = take_till_eol(&mut text);

        match operator {
            Operator::Equals => {}
            Operator::Colon | Operator::Colon2 => loop {
                let (result, substitutions) = substitute(&macro_body, &macro_table)?;
                if substitutions == 0 {
                    break;
                } else {
                    macro_body = result
                }
            },
            Operator::Colon3 => {
                macro_body = substitute(&macro_body, &macro_table)?.0;
            }
            Operator::Bang => {
                macro_body = substitute(&macro_body, &macro_table)?.0;
                let Ok(result) = Command::new("sh")
                    .args(["-c", &macro_body])
                    .output()
                else {
                    Err(PreprocError::CommandFailed)?
                };
                macro_body = String::from_utf8_lossy(&result.stdout).to_string();
            }
            Operator::QuestionMark => {
                if let Some(body) = macro_table.remove(&macro_name) {
                    macro_body = body
                }
            }
            Operator::Plus => {
                if let Some(body) = macro_table.remove(&macro_name) {
                    macro_body = format!("{} {}", body, macro_body);
                }
            }
        }

        macro_table.insert(macro_name, macro_body);
    }

    Ok(macro_table)
}

pub static ENV_MACROS: AtomicBool = AtomicBool::new(false);

enum MacroFunction {
    FilterOut,
    Shell,
    And,
    Or,
    If,
}

fn detect_function(s: impl AsRef<str>) -> Option<MacroFunction> {
    match s.as_ref() {
        "filter-out" => Some(MacroFunction::FilterOut),
        "shell" => Some(MacroFunction::Shell),
        "and" => Some(MacroFunction::And),
        "or" => Some(MacroFunction::Or),
        "if" => Some(MacroFunction::If),
        _ => None,
    }
}
fn parse_function(f: MacroFunction, src: &mut Peekable<impl Iterator<Item = char>>, table: &HashMap<String, String>) -> Result<String> {
    let mut args = String::new();
    while let Some(&c) = src.peek() {
        if c == ')' || c == '}' { break; }
        args.push(c);
        src.next();
    }
    
    match f {
        MacroFunction::FilterOut => {
            let mut args = args.split(',');
            let Some(pattern) = args.next() else { Err(PreprocError::CommandFailed)? };
            let (pattern, _) = substitute(pattern, table)?;
            let Some(text) = args.next() else { Err(PreprocError::CommandFailed)? };
            let (text, _) = substitute(text, table)?;
            
            let patterns = pattern.split_whitespace();
            let words = text.split_whitespace();
            let pattern_set = HashSet::<&str>::from_iter(patterns);
            
            let result = words
                .filter(|s| pattern_set.contains(s))
                .fold(String::new(), |acc, s| acc + s);
            
            Ok(result)
        },
        MacroFunction::Shell => {
            let output = Command::new("sh").args(["-c", &args]).output();
            let mut result = output.into_iter()
                .filter_map(|x| String::from_utf8(x.stdout).ok());
            result.next().ok_or(PreprocError::CommandFailed)
        },
        MacroFunction::If => {
            let mut args = args.split(',');
            let Some(cond    ) = args.next() else { Err(PreprocError::CommandFailed)? };
            let Some(on_true ) = args.next() else { Err(PreprocError::CommandFailed)? };
            let on_false = args.next();
            
            let (result,_) = substitute(cond, table)?;
            let cond = result.split_whitespace().next().is_none();
            let (output, _) = if cond {
                substitute(on_true, table)?
            } else {
                on_false.iter()
                    .flat_map(|x| substitute(x, table))
                    .next().unwrap_or_default()
            };
            
            Ok(output)
        }
        MacroFunction::And => {
            let args = args.split(',');
            let expanded = args.map(|x| substitute(x, table));
            let is_true = expanded.clone().all(|x|
                if let Ok((s, _)) = x {
                    s.split_whitespace().next().is_some()
                } else {
                    false
                }
            );
            
            let (result, _) = expanded.last().ok_or(PreprocError::CommandFailed)??;
            Ok(if is_true { result } else { String::new() })
        }
        MacroFunction::Or => {
            let args = args.split(',');
            let expanded = args.map(|x| substitute(x, table));
            let chosen = expanded.clone().find(|x|
                if let Ok((s, _)) = x {
                    s.split_whitespace().next().is_some()
                } else {
                    false
                }
            );
            
            Ok(if let Some(x) = chosen { x?.0 } else { String::new() })
        }
    }
}

fn substitute(source: &str, table: &HashMap<String, String>) -> Result<(String, u32)> {
    let env_macros = ENV_MACROS.load(Acquire);

    let mut substitutions = 0;
    let mut result = String::with_capacity(source.len());

    let mut letters = source.chars().peekable();
    while let Some(letter) = letters.next() {
        if letter != '$' {
            result.push(letter);
            continue;
        }

        let Some(letter) = letters.next() else {
            Err(PreprocError::UnexpectedEOF)?
        };

        match letter {
            // Internal macros - we leave them "as is"
            // yet as they will be dealt with in the
            // parsing stage with more context available
            c @ ('$' | '@' | '%' | '?' | '<' | '*') => {
                result.push('$');
                result.push(c);
                continue;
            }
            c if suitable_ident(&c) => {
                let env_macro = if env_macros {
                    std::env::var(c.to_string()).ok()
                } else {
                    None
                };
                let table_macro = table.get(&c.to_string()).cloned();
                let Some(macro_body) = env_macro.or(table_macro) else {
                    Err(PreprocError::UndefinedMacro(c.to_string()))?
                };
                result.push_str(&macro_body);
                substitutions += 1;
                continue;
            }
            '(' | '{' => {
                skip_blank(&mut letters);
                let Ok(macro_name) = get_ident(&mut letters) else {
                    Err(PreprocError::BadMacroName)?
                };
                
                if let Some(name) = detect_function(&macro_name) {
                    let macro_body = parse_function(name, &mut letters, table)?;
                    result.push_str(&macro_body);
                    substitutions += 1;
                    continue;
                }
                
                skip_blank(&mut letters);
                while let Some(c) = letters.peek() { if *c == '}' || *c == ')' { break; } }
                let Some(finilizer) = letters.next() else {
                    Err(PreprocError::UnexpectedEOF)?
                };
                if !matches!(finilizer, ')' | '}') {
                    Err(dbg!(PreprocError::UnexpectedSymbol(finilizer)))?
                }

                let env_macro = if env_macros {
                    std::env::var(&macro_name).ok()
                } else {
                    None
                };
                let table_macro = table.get(&macro_name).cloned();
                let Some(macro_body) = env_macro.or(table_macro) else {
                    Err(PreprocError::UndefinedMacro(macro_name.to_string()))?
                };
                result.push_str(&macro_body);
                substitutions += 1;

                continue;
            }
            c => Err(dbg!(PreprocError::UnexpectedSymbol(c)))?,
        }
    }

    Ok((result, substitutions))
}

/// Copy-pastes included makefiles into single one recursively.
/// Pretty much the same as C preprocessor and `#include` directive
fn process_include_lines(source: &str, table: &HashMap<String, String>) -> (String, usize) {
    let mut counter = 0;
    let result = source
        .lines()
        .map(|x| {
            if let Some(s) = x.strip_prefix("include") {
                counter += 1;
                let s = s.trim();
                let (source, _) = substitute(s, table).unwrap_or_default();
                let path = Path::new(&source);

                fs::read_to_string(path).unwrap()
            } else {
                x.to_string()
            }
        })
        .map(|mut x| {
            x.push('\n');
            x
        })
        .collect::<String>();
    (result, counter)
}

fn remove_variables(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.contains('='))
        .map(|x| {
            let mut x = x.to_string();
            x.push('\n');
            x
        })
        .collect::<String>()
}

/// Processes `include`s and macros
pub fn preprocess(source: &str) -> Result<String> {
    let mut source = source.to_string();
    let mut includes = 1;
    let mut table = generate_macro_table(&source)?;

    while includes > 0 {
        (source, includes) = process_include_lines(&source, &HashMap::new());
        table = generate_macro_table(&source)?;
    }

    source = remove_variables(&source);

    loop {
        let (result, substitutions) = substitute(&source, &table)?;
        if substitutions == 0 {
            break Ok(result);
        } else {
            source = result
        }
    }
}
