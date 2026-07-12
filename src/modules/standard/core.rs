// Core module for Forthic
//
// Essential interpreter operations for stack manipulation, variables, and control flow.
//
// ## Categories
// - Stack: DROP, DUP, SWAP
// - Variables: VARIABLES, !, @, !@
// - Control: NOP, NULL, ARRAY?, DEFAULT, DEFAULT-RUN, IF, IF-RUN, WHEN
// - Execution: RUN
// - Predicates: NULL?, EMPTY?, STRING?, NUMBER?, RECORD?
// - Debug: PEEK!, STACK!
// - Errors: TRY, OK?, ERROR?, UNWRAP, UNWRAP-OR (Rust Result semantics:
//   'CODE' TRY UNWRAP is CODE — mirrored with forthic-ts)
// - Options: ~> (converts array to WordOptions)

use crate::errors::ForthicError;
use crate::literals::ForthicValue;
use crate::module::{register_words, InterpreterContext, Module, ModuleWord};
use crate::word_options::WordOptions;
use indexmap::IndexMap;
use std::sync::Arc;

/// Build an `{"ok": payload}` outcome record (TRY / MAP outcomes)
pub(crate) fn ok_outcome(payload: ForthicValue) -> ForthicValue {
    let mut outcome = IndexMap::new();
    outcome.insert("ok".to_string(), payload);
    ForthicValue::Record(outcome)
}

/// Build an `{"error": {message, error_type}}` outcome record. The error
/// info uses the same field names as the JSON-RPC wire ErrorInfo — one
/// error representation everywhere.
pub(crate) fn error_outcome(e: &ForthicError) -> ForthicValue {
    let mut info = IndexMap::new();
    info.insert("message".to_string(), ForthicValue::String(e.to_string()));
    info.insert(
        "error_type".to_string(),
        ForthicValue::String(e.type_name().to_string()),
    );
    let mut outcome = IndexMap::new();
    outcome.insert("error".to_string(), ForthicValue::Record(info));
    ForthicValue::Record(outcome)
}

/// CoreModule provides core interpreter operations
pub struct CoreModule {
    module: Module,
}

/// Resolved INTERPOLATE/PRINT options
struct InterpOptions {
    separator: String,
    null_text: String,
    json: bool,
}

impl CoreModule {
    /// Create a new CoreModule
    pub fn new() -> Self {
        let mut module = Module::new("core".to_string());

        // Register all words
        Self::register_stack_words(&mut module);
        Self::register_variable_words(&mut module);
        Self::register_control_words(&mut module);
        Self::register_error_words(&mut module);
        Self::register_flow_words(&mut module);
        Self::register_predicate_words(&mut module);
        Self::register_debug_words(&mut module);
        Self::register_output_words(&mut module);
        Self::register_options_words(&mut module);

        Self { module }
    }

    // ===== Output & Interpolation (Batch 4) =====
    //
    // The ONE interpolation grammar (settled with Rino 2026-07-11):
    // `${name}` holes, like ts template literals — but holes are VARIABLE
    // NAMES ONLY, never expressions. Interpolation is injection-safe by
    // construction: rendering a template can never execute words (the same
    // reasoning that made JQ paths data instead of interpolated source).
    // Computation belongs on the stack: `... .total ! "Sum: ${total}"`.

    fn register_output_words(module: &mut Module) {
        register_words!(module, {
            "INTERPOLATE" => Self::word_interpolate,
            "PRINT" => Self::word_print,
            "USE-MODULES" => Self::word_use_modules,
        });
    }

