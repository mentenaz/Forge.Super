# Forge Super: Design Write-up

**Date:** 2026-10-26
**Status:** Design phase, nothing built yet beyond the root `Cargo.toml` fix
**Supersedes:** `forge-super-struktuur.md` (written before the vendoring and licence decisions)

Confidence tags used below:
- **[Verified]** seen in output from `E:\zed_Source`
- **[Inferred]** my reading of the code or my recollection of how Zed works; not confirmed
- **[Open]** undecided or not yet checked

---

## 1. Goal

Stop maintaining a hard fork of Zed. Every rename of paths, sources and branding in the fork touches the files that upstream also touches, so each pull means merge conflicts. Replace it with a clean repo that:

- takes a **pinned, read-only snapshot** of the Zed crates it needs,
- keeps all Mentenaz-specific code **outside** that snapshot,
- uses **Mentenaz.GPUI** as the single gpui,
- cuts everything that talks to Zed-hosted services,
- is fed by an **upstream watcher** that reports what changed upstream, so changes can be ported deliberately.

Distribution targets: winget, GitHub Releases, possibly the Microsoft Store. No built-in auto-updater.

---

## 2. Decisions made

| # | Decision | Notes |
|---|---|---|
| 1 | Three layers: `Mentenaz.GPUI` (library), `forge-super` (product), upstream watcher | |
| 2 | Zed is consumed as a **vendored snapshot** in `vendor/zed/`, never edited in place | Git-dep model rejected: Zed is not published as a library, and ~10 to 20 upstream commits/day break every bump |
| 3 | Own code lives in `crates/` and `crates/overrides/` | A Zed crate that must change is copied into `overrides/` and the root workspace points at the copy |
| 4 | **Mentenaz.GPUI is the only gpui** | No second copy of gpui in the build (avoids duplicate-type errors) |
| 5 | Licence: **GPL-3.0-or-later** for forge-super | Forced by `workspace`, `project`, `editor`, `client` etc. being GPL; the user chose it |
| 6 | Mentenaz.GPUI stays Apache-2.0 | Apache-2.0 code may be used inside a GPL-3.0 product |
| 7 | Package model per feature: `core` (no gpui) + `ui` (gpui) | Recommended, not explicitly confirmed by the user |
| 8 | Pin gpui with `rev`, commit `Cargo.lock`, use a gitignored `.cargo/config.toml` `[patch]` for local gpui work | |
| 9 | No Zed auto-updater. Updates come via winget / GitHub Releases / Store | |
| 10 | **Extensions: DEFERRED** (see section 9) | |
| 11 | **Scope: rebuild the working Zed edition (`E:\zed_Source`) with the same functionality**, removing only what talks to Zed-hosted services (`*.zed.*`) and the Zed branding | Decided 2026-10-26 |
| 12 | **The product window is an own gpui-component shell** (`crates/app/src/shell/`), not Zed's `Workspace` UI. Its look is settled first; the existing gpui-component panels are then ported into it. Zed's editor, terminal etc. are embedded as content in its tabs and docks | Decided 2026-10-26. See 12.4 for the embedding risk, 12.6 for the order |
| 13 | **All shared functionality goes in `crates/shared/`**, including code features use from each other (script runner, `runtime_processes`, `ide_env`, ...) | Decided 2026-10-26 |
| 14 | Vendor Zed at upstream **`d86aa6e6`** (2026-10-22), the base the Zed edition and Mentenaz.GPUI are on | See section 12 |
| 15 | v1 ships via **winget and GitHub Releases**. Microsoft Store is out of scope until tested with another platform first | Decided 2026-10-26 |

---

## 3. Repository layout

