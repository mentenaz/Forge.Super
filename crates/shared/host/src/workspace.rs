//! The workspace model, one per window, observable by panels:
//! `cx.observe(&Workspace::model(cx), …)` re-renders a panel when the
//! workspace changes. Phase 1 has the root folder; files and git are added in
//! phase 1.5 (design doc §13.5) without changing this shape.

use std::path::{Path, PathBuf};

use gpui::{App, AppContext, Context, Entity, Global};

pub struct Workspace {
    root: Option<PathBuf>,
}

struct GlobalWorkspace(Entity<Workspace>);

impl Global for GlobalWorkspace {}

pub(crate) fn init(cx: &mut App) {
    let model = cx.new(|_| Workspace { root: None });
    cx.set_global(GlobalWorkspace(model));
}

impl Workspace {
    /// The workspace model, to read or observe.
    pub fn model(cx: &App) -> Entity<Workspace> {
        cx.global::<GlobalWorkspace>().0.clone()
    }

    /// The workspace's root folder, if one is open.
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Set by the shell when a folder is opened; notifies observers.
    pub fn set_root(&mut self, root: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.root != root {
            self.root = root;
            cx.notify();
        }
    }
}