    /// USE-MODULES: ( names [options] -- ) — import registered modules
    /// into the app module. Each entry is either a name string or a
    /// `[name prefix]` pair. `[.prefixed TRUE] ~>` prefixes plain names
    /// with themselves; an explicit pair prefix ALWAYS wins over the
    /// option (ts contract). NULL names is a no-op; an unregistered name
    /// errors with UnknownModule.
    fn word_use_modules(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let options = Self::pop_word_options(context);
        let prefixed = options
            .as_ref()
            .and_then(|o| o.get_bool("prefixed"))
            .unwrap_or(false);
        let names = context.stack_pop()?;
        let entries = match names {
            ForthicValue::Array(entries) => entries,
            ForthicValue::Null => return Ok(()),
            other => {
                return Err(ForthicError::InvalidOperation {
                    forthic: String::new(),
                    message: format!("USE-MODULES requires an array of names, got {other:?}"),
                    location: None,
                    cause: None,
                })
            }
        };
        for entry in entries {
            let (name, prefix) = match entry {
                ForthicValue::String(name) => {
                    let prefix = if prefixed {
                        name.clone()
                    } else {
                        String::new()
                    };
                    (name, prefix)
                }
                ForthicValue::Array(pair) => match (pair.first(), pair.get(1)) {
                    (Some(ForthicValue::String(n)), Some(ForthicValue::String(p)))
                        if pair.len() == 2 =>
                    {
                        (n.clone(), p.clone())
                    }
                    _ => {
                        return Err(ForthicError::InvalidOperation {
                            forthic: String::new(),
                            message: "USE-MODULES entries must be 'name' or ['name' 'prefix']"
                                .to_string(),
                            location: None,
                            cause: None,
                        })
                    }
                },
                _ => {
                    return Err(ForthicError::InvalidOperation {
                        forthic: String::new(),
                        message: "USE-MODULES entries must be 'name' or ['name' 'prefix']"
                            .to_string(),
                        location: None,
                        cause: None,
                    })
                }
            };
            context.use_module(&name, &prefix)?;
        }
        Ok(())
    }

    /// Pop a WordOptions value if one sits on top of the stack
    fn pop_word_options(
        context: &mut dyn InterpreterContext,
    ) -> Option<crate::word_options::WordOptions> {
        if matches!(context.stack_peek(), Some(ForthicValue::WordOptions(_))) {
            if let Ok(ForthicValue::WordOptions(options)) = context.stack_pop() {
                return Some(options);
            }
        }
        None
    }

    fn pop_interp_options(context: &mut dyn InterpreterContext) -> InterpOptions {
        let options = Self::pop_word_options(context);
        let mut opts = InterpOptions {
            separator: ", ".to_string(),
            // Template-first default: an unset/NULL hole renders as
            // nothing ("Hello ${name}!" must not say "Hello null!");
            // opt into visible nulls with [.null_text 'null']
            null_text: String::new(),
            json: false,
        };
        if let Some(options) = options {
            if let Some(ForthicValue::String(s)) = options.get("separator") {
                opts.separator = s.clone();
            }
            if let Some(ForthicValue::String(s)) = options.get("null_text") {
                opts.null_text = s.clone();
            }
            if let Some(b) = options.get_bool("json") {
                opts.json = b;
            }
        }
        opts
    }

    /// INTERPOLATE: ( string [options] -- string ) — fill `${name}` holes
    /// from variables (READ-ONLY — a hole never creates a variable, unlike
    /// `@`). The dot is optional (`${.name}` == `${name}`); whitespace in
    /// the body trims; a miss or NULL renders as null_text. `\${` escapes
    /// a literal `${`. A non-name hole body is an ERROR — holes are
    /// variable names, never expressions. Options: separator (", "),
    /// null_text (""), json (FALSE). NULL template stays NULL.
    fn word_interpolate(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let opts = Self::pop_interp_options(context);
        let value = context.stack_pop()?;
        let template = match value {
            ForthicValue::Null => {
                context.stack_push(ForthicValue::Null);
                return Ok(());
            }
            ForthicValue::String(s) => s,
            other => crate::modules::standard::string::StringModule::stringify(&other),
        };
        let result = Self::interpolate_string(context, &template, &opts)?;
        context.stack_push(ForthicValue::String(result));
        Ok(())
    }

    /// PRINT: ( value [options] -- ) — print to stdout, pushing NOTHING.
    /// Strings interpolate `${name}` holes first; other values format via
    /// the same rendering rules. Reaches stdout only — safe under the
    /// jsonrpc transport (HTTP, not stdio).
    fn word_print(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let opts = Self::pop_interp_options(context);
        let value = context.stack_pop()?;
        let result = match &value {
            ForthicValue::String(s) => Self::interpolate_string(context, s, &opts)?,
            other => Self::value_to_string(other, &opts),
        };
        println!("{result}");
        Ok(())
    }