```
forge-super/
  Cargo.toml                 (virtual workspace, no [package])
  Cargo.lock                 (committed, this is a binary)
  LICENSE                    (GPL-3.0)
  NOTICE                     (credits Zed as the origin of vendored code)
  upstream-sync.json         (last reviewed Zed sha + Mentenaz.GPUI sha, bumped together)
  upstream-watch.toml        (watchlist and buckets, see section 8)
  .cargo/config.toml         (GITIGNORED: local [patch] for gpui)
  vendor/
    zed/                     (read-only snapshot of the chosen Zed crates)
  crates/
    app/                     (binary: own main, sets base URL, wiring)
    branding/                (app name, config paths, URL scheme, endpoints)
    overrides/               (own versions of client, telemetry, title_bar, ...)
    shared/                  (small shared crates, e.g. process spawning)
    features/
      node/    core/  ui/
      dotnet/  core/  ui/
      helm/    core/  ui/
  reports/                   (watcher output, one markdown file per run)
```

**Feature crates**
- `core` is gpui-free so Tauri, a CLI and tests can reuse it. Inside it, `logic/` is pure domain (types, state, rules) and `backend/` is IO (processes, files, network, CLI wrappers).
- `ui` is one crate with `panel.rs` and `workspace.rs` (the dock view and the main-area view) sharing state.
- Split large files by **modules, not crates**. Aim for 200 to 500 lines per file; `mod.rs` only does `pub use`.
- `ui` depends on `core`, never the reverse. Features do not depend on each other; shared code goes to `crates/shared/`.
- Crate names are lowercase kebab-case and unique (`node-core`, `node-ui`), because directory names `core` and `ui` repeat.

**Root `Cargo.toml` (corrected)**
The earlier root had two `[package]` blocks and a `[dependencies]` table. A root with `[package]` needs a `src/main.rs`, which caused the rust-analyzer error. It must be a virtual workspace:

```toml
[workspace]
resolver = "2"
members = [
    "crates/app",
    "crates/branding",
    "crates/overrides/*",
    "crates/shared/*",
    "crates/features/*/core",
    "crates/features/*/ui",
    # plus the chosen vendor/zed crates
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "GPL-3.0-or-later"

[workspace.dependencies]
gpui = { git = "https://github.com/mentenaz/Mentenaz.GPUI.git", rev = "<sha>" }
gpui-component = { git = "https://github.com/mentenaz/Mentenaz.GPUI.git", rev = "<sha>" }
gpui_platform = { git = "https://github.com/mentenaz/Mentenaz.GPUI.git", rev = "<sha>" }
node-core = { path = "crates/features/node/core" }
node-ui = { path = "crates/features/node/ui" }
dotnet-core = { path = "crates/features/dotnet/core" }
dotnet-ui = { path = "crates/features/dotnet/ui" }
```

Each crate carries its own `Cargo.toml` and inherits with `version.workspace = true`, `edition.workspace = true`, `license.workspace = true`, `gpui.workspace = true`. Every directory matched by a glob must contain a `Cargo.toml`. `edition = "2024"` needs Rust 1.85 or newer. The vendored Zed crates use `x.workspace = true` heavily, so the root `[workspace.dependencies]` must also carry every entry those crates use (copy Zed's table, then cut). Wildcards such as `gpui-*` do not work; list each gpui crate.

---

## 4. gpui strategy

- **[Verified]** Zed's `gpui` and `gpui_base` are Apache-2.0; `http_client` is Apache-2.0.
- Zed crates resolve `gpui.workspace = true` against the root, so swapping the root entries to Mentenaz.GPUI replaces gpui everywhere without a second copy.
- **Lockstep rule:** the Zed commit in `vendor/zed` must be the commit Mentenaz.GPUI's gpui crates are based on. Record both in `upstream-sync.json` and bump them together. `editor`, `workspace` and `ui` call gpui internals directly, so drift breaks the build.
- The watcher's `crates/gpui*` feed targets **Mentenaz.GPUI**, not forge-super. It is the busiest feed (133 file changes in 30 days in the fork's history).
- **[Open]** Which Zed commit Mentenaz.GPUI is based on, and where gpui itself (not just gpui-component) was modified. The user mentioned having to change things when building on gpui-component and later writing a theme helper function. If gpui itself diverged, compare before committing to lockstep:
  `git diff --no-index --stat E:\zed_Source\crates\gpui\src <Mentenaz.GPUI path>\crates\gpui\src`

