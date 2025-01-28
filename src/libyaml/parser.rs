use crate::libyaml::error::{Error, Mark};
use std::borrow::Cow;
use unsafe_libyaml as sys;

#[repr(transparent)]
struct PinnedHandle(sys::yaml_parser_t, std::marker::PhantomPinned);

impl PinnedHandle {
    fn init(&mut self, input: *const [u8]) {
        unsafe {
            let this = &raw mut self.0;
            if sys::yaml_parser_initialize(this).fail {
                panic!("malloc error: {}", Error::get_parser_error(&self.0));
            }
            sys::yaml_parser_set_encoding(this, sys::YAML_UTF8_ENCODING);
            sys::yaml_parser_set_input_string(this, input as _, input.len() as u64);
        }
    }
}

impl Drop for PinnedHandle {
    fn drop(&mut self) {
        unsafe { sys::yaml_parser_delete(&mut self.0) }
    }
}

#[derive(Debug)]
pub enum Event<'input> {
    StreamStart,
    StreamEnd,
    DocumentStart,
    DocumentEnd,
    Alias(Anchor),
    Scalar(Scalar<'input>),
    SequenceStart(SequenceStart),
    SequenceEnd,
    MappingStart(MappingStart),
    MappingEnd,
}

#[derive(Debug)]
pub struct Scalar<'input> {
    pub anchor: Option<Anchor>,
    pub tag: Option<Tag>,
    pub value: ScalarValue,
    pub style: ScalarStyle,
    pub repr: Option<&'input [u8]>,
}

#[derive(Debug)]
pub struct SequenceStart {
    pub anchor: Option<Anchor>,
    pub tag: Option<Tag>,
}

#[derive(Debug)]
pub struct MappingStart {
    pub anchor: Option<Anchor>,
    pub tag: Option<Tag>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Anchor(Box<[u8]>);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScalarStyle {
    Plain,
    SingleQuoted,
    DoubleQuoted,
    Literal,
    Folded,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tag(Box<[u8]>);

impl Tag {
    pub const NULL: &'static [u8] = b"tag:yaml.org,2002:null";
    pub const BOOL: &'static [u8] = b"tag:yaml.org,2002:bool";
    pub const INT: &'static [u8] = b"tag:yaml.org,2002:int";
    pub const FLOAT: &'static [u8] = b"tag:yaml.org,2002:float";
}

impl AsRef<[u8]> for Tag {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScalarValue(Box<[u8]>);

impl AsRef<[u8]> for ScalarValue {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

struct ParserPinned<'input> {
    handle: PinnedHandle,
    input: Cow<'input, [u8]>,
}

pub struct Parser<'input> {
    pinned: Box<ParserPinned<'input>>,
}

impl<'input> Parser<'input> {
    pub fn new(input: Cow<'input, [u8]>) -> Parser<'input> {
        let mut pinned = Box::<ParserPinned<'input>>::new(ParserPinned {
            handle: unsafe { std::mem::zeroed() },
            input,
        });
        pinned.handle.init(pinned.input.as_ref());
        Parser { pinned }
    }

    pub fn next(&mut self) -> Result<(Event<'input>, Mark), super::error::Error> {
        let parser = &raw mut self.pinned.handle.0;
        let input = &self.pinned.input;
        unsafe {
            let mut sys_event = std::mem::zeroed::<sys::yaml_event_t>();
            if (*parser).error != sys::YAML_NO_ERROR {
                return Err(Error::get_parser_error(parser));
            }
            if sys::yaml_parser_parse(parser, &mut sys_event).fail {
                return Err(Error::get_parser_error(parser));
            }
            let event = convert_event(&sys_event, input);
            let mark = Mark {
                sys: sys_event.start_mark,
            };
            sys::yaml_event_delete(&mut sys_event);
            Ok((event, mark))
        }
    }
}

unsafe fn convert_event<'input>(
    sys: &sys::yaml_event_t,
    input: &Cow<'input, [u8]>,
) -> Event<'input> {
    unsafe fn parse_anchor(anchor: *const u8) -> Option<Anchor> {
        if anchor.is_null() {
            return None;
        }
        let cstr = unsafe { std::ffi::CStr::from_ptr(anchor.cast()) };
        Some(Anchor(Box::from(cstr.to_bytes())))
    }

    unsafe fn parse_tag(tag: *const u8) -> Option<Tag> {
        if tag.is_null() {
            return None;
        }
        let cstr = unsafe { std::ffi::CStr::from_ptr(tag.cast()) };
        Some(Tag(Box::from(cstr.to_bytes())))
    }

    unsafe fn parse_value(value: *mut u8, length: u64) -> Option<ScalarValue> {
        if value.is_null() {
            return None;
        }
        let slice = unsafe { std::slice::from_raw_parts(value, length as usize) };
        Some(ScalarValue(Box::from(slice)))
    }

    match sys.type_ {
        sys::YAML_STREAM_START_EVENT => Event::StreamStart,
        sys::YAML_STREAM_END_EVENT => Event::StreamEnd,
        sys::YAML_DOCUMENT_START_EVENT => Event::DocumentStart,
        sys::YAML_DOCUMENT_END_EVENT => Event::DocumentEnd,
        sys::YAML_ALIAS_EVENT => {
            Event::Alias(unsafe { parse_anchor(sys.data.alias.anchor) }.unwrap())
        }
        sys::YAML_SCALAR_EVENT => Event::Scalar(Scalar {
            anchor: unsafe { parse_anchor(sys.data.scalar.anchor) },
            tag: unsafe { parse_tag(sys.data.scalar.tag) },
            value: unsafe { parse_value(sys.data.scalar.value, sys.data.scalar.length).unwrap() },
            style: match unsafe { sys.data.scalar.style } {
                sys::YAML_PLAIN_SCALAR_STYLE => ScalarStyle::Plain,
                sys::YAML_SINGLE_QUOTED_SCALAR_STYLE => ScalarStyle::SingleQuoted,
                sys::YAML_DOUBLE_QUOTED_SCALAR_STYLE => ScalarStyle::DoubleQuoted,
                sys::YAML_LITERAL_SCALAR_STYLE => ScalarStyle::Literal,
                sys::YAML_FOLDED_SCALAR_STYLE => ScalarStyle::Folded,
                sys::YAML_ANY_SCALAR_STYLE | _ => unreachable!(),
            },
            repr: if let Cow::Borrowed(input) = input {
                Some(&input[sys.start_mark.index as usize..sys.end_mark.index as usize])
            } else {
                None
            },
        }),
        sys::YAML_SEQUENCE_START_EVENT => Event::SequenceStart(SequenceStart {
            anchor: unsafe { parse_anchor(sys.data.sequence_start.anchor) },
            tag: unsafe { parse_tag(sys.data.sequence_start.tag) },
        }),
        sys::YAML_SEQUENCE_END_EVENT => Event::SequenceEnd,
        sys::YAML_MAPPING_START_EVENT => Event::MappingStart(MappingStart {
            anchor: unsafe { parse_anchor(sys.data.mapping_start.anchor) },
            tag: unsafe { parse_tag(sys.data.mapping_start.tag) },
        }),
        sys::YAML_MAPPING_END_EVENT => Event::MappingEnd,
        sys::YAML_NO_EVENT => unreachable!(),
        _ => unimplemented!(),
    }
}