    fn interpolate_string(
        context: &mut dyn InterpreterContext,
        template: &str,
        opts: &InterpOptions,
    ) -> Result<String, ForthicError> {
        // `\${` escapes a literal `${`: swap for a NUL-fenced placeholder
        // so the hole regex can't see it, restore after
        const ESCAPED_HOLE: &str = "\x00ESCAPED_HOLE\x00";
        let escaped = template.replace("\\${", ESCAPED_HOLE);

        let hole = regex::Regex::new(r"\$\{([^{}]*)\}").expect("static regex");
        let mut result = String::new();
        let mut last = 0;
        for caps in hole.captures_iter(&escaped) {
            let whole = caps.get(0).expect("group 0");
            result.push_str(&escaped[last..whole.start()]);
            let name = Self::hole_name(caps.get(1).expect("group 1").as_str())?;
            // READ-ONLY lookup: templates render state, never mutate it —
            // a miss renders as null_text and creates nothing
            let value = context
                .find_variable_value(&name)
                .unwrap_or(ForthicValue::Null);
            result.push_str(&Self::value_to_string(&value, opts));
            last = whole.end();
        }
        result.push_str(&escaped[last..]);
        Ok(result.replace(ESCAPED_HOLE, "${"))
    }

    /// Validate a hole body into a variable name. Holes are names ONLY —
    /// `${1 + 2}` is a hard error, not a template feature, so interpolation
    /// can never execute code (injection-safe by construction). `__` names
    /// are reserved, same as `!` / `@`.
    fn hole_name(body: &str) -> Result<String, ForthicError> {
        let trimmed = body.trim();
        let name = trimmed.strip_prefix('.').unwrap_or(trimmed);
        let valid = !name.is_empty()
            && name
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !valid {
            return Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: format!(
                    "Invalid interpolation hole '${{{body}}}': holes are variable names \
                     (${{name}} or ${{.name}}), not expressions. \
                     Escape a literal with \\${{"
                ),
                location: None,
                cause: None,
            });
        }
        if name.starts_with("__") {
            return Err(ForthicError::InvalidVariableName {
                forthic: "".to_string(),
                varname: name.to_string(),
                location: None,
                cause: None,
            });
        }
        Ok(name.to_string())
    }

    /// Shared value rendering for INTERPOLATE/PRINT: NULL -> null_text;
    /// json option -> compact JSON; arrays join with separator (elements
    /// render recursively, so NULL elements also use null_text);
    /// records -> JSON
    fn value_to_string(value: &ForthicValue, opts: &InterpOptions) -> String {
        match value {
            ForthicValue::Null => opts.null_text.clone(),
            _ if opts.json => {
                crate::modules::standard::json::JSONModule::forthic_to_json(value).to_string()
            }
            ForthicValue::Array(items) => items
                .iter()
                .map(|v| Self::value_to_string(v, opts))
                .collect::<Vec<_>>()
                .join(&opts.separator),
            other => crate::modules::standard::string::StringModule::stringify(other),
        }
    }

    // ===== Control Flow & Execution =====

    fn register_flow_words(module: &mut Module) {
        for (name, handler) in [
            (
                "RUN",
                Self::word_run as fn(&mut dyn InterpreterContext) -> Result<(), ForthicError>,
            ),
            ("IF", Self::word_if),
            ("IF-RUN", Self::word_if_run),
            ("WHEN", Self::word_when),
            ("DEFAULT-RUN", Self::word_default_run),
        ] {
            let word = Arc::new(ModuleWord::new(name.to_string(), handler));
            module.add_exportable_word(word);
        }
    }

    fn pop_forthic(
        context: &mut dyn InterpreterContext,
        word: &str,
    ) -> Result<Option<String>, ForthicError> {
        match context.stack_pop()? {
            ForthicValue::String(s) => Ok(Some(s)),
            ForthicValue::Null => Ok(None),
            other => Err(ForthicError::InvalidOperation {
                forthic: String::new(),
                message: format!("{word} requires a Forthic string, got {other:?}"),
                location: None,
                cause: None,
            }),
        }
    }

    /// RUN: ( forthic -- ? ) — run a Forthic string in the current context;
    /// whatever it produces stays on the stack
    fn word_run(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        if let Some(forthic) = Self::pop_forthic(context, "RUN")? {
            if !forthic.is_empty() {
                context.run(&forthic)?;
            }
        }
        Ok(())
    }

    /// IF: ( bool then_value else_value -- chosen ) — PURE VALUE SELECTION
    /// (post-scrub ts contract: IF does not execute anything; for lazy code
    /// execution use IF-RUN)
    fn word_if(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let else_value = context.stack_pop()?;
        let then_value = context.stack_pop()?;
        let bool_val = context.stack_pop()?;
        context.stack_push(if bool_val.is_truthy() {
            then_value
        } else {
            else_value
        });
        Ok(())
    }

    /// IF-RUN: ( bool then_forthic else_forthic -- ? ) — conditional code
    /// execution; both branches are Forthic strings (NULL = do nothing)
    fn word_if_run(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let else_forthic = Self::pop_forthic(context, "IF-RUN")?;
        let then_forthic = Self::pop_forthic(context, "IF-RUN")?;
        let bool_val = context.stack_pop()?;
        let branch = if bool_val.is_truthy() {
            then_forthic
        } else {
            else_forthic
        };
        if let Some(forthic) = branch {
            if !forthic.is_empty() {
                context.run(&forthic)?;
            }
        }
        Ok(())
    }

    /// WHEN: ( bool forthic -- ? ) — one-sided: run the code if truthy
    fn word_when(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let forthic = Self::pop_forthic(context, "WHEN")?;
        let bool_val = context.stack_pop()?;
        if bool_val.is_truthy() {
            if let Some(forthic) = forthic {
                if !forthic.is_empty() {
                    context.run(&forthic)?;
                }
            }
        }
        Ok(())
    }

    /// DEFAULT-RUN: ( value forthic -- result ) — lazy default: the forthic
    /// only runs when value is NULL or "" (its result replaces the value)
    fn word_default_run(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let forthic = Self::pop_forthic(context, "DEFAULT-RUN")?;
        let value = context.stack_pop()?;
        let is_empty = matches!(&value, ForthicValue::Null)
            || matches!(&value, ForthicValue::String(s) if s.is_empty());
        if is_empty {
            if let Some(forthic) = forthic {
                context.run(&forthic)?;
                let result = context.stack_pop()?;
                context.stack_push(result);
                return Ok(());
            }
            context.stack_push(ForthicValue::Null);
        } else {
            context.stack_push(value);
        }
        Ok(())
    }

    // ===== Predicates =====

    fn register_predicate_words(module: &mut Module) {
        for (name, handler) in [
            (
                "NULL?",
                Self::word_null_q as fn(&mut dyn InterpreterContext) -> Result<(), ForthicError>,
            ),
            ("EMPTY?", Self::word_empty_q),
            ("STRING?", Self::word_string_q),
            ("NUMBER?", Self::word_number_q),
            ("RECORD?", Self::word_record_q),
        ] {
            let word = Arc::new(ModuleWord::new(name.to_string(), handler));
            module.add_exportable_word(word);
        }
    }

    fn word_null_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let value = context.stack_pop()?;
        context.stack_push(ForthicValue::Bool(value.is_null()));
        Ok(())
    }

    /// EMPTY?: null, "", or a container with no entries
    fn word_empty_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let value = context.stack_pop()?;
        let empty = match &value {
            ForthicValue::Null => true,
            ForthicValue::String(s) => s.is_empty(),
            ForthicValue::Array(a) => a.is_empty(),
            ForthicValue::Record(r) => r.is_empty(),
            _ => false,
        };
        context.stack_push(ForthicValue::Bool(empty));
        Ok(())
    }

    fn word_string_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let value = context.stack_pop()?;
        context.stack_push(ForthicValue::Bool(matches!(value, ForthicValue::String(_))));
        Ok(())
    }

    /// NUMBER?: Infinity is a number; NaN is not (ts #31 contract)
    fn word_number_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let value = context.stack_pop()?;
        let is_number = match value {
            ForthicValue::Int(_) => true,
            ForthicValue::Float(f) => !f.is_nan(),
            _ => false,
        };
        context.stack_push(ForthicValue::Bool(is_number));
        Ok(())
    }

    fn word_record_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let value = context.stack_pop()?;
        context.stack_push(ForthicValue::Bool(matches!(value, ForthicValue::Record(_))));
        Ok(())
    }

    // ===== Debug =====

    fn register_debug_words(module: &mut Module) {
        register_words!(module, {
            "PEEK!" => Self::word_peek_bang,
            "STACK!" => Self::word_stack_bang,
        });
    }

    /// PEEK!: print top of stack and intentionally stop execution
    fn word_peek_bang(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        match context.stack_peek() {
            Some(value) => println!(
                "{}",
                crate::modules::standard::json::JSONModule::forthic_to_json(value)
            ),
            None => println!("<STACK EMPTY>"),
        }
        Err(ForthicError::IntentionalStop {
            message: "PEEK!".to_string(),
        })
    }

    /// STACK!: print the whole stack (top first) as pretty JSON and stop
    fn word_stack_bang(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let mut items = context.stack_snapshot();
        items.reverse();
        let json: Vec<_> = items
            .iter()
            .map(crate::modules::standard::json::JSONModule::forthic_to_json)
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| "<UNPRINTABLE>".to_string())
        );
        Err(ForthicError::IntentionalStop {
            message: "STACK!".to_string(),
        })
    }

    // ===== Error Handling (Rust Result semantics; see backlog item 20) =====

    fn register_error_words(module: &mut Module) {
        register_words!(module, {
            "TRY" => Self::word_try,
            "OK?" => Self::word_ok_q,
            "ERROR?" => Self::word_error_q,
            "UNWRAP" => Self::word_unwrap,
            "UNWRAP-OR" => Self::word_unwrap_or,
        });
    }

    /// TRY: ( forthic -- outcome )
    ///
    /// Runs the code, capturing the outcome as data: `{"ok": value}` on
    /// success (value = top of stack if the run changed the stack; NULL for
    /// no-net-effect code), `{"error": {message, error_type}}` on failure.
    /// Transactional for the stack: on failure it is restored to its
    /// pre-TRY state and modules left open by the failed code are unwound.
    /// Side effects (variable writes) persist — catch_unwind semantics.
    /// Law: `'CODE' TRY UNWRAP` ≡ `CODE`.
    ///
    /// For error-tolerant mapping use MAP's outcomes option
    /// (`[.outcomes TRUE] ~> MAP`): TRY inside MAP would transactionally
    /// restore the item MAP pushed, stranding it beneath the outcome.
    fn word_try(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let forthic = match context.stack_pop()? {
            ForthicValue::String(s) => s,
            other => {
                return Err(ForthicError::InvalidOperation {
                    forthic: String::new(),
                    message: format!("TRY requires a Forthic string, got {other:?}"),
                    location: None,
                    cause: None,
                })
            }
        };

        let snapshot = context.stack_snapshot();
        let module_depth = context.module_stack_depth();

        match context.run(&forthic) {
            Ok(()) => {
                // Payload = top of stack IF the run changed the stack at all
                // (covers transforms like '2 *' that consume and push,
                // leaving the length unchanged). Note: rs compares
                // structurally (PartialEq) where ts compares identity —
                // code that replaces a value with an equal one reads as
                // "unchanged" here; behaviorally indistinguishable.
                let after = context.stack_snapshot();
                let unchanged = after == snapshot;
                let payload = if !unchanged && !after.is_empty() {
                    context.stack_pop()?
                } else {
                    ForthicValue::Null
                };
                context.stack_push(ok_outcome(payload));
            }
            Err(e) => {
                context.stack_restore(snapshot);
                while context.module_stack_depth() > module_depth {
                    let _ = context.module_stack_pop();
                }
                context.stack_push(error_outcome(&e));
            }
        }
        Ok(())
    }

    fn is_outcome_with(value: &ForthicValue, key: &str) -> bool {
        matches!(value, ForthicValue::Record(rec) if rec.contains_key(key))
    }

    fn word_ok_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let outcome = context.stack_pop()?;
        context.stack_push(ForthicValue::Bool(Self::is_outcome_with(&outcome, "ok")));
        Ok(())
    }

    fn word_error_q(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let outcome = context.stack_pop()?;
        context.stack_push(ForthicValue::Bool(Self::is_outcome_with(&outcome, "error")));
        Ok(())
    }

    /// UNWRAP: ( outcome -- value ) — ok: push the value; error: re-raise
    /// preserving message and error_type (like Rust, the concrete variant
    /// becomes a generic wrapper — Err(e).unwrap() panics with Debug of e)
    fn word_unwrap(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let outcome = context.stack_pop()?;
        if let ForthicValue::Record(rec) = &outcome {
            if let Some(value) = rec.get("ok") {
                context.stack_push(value.clone());
                return Ok(());
            }
            if let Some(info) = rec.get("error") {
                let (message, error_type) = match info {
                    ForthicValue::Record(info) => (
                        info.get("message")
                            .and_then(|m| m.as_string())
                            .unwrap_or("UNWRAP of error outcome")
                            .to_string(),
                        info.get("error_type")
                            .and_then(|t| t.as_string())
                            .map(str::to_string),
                    ),
                    _ => ("UNWRAP of error outcome".to_string(), None),
                };
                let suffix = error_type.map(|t| format!(" ({t})")).unwrap_or_default();
                return Err(ForthicError::InvalidOperation {
                    forthic: String::new(),
                    message: format!("{message}{suffix}"),
                    location: None,
                    cause: None,
                });
            }
        }
        Err(ForthicError::InvalidOperation {
            forthic: String::new(),
            message: "UNWRAP requires a TRY outcome record with an 'ok' or 'error' key".to_string(),
            location: None,
            cause: None,
        })
    }

    /// UNWRAP-OR: ( outcome default -- value ) — ok wins even when the ok
    /// value is NULL (failure is not nullness)
    fn word_unwrap_or(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let default_value = context.stack_pop()?;
        let outcome = context.stack_pop()?;
        if let ForthicValue::Record(rec) = &outcome {
            if let Some(value) = rec.get("ok") {
                context.stack_push(value.clone());
                return Ok(());
            }
            if rec.contains_key("error") {
                context.stack_push(default_value);
                return Ok(());
            }
        }
        Err(ForthicError::InvalidOperation {
            forthic: String::new(),
            message: "UNWRAP-OR requires a TRY outcome record with an 'ok' or 'error' key"
                .to_string(),
            location: None,
            cause: None,
        })
    }

    /// Get the underlying module
    pub fn module(&self) -> &Module {
        &self.module
    }

    /// Get a mutable reference to the underlying module
    pub fn module_mut(&mut self) -> &mut Module {
        &mut self.module
    }

    // ===== Stack Operations =====

    fn register_stack_words(module: &mut Module) {
        // DROP (pop top of stack — ts-canonical name; the classic POP is dropped)
        let word = Arc::new(ModuleWord::new("DROP".to_string(), Self::word_drop));
        module.add_exportable_word(word);

        // DUP
        let word = Arc::new(ModuleWord::new("DUP".to_string(), Self::word_dup));
        module.add_exportable_word(word);

        // SWAP
        let word = Arc::new(ModuleWord::new("SWAP".to_string(), Self::word_swap));
        module.add_exportable_word(word);
    }

    fn word_drop(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        context.stack_pop()?;
        Ok(())
    }

    fn word_dup(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        context.stack_push(val.clone());
        context.stack_push(val);
        Ok(())
    }

    fn word_swap(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let b = context.stack_pop()?;
        let a = context.stack_pop()?;
        context.stack_push(b);
        context.stack_push(a);
        Ok(())
    }

    // ===== Variable Operations =====

    fn register_variable_words(module: &mut Module) {
        // VARIABLES
        let word = Arc::new(ModuleWord::new(
            "VARIABLES".to_string(),
            Self::word_variables,
        ));
        module.add_exportable_word(word);

        // !
        let word = Arc::new(ModuleWord::new("!".to_string(), Self::word_store));
        module.add_exportable_word(word);

        // @
        let word = Arc::new(ModuleWord::new("@".to_string(), Self::word_fetch));
        module.add_exportable_word(word);

        // !@
        let word = Arc::new(ModuleWord::new("!@".to_string(), Self::word_store_fetch));
        module.add_exportable_word(word);
    }

    fn word_variables(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        if let ForthicValue::Array(varnames) = val {
            let cur_module = context.cur_module_mut();

            for varname_val in varnames {
                if let ForthicValue::String(varname) = varname_val {
                    // Validate variable name - no __ prefix allowed
                    if varname.starts_with("__") {
                        return Err(ForthicError::InvalidVariableName {
                            forthic: "".to_string(),
                            varname,
                            location: None,
                            cause: None,
                        });
                    }
                    cur_module.add_variable(varname, ForthicValue::Null);
                }
            }
        }

        Ok(())
    }

    fn word_store(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let varname_val = context.stack_pop()?;
        let value = context.stack_pop()?;

        if let ForthicValue::String(varname) = varname_val {
            // Validate variable name - no __ prefix allowed
            if varname.starts_with("__") {
                return Err(ForthicError::InvalidVariableName {
                    forthic: "".to_string(),
                    varname,
                    location: None,
                    cause: None,
                });
            }

            let cur_module = context.cur_module_mut();

            // Get or create variable
            if cur_module.get_variable(&varname).is_none() {
                cur_module.add_variable(varname.clone(), ForthicValue::Null);
            }

            // Set value
            if let Some(var) = cur_module.get_variable_mut(&varname) {
                var.set_value(value);
            }
        }

        Ok(())
    }

    fn word_fetch(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let varname_val = context.stack_pop()?;

        if let ForthicValue::String(varname) = varname_val {
            // Validate variable name - no __ prefix allowed
            if varname.starts_with("__") {
                return Err(ForthicError::InvalidVariableName {
                    forthic: "".to_string(),
                    varname,
                    location: None,
                    cause: None,
                });
            }

            // Get or create variable
            let value = {
                let cur_module = context.cur_module_mut();

                if cur_module.get_variable(&varname).is_none() {
                    cur_module.add_variable(varname.clone(), ForthicValue::Null);
                }

                // Get value
                cur_module
                    .get_variable(&varname)
                    .map(|var| var.get_value().clone())
                    .unwrap_or(ForthicValue::Null)
            };

            context.stack_push(value);
        } else {
            context.stack_push(ForthicValue::Null);
        }

        Ok(())
    }

    fn word_store_fetch(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let varname_val = context.stack_pop()?;
        let value = context.stack_pop()?;

        if let ForthicValue::String(varname) = varname_val {
            // Validate variable name - no __ prefix allowed
            if varname.starts_with("__") {
                return Err(ForthicError::InvalidVariableName {
                    forthic: "".to_string(),
                    varname,
                    location: None,
                    cause: None,
                });
            }

            let cur_module = context.cur_module_mut();

            // Get or create variable
            if cur_module.get_variable(&varname).is_none() {
                cur_module.add_variable(varname.clone(), ForthicValue::Null);
            }

            // Set value
            if let Some(var) = cur_module.get_variable_mut(&varname) {
                var.set_value(value.clone());
            }

            // Push value back
            context.stack_push(value);
        } else {
            context.stack_push(value);
        }

        Ok(())
    }

    // ===== Control Flow Operations =====

    fn register_control_words(module: &mut Module) {
        // NOP
        let word = Arc::new(ModuleWord::new("NOP".to_string(), Self::word_nop));
        module.add_exportable_word(word);

        // NULL
        let word = Arc::new(ModuleWord::new("NULL".to_string(), Self::word_null));
        module.add_exportable_word(word);

        // ARRAY?
        let word = Arc::new(ModuleWord::new("ARRAY?".to_string(), Self::word_is_array));
        module.add_exportable_word(word);

        // DEFAULT
        let word = Arc::new(ModuleWord::new("DEFAULT".to_string(), Self::word_default));
        module.add_exportable_word(word);
    }

    fn word_nop(_context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        // No-op
        Ok(())
    }

    fn word_null(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        context.stack_push(ForthicValue::Null);
        Ok(())
    }

    fn word_is_array(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;
        let is_array = matches!(val, ForthicValue::Array(_));
        context.stack_push(ForthicValue::Bool(is_array));
        Ok(())
    }

    fn word_default(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let default_value = context.stack_pop()?;
        let value = context.stack_pop()?;

        match value {
            ForthicValue::Null => context.stack_push(default_value),
            ForthicValue::String(s) if s.is_empty() => context.stack_push(default_value),
            _ => context.stack_push(value),
        }

        Ok(())
    }

    // ===== Options Operations =====

    fn register_options_words(module: &mut Module) {
        // ~>
        let word = Arc::new(ModuleWord::new("~>".to_string(), Self::word_to_options));
        module.add_exportable_word(word);
    }

    fn word_to_options(context: &mut dyn InterpreterContext) -> Result<(), ForthicError> {
        let val = context.stack_pop()?;

        if let ForthicValue::Array(arr) = val {
            // Convert to WordOptions
            match WordOptions::from_flat_array(&arr) {
                Ok(options) => context.stack_push(ForthicValue::WordOptions(options)),
                Err(_) => context.stack_push(ForthicValue::Null),
            }
        } else {
            // Not an array, push back null
            context.stack_push(ForthicValue::Null);
        }

        Ok(())
    }
}

