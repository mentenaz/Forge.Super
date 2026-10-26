# Forge Super: Struktuur

## 1. Drie lae

| Laag | Repo | Rol |
|------|------|-----|
| Biblioteek | `mentenaz/gpui` | Herbruikbare gpui-logika vir alle toekomstige gpui-projekte |
| Produk | `mentenaz/forge-super` | Die IDE: eie naam, paaie, update-bronne en panele |
| Upstream-watcher | (outomatisering) | Trek Zed upstream, lys nuwe implementasies wat geporteer kan word |

Doel: geen hard fork van Zed meer nie, want dit veroorsaak ure se merge-konflikte.

## 2. Repo-uitleg

```
forge-super/
  Cargo.toml              (workspace + gedeelde dependencies)
  Cargo.lock              (WORD gecommit, dis 'n binary)
  .cargo/config.toml      (GITIGNORED, plaaslike [patch] vir gpui)
  crates/
    app/                  (binary, wiring, registreer panele)
    branding/             (app-naam, config-paaie, update-bronne)
    shared/               (klein gedeelde crates, bv. process)
    features/
      node/
        core/             (crate: node-core, GPUI-VRY)
          src/
            lib.rs
            logic/        (suiwer domein: tipes, state, reëls, opdragte)
            backend/      (IO: prosesse, lêers, netwerk, CLI-wrappers)
        ui/               (crate: node-ui, gebruik gpui)
          src/
            panel.rs      (sybalk/dock-aansig)
            workspace.rs  (hoofarea-aansig)
      dotnet/
        core/
        ui/
      helm/
        core/
        ui/
```

## 3. Reëls

- **Een package (eie `Cargo.toml`) per `core` en per `ui`.** Unieke crate-name: `node-core`, `node-ui`, ens.
- **`ui` hang van `core` af, nooit andersom nie.** So bly `core` herbruikbaar (Tauri, CLI, toetse).
- **Feature-crates hang nie van mekaar af nie.** Gedeelde goed gaan na `crates/shared/`.
- **Groot lêers (2k tot 7k reëls) los jy op met modules, nie crates nie.** Mik vir ~200 tot 500 reëls per lêer; `mod.rs` doen net `pub use`.
- **Crate-name is lowercase kebab-case.**
- Later kan `backend` na 'n eie crate/repo uitgetrek word (soos `forge.dotnet.toolchain`) sonder om imports te breek, want die modulepaaie bly dieselfde.

## 4. Workspace `Cargo.toml`

```toml
[workspace]
members = [
  "crates/app",
  "crates/branding",
  "crates/shared/*",
  "crates/features/*/core",
  "crates/features/*/ui",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"

[workspace.dependencies]
# Elke gpui-crate afsonderlik lys (wildcards soos gpui-* werk nie)
gpui = { git = "https://github.com/mentenaz/gpui", rev = "<sha>" }
# gpui-xxx = { git = "https://github.com/mentenaz/gpui", rev = "<sha>" }
serde = { version = "1", features = ["derive"] }
```

Elke feature se `Cargo.toml` erf dit:

```toml
# crates/features/node/ui/Cargo.toml
[package]
name = "node-ui"
version.workspace = true
edition.workspace = true

[dependencies]
node-core = { path = "../core" }
gpui.workspace = true
```

## 5. gpui-weergawes

- **Pin met `rev`** (volle sha). Almal bou op presies daardie commit tot jy self bump.
- **Commit `Cargo.lock`.**
- Later, as dit stabiel is: `tag = "v0.1.0"`.
- Bumps is klein, eie commits: `bump gpui to abc1234`.

### Plaaslike toetsing met `[patch]` (gitignored)

```toml
# .cargo/config.toml
[patch."https://github.com/mentenaz/gpui"]
gpui = { path = "H:/mentenaz/gpui/crates/gpui" }
```

- Die URL moet **presies** ooreenstem met die dependency-URL.
- Patch **al** die gpui-crates wat jy gebruik, anders kry jy dubbele tipes.
- Werk dit nie, verwyder die lêer en jy is terug by die gepinde rev.
- Werk dit: push gpui, bump die `rev`, verwyder die patch.
- Hang `Cargo.lock` vas: `cargo update -p gpui`.

## 6. Upstream-watcher

- Loop op 'n skedule, diff Zed upstream sedert die laaste gesinkroniseerde commit.
- Groepeer commits per crate/area.
- Uitvoer as changelog met buckets: **relevant / miskien / ignoreer** (bv. ignoreer collab, cloud, telemetry; flag editor, workspace, terminal, language).

## 7. Oop besluite

1. **Hoe konsumeer forge-super Zed-kode?** Vendored snapshot, git dep op 'n rev, of 'n patch-laag per crate. Hang af van hoeveel editor-internals gewysig moet word teenoor net panele byvoeg en rebrand.
2. **Bevat `mentenaz/gpui` net gpui, of ook die Zed-crates wat jy nodig het?**
3. **Deel `panel` en `workspace` state?** Bepaal of state in `core` of in `ui` leef.
4. **Watcher:** net rapporteer, of ook 'n draft branch/PR met kandidaat-patches oopmaak?
5. **Pakket-model:** `core` + `ui` per feature is die aanbeveling (opsie B), nog nie finaal bevestig nie.