---

## 5. Cutting Zed's services

**Dependency footprint [Verified]:** `workspace`, `project`, `editor` and `title_bar` pull **115 of Zed's 300** workspace crates (an earlier "191" figure was a counting error).

**The service entanglement sits in about seven places [Verified from reverse-dependency output]:**

| Point | Action | Why |
|---|---|---|
| `title_bar` | Rewrite in `overrides/` | Depends on 7 service crates: `auto_update`, `call`, `channel`, `livekit_client`, `cloud_api_types`, `telemetry`, `agent_settings`. Rewriting it drops `call` and `livekit_client` from the build |
| `client` | Stub in `overrides/` | The only crate pulling `cloud_api_client`. `rpc::*` and `user::*` are re-exported by `client`, so `proto`, `TypedEnvelope`, `PeerId`, `ErrorCode`, `RpcError`, `User`, `UserStore`, `Collaborator`, `ParticipantIndex` come from those, not from `client` itself |
| `telemetry` | No-op stub with the same macro API | Used by `workspace`, `editor`, `fs`, `dap`, `remote`, `recent_projects`, `extension_host`, `call`, `client`, `title_bar` |
| `agent_settings` | Stub | Used by `title_bar` and `workspace`; pulls `language_model` |
| `recent_projects` | Patch | The only crate pulling `extension_host` |
| `remote_connection` | Patch | Pulls `auto_update` |
| `notifications` | Patch | Pulls `channel` |

**Client surface the stub must provide [Verified, lower bound]:** `Client`, `Client::new`, `client::init`, `Status`, `PendingEntitySubscription`, `Subscription`, `CONNECTION_TIMEOUT`, `ProxySettings`, `zed_urls`, `parse_zed_link`. Not yet checked: where `ProjectId`, `ChannelId` and `ProxySettings` are defined. `dap`, `repl` and `language_tools` only need `proto`.

**Type crates that must stay** (other crates depend on them): `cloud_api_types` (Apache-2.0), `cloud_llm_client`, `zeta_prompt`, `language_model_core`, `telemetry_events`. **[Open]** whether they are passive (serde types only). The check that was meant to confirm this returned nothing because `$meta` was not loaded.

**Crates not taken at all:** `cloud_api_client`, `edit_prediction*`, `language_models_cloud`, `web_search_providers`, `auto_update`, `collab*`, `call`, `channel`, `livekit_client`, `feedback`, `agent*`, `ai_onboarding`, `zed` (own `app` instead).

**`http_client` [Verified]:** Apache-2.0, so freely editable. Delete the four `build_zed_*` URL builders (`zed.dev` maps to `api.zed.dev`, `cloud.zed.dev`, `llm-staging.zed.dev`). The `other => other` arm means a non-Zed base URL is used as-is, so nothing is sent to Zed once the base URL is not `zed.dev`. The base URL is set by `http.set_base_url(..)` in `crates/zed/src/main.rs:972`, which is the binary crate that is not vendored; the own `app` crate does this with a value from `branding`.

**Identity strings to replace (put all in `branding`):**
- `zed://` URL scheme (`open_listener.rs`). It is registered with the OS and would collide with a real Zed install. Use something like `mentenaz-forge://`.
- `ZED_SERVER_URL` environment variable and `server_url` setting (`client.rs:63` onward, `default.json`).
- Feedback and issue URLs and `hi@zed.dev` (`feedback.rs`).
- User-Agent strings, OAuth client metadata, bundle IDs, publisher names, window titles, installer metadata.
- The `@zed-industries/vscode-langservers-extracted` npm package referenced by `extensions/html`. **[Open]** whether the unscoped upstream package works as a replacement.

**Verified harmless:** `lsp_store.rs` (18,000 lines) has only three comment links to Zed GitHub issues and no `zed.dev` URL. The terminal, git-hosting and gpui crates only contain GitHub links in comments and tests. Node runtime downloads Node from `nodejs.org`, not a Zed server. `registry.npmjs.org` and `github.com` fetches are public services.

