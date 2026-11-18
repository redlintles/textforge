use crate::{
    parser::resolve_var::TokenWrapper,
    utils::errors::{
        TextForgeError,
        TextForgeErrorCode::{self, VariableNotFound},
    },
};
use std::path::Path;
use std::{borrow::Cow, collections::HashMap, sync::Arc};

use super::{
    debug_trace::{DebugTrace, MAX_LISTED_REPLACEMENTS, Replacement, Size},
    static_context::StaticContext,
};
#[derive(Clone)]
pub enum VarValues {
    String(String),
    Usize(usize),
    Token(TokenWrapper),
}

#[cfg(feature = "test_access")]
pub enum ToClean {
    Block(String),
    Var(String),
}

/// Replacements recorded for the debug trace: at most
/// [`MAX_LISTED_REPLACEMENTS`] entries plus the total count. Character offsets
/// advance incrementally, so recording is O(n) over the whole pass.
#[derive(Default)]
struct ReplacementLog {
    listed: Vec<Replacement>,
    total: usize,
    scanned_bytes: usize,
    scanned_chars: usize,
}

impl ReplacementLog {
    fn record(
        &mut self,
        input: &str,
        rule: usize,
        regex: &regex::Regex,
        found: regex::Match<'_>,
        replacement: &str,
    ) {
        self.total += 1;
        if self.listed.len() == MAX_LISTED_REPLACEMENTS {
            return;
        }
        self.scanned_chars += input[self.scanned_bytes..found.start()].chars().count();
        self.scanned_bytes = found.start();
        let start = self.scanned_chars;
        self.listed.push(Replacement {
            rule: rule + 1,
            pattern: regex.as_str().to_owned(),
            original: found.as_str().to_owned(),
            replacement: replacement.to_owned(),
            start,
            end: start + found.as_str().chars().count(),
        });
    }
}

/// A `tryi` snapshot, from [`GlobalExecutionContext::snapshot`]: the state of
/// `end`, and a level of the undo journal, which records the value before
/// the first change of each variable, block and replacement map while the
/// snapshot is open. Nothing is copied when it opens, and without an open
/// snapshot a change costs one check. Give it back with `restore` (the
/// instruction failed) or `release` (it succeeded), innermost first.
#[must_use]
pub(crate) struct Snapshot {
    is_halted: bool,
}

/// One value before its first change under an open snapshot.
enum Undo {
    Variable(String, Option<VarEntry>),
    Block(String, Option<Arc<Vec<TokenWrapper>>>),
    ReplaceMap(String, Option<Vec<(Arc<regex::Regex>, String)>>),
}

/// What a snapshot level already recorded, so each name is recorded once.
#[derive(Default)]
struct Level {
    mark: usize,
    /// Names recorded: variables, blocks, replacement maps.
    recorded: [std::collections::HashSet<String>; 3],
}

#[derive(Default)]
struct UndoJournal {
    entries: Vec<Undo>,
    levels: Vec<Level>,
}

impl UndoJournal {
    /// Whether the change of `name` (of kind `kind`) must be recorded: a
    /// snapshot is open and the name was not recorded since.
    fn wants(&mut self, kind: usize, name: &str) -> bool {
        match self.levels.last_mut() {
            None => false,
            Some(level) if level.recorded[kind].contains(name) => false,
            Some(level) => level.recorded[kind].insert(name.to_owned()),
        }
    }
}

fn same_value(a: &VarValues, b: &VarValues) -> bool {
    match (a, b) {
        (VarValues::String(a), VarValues::String(b)) => a == b,
        (VarValues::Usize(a), VarValues::Usize(b)) => a == b,
        (VarValues::Token(a), VarValues::Token(b)) => {
            a.to_textforge_line() == b.to_textforge_line()
        }
        _ => false,
    }
}

