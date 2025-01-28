use std::ffi::CStr;
use unsafe_libyaml as sys;

pub struct Error {
    kind: sys::yaml_error_type_t,
    problem: Option<&'static CStr>,
    problem_offset: u64,
    problem_mark: Mark,
    context: Option<&'static CStr>,
    context_mark: Mark,
}

impl Error {
    pub unsafe fn get_parser_error(parser: *const sys::yaml_parser_t) -> Self {
        let parser = unsafe { &(*parser) };
        Error {
            kind: parser.error,
            problem: if !parser.problem.is_null() {
                Some(unsafe { CStr::from_ptr(parser.problem) })
            } else {
                None
            },
            problem_offset: parser.problem_offset,
            problem_mark: Mark {
                sys: parser.problem_mark,
            },
            context: if !parser.context.is_null() {
                Some(unsafe { CStr::from_ptr(parser.context) })
            } else {
                None
            },
            context_mark: Mark {
                sys: parser.context_mark,
            },
        }
    }

    pub unsafe fn get_emitter_error(emitter: *const sys::yaml_emitter_t) -> Self {
        let emitter = unsafe { &(*emitter) };
        Error {
            kind: emitter.error,
            problem: if !emitter.problem.is_null() {
                Some(unsafe { CStr::from_ptr(emitter.problem) })
            } else {
                None
            },
            problem_offset: 0,
            problem_mark: Default::default(),
            context: None,
            context_mark: Default::default(),
        }
    }

    pub fn mark(&self) -> Mark {
        self.problem_mark
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(problem) = self.problem {
            write!(formatter, "{}", problem.to_string_lossy())?;
        } else {
            write!(formatter, "libyaml parser failed but there is no error")?;
        }
        if self.problem_mark.sys.line != 0 || self.problem_mark.sys.column != 0 {
            write!(formatter, " at {}", self.problem_mark)?;
        } else if self.problem_offset != 0 {
            write!(formatter, " at position {}", self.problem_offset)?;
        }
        if let Some(context) = &self.context {
            write!(formatter, ", {}", context.to_string_lossy())?;
            if (self.context_mark.sys.line != 0 || self.context_mark.sys.column != 0)
                && (self.context_mark.sys.line != self.problem_mark.sys.line
                    || self.context_mark.sys.column != self.problem_mark.sys.column)
            {
                write!(formatter, " at {}", self.context_mark)?;
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut formatter = formatter.debug_struct("Error");
        if let Some(kind) = match self.kind {
            sys::YAML_MEMORY_ERROR => Some("MEMORY"),
            sys::YAML_READER_ERROR => Some("READER"),
            sys::YAML_SCANNER_ERROR => Some("SCANNER"),
            sys::YAML_PARSER_ERROR => Some("PARSER"),
            sys::YAML_COMPOSER_ERROR => Some("COMPOSER"),
            sys::YAML_WRITER_ERROR => Some("WRITER"),
            sys::YAML_EMITTER_ERROR => Some("EMITTER"),
            _ => None,
        } {
            formatter.field("kind", &format_args!("{}", kind));
        }
        formatter.field("problem", &self.problem);
        if self.problem_mark.sys.line != 0 || self.problem_mark.sys.column != 0 {
            formatter.field("problem_mark", &self.problem_mark);
        } else if self.problem_offset != 0 {
            formatter.field("problem_offset", &self.problem_offset);
        }
        if let Some(context) = &self.context {
            formatter.field("context", context);
            if self.context_mark.sys.line != 0 || self.context_mark.sys.column != 0 {
                formatter.field("context_mark", &self.context_mark);
            }
        }
        formatter.finish()
    }
}

#[derive(Copy, Clone)]
pub struct Mark {
    pub(super) sys: sys::yaml_mark_t,
}

impl Mark {
    pub fn index(&self) -> u64 {
        self.sys.index
    }

    pub fn line(&self) -> u64 {
        self.sys.line
    }

    pub fn column(&self) -> u64 {
        self.sys.column
    }
}

impl Default for Mark {
    fn default() -> Self {
        Self {
            sys: unsafe { std::mem::zeroed() },
        }
    }
}

impl std::fmt::Display for Mark {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.sys.line != 0 || self.sys.column != 0 {
            write!(
                formatter,
                "line {} column {}",
                self.sys.line + 1,
                self.sys.column + 1,
            )
        } else {
            write!(formatter, "position {}", self.sys.index)
        }
    }
}

impl std::fmt::Debug for Mark {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut formatter = formatter.debug_struct("Mark");
        if self.sys.line != 0 || self.sys.column != 0 {
            formatter.field("line", &(self.sys.line + 1));
            formatter.field("column", &(self.sys.column + 1));
        } else {
            formatter.field("index", &self.sys.index);
        }
        formatter.finish()
    }
}