---

## 6. Licensing and legal [not legal advice]

| Crate | Licence [Verified] |
|---|---|
| `gpui`, `gpui_base`, `http_client`, `cloud_api_types`, `extension_api` | Apache-2.0 |
| `proto`, `rpc`, `client`, `workspace`, `project`, `editor`, `terminal`, `extension`, `extension_host`, `extension_cli` | GPL-3.0-or-later |

Obligations that follow from shipping a GPL-3.0-or-later binary:
1. Ship a `LICENSE` file; set `license = "GPL-3.0-or-later"` in `[workspace.package]`.
2. Keep Zed's copyright notices in every copied file; add own notices alongside; credit Zed in `NOTICE` and the README as a factual origin, never as the product name.
3. Make the corresponding source available for every release (the GitHub repo with a tag per release suffices).
4. Run `cargo deny check licenses` to catch incompatible dependencies (for example AGPL collab crates pulled in by accident) and `cargo about` to generate the third-party licence list.

Trademark: the product cannot carry the name "Zed" in anything a user sees (app name, binary, icons, window title, About dialog, installer, winget `PackageIdentifier`). "Mentenaz Forge" is the product name.

Zed's Terms of Service apply to Zed's hosted Service, not to the source licences. The relevant points for a redistributed fork: section 2.4(a) bars passing Service access to third parties, 2.4(b) bars use of non-public APIs, and 9.4 lets Zed change or end the Service at any time. That is the reason every Zed-hosted dependency is cut instead of redirected.

**[Open]** Microsoft Store: the Store terms and GPL can conflict. winget and GitHub Releases are not affected. Check before taking the Store route.
**[Open]** A lawyer should confirm the trademark and licence handling before the first public release. Writing to `legal@zed.dev` for written confirmation that a renamed fork is acceptable is an option.

---

## 7. Updates and distribution

- No in-app updater. winget and the Store manage updates; a built-in updater would fight them.
- winget: manifest submitted by PR to the community repo, installer hosted on GitHub Releases.
- apt: there is no central register for third parties; this means hosting an own apt repo or a PPA. **[Open]**, check the rules before committing.
- For a plain GitHub download, an optional "newer release available" check that only reads the own repo's Releases API can be added later.

---

## 8. Upstream watcher

**Purpose:** after each upstream fetch, report what changed that affects the chosen crates, with enough detail not to miss fixes, without reading 100 commits.

**Mechanism (no AI needed for the core):**
1. `git fetch upstream` (a partial clone `--filter=blob:none` keeps it light).
2. Diff from the last reviewed sha in `upstream-sync.json`; the marker moves forward automatically after each run, so a missed week is just a bigger report.
3. Filter by path with a **watchlist of crates**. Every commit touching a watchlist crate is listed, whatever it is called; nothing is dropped for being labelled "fix".
4. Use `cargo metadata` to find **reverse dependencies**, so a change in an underlying crate that affects a watched crate is flagged.
5. Separately flag changes to `Cargo.toml`, `Cargo.lock` and `rust-toolchain.toml`.
6. Labels from title keywords: `fix`, `crash`, `panic`, `regression`, `security` = **fix**; `breaking`, `remove`, `rename` or a change in a `pub` file = **API risk**. A wrong label only costs sort order, nothing is discarded.
7. Zed uses squash merges, so each commit is one PR; the `#12345` in the title gives a link.
8. `ported.json` records which PRs were reviewed or ported.

**Config (`upstream-watch.toml`):** watchlist crates, ignore globs (collab, cloud, telemetry, CI), keyword lists, bucket rules.

**Report:** one markdown file per run in `reports/`, grouped as: touches your crates, dependency and toolchain changes, API risk, everything else (titles only).

**Schedule:** run daily (small reports), read weekly. Where it runs: GitHub Actions cron (free, no machine needed) or Windows Task Scheduler with a PowerShell script. **[Open]** whether the output should be a GitHub Issue/PR or a file in the repo.