// First thought of a simple hashmap, but it wouldn't suffice my needs
#[allow(dead_code)]
#[derive(Clone)]
pub struct VarEntry {
    pub value: VarValues,
    pub mutable: bool,
}
// This Object will be re-created every time the program starts.
// Some tokens could access this object for additional data
pub struct GlobalExecutionContext {
    variables: HashMap<String, VarEntry>,
    /// Shared, so `cblk` and the undo journal hold a block without copying it.
    blocks: HashMap<String, Arc<Vec<TokenWrapper>>>,
    replace_maps: HashMap<String, Vec<(std::sync::Arc<regex::Regex>, String)>>,
    /// What changed since each open `tryi` snapshot (see [`Snapshot`]).
    undo: UndoJournal,
    pub is_halted: bool,
    /// Nested step trace; `Some` only during debug processing.
    trace: Option<DebugTrace>,
    /// Read-only facts about the run, shared with concurrent tasks.
    static_ctx: Arc<StaticContext>,
    /// Limits for instructions that multiply work (from the profile).
    limits: crate::api::Limits,
    /// Directory of the pipeline file being run, where relative paths of the
    /// file instructions start; `None` for pipelines built in code.
    base_dir: Option<Arc<Path>>,
}

impl Default for GlobalExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

// Variable Concept

// val {name} = {value}; for immutable variables
// var {name} = {value}; for mutable variables
// then the user could reference the variable by ${name} syntax
// And alter it throught mut {name} = {new_value}; instruction if it's mutable.

// Block concept

// blk {name} assoc {instruction};
// if {name} block doesn't exist yet, it's created
// The instruction will be parsed to a Box<dyn InstructionMethods> and then,
// added to the {name} block in the Context's blocks hashmap.
// If the user wish to add multiple instructions to a single block, it should do one per `blk assoc` line.
// Once the user is done with composing a block
// cblk {name}; will execute all instructions stored in the {name} block;

pub trait GlobalContextMethods {
    fn add_to_block(&mut self, block_id: &str, token: TokenWrapper) -> Result<(), TextForgeError>;
    fn get_formatted_block_items(&mut self, block_id: &str) -> Result<String, TextForgeError>;

    fn add_var(&mut self, id: &str, var_entry: VarEntry) -> Result<(), TextForgeError>;
    fn rm_var(&mut self, var_id: &str) -> Result<(), TextForgeError>;
    fn get_var(&self, var_id: &str) -> Result<&VarEntry, TextForgeError>;
    fn get_mut_var(&mut self, var_id: &str) -> Result<&mut VarEntry, TextForgeError>;

    // It would require a more complex implementation. but would help optimizing textforge in the future. This will remove data that will no longer be used from the context.
    #[cfg(feature = "test_access")]
    fn clean_context(&mut self) {}
    // `cblk` uses the shared forms (`take_shared_block`); these stay for tests.
    #[cfg_attr(not(feature = "test_access"), allow(dead_code))]
    fn take_block(&mut self, block_id: &str) -> Result<Vec<TokenWrapper>, TextForgeError>;
    #[cfg_attr(not(feature = "test_access"), allow(dead_code))]
    fn put_block(&mut self, block_id: &str, block: Vec<TokenWrapper>);
    fn halt(&mut self) -> bool;
}

impl GlobalExecutionContext {
    pub(crate) fn define_replace_rule(
        &mut self,
        name: &str,
        regex: std::sync::Arc<regex::Regex>,
        replacement: String,
    ) {
        self.record_replace_map(name);
        let rules = self.replace_maps.entry(name.to_owned()).or_default();
        if let Some(rule) = rules
            .iter_mut()
            .find(|(existing, _)| existing.as_str() == regex.as_str())
        {
            rule.1 = replacement;
        } else {
            rules.push((regex, replacement));
        }
    }

