use crate::builtin::set::SetOptions;
use crate::builtin::{get_builtin_utility, get_special_builtin_utility};
use crate::parse::command::{
    Assignment, CaseItem, Command, CommandType, CompleteCommand, CompoundCommand, Conjunction,
    FunctionDefinition, If, LogicalOp, Name, Pipeline, Redirection, SimpleCommand,
};
use crate::parse::command_parser::CommandParser;
use crate::parse::word::Word;
use crate::parse::word_parser::parse_word;
use crate::parse::{AliasTable, ParserError};
use crate::shell::environment::{CannotModifyReadonly, Environment, Value};
use crate::shell::opened_files::{OpenedFile, OpenedFiles};
use crate::utils::{close, dup2, fork, pipe, waitpid, OsError, OsResult};
use crate::wordexp::{expand_word, expand_word_to_string, word_to_pattern};
use nix::errno::Errno;
use nix::sys::wait::WaitStatus;
use nix::unistd::{execve, getpid, getppid, ForkResult, Pid};
use nix::{libc, NixPath};
use std::collections::HashMap;
use std::ffi::{CString, OsString};
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{read_to_string, Read, Write};
use std::os::fd::{AsRawFd, IntoRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub mod environment;
pub mod opened_files;

fn find_in_path(command: &str, env_path: &str) -> Option<String> {
    for path in env_path.split(':') {
        let mut command_path = PathBuf::from(path);
        command_path.push(command);
        if command_path.is_file() {
            return Some(command_path.into_os_string().to_string_lossy().into());
        }
    }
    None
}

#[derive(Clone, Debug)]
pub enum CommandExecutionError {
    SpecialBuiltinError,
    BuiltinError,
    SpecialBuiltinRedirectionError(String),
    RedirectionError(String),
    VariableAssignmentError(CannotModifyReadonly),
    ExpansionError(String),
    CommandNotFound(String),
    OsError(OsError),
}

impl From<OsError> for CommandExecutionError {
    fn from(value: OsError) -> Self {
        Self::OsError(value)
    }
}

impl From<CannotModifyReadonly> for CommandExecutionError {
    fn from(value: CannotModifyReadonly) -> Self {
        CommandExecutionError::VariableAssignmentError(value)
    }
}

type CommandExecutionResult<T> = Result<T, CommandExecutionError>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ControlFlowState {
    Break(u32),
    Continue(u32),
    Return,
    None,
}

impl ControlFlowState {
    fn go_to_outer_loop(&mut self) {
        match *self {
            ControlFlowState::Break(1) => *self = ControlFlowState::None,
            ControlFlowState::Break(n) => {
                assert_ne!(n, 0);
                *self = ControlFlowState::Break(n - 1)
            }
            ControlFlowState::Continue(1) => *self = ControlFlowState::None,
            ControlFlowState::Continue(n) => {
                assert_ne!(n, 0);
                *self = ControlFlowState::Continue(n - 1);
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
pub struct Shell {
    pub environment: Environment,
    pub program_name: String,
    pub positional_parameters: Vec<String>,
    pub opened_files: OpenedFiles,
    pub functions: HashMap<Name, Rc<CompoundCommand>>,
    pub most_recent_pipeline_exit_status: i32,
    pub last_command_substitution_status: i32,
    pub shell_pid: i32,
    pub most_recent_background_command_pid: Option<i32>,
    pub current_directory: OsString,
    pub set_options: SetOptions,
    pub alias_table: AliasTable,
    pub control_flow_state: ControlFlowState,
    pub loop_depth: u32,
    pub is_interactive: bool,
    pub last_lineno: u32,
}

impl Shell {
    pub fn eprint(&self, message: &str) {
        self.opened_files.stderr().write_str(message);
    }

    pub fn assign(&mut self, name: String, value: String, export: bool) {
        if self.environment.set(name, value, export).is_err() {
            self.opened_files
                .stderr()
                .write_str("sh: cannot assign to readonly variable\n");
            if !self.is_interactive {
                std::process::exit(2)
            }
        }
    }

    fn handle_error(&self, err: CommandExecutionError) -> i32 {
        match err {
            CommandExecutionError::SpecialBuiltinError => {
                if !self.is_interactive {
                    std::process::exit(1)
                }
                1
            }
            CommandExecutionError::BuiltinError => 1,
            CommandExecutionError::SpecialBuiltinRedirectionError(err) => {
                self.eprint(&format!("{err}\n"));
                if !self.is_interactive {
                    std::process::exit(1)
                }
                1
            }
            CommandExecutionError::RedirectionError(err) => {
                self.eprint(&format!("{err}\n"));
                1
            }
            CommandExecutionError::VariableAssignmentError(err) => {
                self.eprint(&format!("{err}\n"));
                1
            }
            CommandExecutionError::ExpansionError(err) => {
                self.eprint(&format!("{err}\n"));
                1
            }
            CommandExecutionError::CommandNotFound(command_name) => {
                self.eprint(&format!("sh: '{command_name}' not found\n"));
                127
            }
            CommandExecutionError::OsError(err) => {
                self.eprint(&format!("{err}\n"));
                std::process::exit(1)
            }
        }
    }

    fn exec(&self, command: &str, args: &[String]) -> OsResult<i32> {
        match fork()? {
            ForkResult::Child => {
                for (id, file) in &self.opened_files.opened_files {
                    let dest = *id as i32;
                    let src = match file {
                        OpenedFile::Stdin => libc::STDIN_FILENO,
                        OpenedFile::Stdout => libc::STDOUT_FILENO,
                        OpenedFile::Stderr => libc::STDERR_FILENO,
                        OpenedFile::ReadFile(file)
                        | OpenedFile::WriteFile(file)
                        | OpenedFile::ReadWriteFile(file) => file.as_raw_fd(),
                        OpenedFile::HereDocument(contents) => {
                            let (read_pipe, write_pipe) = pipe()?;
                            nix::unistd::write(write_pipe, contents.as_bytes()).map_err(|err| {
                                OsError::from(format!("sh: internal call to write failed ({err})"))
                            })?;
                            dup2(read_pipe.as_raw_fd(), dest)?;
                            continue;
                        }
                    };
                    dup2(src, dest)?;
                }
                let command = CString::new(command).unwrap();
                let args = args
                    .iter()
                    .map(|s| CString::new(s.as_str()).unwrap())
                    .collect::<Vec<_>>();
                let env = self
                    .environment
                    .variables
                    .iter()
                    .filter_map(|(name, value)| {
                        if value.export {
                            // TODO: look into this unwrap
                            value
                                .value
                                .as_ref()
                                .map(|v| CString::new(format!("{name}={}", v)).unwrap())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<CString>>();
                // unwrap is safe here, because execve will only return if it fails
                let err = execve(&command, &args, &env).unwrap_err();
                if err == Errno::ENOEXEC {
                    // TODO: the spec says that we should try to execute the file as a shell script
                    // before returning error
                    todo!()
                }
                std::process::exit(126);
            }
            ForkResult::Parent { child } => match waitpid(child)? {
                WaitStatus::Exited(_, status) => Ok(status),
                _ => todo!(),
            },
        }
    }

    fn perform_assignments(
        &mut self,
        assignments: &[Assignment],
        export: bool,
    ) -> CommandExecutionResult<()> {
        for assignment in assignments {
            let word_str = expand_word_to_string(&assignment.value, true, self)?;
            // TODO: should look into using Rc for Environment
            self.assign(assignment.name.to_string(), word_str, export);
        }
        Ok(())
    }

    fn interpret_simple_command(
        &mut self,
        simple_command: &SimpleCommand,
    ) -> CommandExecutionResult<i32> {
        let mut expanded_words = Vec::new();
        // reset
        self.last_command_substitution_status = 0;
        for word in &simple_command.words {
            expanded_words.extend(expand_word(word, false, self)?);
        }
        if expanded_words.is_empty() {
            // no commands to execute, perform assignments and redirections
            self.perform_assignments(&simple_command.assignments, false)?;
            if !simple_command.redirections.is_empty() {
                let mut subshell = self.clone();
                subshell
                    .opened_files
                    .redirect(&simple_command.redirections, self)?;
                (&simple_command.redirections, &mut subshell);
            }
            return Ok(self.last_command_substitution_status);
        }

        if expanded_words[0].contains('/') {
            if !Path::new(&expanded_words[0]).exists() {
                return Err(CommandExecutionError::CommandNotFound(
                    expanded_words[0].clone(),
                ));
            }
            let mut command_environment = self.clone();
            command_environment.perform_assignments(&simple_command.assignments, true)?;
            command_environment
                .opened_files
                .redirect(&simple_command.redirections, self)?;
            let command = &expanded_words[0];
            let arguments = expanded_words
                .iter()
                .map(|w| w.clone())
                .collect::<Vec<String>>();
            command_environment
                .exec(&command, &arguments)
                .map_err(|err| err.into())
        } else {
            if let Some(special_builtin_utility) = get_special_builtin_utility(&expanded_words[0]) {
                // the standard does not specify if the variables should have the export attribute.
                // Bash exports them, we do the same here (neither sh, nor zsh do it though)
                self.perform_assignments(&simple_command.assignments, true)?;
                let mut opened_files = self.opened_files.clone();
                opened_files
                    .redirect(&simple_command.redirections, self)
                    .map_err(|err| {
                        if let CommandExecutionError::RedirectionError(err) = err {
                            CommandExecutionError::SpecialBuiltinRedirectionError(err)
                        } else {
                            err
                        }
                    })?;
                let status = special_builtin_utility.exec(&expanded_words[1..], self, opened_files);
                return Ok(status);
            }

            if let Some(function_body) = self.functions.get(expanded_words[0].as_str()).cloned() {
                let mut args = expanded_words[1..].to_vec();
                // assignments affect the current environment and are marked for export,
                // same as special builtin utilities
                self.perform_assignments(&simple_command.assignments, true)?;
                let mut previous_opened_files = self.opened_files.clone();
                previous_opened_files.redirect(&simple_command.redirections, self)?;
                std::mem::swap(&mut self.opened_files, &mut previous_opened_files);
                std::mem::swap(&mut args, &mut self.positional_parameters);

                let result =
                    self.interpret_compound_command(&function_body, &simple_command.redirections);

                std::mem::swap(&mut args, &mut self.positional_parameters);
                std::mem::swap(&mut self.opened_files, &mut previous_opened_files);
                return result;
            }

            if let Some(builtin_utility) = get_builtin_utility(&expanded_words[0]) {
                let mut opened_files = self.opened_files.clone();
                opened_files.redirect(&simple_command.redirections, self)?;
                let mut command_env = self.environment.clone();
                self.perform_assignments(&simple_command.assignments, false)?;
                std::mem::swap(&mut self.environment, &mut command_env);
                return Ok(builtin_utility.exec(
                    &expanded_words[1..],
                    self,
                    opened_files,
                    command_env,
                ));
            }

            let mut command_environment = self.clone();
            command_environment.perform_assignments(&simple_command.assignments, true)?;
            command_environment
                .opened_files
                .redirect(&simple_command.redirections, self)?;
            // TODO: fix unwrap with proper error
            let path = self.environment.get_str_value("PATH").unwrap();
            if let Some(command) = find_in_path(&expanded_words[0], path) {
                let arguments = expanded_words
                    .iter()
                    .map(|w| w.clone())
                    .collect::<Vec<String>>();
                command_environment
                    .exec(&command, &arguments)
                    .map_err(|err| err.into())
            } else {
                Err(CommandExecutionError::CommandNotFound(
                    expanded_words[0].clone(),
                ))
            }
        }
    }

    fn interpret_for_clause(
        &mut self,
        iter_var: Name,
        iter_words: &[Word],
        body: &CompleteCommand,
    ) -> CommandExecutionResult<i32> {
        let mut result = 0;
        self.loop_depth += 1;
        'outer: for word in iter_words {
            let items = expand_word(word, false, self)?;
            for item in items {
                self.assign(iter_var.to_string(), item, false);
                result = self.interpret(body);
                match self.control_flow_state {
                    ControlFlowState::Break(_) => {
                        self.control_flow_state.go_to_outer_loop();
                        break 'outer;
                    }
                    ControlFlowState::Continue(n) => {
                        self.control_flow_state.go_to_outer_loop();
                        if n > 1 {
                            break 'outer;
                        } else {
                            continue 'outer;
                        }
                    }
                    ControlFlowState::Return => {
                        break 'outer;
                    }
                    _ => {}
                }
            }
        }
        self.loop_depth -= 1;
        Ok(result)
    }

    fn interpret_case_clause(
        &mut self,
        arg: &Word,
        cases: &[CaseItem],
    ) -> CommandExecutionResult<i32> {
        let arg = expand_word_to_string(arg, false, self)?;
        let arg_cstr = CString::new(arg).expect("invalid pattern");
        for case in cases {
            for pattern in &case.pattern {
                let pattern = word_to_pattern(pattern, self)?;
                if pattern.matches(&arg_cstr) {
                    return Ok(self.interpret(&case.body));
                }
            }
        }
        Ok(0)
    }

    fn interpret_if_clause(&mut self, if_chain: &[If], else_body: &Option<CompleteCommand>) -> i32 {
        assert!(!if_chain.is_empty(), "parsed if without else");
        for if_ in if_chain {
            if self.interpret(&if_.condition) == 0 {
                return self.interpret(&if_.body);
            }
        }
        if let Some(else_body) = else_body {
            self.interpret(else_body)
        } else {
            0
        }
    }

    fn interpret_loop_clause(
        &mut self,
        condition: &CompleteCommand,
        body: &CompleteCommand,
        continue_if_zero: bool,
    ) -> i32 {
        let status = 0;
        loop {
            let condition = self.interpret(condition);
            if (condition == 0 && !continue_if_zero) || (condition != 0 && continue_if_zero) {
                break;
            }
            self.loop_depth += 1;
            self.interpret(body);
            self.loop_depth -= 1;
            match self.control_flow_state {
                ControlFlowState::Break(_) => {
                    self.control_flow_state.go_to_outer_loop();
                    break;
                }
                ControlFlowState::Continue(n) => {
                    self.control_flow_state.go_to_outer_loop();
                    if n > 1 {
                        break;
                    } else {
                        continue;
                    }
                }
                ControlFlowState::Return => {
                    break;
                }
                _ => {}
            }
        }
        status
    }

    fn interpret_compound_command(
        &mut self,
        compound_command: &CompoundCommand,
        redirections: &[Redirection],
    ) -> CommandExecutionResult<i32> {
        let mut prev_opened_files = self.opened_files.clone();
        prev_opened_files.redirect(redirections, self)?;
        std::mem::swap(&mut self.opened_files, &mut prev_opened_files);
        let result = match compound_command {
            CompoundCommand::BraceGroup(command) => Ok(self.interpret(command)),
            CompoundCommand::Subshell(commands) => {
                let mut subshell = self.clone();
                Ok(subshell.interpret(commands))
            }
            CompoundCommand::ForClause {
                iter_var,
                words,
                body,
            } => self.interpret_for_clause(iter_var.clone(), words, body),
            CompoundCommand::CaseClause { arg, cases } => self.interpret_case_clause(arg, cases),
            CompoundCommand::IfClause {
                if_chain,
                else_body,
            } => Ok(self.interpret_if_clause(if_chain, else_body)),
            CompoundCommand::WhileClause { condition, body } => {
                Ok(self.interpret_loop_clause(condition, body, true))
            }
            CompoundCommand::UntilClause { condition, body } => {
                Ok(self.interpret_loop_clause(condition, body, false))
            }
        };
        std::mem::swap(&mut self.opened_files, &mut prev_opened_files);
        result
    }

    fn define_function(&mut self, definition: &FunctionDefinition) {
        self.functions
            .insert(definition.name.clone(), definition.body.clone());
    }

    fn interpret_command(&mut self, command: &Command) -> i32 {
        self.assign("LINENO".to_string(), command.lineno.to_string(), false);
        let execution_result = match &command.type_ {
            CommandType::SimpleCommand(simple_command) => {
                self.interpret_simple_command(simple_command)
            }
            CommandType::CompoundCommand {
                command,
                redirections,
            } => self.interpret_compound_command(command, redirections),
            CommandType::FunctionDefinition(function) => {
                self.define_function(function);
                Ok(0)
            }
        };

        match execution_result {
            Ok(result) => result,
            Err(err) => self.handle_error(err),
        }
    }

    fn interpret_pipeline(&mut self, pipeline: &Pipeline) -> OsResult<i32> {
        let pipeline_exit_status;
        if pipeline.commands.len() == 1 {
            let command = &pipeline.commands[0];
            pipeline_exit_status = self.interpret_command(command);
        } else {
            let mut current_stdin = libc::STDIN_FILENO;
            for command in pipeline.commands.iter().take(pipeline.commands.len() - 1) {
                let (read_pipe, write_pipe) = pipe()?;
                match fork()? {
                    ForkResult::Child => {
                        drop(read_pipe);
                        dup2(current_stdin, libc::STDIN_FILENO)?;
                        dup2(write_pipe.as_raw_fd(), libc::STDOUT_FILENO)?;
                        let return_status = self.interpret_command(command);
                        if current_stdin != libc::STDIN_FILENO {
                            close(current_stdin)?;
                        }
                        std::process::exit(return_status);
                    }
                    ForkResult::Parent { .. } => {
                        if current_stdin != libc::STDIN_FILENO {
                            close(current_stdin)?;
                        }
                        current_stdin = read_pipe.into_raw_fd();
                    }
                }
            }

            match fork()? {
                ForkResult::Child => {
                    dup2(current_stdin, libc::STDIN_FILENO)?;
                    let return_status = self.interpret_command(pipeline.commands.last().unwrap());
                    close(current_stdin)?;
                    std::process::exit(return_status);
                }
                ForkResult::Parent { child } => {
                    close(current_stdin)?;
                    match waitpid(child)? {
                        WaitStatus::Exited(_, status) => pipeline_exit_status = status,
                        _ => todo!(),
                    }
                }
            }
        }
        self.most_recent_pipeline_exit_status = if pipeline.negate_status {
            (pipeline_exit_status == 0) as i32
        } else {
            pipeline_exit_status
        };
        Ok(self.most_recent_pipeline_exit_status)
    }

    fn interpret_conjunction(&mut self, conjunction: &Conjunction) -> i32 {
        let mut status = 0;
        let mut i = 0;
        while i < conjunction.elements.len() {
            let (pipeline, op) = &conjunction.elements[i];
            status = match self.interpret_pipeline(pipeline) {
                Ok(status) => status,
                Err(err) => {
                    self.eprint(&format!("{err}\n"));
                    std::process::exit(1)
                }
            };
            if self.control_flow_state != ControlFlowState::None {
                return status;
            }
            if status != 0 && *op == LogicalOp::And {
                // false && other ... -> skip other
                i += 1;
            } else if status == 0 && *op == LogicalOp::Or {
                // true || other ... -> skip other
                i += 1;
            }
            i += 1;
        }
        status
    }

    fn interpret(&mut self, command: &CompleteCommand) -> i32 {
        let mut status = 0;
        for conjunction in &command.commands {
            status = self.interpret_conjunction(conjunction);
            if self.control_flow_state != ControlFlowState::None {
                return status;
            }
        }
        status
    }

    pub fn execute_in_subshell(&mut self, program: &str) -> OsResult<String> {
        let (read_pipe, write_pipe) = pipe()?;
        match fork()? {
            ForkResult::Child => {
                drop(read_pipe);
                dup2(write_pipe.as_raw_fd(), libc::STDOUT_FILENO)?;
                self.execute_program(program).unwrap();
                std::process::exit(self.most_recent_pipeline_exit_status);
            }
            ForkResult::Parent { child } => {
                drop(write_pipe);
                match waitpid(child)? {
                    WaitStatus::Exited(_, _) => {
                        let read_file = File::from(read_pipe);
                        let mut output = read_to_string(&read_file).unwrap();
                        let new_len = output.trim_end_matches('\n').len();
                        output.truncate(new_len);
                        Ok(output)
                    }
                    _ => todo!(),
                }
            }
        }
    }

    pub fn execute_program(&mut self, program: &str) -> Result<(), ParserError> {
        let mut parser = CommandParser::new(program, self.last_lineno)?;
        loop {
            let command = parser.parse_next_command(&self.alias_table)?;
            if let Some(command) = command {
                self.interpret(&command);
            } else {
                break;
            }
        }
        self.last_lineno = parser.lineno() - 1;
        Ok(())
    }

    pub fn initialize_from_system(
        program_name: String,
        args: Vec<String>,
        set_options: SetOptions,
        is_interactive: bool,
    ) -> Shell {
        // > If a variable is initialized from the environment, it shall be marked for
        // > export immediately
        let environment = Environment::from(
            std::env::vars()
                .into_iter()
                .map(|(k, v)| (k, Value::new_exported(v))),
        );

        let mut shell = Shell {
            environment,
            program_name,
            positional_parameters: args,
            shell_pid: getpid().as_raw(),
            // TODO: handle error
            current_directory: std::env::current_dir().unwrap().into_os_string(),
            set_options,
            is_interactive,
            ..Default::default()
        };
        shell.assign("PPID".to_string(), getppid().to_string(), false);
        shell.assign("IFS".to_string(), " \t\n".to_string(), false);
        shell.assign("PS1".to_string(), "\\$ ".to_string(), false);
        shell.assign("PS2".to_string(), "> ".to_string(), false);
        shell.assign("PS4".to_string(), "+ ".to_string(), false);
        shell
    }

    fn get_var_and_expand(&mut self, var: &str, default_if_err: &str) -> String {
        let var = self.environment.get_str_value(var).unwrap_or_default();
        match parse_word(var, 0, false) {
            Ok(word) => match expand_word_to_string(&word, false, self) {
                Ok(str) => str,
                Err(err) => {
                    self.handle_error(err);
                    default_if_err.to_string()
                }
            },
            Err(err) => {
                eprintln!("sh: error parsing contents of {var}: {}", err.message);
                if !self.is_interactive {
                    std::process::exit(1)
                }
                default_if_err.to_string()
            }
        }
    }

    pub fn get_ps1(&mut self) -> String {
        self.get_var_and_expand("PS1", "\\$ ")
    }

    pub fn get_ps2(&mut self) -> String {
        self.get_var_and_expand("PS2", "> ")
    }
}

impl Default for Shell {
    fn default() -> Self {
        Shell {
            environment: Environment::from([("IFS".to_string(), Value::new(" \t\n".to_string()))]),
            program_name: "sh".to_string(),
            positional_parameters: Vec::default(),
            opened_files: OpenedFiles::default(),
            functions: HashMap::default(),
            most_recent_pipeline_exit_status: 0,
            last_command_substitution_status: 0,
            shell_pid: 0,
            most_recent_background_command_pid: None,
            current_directory: OsString::from("/"),
            set_options: SetOptions::default(),
            alias_table: AliasTable::default(),
            control_flow_state: ControlFlowState::None,
            loop_depth: 0,
            is_interactive: false,
            last_lineno: 0,
        }
    }
}
