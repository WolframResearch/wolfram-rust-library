use std::io::Cursor;

use wolfram_library_link::{
    self as wll,
    stream::{
        register_input_stream_method, InputStreamMethod, OpenRequest, ReaderInputStream,
        StreamError,
    },
};

/// A stream method that serves back the name the stream was opened with.
struct EchoName;

impl InputStreamMethod for EchoName {
    type Stream = ReaderInputStream;

    fn open(&self, request: &mut OpenRequest) -> Result<Self::Stream, StreamError> {
        let contents = format!("you opened: {}", request.name());

        Ok(ReaderInputStream::new(Cursor::new(contents.into_bytes())))
    }
}

#[wll::init]
fn init() {
    register_input_stream_method("RustEcho", EchoName);
}
