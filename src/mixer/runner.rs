//! Process execution seam.
//!
//! Every external program the mixer drives (`pactl`, `pipewire`, `pw-dump`, `pw-cli`, `pgrep`,
//! `kill`, `wpctl`) is reached through [`CommandRunner`]. [`SystemRunner`] runs real processes
//! with a timeout; [`FakeRunner`] records calls and returns canned output so the whole engine is
//! unit-tested on any host, including ones without PipeWire.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::{Error, Result};

/// Default upper bound for a single short-lived command.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Lines of a spawned child's stderr kept for error reports.
const STDERR_TAIL_LINES: usize = 40;

/// Result of a finished command.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandOutput {
    /// Exit code; `None` when the process was killed by a signal.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    /// Successful output with the given stdout.
    pub fn ok(stdout: impl Into<String>) -> Self {
        Self {
            code: Some(0),
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    /// Failed output with the given exit code and stderr.
    pub fn failed(code: i32, stderr: impl Into<String>) -> Self {
        Self {
            code: Some(code),
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }

    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Runs external programs. Implementations must never block indefinitely.
pub trait CommandRunner: Send + Sync {
    /// Run `program` to completion and capture its output.
    ///
    /// A program that cannot be found yields `Error::Io` with kind `NotFound`
    /// (see [`is_not_found`]); a program that exceeds the runner's timeout is killed and yields
    /// `Error::Audio`.
    fn run(&self, program: &str, args: &[String]) -> Result<CommandOutput>;

    /// Start `program` in the background. Its stdout is delivered line by line through
    /// [`ChildHandle::take_stdout`].
    fn spawn(&self, program: &str, args: &[String]) -> Result<ChildHandle>;
}

/// True when `err` means "the program does not exist".
pub fn is_not_found(err: &Error) -> bool {
    matches!(err, Error::Io(e) if e.kind() == std::io::ErrorKind::NotFound)
}

/// Process control behind a [`ChildHandle`].
pub trait ChildControl: Send {
    /// `Some(exit code)` once the process has exited (`-1` when killed by a signal).
    fn try_wait(&mut self) -> Result<Option<i32>>;

    /// Ask the process to exit (SIGTERM on Unix), wait up to `grace`, then kill it.
    fn terminate(&mut self, grace: Duration) -> Result<()>;

    /// Last lines the process wrote to stderr, for error messages.
    fn stderr_tail(&self) -> String {
        String::new()
    }
}

/// A background process started through [`CommandRunner::spawn`].
///
/// Dropping the handle terminates the process, so a mixer that goes away never leaves its
/// PipeWire child behind.
pub struct ChildHandle {
    pid: u32,
    control: Box<dyn ChildControl>,
    stdout: Option<Receiver<String>>,
    exit_code: Option<i32>,
}

impl ChildHandle {
    pub fn new(pid: u32, control: Box<dyn ChildControl>, stdout: Option<Receiver<String>>) -> Self {
        Self {
            pid,
            control,
            stdout,
            exit_code: None,
        }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// `Some(exit code)` once the process has exited.
    pub fn try_wait(&mut self) -> Result<Option<i32>> {
        if self.exit_code.is_some() {
            return Ok(self.exit_code);
        }
        let status = self.control.try_wait()?;
        self.exit_code = status;
        Ok(status)
    }

    pub fn is_running(&mut self) -> bool {
        matches!(self.try_wait(), Ok(None))
    }

    /// Terminate the process: polite signal first, kill after `grace`.
    pub fn terminate(&mut self, grace: Duration) -> Result<()> {
        if self.try_wait()?.is_some() {
            return Ok(());
        }
        self.control.terminate(grace)?;
        if self.exit_code.is_none() {
            self.exit_code = Some(self.control.try_wait()?.unwrap_or(-1));
        }
        Ok(())
    }

    /// Take the receiver of stdout lines. Returns `None` on the second call.
    pub fn take_stdout(&mut self) -> Option<Receiver<String>> {
        self.stdout.take()
    }

    pub fn stderr_tail(&self) -> String {
        self.control.stderr_tail()
    }
}

impl Drop for ChildHandle {
    fn drop(&mut self) {
        if self.exit_code.is_none() {
            if let Err(e) = self.control.terminate(Duration::from_millis(500)) {
                tracing::warn!("failed to terminate child process {}: {e}", self.pid);
            }
        }
    }
}

impl std::fmt::Debug for ChildHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChildHandle")
            .field("pid", &self.pid)
            .field("exit_code", &self.exit_code)
            .finish()
    }
}

/// Runs real processes.
///
/// Every command gets `LC_ALL=C`: `pactl` translates its text output (`Sink #`, `Mute: no`,
/// `Default Sink:`), and the text parser only understands the untranslated form.
#[derive(Clone, Debug)]
pub struct SystemRunner {
    timeout: Duration,
}

impl Default for SystemRunner {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

impl SystemRunner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Runner whose [`CommandRunner::run`] kills commands that take longer than `timeout`.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }

    fn command(program: &str, args: &[String]) -> Command {
        let mut command = Command::new(program);
        command
            .args(args)
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
}

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[String]) -> Result<CommandOutput> {
        let mut child = Self::command(program, args).spawn()?;
        let stdout = read_to_end_in_background(child.stdout.take());
        let stderr = read_to_end_in_background(child.stderr.take());

        let deadline = Instant::now() + self.timeout;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if Instant::now() >= deadline {
                if let Err(e) = child.kill() {
                    tracing::debug!("kill after timeout failed for {program}: {e}");
                }
                if let Err(e) = child.wait() {
                    tracing::debug!("wait after timeout failed for {program}: {e}");
                }
                return Err(Error::Audio(format!(
                    "`{program}` did not finish within {} ms",
                    self.timeout.as_millis()
                )));
            }
            thread::sleep(Duration::from_millis(5));
        };

        Ok(CommandOutput {
            code: status.code(),
            stdout: collect_background(stdout),
            stderr: collect_background(stderr),
        })
    }

    fn spawn(&self, program: &str, args: &[String]) -> Result<ChildHandle> {
        let mut child = Self::command(program, args).spawn()?;
        let pid = child.id();

        let stdout = child.stdout.take().map(|out| {
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                for line in BufReader::new(out).lines() {
                    let Ok(line) = line else { break };
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
            rx
        });

        let tail = Arc::new(Mutex::new(VecDeque::with_capacity(STDERR_TAIL_LINES)));
        if let Some(err) = child.stderr.take() {
            let tail = Arc::clone(&tail);
            thread::spawn(move || {
                for line in BufReader::new(err).lines() {
                    let Ok(line) = line else { break };
                    let mut tail = tail.lock();
                    if tail.len() == STDERR_TAIL_LINES {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                }
            });
        }

        Ok(ChildHandle::new(
            pid,
            Box::new(SystemChild {
                child,
                stderr_tail: tail,
            }),
            stdout,
        ))
    }
}

fn read_to_end_in_background<R: Read + Send + 'static>(pipe: Option<R>) -> Option<Receiver<String>> {
    let mut pipe = pipe?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = Vec::new();
        if let Err(e) = pipe.read_to_end(&mut buf) {
            tracing::debug!("reading child output failed: {e}");
        }
        if tx.send(String::from_utf8_lossy(&buf).into_owned()).is_err() {
            tracing::trace!("child output reader outlived its receiver");
        }
    });
    Some(rx)
}

