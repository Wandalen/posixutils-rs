use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::fs;
use std::iter::Peekable;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Acquire;

#[derive(Debug)]
pub struct PreprocError(pub Vec<String>);

impl PreprocError {
    fn join(self, other: Self) -> Self {
        let mut errors = self.0;
        errors.extend(other.0);
        Self(errors)
    }
}

impl Display for PreprocError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for s in self.0.iter() { writeln!(f, "{}", s)?; }
        Ok(())
    }
}

impl std::error::Error for PreprocError {}

macro_rules! error {
    ($($e:expr),+) => { Err(PreprocError(vec![format!($($e),+)])) };
}

type Result<T> = std::result::Result<T, PreprocError>;

fn skip_blank(letters: &mut Peekable<impl Iterator<Item=char>>) {
    while let Some(letter) = letters.peek() {
        if !letter.is_whitespace() {
            break;
        };
        letters.next();
    }
}

fn suitable_ident(c: &char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '.')
}

fn get_ident(letters: &mut Peekable<impl Iterator<Item=char>>) -> Result<String> {
    let mut ident = String::new();

    while let Some(letter) = letters.peek() {
        if !suitable_ident(letter) {
            break;
        };
        ident.push(letter.clone());
        letters.next();
    }

    if ident.is_empty() { error!("Empty ident") } else { Ok(ident) }
}

fn get_file_path(letters: &mut Peekable<impl Iterator<Item=char>>) -> Result<String> {
    let mut ident = String::new();

    while let Some(letter) = letters.peek() {
        if !(letter.is_alphanumeric() || matches!(letter, '_' | '.' | '/')) {
            break;
        };
        ident.push(letter.clone());
        letters.next();
    }

    if ident.is_empty() { error!("Empty ident") } else { Ok(ident) }
}

fn take_till_eol(letters: &mut Peekable<impl Iterator<Item=char>>) -> String {
    let mut content = String::new();

    while let Some(letter) = letters.peek() {
        if matches!(letter, '\n' | '#') {
            break;
        };
        content.push(letter.clone());
        letters.next();
    }

    content
}

