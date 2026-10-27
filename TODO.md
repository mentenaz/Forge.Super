# Forge Super: to-do list

Phases are as in `forge-super-design.md` ┬º12.6; this file is the working checklist for them. Tick items as they land, and record each phase's result in the design doc (┬º13.x) when it is done.

Sizes are lines of Rust in the Zed edition (`E:\zed_Source\crates\ÔÇª`). "Zed lines" counts the lines in a panel that use a Zed API, i.e. what has to be rewired; the rest is copied.

---

## Housekeeping (before phase 4)

- [x] Decide on the stray `cargo fmt` changes in `crates/app/src/shell/` and `crates/shared/host/src/` (layout only): committed as formatting-only commit.
- [x] Commit and push phase 3 (`ide-env`, `git-credential`, `runtime-processes`(+`-ui`), `script-runner`, `script-runner-ui`, host command API, docs).
- [x] Fix the two existing clippy warnings: `crates/shared/host/src/state.rs:56` (`let _ =` on unit) and `crates/app/src/shell/tab_bar.rs:113` (`needless_range_loop`).
- [ ] Known issue: widening the left dock squeezes the right dock (gpui-component's resizable scales panels proportionally).
- [ ] Syntax highlighting in the read-only file view (Mentenaz.GPUI's tree-sitter feature isn't wired up). Optional, since Zed's editor replaces the view in phase 5.

---

## Done

- [x] **Phase 0: Shell look.** Title bar with application menu and project, worktree and branch pickers; Zed's three-dock model; per-pane tab bars (split, pinning, drag and drop, folder hints, breadcrumbs); status bar; notifications; Forge Dark theme at 13px.\n- [x] **Phase 1: Host API and branding.** `forge-host` (panel registration, `Host`, `Settings`, `State`), `branding`.\n- [x] **Phase 1.5: Workspace backbone.** `forge-workspace` (file tree, file watching, git through the CLI), the Explorer and Git panels.\n- [x] **Phase 2: Foundation.** gpui and toolchain pinned, `cargo deny`, `cargo about`, `LICENSE` and `NOTICE`.\n- [x] **Phase 3: Shared code.** `forge-ide-env`, `forge-git-credential`, `forge-runtime-processes`(+`-ui`), `forge-script-runner`, the Script Runner panel, and `Host::run_command` with outcome and busy state.

---

## Phase 4: Port the gpui-component panels

**Approach:** copy each panel and its backend as they are, then rewire the 1ÔÇô3% of lines that use Zed. The backend goes to `crates/features/<name>/core` (no gpui) and the panel to `crates/features/<name>/ui`. Each panel replaces its placeholder in `crates/app/src/shell/panels.rs` by registering with the same id first (`main.rs` start-up order).

### Rewiring table (same for every panel)

| Zed edition                                                                 | Forge Super                                                                                                |
| --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `impl Panel` (position, icon, size, toggle action, priority)                | `register_panel(PanelInfo { id, title, icon, tooltip, default_dock, order }, build)` in the crate's `init` |
| `<Name>PanelSettings` (`button`, `dock`) + `settings::update_settings_file` | `Settings::panel(id, "dock")`, plus the new hide-button setting (4.0)                                      |
| `workspace::HideStatusItem`                                                 | the hide-button setting (4.0)                                                                              |
| `workspace.project().worktrees()ÔÇª` / `first_worktree_path`                  | `Workspace::model(cx).read(cx).root()`                                                                     |
| `project::Event` / `PathChange` (file changed)                              | workspace file events (4.0)                                                                                |
| `project::git_store`                                                        | `Workspace::git()` (`GitState`)                                                                            |
| `db::kvp` (saved choices)                                                   | `State::get` / `State::set` (dotted keys, e.g. `node.version.<root>`)                                      |
| `fs::Fs` / `fs::read_to_string` / `fs::write`                               | `std::fs`, on a background task                                                                            |
| `script_runner_panel` entity, `run_external`, `is_running`, `cx.observe`    | `Host::run_command(CommandSpec)`; await the returned task; `Host::is_command_running`                      |
| `ide_env::` / `runtime_processes::`                                         | `forge_ide_env::` / `forge_runtime_processes::` (+ `forge_runtime_processes_ui::ProcessList`)              |
| `workspace.open_panel` / `reveal_panel` / `toggle_panel_focus`              | `Host::show_panel` / `Host::toggle_panel`                                                                  |
| `workspace::register_serializable_item` (manager pages as tabs)             | `Host::open_tab(TabRequest)` + restorable tabs (4.0)                                                       |
| `workspace::Toast` / notifications                                          | `Host::notify(kind, title, message)`                                                                       |
| `zed_actions::ÔÇª` (open another panel's page)                                | `Host::show_panel` / `Host::open_tab`, or a host command                                                   |
| `ui::IconName::ÔÇª`                                                           | gpui-component `IconName`, or `icons/zed_*.svg` copied into `crates/app/assets/icons` (add to `NOTICE`)    |
| `gpui_util::new_std_command`                                                | a no-console-window `Command` helper in `crates/shared` (see 4.0)                                          |
| `http_client::HttpClient`                                                   | the shared HTTP client (4.0)                                                                               |
| `paths::ÔÇª` (data folders)                                                   | `branding::data_dir()`                                                                                     |
| `zed_credentials_provider`                                                  | the shared credential store (4.0)                                                                          |

### Rules while porting

- Feature crates must not depend on each other. A runtime panel and its package manager become **one feature** with two panels. Panels that read other features (Dashboard) use those features' `core` crates, never their `ui`.
- No user-visible "Zed" text, and no `*.zed.*` URLs.
- Keep files at ~200ÔÇô500 lines; split big panels into modules (`mod.rs` only does `pub use`).
- Backends keep their tests; each port adds a headless gpui test for the main flow, like `script-runner-ui`'s.
- Run `cargo deny check` after adding dependencies.

### 4.0 Host API and shared additions (first)

- [x] **Decide:** HTTP client crate: `reqwest`. No Zed service URLs.
- [x] `crates/shared/http` (`forge-http`): a small async client (GET with JSON, download to file with progress, user agent from `branding`), used by the npm/NuGet/PyPI/crates.io searches, the Python version downloads and Helm.
- [x] `crates/shared/process` (`forge-process`): `new_command(program)` without a console window, replacing `gpui_util::new_std_command` in every backend.
- [ ] Workspace file events: `forge_host::Workspace` emits an event for changed paths (from the existing watcher), so panels can rescan when `package.json`, `*.csproj`, `Cargo.toml` or `pyproject.toml` change (replaces `project::Event` / `PathChange`).
- [ ] Restorable tabs: a tab kind registry (`register_tab_kind(kind, restore)`), with open tabs saved in `State` and reopened at start-up (replaces `register_serializable_item`, used by the manager pages and Dashboard).
- [ ] Hide a panel's status-bar button: `panels.<id>.button = false` in settings, plus "Hide button" in the button's context menu (replaces `HideStatusItem`).
- [ ] `crates/shared/credentials` (`forge-credentials`): read, write and delete secrets in Windows Credential Manager (the `keyring` crate), keyed with `branding::PRODUCT.id` (replaces `zed_credentials_provider`, used by Helm and Database).
- [ ] Settings defaults per panel: each feature can supply defaults for its own `settings.json` keys.
- [ ] Update `forge-super-design.md` ┬º13 (host API) and CLAUDE.md with the additions.

### 4.1 Cockpit, Processes, Script Runner (prove the approach)

- [x] Script Runner: done in phase 3.
- [ ] **Cockpit** (`cockpit_panel`, 1,117 lines, 26 Zed lines): `cockpit-ui` (system metrics through `sysinfo`; header). Wire settings (`button`, `dock`) and the hide-button setting.
- [ ] **Processes** (`processes_panel`, 1,162 lines, 22 Zed lines): `processes-ui`. Wire settings and the hide-button setting.
- [ ] Remove the `cockpit` and `processes` placeholders from `panels.rs` (registration replaces them; keep ids and icons).
- [ ] Tests (headless): Cockpit renders metrics; Processes lists this test process.
- [ ] Check in the app: both panels in their docks, dock moves persist, the hide-button setting works.

### 4.2 Runtimes with their package managers

Each is one feature: `<name>-core` (backend + version store) and `<name>-ui` (runtime panel + package-manager panel/pages).

- [ ] **Node** (`node_panel` 4,467 / 48 Zed lines, `npm_manager_panel` 3,342 / 31, `npm_backend` 2,292, `node_backend`, `node_versions`):
  - [ ] `node-core`: `node_backend` + `npm_backend` + `node_versions` (version store under `branding::data_dir()`; `fnm`, check its licence with `cargo deny`).
  - [ ] Register the Node runtime with `forge_ide_env::register` at start-up, so every started process gets the chosen Node.js.
  - [ ] `node-ui`: Node panel (scripts through `Host::run_command`, projects, `.sppkg` rescan after a run, Processes section with `ProcessList`), npm manager pages as restorable tabs (search, details, install/remove/update through `Host::run_command`, reload on completion).
  - [ ] The Node panel's link to Flows: leave a placeholder until Flows is ported (4.3).
  - [ ] Replace the existing `node` template crates.
- [ ] **Python** (`python_panel` 2,605 / 39, `python_manager_panel` 2,538 / 15, `python_backend`, `python_versions`):
  - [ ] `python-core`: backend + version store (downloads through `forge-http`, sha256 check).
  - [ ] Register the Python runtime with `forge_ide_env`.
  - [ ] `python-ui`: Python panel (venvs, interpreter, run, `ProcessList`) and the PyPI manager pages. `language::` use in `python_panel`: check what it needs (probably the interpreter for language servers; defer to phase 5 if so).
- [ ] **.NET** (`dotnet_panel` 1,717 / 32, `nuget_manager_panel` 2,969 / 17, `dotnet_backend`):
  - [ ] `dotnet-core`: backend (SDKs, projects, build/run/test commands).
  - [ ] `dotnet-ui`: .NET panel and NuGet manager pages. Replace the existing `dotnet` template crates.
- [ ] **Rust** (`rust_panel` 1,524 / 34, `cargo_manager_panel` 4,044 / 21, `cargo_backend`):
  - [ ] `rust-core`: backend (Cargo metadata, crates.io registry through `forge-http`).
  - [ ] `rust-ui`: Rust panel and cargo manager pages.
- [ ] For each: the command checks (`forge_script_runner::command::check_*`) are used wherever a name, version or path goes into a command line.
- [ ] For each: tests (backend tests copied; a headless panel test), `cargo deny check`, a look in the app on a scratch project (never the user's real repositories).

### 4.3 Dashboard, Flows, Helm

- [ ] **Dashboard** (`dashboard_panel`, 1,242 / 12 Zed lines): reads the runtimes through `node-core`, `python-core`, `dotnet-core`, `rust-core` and git through `Workspace::git()`, not through the panels. Opens as a restorable tab.
- [ ] **Flows** (`flows_panel` 1,339 / 44 Zed lines, `workflow_engine` 8,082 lines):
  - [ ] `flows-core`: `workflow_engine` (already Zed-free except `ide_env`; uses `reqwest`/`tokio`: align with the HTTP decision).
  - [ ] `flows-ui`: the Flows panel; `db::kvp` to `State`; persistence files under `branding::data_dir()`.
  - [ ] The Designer link waits for phase 6; leave a placeholder.
  - [ ] Hook the Node panel's Flows link up.
- [ ] **Helm** (`helm_panel` 10,612 / 54 Zed lines, `helm_backend` 6,629, `helm_ui`):
  - [ ] `helm-core`: `helm_backend` (GitHub API through `forge-http`).
  - [ ] Accounts and tokens in `forge-credentials`; `gh` CLI import.
  - [ ] Per-repository account: `forge_ide_env::set_git_credential` and `set_git_remote_config`, so terminals and git push/pull use the repository's account (phase 3 groundwork).
  - [ ] `helm-ui`: account switcher, repositories, pulls, issues, releases, workflow runs, collaborators, invitations (28 modules; split by area).
  - [ ] `helm_workspace` (repository view on Zed's editor/markdown) waits for phase 6; placeholder until then.
  - [ ] Test git credentials against the scratch repository only.
- [ ] Update CLAUDE.md (feature list) and the design doc (┬º13.x phase 4 result).

---

## Phase 5: Zed core (editor and terminal)

- [ ] Vendor Zed at `d86aa6e6` (the Zed edition's base); decide the vendoring layout (`vendor/zed/ÔÇª`) and how `gpui` is unified with Mentenaz.GPUI (design doc ┬º12.4, open item 1).
- [ ] Embedding spike first: one Zed editor in a shell tab, before vendoring everything.
- [ ] Stubs that cut every Zed service: `client` (no sign-in, no collaboration), `telemetry` (off), `agent_settings`, auto-update, extensions marketplace; the HTTP base URL from `branding`. Verify with a network capture that nothing reaches `*.zed.*`.
- [ ] Replace `forge-workspace` with Zed's `project` behind the same `forge_host::Workspace` API (`root()` = the first visible worktree); panels unchanged. Add `worktrees()` only if multi-folder is ever wanted.
- [ ] Editor tabs through `Host::set_file_opener` (replaces the read-only file view).
- [ ] Zed's terminal in the bottom dock; the terminal environment gets `forge_ide_env::apply_to` (runtimes) and `apply_git_credential_env` (credential helper).
- [ ] Language servers, diagnostics, project search; replace the fake data in `crates/app/src/shell/dummy.rs` (LSP menu, diagnostics, active file) with the real sources, then delete `dummy.rs`.
- [ ] Zed's git panel and pickers over `project` (Git panel stand-in retired, or kept if preferred).
- [ ] `cargo deny check` and `cargo about` over the vendored crates; `NOTICE` updated; no user-visible "Zed" text.
- [ ] Done when: the shell opens a folder and edits with language servers, with no Zed traffic.

## Phase 6: Panels built on the editor

- [ ] **Designer** (`designer_panel`): project items in editor tabs; `fs::` calls to `std::fs`; `register_project_item` to Zed's project API (now vendored).
- [ ] **Database** (`database_panel`, SQL workbench): editor buffers for SQL, credentials in `forge-credentials`, `db::kvp` to `State`, toasts to `Host::notify`.
- [ ] **Helm repository view** (`helm_workspace`, markdown and editor).
- [ ] Hook up the placeholders left in Flows (Designer) and Helm (repository view).

## Phase 7: Zed patches as overrides

- [ ] Git account handling in Zed's git (per-repository credentials and `GIT_CONFIG_*` from `forge_ide_env::with_git_remote_config`), as the Zed edition patched `git/src/repository.rs` and `project/src/git_store.rs`.
- [ ] Terminal environment hooks (`project/src/terminals.rs`): runtimes and the credential helper.
- [ ] Debugger (`dap_adapters`: Node.js from `forge_ide_env`).
- [ ] Editor hooks the Zed edition added.
- [ ] Each patch kept as a small override with a note of what it changes (design doc ┬º12.1).

## Phase 8: Distribution

- [ ] Release build: `windows_subsystem = \"windows\"`, app icon and version resource, release `branding::PRODUCT` (ids without `-dev2`, folder without `Dev2`).
- [ ] Installer (MSI or MSIX) and code signing.
- [ ] GitHub Actions: build, test, `cargo deny check`, `cargo about generate`, attach `THIRD_PARTY_LICENSES.md` to the release.
- [ ] GitHub Releases, then the winget manifest (`winget-pkgs` PR).
- [ ] Update check against GitHub Releases (no Zed update servers).
- [ ] Microsoft Store: deferred; test another platform first.

## Phase 9: Upstream watcher

- [ ] `upstream-watch.toml` and `upstream-sync.json` (design doc ┬º8): which Zed crates are tracked and at which commit.
- [ ] A job that reports new upstream commits touching the vendored crates and the patched files.
- [ ] Same for Mentenaz.GPUI's upstreams (gpui, gpui-component).

---

## Open decisions

- [x] HTTP client: `reqwest`.
- [ ] One feature per runtime with its package manager (suggested), e.g. `node` = Node panel + npm manager.
- [x] The stray formatting changes: committed as formatting-only commit.
- [ ] Phase 5: how Zed's `gpui` and Mentenaz.GPUI's `gpui` become one (design doc open item 1).