fn collect_background(rx: Option<Receiver<String>>) -> String {
    // The pipe closes when the process exits; the bounded wait only matters if the program
    // left a grandchild holding the pipe open.
    rx.and_then(|rx| rx.recv_timeout(Duration::from_secs(1)).ok())
        .unwrap_or_default()
}

struct SystemChild {
    child: Child,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
}

impl ChildControl for SystemChild {
    fn try_wait(&mut self) -> Result<Option<i32>> {
        Ok(self.child.try_wait()?.map(|status| status.code().unwrap_or(-1)))
    }

    fn terminate(&mut self, grace: Duration) -> Result<()> {
        if self.child.try_wait()?.is_some() {
            return Ok(());
        }

        #[cfg(unix)]
        if let Err(e) = send_sigterm(self.child.id()) {
            tracing::debug!("SIGTERM to {} failed: {e}", self.child.id());
        }

        let deadline = Instant::now() + grace;
        while Instant::now() < deadline {
            if self.child.try_wait()?.is_some() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }

        self.child.kill()?;
        self.child.wait()?;
        Ok(())
    }

    fn stderr_tail(&self) -> String {
        self.stderr_tail.lock().iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

#[cfg(unix)]
fn send_sigterm(pid: u32) -> std::io::Result<()> {
    use rustix::process::{Pid, Signal, kill_process};

    let raw = i32::try_from(pid).map_err(|_| std::io::Error::other("pid out of range"))?;
    let pid = Pid::from_raw(raw).ok_or_else(|| std::io::Error::other("invalid pid"))?;
    kill_process(pid, Signal::TERM).map_err(std::io::Error::from)
}

// ---------------------------------------------------------------------------------------------
// Fake runner
// ---------------------------------------------------------------------------------------------

/// One recorded call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invocation {
    pub program: String,
    pub args: Vec<String>,
}

impl Invocation {
    /// `program arg1 arg2 ...`, for readable assertions.
    pub fn command_line(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// What a [`FakeRunner`] returns for a matching call.
#[derive(Clone, Debug)]
pub enum FakeOutcome {
    Output(CommandOutput),
    /// Behave as if the program is not installed.
    NotFound,
    /// Behave as if the program hung and was killed.
    TimedOut,
}

struct FakeRule {
    program: String,
    args: Vec<String>,
    prefix: bool,
    outcomes: VecDeque<FakeOutcome>,
    /// Only active while a spawned child of this program is running.
    while_running: Option<String>,
}

impl FakeRule {
    fn matches(&self, program: &str, args: &[String], running: &HashSet<String>) -> bool {
        if self.program != program {
            return false;
        }
        if self.while_running.as_ref().is_some_and(|p| !running.contains(p)) {
            return false;
        }
        if self.prefix {
            args.len() >= self.args.len() && args[..self.args.len()] == self.args[..]
        } else {
            args == self.args.as_slice()
        }
    }

    fn next(&mut self) -> FakeOutcome {
        if self.outcomes.len() > 1 {
            if let Some(outcome) = self.outcomes.pop_front() {
                return outcome;
            }
        }
        self.outcomes
            .front()
            .cloned()
            .unwrap_or(FakeOutcome::Output(CommandOutput::ok("")))
    }
}

#[derive(Default)]
struct FakeChildState {
    program: String,
    exit_code: Option<i32>,
    stdout: Option<Sender<String>>,
}

#[derive(Default)]
struct FakeState {
    rules: Vec<FakeRule>,
    missing: HashSet<String>,
    calls: Vec<Invocation>,
    spawns: Vec<Invocation>,
    spawn_stdout: HashMap<String, Vec<String>>,
    spawn_failures: HashSet<String>,
    exit_on_spawn: HashMap<String, (usize, i32)>,
    children: Vec<Arc<Mutex<FakeChildState>>>,
    next_pid: u32,
}

/// Scriptable [`CommandRunner`] for tests.
///
/// Rules are checked newest first, so a test can override an earlier response. A call that
/// matches no rule succeeds with empty output. Clones share state, so a test can keep one
/// clone for assertions after handing another to the mixer.
#[derive(Clone, Default)]
pub struct FakeRunner {
    state: Arc<Mutex<FakeState>>,
}

impl FakeRunner {
    pub fn new() -> Self {
        Self::default()
    }

    fn add_rule(&self, program: &str, args: &[&str], prefix: bool, outcomes: Vec<FakeOutcome>) {
        self.state.lock().rules.push(FakeRule {
            program: program.to_string(),
            args: args.iter().map(|a| (*a).to_string()).collect(),
            prefix,
            outcomes: outcomes.into(),
            while_running: None,
        });
    }

    /// Respond to an exact call only while a spawned child of `spawned` is running (e.g. list
    /// the mixer's nodes only while its `pipewire` child is up).
    pub fn on_while_running(&self, spawned: &str, program: &str, args: &[&str], output: CommandOutput) {
        self.state.lock().rules.push(FakeRule {
            program: program.to_string(),
            args: args.iter().map(|a| (*a).to_string()).collect(),
            prefix: false,
            outcomes: VecDeque::from([FakeOutcome::Output(output)]),
            while_running: Some(spawned.to_string()),
        });
    }

    /// The next `count` spawns of `program` produce children that have already exited with
    /// `code`, as if the program crashed during start-up.
    pub fn exit_on_spawn(&self, program: &str, count: usize, code: i32) {
        self.state
            .lock()
            .exit_on_spawn
            .insert(program.to_string(), (count, code));
    }

    /// Respond to `program` called with exactly `args`.
    pub fn on(&self, program: &str, args: &[&str], output: CommandOutput) {
        self.add_rule(program, args, false, vec![FakeOutcome::Output(output)]);
    }

    /// Respond to `program` called with arguments starting with `args_prefix`.
    pub fn on_prefix(&self, program: &str, args_prefix: &[&str], output: CommandOutput) {
        self.add_rule(program, args_prefix, true, vec![FakeOutcome::Output(output)]);
    }

    /// Respond to successive exact calls with successive outcomes; the last one repeats.
    pub fn on_sequence(&self, program: &str, args: &[&str], outcomes: Vec<FakeOutcome>) {
        self.add_rule(program, args, false, outcomes);
    }

    /// Respond to an exact call with a non-output outcome (missing program, timeout).
    pub fn on_outcome(&self, program: &str, args: &[&str], outcome: FakeOutcome) {
        self.add_rule(program, args, false, vec![outcome]);
    }

    /// Every call to `program` (run or spawn) fails as "not installed".
    pub fn set_missing(&self, program: &str) {
        self.state.lock().missing.insert(program.to_string());
    }

    /// Spawning `program` fails with an I/O error other than "not found".
    pub fn set_spawn_failure(&self, program: &str) {
        self.state.lock().spawn_failures.insert(program.to_string());
    }

    /// Lines a spawned `program` writes to stdout. The stream stays open until the child exits.
    pub fn set_spawn_stdout(&self, program: &str, lines: &[&str]) {
        self.state
            .lock()
            .spawn_stdout
            .insert(program.to_string(), lines.iter().map(|l| (*l).to_string()).collect());
    }

    pub fn calls(&self) -> Vec<Invocation> {
        self.state.lock().calls.clone()
    }

    /// Calls to `program`, rendered as command lines.
    pub fn command_lines(&self, program: &str) -> Vec<String> {
        self.state
            .lock()
            .calls
            .iter()
            .filter(|c| c.program == program)
            .map(Invocation::command_line)
            .collect()
    }

    pub fn spawns(&self) -> Vec<Invocation> {
        self.state.lock().spawns.clone()
    }

    pub fn clear_calls(&self) {
        let mut state = self.state.lock();
        state.calls.clear();
        state.spawns.clear();
    }

    /// Write `line` to the stdout of every running child of `program`.
    pub fn push_stdout(&self, program: &str, line: &str) {
        for child in &self.state.lock().children {
            let child = child.lock();
            if child.program != program {
                continue;
            }
            if let Some(tx) = &child.stdout {
                if tx.send(line.to_string()).is_err() {
                    tracing::trace!("fake child stdout receiver dropped");
                }
            }
        }
    }

    /// Make every running child of `program` exit with `code`, as if it crashed.
    pub fn crash_children(&self, program: &str, code: i32) {
        for child in &self.state.lock().children {
            let mut child = child.lock();
            if child.program == program && child.exit_code.is_none() {
                child.exit_code = Some(code);
                child.stdout = None;
            }
        }
    }

    /// Number of spawned children of `program` that have not exited.
    pub fn running_children(&self, program: &str) -> usize {
        self.state
            .lock()
            .children
            .iter()
            .filter(|c| {
                let c = c.lock();
                c.program == program && c.exit_code.is_none()
            })
            .count()
    }
}

fn fake_not_found(program: &str) -> Error {
    Error::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("{program}: not found"),
    ))
}

impl CommandRunner for FakeRunner {
    fn run(&self, program: &str, args: &[String]) -> Result<CommandOutput> {
        let mut state = self.state.lock();
        state.calls.push(Invocation {
            program: program.to_string(),
            args: args.to_vec(),
        });
        if state.missing.contains(program) {
            return Err(fake_not_found(program));
        }
        let running: HashSet<String> = state
            .children
            .iter()
            .filter_map(|c| {
                let c = c.lock();
                c.exit_code.is_none().then(|| c.program.clone())
            })
            .collect();
        let outcome = state
            .rules
            .iter_mut()
            .rev()
            .find(|rule| rule.matches(program, args, &running))
            .map(FakeRule::next)
            .unwrap_or(FakeOutcome::Output(CommandOutput::ok("")));
        match outcome {
            FakeOutcome::Output(output) => Ok(output),
            FakeOutcome::NotFound => Err(fake_not_found(program)),
            FakeOutcome::TimedOut => Err(Error::Audio(format!("`{program}` did not finish (fake timeout)"))),
        }
    }