**Optional AI layer:** a one-line summary per PR and an "will this clash with my overrides" check. A bonus on top, not required.

**Churn measurement [Verified, fork history, last 30 days]:** `gpui` 133, `project` 84, `editor` 76, `language` 37, `settings_content` 29, `ui` 21, `remote` 20, `markdown` 19. This includes own work (for example `npm_backend` 16). **[Open]** rerun against `upstream/main` for a clean figure.

**Sync cadence:** with a vendored snapshot, bump a few times a year and use the watcher to pick up fixes and security patches in between.

---

## 9. Extensions: DEFERRED

**Status: deferred.** The user put extensions on hold on 2026-10-26. The investigation is recorded here so it can be resumed without redoing it. Nothing below is decided except where stated.

### 9.1 Interim v1 behaviour (while deferred)
- No registry. Nothing reaches Zed's servers.
- Neutralise the registry calls: empty `auto_install_extensions` in the shipped `default.json` (it currently contains `"html": true`, so the app would try to install HTML from the registry on every start and fail), disable `check_for_updates`, and disable extension suggestions (`suggest_extensions`, `extension_suggestions.rs`).
- `extension` and `extension_host` stay in the build graph (`extension` is pulled by `project` and `snippet_provider`, `extension_host` by `recent_projects`), so they must compile, with their network calls neutralised.
- Dev extensions are not blocked: `install_dev_extension` exists at `extension_host.rs:1119`. **[Inferred]** it compiles the extension locally, which needs a Rust toolchain on the user's machine.

### 9.2 Registry contract [Verified]
Five GET paths, all built through `build_zed_api_url`:

| Path | Parameters the client sends | Response |
|---|---|---|
| `/extensions` | `max_schema_version`, `filter` (search), `provides` (comma list, kebab-case) | `{ "data": [ExtensionMetadata] }` |
| `/extensions/updates` | `min/max_schema_version`, `min/max_wasm_api_version`, `ids` (comma list) | `{ "data": [...] }`, only the requested ids |
| `/extensions/{id}` | none | `{ "data": [...] }`, all versions of one extension |
| `/extensions/{id}/download` | schema and wasm API ranges | `.tar.gz` |
| `/extensions/{id}/{version}/download` | none | `.tar.gz` |

`ExtensionMetadata` (in `crates/cloud_api_types/src/extension.rs`, Apache-2.0) flattens the manifest into the object:
`id`, `name`, `version`, `description`, `authors`, `repository`, `schema_version`, `wasm_api_version`, `provides` (`languages`, `themes`, `icon-themes`, `grammars`, `language-servers`, `debug-adapters`, `context-servers`, `snippets`, plus deprecated `agent-servers`, `slash-commands`, `indexed-docs-providers`), `published_at`, `download_count`.

Behaviour notes:
- 4xx responses become errors; the body goes straight to serde.
- The updates call keeps an entry when its version differs from the installed one or it is not installed, so a static "return everything" file would mark every uninstalled extension as an update. The server must honour `ids`.
- The download is read fully into memory, and if `Content-Length` is present it must equal the byte count exactly: no transport compression on the download path (cPanel `mod_deflate` can break it). Redirects are followed, so a 302 to a GitHub Release asset works **[Inferred, untested]**.
- Extracted to a temp dir and renamed into place; the tarball root must be the extension contents itself (`extension.toml` at the root), no wrapper folder.
- There is no hash or signature check, only the length check. Extensions can download and run language-server binaries, so anything published runs on users' machines. Minimum: HTTPS only, release assets under an account with 2FA. Later: add a SHA-256 field and verification via an override.
- `is_version_compatible` lets an entry through when `wasm_api_version` is missing; always set it anyway.

