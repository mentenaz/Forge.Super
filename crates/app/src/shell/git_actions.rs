//! Running a git operation from the shell's UI and reporting the outcome:
//! git's own message as a warning on failure, an optional confirmation on
//! success.

use forge_host::{GitError, Host, NoticeKind};
use gpui::{App, SharedString, Task, Window};

pub(super) fn report_git(
    task: Task<Result<(), GitError>>,
    failure_title: &'static str,
    success: Option<(SharedString, SharedString)>,
    window: &mut Window,
    cx: &mut App,
) {
    window
        .spawn(cx, async move |cx| {
            let result = task.await;
            let _ = cx.update(|window, cx| match result {
                Err(err) => Host::notify(NoticeKind::Warning, failure_title, err.0, window, cx),
                Ok(()) => {
                    if let Some((title, message)) = success {
                        Host::notify(NoticeKind::Success, title, message, window, cx);
                    }
                }
            });
        })
        .detach();
}