fn generate_macro_table(source: &str) -> std::result::Result<HashMap<String, String>, PreprocError> {
    let macro_defs = source.lines().filter(|line| line.contains('='));
    let mut macro_table = HashMap::<String, String>::new();

    let errors = macro_defs.map(|def| -> Result<()> {
        enum Operator {
            Equals,
            Colon,
            Colon2,
            Colon3,
            Bang,
            QuestionMark,
            Plus,
        }

        let mut immediate = false;
        let mut text = def.chars().peekable();

        let mut macro_name = get_ident(&mut text)?;
        if macro_name == "export" {
            skip_blank(&mut text);
            macro_name = get_ident(&mut text)?;
        }
        skip_blank(&mut text);
        let Some(symbol) = text.next() else {
            error!("Unexpected end of line!")?
        };
        let operator = match symbol {
            '=' => { Operator::Equals }
            ':' => {
                let mut count = 1;
                while let Some(':') = text.peek() {
                    count += 1;
                    text.next();
                }
                let Some('=') = text.next() else {
                    error!("Expected `=` after `:` in macro definition")?
                };

                immediate = true;
                match count {
                    1 => Operator::Colon,
                    2 => Operator::Colon2,
                    3 => Operator::Colon3,
                    _ => error!("Too many columns for assignment operator!")?
                }
            }
            '!' => {
                let Some('=') = text.next() else {
                    error!("Expected `=` after `!` in macro definition")?
                };
                Operator::Bang
            }
            '?' => {
                let Some('=') = text.next() else {
                    error!("Expected `=` after `?` in macro definition")?
                };
                Operator::QuestionMark
            }
            '+' => {
                let Some('=') = text.next() else {
                    error!("Expected `=` after `+` in macro definition")?
                };
                Operator::Plus
            }
            c => error!("Unexpected symbol `{}` in macro definition", c)?,
        };
        skip_blank(&mut text);
        let mut macro_body = take_till_eol(&mut text);

        match operator {
            Operator::Equals => {}
            Operator::Colon | Operator::Colon2 => {
                loop {
                    let (result, substitutions) = substitute(&macro_body, &macro_table)?;
                    if substitutions == 0 { break; } else { macro_body = result }
                };
            }
            Operator::Colon3 => {
                macro_body = substitute(&macro_body, &macro_table)?.0;
            }
            Operator::Bang => {
                macro_body = substitute(&macro_body, &macro_table)?.0;
                let Ok(result) = std::process::Command::new("sh").args(["-c", &macro_body]).output() else { error!("Command execution failed")? };
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

        Ok(())
    }).filter_map(|x| if let Err(error) = x { Some(error.0) } else { None }).flatten().collect::<Vec<_>>();

    if errors.is_empty() { Ok(macro_table) } else { Err(PreprocError(errors)) }
}

pub static ENV_MACROS: AtomicBool = AtomicBool::new(false);

fn substitute(source: &str, table: &HashMap<String, String>) -> Result<(String, u32)> {
    let env_macros = ENV_MACROS.load(Acquire);
    
    let mut substitutions = 0;
    let mut result = String::with_capacity(source.len());
    let mut errors = PreprocError(vec![]);

    let mut letters = source.chars().peekable();
    while let Some(letter) = letters.next() {
        if letter != '$' {
            result.push(letter);
            continue;
        }

        // TODO: Make proper error handling
        let Some(letter) = letters.next() else {
            errors.0.push("Unexpected EOF after `$` symbol".to_string());
            continue;
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
                let env_macro = if env_macros { std::env::var(&c.to_string()).ok() } else { None };
                let table_macro = table.get(&c.to_string()).cloned();
                let Some(macro_body) = env_macro.or(table_macro) else {
                    errors.0.push(format!("Undefined macro `{}`", c));
                    continue;
                };
                result.push_str(&macro_body);
                substitutions += 1;
                continue;
            }
            '(' | '{' => {
                skip_blank(&mut letters);
                let Ok(macro_name) = get_ident(&mut letters) else {
                    errors.0.push("Could not get a macro name".to_string());
                    continue;
                };
                skip_blank(&mut letters);
                let Some(finilizer) = letters.next() else {
                    errors.0.push("Unexpected EOF at the end of macro expansion".to_string());
                    continue;
                };
                if !matches!(finilizer, ')' | '}') {
                    errors.0.push(format!("Unexpected `{}` at the end of macro expansion", finilizer));
                    continue;
                }

                let env_macro = if env_macros { std::env::var(&macro_name).ok() } else { None };
                let table_macro = table.get(&macro_name).cloned();
                let Some(macro_body) = env_macro.or(table_macro) else {
                    errors.0.push(format!("Undefined macro `{}`", macro_name));
                    continue;
                };
                result.push_str(&macro_body);
                substitutions += 1;

                continue;
            }
            // TODO: Make proper error handling
            c => {
                panic!("Unexpected `{}` after `$` symbol", c);
            }
        }
    }

    if errors.0.is_empty() { Ok((result, substitutions)) } else { Err(errors) }
}

fn process_include_lines(source: &str, table: &HashMap<String, String>) -> (String, usize) {
    let mut counter = 0;
    let mut result = source.lines().map(|x|
        if let Some(s) = x.strip_prefix("include") {
            counter += 1;
            let s = s.trim();
            let (source, _) = substitute(s, table).unwrap_or_default();
            let path = Path::new(&source);
            let mut s = fs::read_to_string(path).unwrap();
            s
        } else { x.to_string() }).map(|mut x| { x.push_str("\n"); x}).collect::<String>();
    (result, counter)
}

fn remove_variables(source: &str) -> String {
    source.lines().filter(|line| !line.contains('=')).map(|mut x| {
        let mut x = x.to_string();
        x.push_str("\n"); x }
    ).collect::<String>()
}

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