    pub(crate) fn execute_replace_map<'a>(
        &mut self,
        name: &str,
        input: Cow<'a, str>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if !self.trace.as_ref().is_some_and(DebugTrace::will_show) {
            return self.replace_with_map(name, input, None);
        }
        let mut log = ReplacementLog::default();
        let before = Size::of(&input);
        let output = self.replace_with_map(name, input, Some(&mut log))?;
        let rules = self.replace_maps.get(name).map_or(0, Vec::len);
        if let Some(trace) = self.trace.as_mut() {
            let after = Size::of(&output);
            trace.replace_map(name, rules, &log.listed, log.total, before, after);
        }
        Ok(output)
    }

    /// One non-cascading pass over the original input. When `log` is given,
    /// each replacement is recorded from this same pass (debug only).
    fn replace_with_map<'a>(
        &self,
        name: &str,
        input: Cow<'a, str>,
        mut log: Option<&mut ReplacementLog>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let rules = self.replace_maps.get(name).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::ValidationError(
                    format!("no replacement map is named `{name}`; define it with `rmapd` before `rmape`").into(),
                ),
                "",
                name.to_owned(),
            )
        })?;
        // All candidates refer to the original input. Re-search from the new
        // cursor when a competing match overlaps: advancing an independent
        // captures_iter could skip valid overlapping occurrences of that rule.
        let mut matches: Vec<_> = rules
            .iter()
            .map(|(regex, _)| regex.captures(&input))
            .collect();
        let mut output = String::new();
        let mut cursor = 0;
        let mut last_end = None;
        loop {
            let mut chosen = None;
            for (index, candidate) in matches.iter_mut().enumerate() {
                if candidate
                    .as_ref()
                    .and_then(|caps| caps.get(0))
                    .is_some_and(|m| {
                        m.start() < cursor || (m.is_empty() && last_end == Some(m.start()))
                    })
                {
                    *candidate = rules[index].0.captures_at(&input, cursor);
                    if candidate
                        .as_ref()
                        .and_then(|caps| caps.get(0))
                        .is_some_and(|m| m.is_empty() && last_end == Some(m.start()))
                    {
                        *candidate = input[cursor..].chars().next().and_then(|c| {
                            rules[index].0.captures_at(&input, cursor + c.len_utf8())
                        });
                    }
                }
                if let Some(m) = candidate.as_ref().and_then(|caps| caps.get(0))
                    && chosen.is_none_or(|(_, start)| m.start() < start)
                {
                    chosen = Some((index, m.start()));
                }
            }
            let Some((index, _)) = chosen else {
                break;
            };
            if let Some(caps) = matches[index].take()
                && let Some(m) = caps.get(0)
            {
                output.push_str(&input[cursor..m.start()]);
                let replaced_from = output.len();
                caps.expand(&rules[index].1, &mut output);
                if let Some(log) = log.as_deref_mut() {
                    log.record(&input, index, &rules[index].0, m, &output[replaced_from..]);
                }
                cursor = m.end();
                last_end = Some(cursor);
            }
            let search = if last_end == Some(cursor) && cursor == input.len() {
                None
            } else {
                Some(cursor)
            };
            matches[index] = search.and_then(|at| rules[index].0.captures_at(&input, at));
        }
        drop(matches);
        if last_end.is_none() {
            return Ok(input);
        }
        output.push_str(&input[cursor..]);
        Ok(Cow::Owned(output))
    }

    /// The state an instruction can change besides the text: variables,
    /// blocks, replacement maps and whether `end` ran. `tryi` takes it before
    /// an instruction that runs others and gives it back when that
    /// instruction fails, so a failed instruction leaves no trace.
    pub(crate) fn snapshot(&mut self) -> Snapshot {
        self.undo.levels.push(Level {
            mark: self.undo.entries.len(),
            recorded: Default::default(),
        });
        Snapshot {
            is_halted: self.is_halted,
        }
    }

    /// Close the innermost snapshot and keep the changes; an enclosing
    /// snapshot keeps what this one recorded.
    pub(crate) fn release(&mut self, snapshot: Snapshot) {
        let _ = snapshot;
        let Some(level) = self.undo.levels.pop() else {
            return;
        };
        match self.undo.levels.last_mut() {
            Some(outer) => {
                for (outer, inner) in outer.recorded.iter_mut().zip(level.recorded) {
                    outer.extend(inner);
                }
            }
            None => self.undo.entries.clear(),
        }
    }

    // Called on every change of the context: the check that no snapshot is
    // open is inlined, the recording is not.
    #[inline(always)]
    fn record_variable(&mut self, name: &str) {
        if !self.undo.levels.is_empty() {
            self.record_variable_slow(name);
        }
    }

    #[cold]
    #[inline(never)]
    fn record_variable_slow(&mut self, name: &str) {
        if self.undo.wants(0, name) {
            let before = self.variables.get(name).cloned();
            self.undo
                .entries
                .push(Undo::Variable(name.to_owned(), before));
        }
    }

    #[inline(always)]
    fn record_block(&mut self, name: &str) {
        if !self.undo.levels.is_empty() {
            self.record_block_slow(name);
        }
    }

    #[cold]
    #[inline(never)]
    fn record_block_slow(&mut self, name: &str) {
        if self.undo.wants(1, name) {
            let before = self.blocks.get(name).cloned();
            self.undo.entries.push(Undo::Block(name.to_owned(), before));
        }
    }

    fn record_replace_map(&mut self, name: &str) {
        if self.undo.wants(2, name) {
            let before = self.replace_maps.get(name).cloned();
            self.undo
                .entries
                .push(Undo::ReplaceMap(name.to_owned(), before));
        }
    }

    /// Close the innermost snapshot and undo what changed since it opened;
    /// returns the names of the variables whose value it put back (for the
    /// debug trace), sorted.
    pub(crate) fn restore(&mut self, snapshot: Snapshot) -> Vec<String> {
        self.is_halted = snapshot.is_halted;
        let Some(level) = self.undo.levels.pop() else {
            return Vec::new();
        };
        let mut changed = Vec::new();
        // Newest first: the oldest record of a name is applied last.
        while self.undo.entries.len() > level.mark {
            let Some(entry) = self.undo.entries.pop() else {
                break;
            };
            match entry {
                Undo::Variable(name, before) => {
                    let current = self.variables.get(&name);
                    let same = match (current, &before) {
                        (Some(current), Some(before)) => same_value(&current.value, &before.value),
                        (None, None) => true,
                        _ => false,
                    };
                    if !same && !changed.contains(&name) {
                        changed.push(name.clone());
                    }
                    match before {
                        Some(entry) => self.variables.insert(name, entry),
                        None => self.variables.remove(&name),
                    };
                }
                Undo::Block(name, before) => {
                    match before {
                        Some(block) => self.blocks.insert(name, block),
                        None => self.blocks.remove(&name),
                    };
                }
                Undo::ReplaceMap(name, before) => {
                    match before {
                        Some(rules) => self.replace_maps.insert(name, rules),
                        None => self.replace_maps.remove(&name),
                    };
                }
            }
        }
        if self.undo.levels.is_empty() {
            self.undo.entries.clear();
        }
        changed.sort();
        changed
    }

    /// Take block `block_id` out to run it, shared, so the undo journal
    /// keeps it without copying its instructions. While it runs the block is
    /// out of the map, so a block cannot run itself.
    pub(crate) fn take_shared_block(
        &mut self,
        block_id: &str,
    ) -> Result<Arc<Vec<TokenWrapper>>, TextForgeError> {
        self.record_block(block_id);
        self.blocks
            .remove(block_id)
            .ok_or_else(|| block_not_found(block_id))
    }

    /// Put back a block taken with [`Self::take_shared_block`].
    pub(crate) fn put_shared_block(&mut self, block_id: &str, block: Arc<Vec<TokenWrapper>>) {
        self.record_block(block_id);
        self.blocks.insert(block_id.to_owned(), block);
    }

    /// Remove a variable and return its entry, if it was defined.
    /// Used by instructions that temporarily shadow a variable.
    pub(crate) fn take_var(&mut self, id: &str) -> Option<VarEntry> {
        self.record_variable(id);
        self.variables.remove(id)
    }

    pub fn new() -> Self {
        GlobalExecutionContext {
            variables: HashMap::new(),
            blocks: HashMap::new(),
            replace_maps: HashMap::new(),
            undo: UndoJournal::default(),
            is_halted: false,
            trace: None,
            static_ctx: StaticContext::shared_default(),
            limits: crate::api::Limits::default(),
            base_dir: None,
        }
    }

    /// Fail when `name` was defined with `val`: a `val` cannot be defined
    /// again, by `val` or `var`. Up to 0.3.1 a second definition replaced it,
    /// so `val` only stopped `mutv` and `eval … in`.
    pub(crate) fn check_not_immutable(&self, name: &str) -> Result<(), TextForgeError> {
        match self.variables.get(name) {
            Some(entry) if !entry.mutable => Err(TextForgeError::new(
                TextForgeErrorCode::NonMutableVariableError(
                    format!("the variable `{name}` cannot be defined again: it was defined with `val`; define it with `var` where it is first defined to make it mutable").into(),
                ),
                "",
                name.to_owned(),
            )),
            _ => Ok(()),
        }
    }

    /// Set the directory relative paths of `wtf`, `apf`, `ppf` and `inf`
    /// start from: the directory of the pipeline file.
    pub(crate) fn set_base_dir(&mut self, base_dir: Option<Arc<Path>>) {
        self.base_dir = base_dir;
    }

    /// The directory relative file paths start from, if the pipeline came from
    /// a file.
    pub(crate) fn base_dir(&self) -> Option<&Arc<Path>> {
        self.base_dir.as_ref()
    }

    /// A fresh context reading the given static facts (pipeline name, input file).
    pub(crate) fn with_static(static_ctx: Arc<StaticContext>) -> Self {
        GlobalExecutionContext {
            static_ctx,
            ..Self::new()
        }
    }

    /// Limits for instructions that multiply work (`rpt`, `padl`, `padr`,
    /// `iter`); the processor sets them from its `ExecutionProfile`.
    pub fn limits(&self) -> crate::api::Limits {
        self.limits
    }

    pub(crate) fn set_limits(&mut self, limits: crate::api::Limits) {
        self.limits = limits;
    }

    /// The shared static context, for a joined pipeline (`ppjn`) to run with.
    pub(crate) fn shared_static(&self) -> Arc<StaticContext> {
        Arc::clone(&self.static_ctx)
    }

    /// Whether debug tracing is on.
    #[cfg(feature = "buffered")]
    pub(crate) fn is_tracing(&self) -> bool {
        self.trace.is_some()
    }

    /// Move the debug trace out, to continue it in another context (`ppjn`).
    pub(crate) fn take_trace(&mut self) -> Option<DebugTrace> {
        self.trace.take()
    }

    /// Continue a debug trace taken from another context.
    pub(crate) fn put_trace(&mut self, trace: DebugTrace) {
        self.trace = Some(trace);
    }

    /// Read-only facts about this run (see [`StaticContext`]).
    pub fn static_context(&self) -> &StaticContext {
        &self.static_ctx
    }

    /// Record nested steps for debug output (see [`DebugTrace`]).
    pub(crate) fn enable_trace(&mut self) {
        self.trace = Some(DebugTrace::default());
    }

    /// The nested step trace, when debug processing enabled it.
    pub(crate) fn trace_mut(&mut self) -> Option<&mut DebugTrace> {
        self.trace.as_mut()
    }

    /// Rendered trace lines since the last call; empty when tracing is off.
    pub(crate) fn take_trace_lines(&mut self) -> Vec<String> {
        self.trace
            .as_mut()
            .map(DebugTrace::take_lines)
            .unwrap_or_default()
    }

    /// The summary of the errors `tryi` ignored in this debug run, if any.
    pub(crate) fn ignored_summary(&self) -> Option<String> {
        self.trace.as_ref().and_then(DebugTrace::ignored_summary)
    }
}

