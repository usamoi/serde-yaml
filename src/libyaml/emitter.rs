use crate::libyaml::error::Error;
use unsafe_libyaml as sys;

#[repr(transparent)]
struct PinnedHandle(sys::yaml_emitter_t, std::marker::PhantomPinned);

impl PinnedHandle {
    fn init(&mut self, handler: sys::yaml_write_handler_t, data: *mut std::ffi::c_void) {
        unsafe {
            let this = &raw mut self.0;
            if sys::yaml_emitter_initialize(this).fail {
                panic!("malloc error: {}", Error::get_emitter_error(this));
            }
            sys::yaml_emitter_set_unicode(this, true);
            sys::yaml_emitter_set_width(this, -1);
            sys::yaml_emitter_set_output(this, handler, data);
        }
    }
}

impl Drop for PinnedHandle {
    fn drop(&mut self) {
        unsafe { sys::yaml_emitter_delete(&mut self.0) }
    }
}

#[derive(Debug)]
pub enum EmitterError {
    Libyaml(Error),
    Io(std::io::Error),
}

#[derive(Debug)]
pub enum Event<'a> {
    StreamStart,
    StreamEnd,
    DocumentStart,
    DocumentEnd,
    Scalar(Scalar<'a>),
    SequenceStart(Sequence),
    SequenceEnd,
    MappingStart(Mapping),
    MappingEnd,
}

#[derive(Debug)]
pub struct Scalar<'a> {
    pub tag: Option<String>,
    pub value: &'a str,
    pub style: ScalarStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarStyle {
    Any,
    Plain,
    SingleQuoted,
    Literal,
}

#[derive(Debug)]
pub struct Sequence {
    pub tag: Option<String>,
}

#[derive(Debug)]
pub struct Mapping {
    pub tag: Option<String>,
}

struct EmitterPinned<W> {
    handle: PinnedHandle,
    writer: Option<W>,
    error: Option<std::io::Error>,
}

pub struct Emitter<W> {
    pinned: Box<EmitterPinned<W>>,
}

impl<W> Emitter<W> {
    pub fn new(write: W) -> Emitter<W>
    where
        W: std::io::Write,
    {
        let mut pinned = Box::<EmitterPinned<W>>::new(EmitterPinned {
            handle: unsafe { std::mem::zeroed() },
            writer: Some(write),
            error: None,
        });
        let handler = handler::<W>;
        let data = (pinned.as_mut() as *mut EmitterPinned<W>).cast();
        pinned.handle.init(handler, data);
        Emitter { pinned }
    }

    pub fn emit(&mut self, event: Event) -> Result<(), EmitterError> {
        let emitter = &raw mut self.pinned.handle.0;
        let error = &mut self.pinned.error;
        unsafe {
            let mut sys_event = std::mem::zeroed::<sys::yaml_event_t>();
            let initialize_status = match event {
                Event::StreamStart => {
                    sys::yaml_stream_start_event_initialize(&mut sys_event, sys::YAML_UTF8_ENCODING)
                }
                Event::StreamEnd => sys::yaml_stream_end_event_initialize(&mut sys_event),
                Event::DocumentStart => {
                    let version_directive = std::ptr::null_mut();
                    let tag_directives_start = std::ptr::null_mut();
                    let tag_directives_end = std::ptr::null_mut();
                    let implicit = true;
                    sys::yaml_document_start_event_initialize(
                        &mut sys_event,
                        version_directive,
                        tag_directives_start,
                        tag_directives_end,
                        implicit,
                    )
                }
                Event::DocumentEnd => {
                    let implicit = true;
                    sys::yaml_document_end_event_initialize(&mut sys_event, implicit)
                }
                Event::Scalar(mut scalar) => {
                    let anchor = std::ptr::null();
                    let tag = scalar.tag.as_mut().map_or_else(std::ptr::null, |tag| {
                        tag.push('\0');
                        tag.as_ptr()
                    });
                    let value = scalar.value.as_ptr();
                    let length = scalar.value.len() as i32;
                    let plain_implicit = tag.is_null();
                    let quoted_implicit = tag.is_null();
                    let style = match scalar.style {
                        ScalarStyle::Any => sys::YAML_ANY_SCALAR_STYLE,
                        ScalarStyle::Plain => sys::YAML_PLAIN_SCALAR_STYLE,
                        ScalarStyle::SingleQuoted => sys::YAML_SINGLE_QUOTED_SCALAR_STYLE,
                        ScalarStyle::Literal => sys::YAML_LITERAL_SCALAR_STYLE,
                    };
                    sys::yaml_scalar_event_initialize(
                        &mut sys_event,
                        anchor,
                        tag,
                        value,
                        length,
                        plain_implicit,
                        quoted_implicit,
                        style,
                    )
                }
                Event::SequenceStart(mut sequence) => {
                    let anchor = std::ptr::null();
                    let tag = sequence.tag.as_mut().map_or_else(std::ptr::null, |tag| {
                        tag.push('\0');
                        tag.as_ptr()
                    });
                    let implicit = tag.is_null();
                    let style = sys::YAML_ANY_SEQUENCE_STYLE;
                    sys::yaml_sequence_start_event_initialize(
                        &mut sys_event,
                        anchor,
                        tag,
                        implicit,
                        style,
                    )
                }
                Event::SequenceEnd => sys::yaml_sequence_end_event_initialize(&mut sys_event),
                Event::MappingStart(mut mapping) => {
                    let anchor = std::ptr::null();
                    let tag = mapping.tag.as_mut().map_or_else(std::ptr::null, |tag| {
                        tag.push('\0');
                        tag.as_ptr()
                    });
                    let implicit = tag.is_null();
                    let style = sys::YAML_ANY_MAPPING_STYLE;
                    sys::yaml_mapping_start_event_initialize(
                        &mut sys_event,
                        anchor,
                        tag,
                        implicit,
                        style,
                    )
                }
                Event::MappingEnd => sys::yaml_mapping_end_event_initialize(&mut sys_event),
            };
            if initialize_status.fail {
                return Err(EmitterError::Libyaml(Error::get_emitter_error(emitter)));
            }
            if sys::yaml_emitter_emit(emitter, &mut sys_event).fail {
                if let Some(error) = error.take() {
                    return Err(EmitterError::Io(error));
                } else {
                    return Err(EmitterError::Libyaml(Error::get_emitter_error(emitter)));
                }
            }
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), EmitterError> {
        let emitter = &raw mut self.pinned.handle.0;
        let error = &mut self.pinned.error;
        unsafe {
            if sys::yaml_emitter_flush(emitter).fail {
                if let Some(error) = error.take() {
                    return Err(EmitterError::Io(error));
                } else {
                    return Err(EmitterError::Libyaml(Error::get_emitter_error(emitter)));
                }
            }
        }
        Ok(())
    }

    pub fn into_inner(mut self) -> W {
        self.pinned.writer.take().expect("writer is already taken")
    }
}

unsafe fn handler<W: std::io::Write>(
    data: *mut std::ffi::c_void,
    buffer: *mut u8,
    size: u64,
) -> i32 {
    let pinned = unsafe { &mut (*data.cast::<EmitterPinned<W>>()) };
    let buf = unsafe { std::slice::from_raw_parts(buffer, size as _) };
    if let Some(x) = &mut pinned.writer {
        if let Err(err) = x.write_all(buf) {
            pinned.error = Some(err);
            0
        } else {
            1
        }
    } else {
        1
    }
}
