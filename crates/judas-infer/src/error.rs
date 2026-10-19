use crate::suggest::find_closest_candidate;
use judas_ast::Span;
use miette::Diagnostic;
use std::fmt::Display;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug, Clone, PartialEq, Eq)]
#[error("{message}")]
#[diagnostic(code(judas::infer::type_error))]
pub struct TypeError {
    pub message: String,

    #[label("{label}")]
    pub span: Span,

    pub label: String,

    #[help]
    pub help: Option<String>,
}

impl TypeError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        let msg = message.into();
        Self {
            message: msg.clone(),
            span,
            label: msg,
            help: None,
        }
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn width_mismatch(
        expected_ty: impl Display,
        found_ty: impl Display,
        expected_w: impl Display,
        found_w: impl Display,
        span: Span,
    ) -> Self {
        let exp_ty_str = expected_ty.to_string();
        let fnd_ty_str = found_ty.to_string();
        let exp_w_str = expected_w.to_string();
        let fnd_w_str = found_w.to_string();

        let help = if let (Ok(exp_num), Ok(fnd_num)) =
            (exp_w_str.parse::<u32>(), fnd_w_str.parse::<u32>())
        {
            if fnd_num < exp_num {
                format!(
                    "Signal is narrower ({fnd_num} bits) than expected ({exp_num} bits).\n\
                     Consider widening with `extend<{exp_w_str}>(...)` or `concat(...)`."
                )
            } else {
                format!(
                    "Signal is wider ({fnd_num} bits) than expected ({exp_num} bits).\n\
                     Consider truncating higher bits with `truncate<{exp_w_str}>(...)`."
                )
            }
        } else {
            format!(
                "Widths must match in hardware assignments.\n\
                 Expected: `{exp_ty_str}` (width `{exp_w_str}`), Found: `{fnd_ty_str}` (width `{fnd_w_str}`).\n\
                 Use `extend<{exp_w_str}>(...)` to widen or `truncate<{exp_w_str}>(...)` to truncate."
            )
        };

        Self {
            message: format!("Type width mismatch: expected `{exp_ty_str}`, found `{fnd_ty_str}`"),
            span,
            label: format!("found `{fnd_ty_str}`, expected `{exp_ty_str}`"),
            help: Some(help),
        }
    }

    pub fn undefined_variable(name: &str, span: Span, candidates: &[String]) -> Self {
        let cand_refs: Vec<&str> = candidates.iter().map(|s| s.as_str()).collect();
        let help = if let Some(suggestion) = find_closest_candidate(name, cand_refs) {
            format!("A signal with a similar name exists: did you mean `{suggestion}`?")
        } else if !candidates.is_empty() {
            format!(
                "`{name}` is not declared in this module. Available signals in scope:\n  {}",
                candidates.join(", ")
            )
        } else {
            format!(
                "Declare `{name}` using `input {name}: ...`, `output {name}: ...`, `register {name}: ...`, or `let {name} = ...`."
            )
        };

        Self {
            message: format!("Undefined variable or signal `{name}`"),
            span,
            label: format!("`{name}` not found in scope"),
            help: Some(help),
        }
    }

    pub fn invalid_clock_trigger(found_ty: impl Display, span: Span) -> Self {
        let found_str = found_ty.to_string();
        Self {
            message: format!("Type mismatch: trigger requires `Clock`, found `{found_str}`"),
            span,
            label: format!("expected `Clock`, found `{found_str}`"),
            help: Some(
                "The trigger of `on rising(...)` / `on falling(...)` must be a signal of type `Clock`.\n\
                 Consider declaring the clock port as: `input clk: Clock`.".into(),
            ),
        }
    }

    pub fn cannot_extend(found_ty: impl Display, span: Span) -> Self {
        let found_str = found_ty.to_string();
        Self {
            message: format!("Cannot extend non-signal/integer type `{found_str}`"),
            span,
            label: format!("cannot extend type `{found_str}`"),
            help: Some(
                "Only integer vectors (`UInt`, `Int`), bits (`Bits`, `Bit`), and boolean signals can be extended.".into(),
            ),
        }
    }

    pub fn cannot_concatenate(found_ty: impl Display, span: Span) -> Self {
        let found_str = found_ty.to_string();
        Self {
            message: format!("Cannot concatenate non-signal/integer type `{found_str}`"),
            span,
            label: format!("cannot concatenate type `{found_str}`"),
            help: Some(
                "`concat(...)` requires signals of type `UInt`, `Int`, `Bits`, `Bit`, or `Bool`."
                    .into(),
            ),
        }
    }

    pub fn arithmetic_requires_integers(found_ty: impl Display, span: Span) -> Self {
        let found_str = found_ty.to_string();
        Self {
            message: format!("Arithmetic operator requires integer types, found `{found_str}`"),
            span,
            label: format!("`{found_str}` does not support arithmetic"),
            help: Some(
                "Arithmetic operators (`+`, `-`, `*`, `/`, `%`) require integers (`UInt<N>` or `Int<N>`).\n\
                 If this is a raw bit vector (`Bits<N>`), consider using `UInt<N>` instead.".into(),
            ),
        }
    }

    pub fn bitwise_requires_bits(found_ty: impl Display, span: Span) -> Self {
        let found_str = found_ty.to_string();
        Self {
            message: format!("Bitwise operator requires integer/bits/bool types, found `{found_str}`"),
            span,
            label: format!("`{found_str}` does not support bitwise operations"),
            help: Some(
                "Bitwise operators (`&`, `|`, `^`, `~`) require `Bits<N>`, `Bit`, `UInt<N>`, `Int<N>`, or `Bool`.".into(),
            ),
        }
    }

    pub fn comparison_requires_numeric(found_ty: impl Display, span: Span) -> Self {
        let found_str = found_ty.to_string();
        Self {
            message: format!("Comparison operator requires numeric or bits types, found `{found_str}`"),
            span,
            label: format!("`{found_str}` cannot be ordered"),
            help: Some(
                "Ordering comparisons (`<`, `<=`, `>`, `>=`) require numeric signals (`UInt`, `Int`, `Bits`).".into(),
            ),
        }
    }

    pub fn shift_amount_must_be_integer(span: Span) -> Self {
        Self {
            message: "Shift amount must be an integer".into(),
            span,
            label: "expected integer shift amount".into(),
            help: Some(
                "The right-hand side of a shift (`<<`, `>>`) must be an integer (`UInt` or `Int`)."
                    .into(),
            ),
        }
    }

    pub fn missing_truncate_arg(span: Span) -> Self {
        Self {
            message: "`truncate<W>` requires exactly 1 width generic argument".into(),
            span,
            label: "missing target width generic argument".into(),
            help: Some("Specify the target bit width, for example: `truncate<8>(signal)`.".into()),
        }
    }

    pub fn truncate_arg_count(span: Span) -> Self {
        Self {
            message: "`truncate` takes exactly 1 argument".into(),
            span,
            label: "expected 1 argument".into(),
            help: Some("Usage: `truncate<WIDTH>(signal)`.".into()),
        }
    }

    pub fn extend_arg_count(span: Span) -> Self {
        Self {
            message: "`extend` takes exactly 1 argument".into(),
            span,
            label: "expected 1 argument".into(),
            help: Some("Usage: `extend(signal)` or `extend<WIDTH>(signal)`.".into()),
        }
    }

    pub fn concat_min_args(span: Span) -> Self {
        Self {
            message: "`concat` requires at least 2 arguments".into(),
            span,
            label: "expected at least 2 arguments".into(),
            help: Some(
                "Usage: `concat(msb, ..., lsb)` to combine multiple signals into one bus.".into(),
            ),
        }
    }

    pub fn module_not_found(name: &str, span: Span, available: &[String]) -> Self {
        let cand_refs: Vec<&str> = available.iter().map(|s| s.as_str()).collect();
        let help = if let Some(suggestion) = find_closest_candidate(name, cand_refs) {
            format!("A module with a similar name exists: did you mean `{suggestion}`?")
        } else if !available.is_empty() {
            format!("Available modules: {}", available.join(", "))
        } else {
            format!("Make sure `module {name}` is defined in the source file.")
        };

        Self {
            message: format!("Undefined module `{name}`"),
            span,
            label: format!("module `{name}` not found"),
            help: Some(help),
        }
    }

    pub fn unknown_port(
        module_name: &str,
        port_name: &str,
        span: Span,
        available_ports: &[String],
    ) -> Self {
        let cand_refs: Vec<&str> = available_ports.iter().map(|s| s.as_str()).collect();
        let help = if let Some(suggestion) = find_closest_candidate(port_name, cand_refs) {
            format!(
                "Port `{port_name}` does not exist on module `{module_name}`: did you mean `{suggestion}`?"
            )
        } else if !available_ports.is_empty() {
            format!(
                "Available ports on `{module_name}`: {}",
                available_ports.join(", ")
            )
        } else {
            format!("Module `{module_name}` has no ports declared.")
        };

        Self {
            message: format!("Module `{module_name}` has no port named `{port_name}`"),
            span,
            label: format!("unknown port `{port_name}`"),
            help: Some(help),
        }
    }

    pub fn repeat_arg_count(span: Span) -> Self {
        Self {
            message: "`repeat` takes exactly 1 generic argument and 1 signal argument".into(),
            span,
            label: "invalid argument count".into(),
            help: Some("Usage: `repeat<COUNT>(signal)` or `signal.repeat<COUNT>()`.".into()),
        }
    }

    pub fn slice_arg_count(span: Span) -> Self {
        Self {
            message: "`slice` requires 1 generic argument and 2 arguments (signal, offset)".into(),
            span,
            label: "invalid argument count".into(),
            help: Some(
                "Usage: `slice<WIDTH>(signal, offset)` or `signal.slice<WIDTH>(offset)`.".into(),
            ),
        }
    }

    pub fn unknown_struct(name: &str, span: Span, available: &[String]) -> Self {
        let cand_refs: Vec<&str> = available.iter().map(|s| s.as_str()).collect();
        let help = if let Some(suggestion) = find_closest_candidate(name, cand_refs) {
            format!("A struct with a similar name exists: did you mean `{suggestion}`?")
        } else if !available.is_empty() {
            format!("Available structs: {}", available.join(", "))
        } else {
            format!("Make sure `struct {name} {{ ... }}` is defined in the source file.")
        };

        Self {
            message: format!("Undefined struct `{name}`"),
            span,
            label: format!("struct `{name}` not found"),
            help: Some(help),
        }
    }

    pub fn unknown_struct_field(
        struct_name: &str,
        field_name: &str,
        span: Span,
        available_fields: &[String],
    ) -> Self {
        let cand_refs: Vec<&str> = available_fields.iter().map(|s| s.as_str()).collect();
        let help = if let Some(suggestion) = find_closest_candidate(field_name, cand_refs) {
            format!(
                "Field `{field_name}` does not exist on struct `{struct_name}`: did you mean `{suggestion}`?"
            )
        } else if !available_fields.is_empty() {
            format!(
                "Available fields on `{struct_name}`: {}",
                available_fields.join(", ")
            )
        } else {
            format!("Struct `{struct_name}` has no fields.")
        };

        Self {
            message: format!("Struct `{struct_name}` has no field named `{field_name}`"),
            span,
            label: format!("unknown field `{field_name}`"),
            help: Some(help),
        }
    }

    pub fn missing_struct_fields(struct_name: &str, span: Span, missing: &[String]) -> Self {
        Self {
            message: format!(
                "Missing fields in instantiation of struct `{struct_name}`: {}",
                missing.join(", ")
            ),
            span,
            label: "missing fields in struct literal".into(),
            help: Some(format!(
                "All fields declared in `{struct_name}` must be initialized: {}.",
                missing.join(", ")
            )),
        }
    }

    pub fn unknown_interface(name: &str, span: Span, available: &[String]) -> Self {
        let cand_refs: Vec<&str> = available.iter().map(|s| s.as_str()).collect();
        let help = if let Some(suggestion) = find_closest_candidate(name, cand_refs) {
            format!("An interface with a similar name exists: did you mean `{suggestion}`?")
        } else if !available.is_empty() {
            format!("Available interfaces: {}", available.join(", "))
        } else {
            format!("Make sure `interface {name} {{ ... }}` is defined in the source file.")
        };

        Self {
            message: format!("Undefined interface `{name}`"),
            span,
            label: format!("interface `{name}` not found"),
            help: Some(help),
        }
    }

    pub fn interface_role_mismatch(iface_name: &str, r1: &str, _r2: &str, span: Span) -> Self {
        Self {
            message: format!(
                "Cannot connect two `{r1}` interface ports of `{iface_name}` together"
            ),
            span,
            label: format!("both ports have role `{r1}`"),
            help: Some(
                "An interface connection must link a `master` port to a `slave` port.".into(),
            ),
        }
    }
}
