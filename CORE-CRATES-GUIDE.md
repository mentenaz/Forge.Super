# Guide: prepare Forge Super for the backend `core` crates (do-it-yourself)

Work in this order. Each step ends with a command that must pass before you move on. Commit after each step. Nothing here ports a backend; that waits until we have talked about the file layout (the 1,000-1,800 line Zed files vs the repo's ~200-500 line rule).

## Step 0: Branch
```
git switch -c core-crates-prep
```

## Step 1: Allow fnm's licence (`deny.toml`)
fnm is `GPL-3.0` (only). In `[licenses] allow` add, with a comment:
```toml
    # fnm (Node version manager, linked as a library) is GPL-3.0-only; binaries
    # that include it are effectively GPL-3.0.
    "GPL-3.0",
    "GPL-3.0-only",
```
Check later (step 5), once fnm is actually in the tree.

## Step 2: Give `forge-http` its own tokio runtime (the one real bug)
reqwest needs a tokio reactor; gpui tasks run on gpui's executor, so today every request would panic "there is no reactor running".

1. Root `Cargo.toml` `[workspace.dependencies]`: `tokio = { version = "1", features = ["rt-multi-thread"] }`.
2. `crates/shared/http/Cargo.toml`: under `[dependencies]` add `tokio = { workspace = true }`; keep the dev-dependency (`features = ["macros", "rt-multi-thread"]`).
3. In `crates/shared/http/src/lib.rs` add:
```rust
use std::sync::OnceLock;

/// reqwest needs a tokio reactor; gpui's executor has none. Every request
/// runs on this runtime and is awaited from anywhere.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("forge-http")
            .enable_all()
            .build()
            .expect("the HTTP runtime starts")
    })
}

fn joined<T>(result: Result<Result<T, HttpError>, tokio::task::JoinError>) -> Result<T, HttpError> {
    result.map_err(|error| HttpError::Other(error.to_string()))?
}
```
4. Wrap the bodies of `get_json`, `get_text`, `download_file` and `HttpTransport::send`: clone `self.client` (cheap), then
```rust
joined(runtime().spawn(async move { /* the existing body, using the cloned client */ }).await)
```
For `download_file` the `on_progress` closure is already `Send + Sync + 'static`, so it moves in. In `transport.rs` `send` does the same.
5. Add a test that proves it (no tokio test macro, so no ambient runtime):
```rust
#[test]
fn works_from_an_executor_without_tokio() {
    let result = futures::executor::block_on(HttpClient::new().get_text("http://127.0.0.1:9"));
    assert!(result.is_err()); // a connection error, not a panic
}
```
Before the fix this test panics; after it, it passes.
6. `cargo test -p forge-http`

## Step 3: `FakeTransport::with_handler`
The Zed tests use `FakeHttpClient::create(|request| ...)` and `with_404_response()`. In `crates/shared/http/src/fake.rs`:
- add a field `handler: Option<Box<dyn Fn(&Request) -> Response + Send + Sync>>`;
- `pub fn with_handler(mut self, f: impl Fn(&Request) -> Response + Send + Sync + 'static) -> Self`;
- `pub fn not_found() -> Self` = `with_handler(|_| Response::ok("").with_status(404))`;
- in `answer()`: exact URL in `responses` first, else `handler`, else the existing `Other` error.
Add one test for each path. `cargo test -p forge-http`.

## Step 4: Workspace dependencies for the ports
Add to root `[workspace.dependencies]` (versions match the Zed edition):
```toml
semver = { version = "1.0", features = ["serde"] }
toml = { version = "0.9", default-features = false, features = ["std", "serde", "parse"] }
urlencoding = "2.1.2"
dunce = "1.0"
sha2 = "0.10"
flate2 = "1"
tar = "0.4"
junction = "1.2"
```
`junction` is Windows-only: a crate that needs it lists `junction.workspace = true` under `[target.'cfg(windows)'.dependencies]` (the workspace table itself has no target section). Do not add `async-tar`, `async-compression` or `async-std`: Python's only archive is `.tar.gz`, so `tar` + `flate2` on a background thread replaces them. Add flows/helm dependencies (`uuid`, `chrono`, `indexmap`, `which`, `datalogic-rs`, `base64`) only when you get to those crates.

## Step 5: The `fnm` dependency
1. Root table: `fnm = { git = "https://github.com/mentenaz/fnm", rev = "4a35f31866ab0cb6117c93087151bca03b0cadbf", default-features = false }` (the `mentenaz` org is already allowed in `deny.toml`). Reference clone: `E:\fnm`, branch `library`.
2. `crates/features/node/core/Cargo.toml`: `fnm.workspace = true`.
3. Needs the MSVC C build tools (fnm pulls `xz2`, which builds liblzma from C). If `cl.exe` is missing, install "Desktop development with C++" in the Visual Studio Build Tools.
4. `cargo check -p node-core`, then `cargo deny check`.
5. Read the deny output top to bottom:
   - `licenses` should now be ok (step 1).
   - `advisories` (`unmaintained = "all"`): for each new advisory add an `ignore` entry in `deny.toml` with a reason, like the existing ones (`{ id = "RUSTSEC-....", reason = "<crate>, via fnm: <why it is harmless here>" }`). Look up each id at rustsec.org first; don't ignore one you haven't read.
   - `bans`: duplicate versions are warnings only.

## Step 6: Mechanical rewires (the cheat sheet you apply while porting)
| Zed edition | Forge Super |
|---|---|
| `gpui_util::new_std_command(x)` | `forge_process::new_command(x)` |
| `ide_env::bin_dirs`, `ide_env::prepend` | `forge_ide_env::bin_dirs`, `forge_ide_env::prepend` |
| copied `path_env.rs` (`merged_path`, `expand_env_vars`, `dedupe_path`) | `forge_ide_env::merged_path` / `path_with(&dirs)` (delete the copy) |
| `paths::data_dir()` | `branding::data_dir()` |
| `http_client::HttpClient` / `FakeHttpClient` | `forge_http::HttpTransport` / `FakeTransport` |
| `smol::block_on` in tests | `futures::executor::block_on` |
| `async-tar` + `GzipDecoder` | `flate2::read::GzDecoder` + `tar::Archive` on a background thread |
| `util::shell::get_windows_bash()` (flows) | small local function (`GIT_INSTALL_ROOT`, else `which git`) |
Find what is left to rewire in a copied crate with:
`grep -rnE "gpui|http_client|paths::|ide_env|smol|util::" crates/features/<name>/core/src`

## Step 7: Create a new feature crate (python, rust)
```
crates/features/python/core/{Cargo.toml,src/lib.rs}
crates/features/python/ui/{Cargo.toml,src/lib.rs}
```
`core/Cargo.toml`: `[package]` with `version.workspace = true`, `edition.workspace = true`, `license.workspace = true`, `publish = false`; no gpui. `ui/Cargo.toml`: depends on `python-core`, `forge-host`, `gpui`, `gpui-component`. Root table: `python-core = { path = "crates/features/python/core" }` and `python-ui = ...`; the workspace globs pick the members up. The app's `main.rs` needs `python_ui::init(cx)` before `shell::init(cx)`. `node` and `dotnet` already exist as empty scaffolds: replace their contents, keep the names.

## Step 8: Port one backend (the repeatable loop, after the layout talk)
1. Copy `E:\zed_Source\crates\<x>_backend\src` into `crates/features/<name>/core/src/backend/`; set the `Cargo.toml` from the Zed one minus Zed-internal crates.
2. Apply step 6 until `cargo check -p <name>-core` passes.
3. `cargo test -p <name>-core`; the test count must equal the Zed crate's (node 6, npm 39, node_versions 21, python 17 + 12, dotnet 24, cargo 108). Tests that call npm/cargo/dotnet/git need those tools on PATH.
4. `cargo deny check`, commit, then split large files in a separate commit with the tests still green.
5. In the `ui` crate's `init`, register the runtime with `forge_ide_env::register(provider)` (a plain `fn(Option<&Path>) -> Vec<PathBuf>`); `core` never depends on `forge-host`.

Recommended order: node-core -> dotnet-core -> rust-core -> python-core (needs steps 2, 3, 4) -> flows-core -> helm-core (needs steps 2, 3).

## Final check
```
cargo check --workspace
cargo test --workspace
cargo deny check
```