    fn spawn(&self, program: &str, args: &[String]) -> Result<ChildHandle> {
        let mut state = self.state.lock();
        state.spawns.push(Invocation {
            program: program.to_string(),
            args: args.to_vec(),
        });
        if state.missing.contains(program) {
            return Err(fake_not_found(program));
        }
        if state.spawn_failures.contains(program) {
            return Err(Error::Io(std::io::Error::other(format!("{program}: spawn failed"))));
        }

        let (tx, rx) = mpsc::channel();
        for line in state.spawn_stdout.get(program).cloned().unwrap_or_default() {
            if tx.send(line).is_err() {
                break;
            }
        }
        state.next_pid += 1;
        let pid = 10_000 + state.next_pid;
        let mut exit_code = None;
        if let Some((remaining, code)) = state.exit_on_spawn.get_mut(program) {
            if *remaining > 0 {
                *remaining -= 1;
                exit_code = Some(*code);
            }
        }
        let child = Arc::new(Mutex::new(FakeChildState {
            program: program.to_string(),
            stdout: if exit_code.is_some() { None } else { Some(tx) },
            exit_code,
        }));
        state.children.push(Arc::clone(&child));
        Ok(ChildHandle::new(pid, Box::new(FakeChild { state: child }), Some(rx)))
    }
}

struct FakeChild {
    state: Arc<Mutex<FakeChildState>>,
}

impl ChildControl for FakeChild {
    fn try_wait(&mut self) -> Result<Option<i32>> {
        Ok(self.state.lock().exit_code)
    }

