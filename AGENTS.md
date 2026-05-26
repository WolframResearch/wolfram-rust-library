## Learned User Preferences
- When the user asks for a command or snippet, provide the exact command directly; do not edit files or notebooks unless explicitly asked.
- For Wolfram notebook build snippets, use the user's `commands = {cargo -> {...}}` command-list shape and apply requested command additions precisely.

## Learned Workspace Facts
- `cargo wl build` prints only generated `manifest.wl` paths to stdout on success, one path per line; Cargo/build diagnostics may still appear on stderr.
- Generated package layout is `<out>/<SystemID>/<libname>/manifest.wl` with the hashed binary in the same folder.
- Supported initial `cargo wl build --system-id` values are `MacOSX-x86-64`, `MacOSX-ARM64`, `Windows-x86-64`, `Linux-x86-64`, `Linux-ARM64`, and `Linux-ARM`; Windows cross-builds require the MinGW linker on `PATH`.
- `wolfram-app-discovery` is a monorepo workspace crate and workspace packages should use the local path dependency instead of crates.io.
- `#[export]` registers functions/signatures in the shared inventory; `generate_loader!` only adds a named WSTP runtime loader and is not required for `cargo wl build`.
- The CLI loads `__wolfram_manifest_data__` for build-time manifest metadata; the on-disk `manifest.wl` is Wolfram Language, not JSON.