impl Default for CoreModule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    //! The ${name} hole grammar and rendering options are pinned here at
    //! the helper level (INTERPOLATE itself is also covered by the Batch 4
    //! integration tests); PRINT shares these internals.

    use super::*;
    use crate::interpreter::Interpreter;

    fn opts() -> InterpOptions {
        InterpOptions {
            separator: ", ".to_string(),
            null_text: String::new(),
            json: false,
        }
    }

    fn interpolate(setup: &str, template: &str) -> String {
        let mut interp = Interpreter::standard("UTC");
        if !setup.is_empty() {
            interp.run(setup).unwrap();
        }
        CoreModule::interpolate_string(&mut interp, template, &opts()).unwrap()
    }

    #[test]
    fn test_holes_are_explicit() {
        assert_eq!(interpolate("7 .x !", "x is ${x}"), "x is 7");
        // The dot-symbol spelling works too; body whitespace trims
        assert_eq!(interpolate("7 .x !", "${.x} leads"), "7 leads");
        assert_eq!(interpolate("7 .x !", "${ x } spaced"), "7 spaced");
        // No hole without the full ${...} shape — bare dots and braces
        // are literal text
        assert_eq!(interpolate("7 .x !", "file.x {x} $x"), "file.x {x} $x");
    }

    #[test]
    fn test_escaped_holes_stay_literal() {
        assert_eq!(
            interpolate("7 .x !", r"literal \${x} here"),
            "literal ${x} here"
        );
        // The escape survives adjacent real holes
        assert_eq!(interpolate("7 .x !", r"\${x} ${x}"), "${x} 7");
    }

    #[test]
    fn test_lookup_is_read_only() {
        let mut interp = Interpreter::standard("UTC");
        let rendered = CoreModule::interpolate_string(&mut interp, "v: ${nope}", &opts()).unwrap();
        assert_eq!(rendered, "v: ", "miss renders as null_text (default '')");
        // ...and the miss created NOTHING (unlike @'s get-or-create)
        assert!(interp.find_variable_value("nope").is_none());
    }

    #[test]
    fn test_non_name_holes_are_errors_not_expressions() {
        let mut interp = Interpreter::standard("UTC");
        for template in ["${1 + 2}", "${}", "${x y}", "${x:-default}", "${9lives}"] {
            let err = CoreModule::interpolate_string(&mut interp, template, &opts()).unwrap_err();
            assert!(
                err.to_string().contains("not expressions"),
                "{template} -> {err}"
            );
        }
    }

    #[test]
    fn test_dunder_hole_names_error() {
        let mut interp = Interpreter::standard("UTC");
        let err = CoreModule::interpolate_string(&mut interp, "${__x}", &opts()).unwrap_err();
        assert!(matches!(err, ForthicError::InvalidVariableName { .. }));
    }

    #[test]
    fn test_value_rendering_options() {
        let mut interp = Interpreter::standard("UTC");
        interp.run("[ 1 NULL 3 ] .items !").unwrap();

        let rendered =
            CoreModule::interpolate_string(&mut interp, "items: ${items}", &opts()).unwrap();
        assert_eq!(
            rendered, "items: 1, , 3",
            "arrays join with separator; NULL elements use null_text"
        );

        let json_opts = InterpOptions {
            json: true,
            ..opts()
        };
        let rendered =
            CoreModule::interpolate_string(&mut interp, "items: ${items}", &json_opts).unwrap();
        assert_eq!(rendered, "items: [1,null,3]");

        let na_opts = InterpOptions {
            separator: " | ".to_string(),
            null_text: "N/A".to_string(),
            json: false,
        };
        let rendered = CoreModule::interpolate_string(&mut interp, "${items}", &na_opts).unwrap();
        assert_eq!(rendered, "1 | N/A | 3");
        let rendered = CoreModule::interpolate_string(&mut interp, "${unset}", &na_opts).unwrap();
        assert_eq!(rendered, "N/A", "misses use null_text too");
    }

    #[test]
    fn test_records_render_as_json() {
        assert_eq!(
            interpolate("[ [ 'a' 1 ] ] REC .rec !", "rec: ${rec}"),
            r#"rec: {"a":1}"#
        );
    }
}