    fn terminate(&mut self, _grace: Duration) -> Result<()> {
        let mut state = self.state.lock();
        if state.exit_code.is_none() {
            state.exit_code = Some(-1);
        }
        state.stdout = None;
        Ok(())
    }

    fn stderr_tail(&self) -> String {
        let state = self.state.lock();
        match state.exit_code {
            Some(code) => format!("{} exited with code {code}", state.program),
            None => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|i| (*i).to_string()).collect()
    }

    #[test]
    fn fake_runner_matches_newest_rule_first() {
        let fake = FakeRunner::new();
        fake.on_prefix("pactl", &["info"], CommandOutput::ok("old"));
        fake.on("pactl", &["info"], CommandOutput::ok("new"));
        let out = fake.run("pactl", &s(&["info"])).unwrap();
        assert_eq!(out.stdout, "new");
        assert_eq!(fake.command_lines("pactl"), vec!["pactl info"]);
    }

    #[test]
    fn fake_runner_sequence_repeats_last() {
        let fake = FakeRunner::new();
        fake.on_sequence(
            "x",
            &[],
            vec![
                FakeOutcome::Output(CommandOutput::ok("1")),
                FakeOutcome::Output(CommandOutput::ok("2")),
            ],
        );
        let outs: Vec<String> = (0..3).map(|_| fake.run("x", &[]).unwrap().stdout).collect();
        assert_eq!(outs, vec!["1", "2", "2"]);
    }

