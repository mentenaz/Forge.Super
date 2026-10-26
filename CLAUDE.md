# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

**Goal:** rebuild the working Zed edition of Forge Super (`E:\zed_Source`, a Zed fork) with the same functionality, minus everything that talks to Zed-hosted services and the Zed branding. The product window is our own gpui-component shell (`crates/app/src/shell/`). Its look is settled first; then the Zed edition's panels (already gpui-component views) are ported into it rather than rebuilt, by swapping their thin Zed layer (`impl Panel`/`Item`, `workspace`/`project`/`settings`/`db` calls) for a small host API in `crates/shared/`. Their backends have no Zed dependencies and move into the feature `core` crates. Zed itself is vendored later, to embed its editor and terminal. All shared code goes in `crates/shared/`. The current decisions and phase order are in `forge-super-design.md` §2 and §12.6.

Forge Super is a Rust IDE built on GPUI (Zed's UI framework), consumed from the fork at `https://github.com/mentenaz/Mentenaz.GPUI.git`. The goal is to *not* hard-fork Zed: reusable gpui logic lives in the gpui repo, and this repo is only the product (its own name, paths, update sources and panels). The design doc is `forge-super-struktuur.md` (written in Afrikaans); it is the source of truth for structure rules and lists open design decisions (§7) that are not settled yet, so don't treat them as decided.

The project is an early scaffold: `crates/app/src/main.rs` opens one window hosting `ForgeShell` (`crates/app/src/shell/`), ported from the shell layout of the previous project at `E:\Forge_GPUI\src\forge_shell.rs` (without its LSP, editor, session-persistence and main-menu parts). Left panels, tab contents and bottom docks are still `PlaceholderPanel`/`PlaceholderView` (`shell/panels.rs`); real ones will come from the feature `ui` crates. App-specific icons and graphics live in `crates/app/assets/` and are embedded via `crates/app/src/assets.rs`, which falls back to gpui-component's bundled icons. The feature crates' source files are empty, and `crates/branding/` and `crates/shared/` are empty directories that are not yet workspace members.

## Commands

```sh
cargo build                              # whole workspace
cargo run -p mentenaz-forge-super        # run the app (binary crate in crates/app)
cargo check -p node-ui                   # check a single crate
cargo test -p node-core                  # tests for one crate
cargo test -p node-core <test_name>      # a single test
cargo update -p gpui                     # refresh the locked gpui commit
```

## Architecture

Workspace members are `crates/app` plus the globs `crates/features/*/core` and `crates/features/*/ui`, so a new feature is picked up automatically once it has `core/` and `ui/` packages. Shared dependency versions, including each feature crate, are declared in the root `[workspace.dependencies]` and consumed with `.workspace = true`; add new features there too.

- `crates/app` (package `mentenaz-forge-super`): the binary. It wires things together and registers the feature panels.
- `crates/features/<name>/core` (`<name>-core`): **GPUI-free** domain code. Planned layout is `src/logic/` (pure types, state, rules, commands) and `src/backend/` (IO: processes, files, network, CLI wrappers). It must stay reusable from non-GPUI frontends (CLI, Tauri, tests).
- `crates/features/<name>/ui` (`<name>-ui`): GPUI views. `panel.rs` is the sidebar/dock view and `workspace.rs` the main-area view.
- Current features: `node`, `dotnet` (`helm` is planned).

### Rules from the design doc

- One package per `core` and per `ui`; crate names are lowercase kebab-case and unique (`node-core`, `node-ui`).
- `ui` depends on `core`, never the reverse. Never add a gpui dependency to a `core` crate.
- Feature crates must not depend on each other; shared code goes in `crates/shared/`.
- Split large files with modules, not new crates: aim for ~200–500 lines per file, and `mod.rs` only does `pub use`.
- `backend/` may later be extracted into its own crate/repo, so keep its module paths stable.

### gpui dependency

A local clone of the gpui library (same repo as the git dependency, `origin` = `mentenaz/Mentenaz.GPUI.git`) is at `E:\mentenaz\gpui`. Read it there to look up APIs, instead of going through `~/.cargo/git` or GitHub. Useful places:
- `crates/gpui` is core GPUI (`App`, `Window`, `Context`, elements, styling).
- `crates/gpui_component` (package `gpui-component`) has the widgets: `Root`, `dock`, `sidebar`, `button`, `input`, `list`, `menu`, `dialog`, `setting`, `status_bar`, etc., one module each under `src/`.
- `crates/gpui_component_story` has runnable usage examples for the components.
- `crates/gpui_platform` provides the platform setup (`current_platform`).

This repo builds against the git revision locked in `Cargo.lock`, not the local clone (unless it's patched in, see below), so the local clone may be ahead of or behind what actually gets compiled. Check `git -C E:/mentenaz/gpui log` against the locked sha in `Cargo.lock` when an API doesn't seem to match.

- `gpui`, `gpui-component` and `gpui_platform` all come from the same git repo. When adding another gpui crate, list it separately in `[workspace.dependencies]` with the same URL (wildcards don't work).
- `Cargo.lock` is committed. The intent is to pin gpui with a full-sha `rev` and bump it in small dedicated commits (`bump gpui to abc1234`); currently the deps are unpinned and rely on the lockfile.
- For local gpui development use a gitignored `.cargo/config.toml` with `[patch."<exact dependency URL>"]` pointing at the local checkout (e.g. `gpui = { path = "E:/mentenaz/gpui/crates/gpui" }`). The URL must match the dependency URL exactly, and **every** gpui crate in use must be patched, otherwise you get duplicate types. Delete the file to go back to the locked revision.
