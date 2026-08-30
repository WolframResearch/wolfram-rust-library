mod test_native_args;
mod test_share_counts;
mod test_threading;

mod test_data_store;
mod test_images;
mod test_numeric_array_conversions;
mod test_streams;
mod test_wstp;
mod test_wxf;

use wolfram_library_link as wll;

/// This library's initialization hook.
///
/// `#[init]` generates the `WolframLibrary_initialize()` the Wolfram Language
/// calls when the library is loaded, and calls
/// [`initialize()`][wll::initialize] for us.
#[wll::init]
fn init() {
    test_streams::register_test_stream_methods();
}