    #[test]
    fn fake_runner_reports_missing_and_timeouts() {
        let fake = FakeRunner::new();
        fake.set_missing("wpctl");
        let err = fake.run("wpctl", &s(&["status"])).unwrap_err();
        assert!(is_not_found(&err));
        fake.on_outcome("pactl", &["info"], FakeOutcome::TimedOut);
        let err = fake.run("pactl", &s(&["info"])).unwrap_err();
        assert!(!is_not_found(&err));
    }

    #[test]
    fn fake_child_lifecycle_and_stdout() {
        let fake = FakeRunner::new();
        fake.set_spawn_stdout("pactl", &["Event 'new' on sink-input #5"]);
        let mut child = fake.spawn("pactl", &s(&["subscribe"])).unwrap();
        let rx = child.take_stdout().unwrap();
        assert_eq!(rx.recv().unwrap(), "Event 'new' on sink-input #5");
        assert!(child.is_running());
        assert_eq!(fake.running_children("pactl"), 1);
        child.terminate(Duration::ZERO).unwrap();
        assert!(!child.is_running());
        assert_eq!(fake.running_children("pactl"), 0);
        assert!(rx.recv().is_err(), "stdout closes when the child exits");
    }

    #[test]
    fn dropping_a_handle_terminates_the_child() {
        let fake = FakeRunner::new();
        let child = fake.spawn("pipewire", &s(&["-c", "x.conf"])).unwrap();
        assert_eq!(fake.running_children("pipewire"), 1);
        drop(child);
        assert_eq!(fake.running_children("pipewire"), 0);
    }
}
