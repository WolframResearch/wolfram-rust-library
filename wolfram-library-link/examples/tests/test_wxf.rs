use wolfram_library_link::export;

#[export(wxf)]
fn test_wxf_bool_input(a: bool) -> u64 {
    if a {
        42
    } else {
        24
    }
}

#[export(wxf)]
fn test_wxf_bool_output(n: i64) -> bool {
    n > 0
}
