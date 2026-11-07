//! UI-independent processing events. Libraries remain silent unless observed.
#[derive(Debug, Clone)]
pub enum Event {
    Stage {
        name: &'static str,
        message: String,
    },
    Info(String),
    Detail(String),
    Success(String),
    Warning(String),
    Skipped(String),
    Count {
        name: &'static str,
        value: usize,
    },
    Progress {
        name: &'static str,
        done: usize,
        total: usize,
    },
}
/// Events do not affect plans or write generated state. Callers may collect,
/// render, or discard them; observer callbacks must not mutate source inputs.
pub type Observer<'a> = dyn FnMut(Event) + 'a;