### 9.3 Versions and channels
- `SchemaVersion::CURRENT` = 1 [Verified].
- Stable and Preview accept WASM API up to **0.7.0** (`since_v0_6_0::MAX_VERSION`); other channels use `latest`, which is probably **0.8.0** (`since_v0_8_0::MAX_VERSION`) **[Inferred]**.
- The release channel the app reports therefore decides which `zed_extension_api` version extensions must be built against. Every Zed sync can shift that range and force a rebuild of every published extension.

### 9.4 Extension format
Source layout (from `extensions/html` [Verified]): `extension.toml`, `Cargo.toml` (`crate-type = ["cdylib"]`, `zed_extension_api = "0.7.0"`), `src/*.rs`, `languages/<lang>/{config.toml, highlights.scm, brackets.scm, indents.scm, injections.scm, outline.scm, overrides.scm}`, a licence file.

`extension.toml` has `id`, `name`, `description`, `version`, `schema_version`, `authors`, `repository`, `[language_servers.<id>]` (with `language_ids`), and `[grammars.<name>]` with a git `repository` and pinned `commit`.

Packaging (`extension_cli`, GPL) [Verified]: compiles the extension and grammars, validates the manifest and tests grammars, languages, themes, snippets and debug-adapter schemas, copies resources, then runs the system `tar -czvf archive.tar.gz -C archive .`. Output is `archive.tar.gz` plus `manifest.json` (a serialised `ExtensionApiManifest`), so a catalogue entry is `manifest.json` plus `id`, `published_at`, `download_count`, and `catalog.json` can be generated by CI. `wasm_api_version` appears to come from the `zed_extension_api` version in `Cargo.toml` **[Inferred]**. `tar` is a build-machine dependency. The CLI's User-Agent ("Zed Extension CLI/...") needs renaming if the tool is forked.

The HTML extension asks the host to install `@zed-industries/vscode-langservers-extracted` from npm (the extension itself makes no network calls; the host does). Node itself comes from `nodejs.org`.

### 9.5 `extensions/` folder in the Zed source [Verified]
Contents: `CSharp`, `db-mcp`, `gh-mcp`, `glsl`, `html`, `powershell`, `proto`, `razor`, `spfx-localization`, `sqls`, `test-extension`, `workflows`. Nothing in the build references them, so **none is bundled into the binary**; they are source only. Some entries (`spfx-localization` almost certainly, perhaps `CSharp`, `powershell`, `db-mcp`, `gh-mcp`, `razor`, `sqls`) look like the user's own and are not upstream. **[Open]** the upstream-vs-own check (`git ls-tree` against `upstream/main`) has not been run. Own extensions make the user their maintainer and licensor.

### 9.6 Options noted for when this resumes
- **Who maintains what:** (1) own extensions only, (2) curated third-party, built from source at a pinned `rev`, (3) open submission, (4) no registry, dev extensions only. Recommendation was 1 plus 4 for v1.
- **Source layout:** one monorepo (e.g. `mentenaz/forge-extensions`) with CI that builds each extension with `extension_cli`, uploads `archive.tar.gz` as a GitHub Release asset, and generates `catalog.json`.
- **Registry hosting:** a small function that serves `catalog.json` and redirects downloads to release assets. Static files break on `/updates`. Azure Function in TypeScript was the leaning; Node on cPanel is the cheaper alternative if the host supports it; PHP was dismissed. **[Open]**
- **Third-party extensions:** each has its own licence. Never scrape Zed's registry (Terms 2.4). Build from source under the licence's terms, one at a time, after reading it.
- **Shipping languages in the box:** either via the registry, or bundle prebuilt archives in the installer and unpack them into `extensions/installed/<id>` on first run **[Inferred]**.
- **Still unknown:** which languages are built into the binary. `crates\languages\src` returned no subdirectories, so the earlier assumption that Rust, Python, TypeScript and so on are built in is unconfirmed. Until that is checked, v1 may have no language support beyond what is compiled in.
- **Questions to answer on resume:** which languages v1 must handle on day one, whether others will publish extensions, hash verification in v1 or later, which release channel the app reports.

---

## 10. Open items summary

