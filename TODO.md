- Backends keep their tests; each port adds a headless gpui test for the main flow, like `script-runner-ui`'s.
- Run `cargo deny check` after adding dependencies.

### 4.0 Host API and shared additions (first)

- [x] **Decide:** HTTP client crate: `reqwest`. No Zed service URLs.
- [x] `crates/shared/http` (`forge-http`): a small async client (GET with JSON, download to file with progress, user agent from `branding`), used by the npm/NuGet/PyPI/crates.io searches, the Python version downloads and Helm.
- [ ] `crates/shared/process` (or add to `forge-script-runner`): `new_command(program)` without a console window, replacing `gpui_util::new_std_command` in every backend.
- [ ] Workspace file events: `forge_host::Workspace` emits an event for changed paths (from the existing watcher), so panels can rescan when `package.json`, `*.csproj`, `Cargo.toml` or `pyproject.toml` change (replaces `project::Event` / `PathChange`).
- [ ] Restorable tabs: a tab kind registry (`register_tab_kind(kind, restore)`), with open tabs saved in `State` and reopened at start-up (replaces `register_serializable_item`, used by the manager pages and Dashboard).
- [ ] Hide a panel's status-bar button: `panels.<id>.button = false` in settings, plus \"Hide button\" in the button's context menu (replaces `HideStatusItem`).
- [ ] `crates/shared/credentials` (`forge-credentials`): read, write and delete secrets in Windows Credential Manager (the `keyring` crate), keyed with `branding::PRODUCT.id` (replaces `zed_credentials_provider`, used by Helm and Database).
- [ ] Settings defaults per panel: each feature can supply defaults for its own `settings.json` keys.
- [ ] Update `forge-super-design.md` §13 (host API) and CLAUDE.md with the additions.

### 4.1 Cockpit, Processes, Script Runner (prove the approach)

- [x] Script Runner: done in phase 3.
