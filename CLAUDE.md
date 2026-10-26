# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

**Goal:** rebuild the working Zed edition of Forge Super (`E:\zed_Source`, a Zed fork) with the same functionality, minus everything that talks to Zed-hosted services and the Zed branding. The product window is our own gpui-component shell (`crates/app/src/shell/`). The Zed edition's panels (already gpui-component views) are ported into it rather than rebuilt, by swapping their thin Zed layer (`impl Panel`/`Item`, `workspace`/`project`/`settings`/`db` calls) for a small host API in `crates/shared/`. Their backends have no Zed dependencies and move into the feature `core` crates. Zed itself is vendored later, to embed its editor and terminal. All shared code goes in `crates/shared/`.

The source of truth for decisions and phase order is `forge-super-design.md` (§2 decisions, §12.6 phases). `forge-super-struktuur.md` (Afrikaans) is the earlier structure write-up it supersedes; kept for history.

Status: phase 0 (shell look) is done. Panels, tab contents and docks still show placeholders; next is the host API (phase 1).

Repositories: this one is `github.com/mentenaz/Forge.Super` (public, GPL-3.0-or-later). The UI library is `github.com/mentenaz/Mentenaz.GPUI` (Apache-2.0), cloned at `E:\mentenaz\gpui`.

## Commands

```sh
cargo build                              # whole workspace
cargo run -p mentenaz-forge-super        # run the app (binary crate in crates/app)
cargo check -p node-ui                   # check a single crate
cargo test -p node-core                  # tests for one crate
cargo test -p node-core <test_name>      # a single test
cargo update -p gpui                     # move Cargo.lock to Mentenaz.GPUI's latest main
```

## Architecture

Workspace members are `crates/app` plus the globs `crates/features/*/core` and `crates/features/*/ui`, so a new feature is picked up automatically once it has `core/` and `ui/` packages. Shared dependency versions, including each feature crate, are declared in the root `[workspace.dependencies]` and consumed with `.workspace = true`; add new features there too. Every crate sets `version`, `edition` and `license` from the workspace.

- `crates/app` (package `mentenaz-forge-super`): the binary and the shell. Wires things together; will register the feature panels.
- `crates/features/<name>/core` (`<name>-core`): **GPUI-free** domain code. `src/logic/` is pure domain (types, state, rules, commands), `src/backend/` is IO (processes, files, network, CLI wrappers). Must stay reusable from non-GPUI frontends (CLI, Tauri, tests).
- `crates/features/<name>/ui` (`<name>-ui`): gpui-component views. `panel.rs` is the dock view, `workspace.rs` the tab-area view. Not yet registered with the shell.
- Current features: `node`, `dotnet` (templates only). `crates/shared/` and `crates/branding/` don't exist yet; they arrive with phases 1–2.

### The shell (`crates/app/src/shell/`)

Laid out like the Zed edition: title bar, left/right/bottom docks, per-pane tab bars with a split, status bar. One concern per module:

- `forge_shell.rs`: `ForgeShell` (all state), actions and key bindings, the overall layout (`render_main_area`).
- `docks.rs`, `panels.rs`: Zed's dock model (three docks, several panels each, one shown); `PanelId` lists every panel with its title, icon and default dock. `PlaceholderPanel`/`PlaceholderView` stand in for real views.
- `tabs.rs`, `pane.rs`, `tab_bar.rs`, `tab_context_menu.rs`, `tab_switcher.rs`, `nav_history.rs`: tab state and operations, panes and breadcrumbs, the tab strip (drag and drop, pinning, folder hints), the right-click menu, Ctrl+Tab switcher, back/forward.
- `title_bar.rs`, `app_menu.rs`, `recent_projects.rs`, `worktree_picker.rs`: title bar, application menu (☰ or menu bar row), project and worktree pickers.
- `status_bar.rs`, `notifications.rs`: status bar (dock buttons, diagnostics, language servers, editor info), toasts and the notification centre.
- `theme.rs`: default theme (Forge Dark from `assets/themes/forge.json`) and font sizes (13px UI, 15px mono, matching the Zed edition).
- `dummy.rs`: **all fake data** (git, language servers, editor, files, notifications). Replace it piece by piece as the host API and feature crates land; don't add fake data elsewhere.

### Assets and licensing

App icons, graphics and the theme live in `crates/app/assets/` and are embedded with rust-embed (`crates/app/src/assets.rs`, falling back to gpui-component's bundled icons). Icons copied from the Zed edition are prefixed `zed_`. Any third-party asset needs an entry (and its licence text if required) in `NOTICE`. No user-visible text may say "Zed" (trademark).

### Rules from the design doc

- One package per `core` and per `ui`; crate names are lowercase kebab-case and unique (`node-core`, `node-ui`).
- `ui` depends on `core`, never the reverse. Never add a gpui dependency to a `core` crate.
- Feature crates must not depend on each other; shared code goes in `crates/shared/`.
- Split large files with modules, not new crates: aim for ~200–500 lines per file, and `mod.rs` only does `pub use`.
- `backend/` may later be extracted into its own crate/repo, so keep its module paths stable.

### gpui dependency

Read the local clone at `E:\mentenaz\gpui` to look up APIs, instead of going through `~/.cargo/git` or GitHub. Useful places:
- `crates/gpui` is core GPUI (`App`, `Window`, `Context`, elements, styling).
- `crates/gpui_component` (package `gpui-component`) has the widgets: `Root`, `dock`, `sidebar`, `button`, `input`, `list`, `menu`, `popover`, `dialog`, `status_bar`, etc., one module each under `src/`. `crates/gpui_base` holds lower-level pieces (input state, resizable, global state).
- `crates/gpui_component_story` has runnable usage examples, including the `Gallery`.
- `crates/gpui_platform` provides the platform setup (`application()`).

This repo builds against the git revision locked in `Cargo.lock`, not the local clone, so the clone may be ahead of or behind what gets compiled. Compare `git -C E:/mentenaz/gpui log` with the locked sha when an API doesn't seem to match.

- `gpui`, `gpui-component`, `gpui-component-assets` and `gpui_platform` all come from the same git repo. When adding another gpui crate, list it separately in `[workspace.dependencies]` with the same URL (wildcards don't work).
- `Cargo.lock` is committed. Library changes go to Mentenaz.GPUI first (commit, push), then `cargo update -p gpui` here.
- To build against unpushed library changes, use a gitignored `.cargo/config.toml` with `[patch."https://github.com/mentenaz/Mentenaz.GPUI.git"]` pointing each of the four crates above at `E:/mentenaz/gpui/crates/...`. Patch **all** of them, or you get duplicate types. While patched, `Cargo.lock` records the gpui crates as local path packages. To leave the patch: push the library change, delete `.cargo/config.toml`, then run a plain `cargo build`, which re-adds the git entries at Mentenaz.GPUI's current `main` (`cargo update -p gpui` can't do this step, because the lock no longer has a git entry for it).