1. ~~Which Zed commit Mentenaz.GPUI is based on~~ (`d86aa6e6`, see 12.2); whether its extra layer changed APIs Zed's crates use; what the theme helper does and whether it is still needed.
2. Watcher output: Issue or PR versus a file; report only versus also opening a draft branch.
3. Passivity of the type crates; location of `ProjectId`, `ChannelId`, `ProxySettings`; size of the `telemetry` and `agent_settings` stubs.
4. ~~Which languages are built into the binary~~ (see 12.3).
5. Microsoft Store and GPL compatibility (deferred, decision 15); apt hosting route; legal review of trademark and licence handling.
6. Clean upstream churn figure; the `pub use`/import audit for crates beyond the ones measured.
7. All extension questions: deferred, see section 9.

## 12. Scope and findings (2026-10-26)

### 12.1 What is being rebuilt [Verified]
`E:\zed_Source` = upstream Zed forked at `d86aa6e6` (2026-10-22) plus 218 own commits. Own work to rebuild:

| Area | Zed-edition crates | Lines |
|---|---|---|
| Helm (GitHub) | `helm_backend`, `helm_panel`, `helm_ui`, `helm_workspace`, `git_credential` | ~23.7k |
| Workflows | `workflow_engine`, `designer_panel`, `flows_panel` (+ `gpui_flow` in Mentenaz.GPUI) | ~21.3k |
| Node | `node_backend`, `node_versions`, `npm_backend`, `npm_bootstrap`, `node_panel`, `npm_manager_panel` | ~12.1k |
| Rust / Cargo | `cargo_backend`, `cargo_manager_panel`, `rust_panel` | ~11.1k |
| Database | `database_backend`, `database_panel` | ~10.0k |
| Python | `python_backend`, `python_versions`, `python_panel`, `python_manager_panel` | ~7.1k |
| System / Ops | `cockpit_panel`, `processes_panel`, `script_runner_panel`, `dashboard_panel`, `runtime_processes`, `ide_env` | ~6.9k |
| .NET | `dotnet_backend`, `dotnet_panel`, `nuget_manager_panel` | ~6.1k |

Plus patches to Zed crates, to redo as overrides: git account handling (`git_ui`, `git`, `project`, `zed_credentials_provider`, `settings_content`), the debugger (`debugger_ui`, `dap_adapters`), panel dock order (`workspace`), the npm hook in `editor`, and branding (`zed`, `util`, `paths`, `release_channel`, `cli`, `install_cli`). Own extensions (deferred): CSharp, powershell, razor, sqls, db-mcp, gh-mcp, spfx-localization.

The panels are gpui-component views that implement Zed's `Panel` (10 crates) or `Item` (9 crates) on the same struct, and use `workspace`, `project`, `settings`, `fs`, `db`, `editor`, `language`. They also use each other (`script_runner_panel` by Node/Python/.NET/Rust, `node_panel` → `flows_panel` → `designer_panel`); per decision 13 that shared code moves to `crates/shared/`.

### 12.2 gpui base [Verified]
gpui diff against Mentenaz.GPUI: upstream `d86aa6e6` = 26 files (+801/−243); `zed_Source` HEAD = 22 files (+228/−188, mostly `taffy.rs`, `window.rs`). Mentenaz.GPUI is the Zed edition's gpui plus a small layer. **[Open]** whether that layer changed APIs `editor`/`workspace` use; Phase 1 shows it.

### 12.3 Built-in languages [Verified]
`crates/languages` compiles in bash, C, C++, CSS, Go, JSON, Python, Rust, TypeScript/TSX, YAML, Tailwind, ESLint. Resolves open item 4: v1 has language support without extensions.

### 12.4 Embedding risk [Open]
With the own shell as the window (decision 12), Zed's `Editor`, terminal and the editor-based panels run as content inside it instead of inside Zed's `Workspace`, which they assume (opening files, go-to-definition, panel registration). Phase 5 starts with a spike that embeds Zed's editor in the shell to measure how much of `Workspace` has to come along. Because most panels are ported before that (12.6), this risk no longer blocks the product.