impl GlobalContextMethods for GlobalExecutionContext {
    fn halt(&mut self) -> bool {
        self.is_halted = true;
        self.is_halted
    }

    fn add_to_block(&mut self, block_id: &str, token: TokenWrapper) -> Result<(), TextForgeError> {
        self.record_block(block_id);
        Arc::make_mut(self.blocks.entry(block_id.to_owned()).or_default()).push(token);

        Ok(())
    }

    fn take_block(&mut self, block_id: &str) -> Result<Vec<TokenWrapper>, TextForgeError> {
        self.take_shared_block(block_id)
            .map(|block| Arc::try_unwrap(block).unwrap_or_else(|shared| shared.as_ref().clone()))
    }

    fn put_block(&mut self, block_id: &str, block: Vec<TokenWrapper>) {
        self.put_shared_block(block_id, Arc::new(block));
    }

    fn get_formatted_block_items(&mut self, block_id: &str) -> Result<String, TextForgeError> {
        use colored::Colorize;

        let block_items = self.blocks.get(block_id).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::BlockNotFound(
                    format!("no block is named `{block_id}`; define it with `blk` before running it with `cblk`").into(),
                ),
                "",
                block_id.to_owned(),
            )
        })?;
        let mut result = String::new();

        let len = block_items.len();
        if len == 0 {
            result.push_str("\t\t\t\t(EMPTY BLOCK)\n");
            return Ok(result);
        }

        for (i, token) in block_items.iter().enumerate() {
            let is_last = i + 1 == len;

            let prefix = if is_last {
                if len == 1 {
                    "(BLOCK CREATED): ".green()
                } else {
                    "(BLOCK ALREADY EXISTS) ADDING: ".green()
                }
            } else {
                // no prefix for older items
                "".normal()
            };

            result.push_str(&format!(
                "\t\t\t\t{}{}\n",
                prefix,
                token.to_textforge_line().yellow()
            ));
        }

        Ok(result)
    }

    fn add_var(&mut self, id: &str, var_entry: VarEntry) -> Result<(), TextForgeError> {
        self.record_variable(id);
        self.variables.insert(id.to_string(), var_entry);
        Ok(())
    }
    fn rm_var(&mut self, var_id: &str) -> Result<(), TextForgeError> {
        let var_id_owned = var_id.to_owned();
        self.record_variable(var_id);

        self.variables.remove(var_id).ok_or_else(|| {
            TextForgeError::new(
                VariableNotFound(Cow::from(format!(
                    "Could not find var {} to remove",
                    var_id_owned
                ))),
                Cow::from("context.rm_var"),
                Cow::from(var_id_owned),
            )
        })?;

        Ok(())
    }

    fn get_var(&self, var_id: &str) -> Result<&VarEntry, TextForgeError> {
        self.variables.get(var_id).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::VariableNotFound(
                    format!("no variable is named `{var_id}`; define it with `var` or `val` before using it").into(),
                ),
                "",
                var_id.to_string(),
            )
        })
    }

    fn get_mut_var(&mut self, var_id: &str) -> Result<&mut VarEntry, TextForgeError> {
        self.record_variable(var_id);
        let v = self.variables.get_mut(var_id).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::VariableNotFound(
                    format!(
                        "no variable is named `{var_id}`; define it with `var` before changing it"
                    )
                    .into(),
                ),
                "",
                var_id.to_string(),
            )
        })?;
        if v.mutable {
            Ok(v)
        } else {
            Err(TextForgeError::new(
                TextForgeErrorCode::NonMutableVariableError(
                    format!("the variable `{var_id}` cannot be changed: it was defined with `val`; define it with `var` where it is first defined to make it mutable").into(),
                ),
                "",
                var_id.to_string(),
            ))
        }
    }
}

fn block_not_found(block_id: &str) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::BlockNotFound(
            format!(
                "no block is named `{block_id}`; define it with `blk` before running it with `cblk`"
            )
            .into(),
        ),
        "",
        block_id.to_string(),
    )
}