### 12.5 Zed coupling of the panels [Verified]
Measured by counting references to Zed crates per panel:

| Panel | Lines | `workspace` | `project` | `settings` | `db` | `editor` |
|---|---|---|---|---|---|---|
| Cockpit, Processes, Script Runner, Dashboard | 1.1–1.5k each | ~10 | 0–7 | 0–6 | 0 | 0 |
| Node, Python, .NET, Rust | 1.5–4.5k | ~12 | 13–30 | 6 | 0–6 | 0 |
| npm / NuGet / Python / Cargo managers | 2.5–4k | ~12 | 13–29 | 0 | 0 | 0 |
| Flows | 1.3k | 10 | 7 | 9 | 16 | 0 |
| Helm panel | 10.6k | 22 | 12 | 35 | 4 | 0 |
| Designer, Database, Helm workspace | 4–12k | 19–45 | 0–46 | 0–4 | 4–12 | 7–11 |

- The `workspace` references are mostly the `impl Panel`/`impl Item` block plus a workspace handle for opening tabs and docks; `project` is mostly the workspace root and file access; `settings`/`db` are panel settings and saved state.
- The backends (`node_backend`, `npm_backend`, `python_backend`, `dotnet_backend`, `cargo_backend`, `helm_backend`, `workflow_engine`, `node_versions`, `python_versions`) have no gpui or Zed dependencies; `database_backend` and `runtime_processes` depend only on gpui.
- Only Designer, the Database SQL workbench and the Helm repository view use Zed's `editor`.

**Consequence:** panels are **ported, not rebuilt or mocked**. Their gpui-component render code is kept; only the thin Zed layer is swapped for a small host API (12.6, phase 1). No dummy-data step for panel contents.

### 12.6 Phases
0. **Shell look:** settle the gpui-component frame (title bar, left panels, tab bar, split view, bottom docks, status bar). Placeholders are enough for the contents.
1. **Host API in `crates/shared/`:** the few things panels currently take from Zed (workspace root, open a tab, toggle a dock, saved state and settings, notifications), implemented by the shell.
2. **Foundation:** pin gpui `rev`, `branding` crate, `LICENSE`/`NOTICE`, `cargo deny`.
3. **Shared code:** script runner, `runtime_processes`, `ide_env`, `git_credential`.
4. **Port the gpui-component panels** onto the host API, backends into `<name>-core`, panels into `<name>-ui`, in this order:
   1. Cockpit, Processes, Script Runner
   2. Node, Python, .NET, Rust, with their package managers
   3. Dashboard, Flows, Helm panel
5. **Zed core:** vendor at `d86aa6e6`, embedding spike first (12.4), then the `client`/`telemetry`/`agent_settings` stubs and the HTTP base URL from `branding`. Embed Zed's editor and terminal in the shell. Done when the shell opens a folder and edits with language servers, with no Zed traffic.
6. **Port the editor-based panels:** Designer, Database (SQL workbench), Helm repository view.
7. **Zed patches as overrides** (12.1): git account handling, debugger, editor hooks.
8. **Distribution:** winget + GitHub Releases.
9. **Upstream watcher.**

Most features are usable before Zed is vendored, so the product can be tested early and the riskiest work (phase 5) starts once the shell and host API are proven.

## 11. Suggested next steps (when work resumes)

Superseded by the phases in 12.6.

1. Fix the root `Cargo.toml` and create the empty crate skeleton; confirm `cargo check` passes.
2. Resolve open item 1 (Mentenaz.GPUI base commit), because it decides whether lockstep vendoring is viable.
3. Vendor a first slice of Zed (start with `gpui`-free leaf crates), swap the root `gpui` entries, and build upward.
4. Write the `client`, `telemetry`, `agent_settings` stubs and the `title_bar` rewrite in `overrides/`.
5. Stand up the watcher with `upstream-watch.toml` and `upstream-sync.json`.
6. Add `LICENSE`, `NOTICE`, `cargo deny` and `cargo about` to CI.
