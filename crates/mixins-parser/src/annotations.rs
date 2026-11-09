//! Explicit declaration annotation policy. Unrecognized macro calls never get
//! silently stripped: macro expansion could alter the declaration's meaning.
use super::{Annotation, Kind, Parser};
const ANNOTATIONS: &[&str] = &[
    "CAPABILITY",
    "SCOPED_CAPABILITY",
    "GUARDED_BY",
    "PT_GUARDED_BY",
    "ACQUIRED_BEFORE",
    "ACQUIRED_AFTER",
    "REQUIRES",
    "REQUIRES_SHARED",
    "ACQUIRE",
    "ACQUIRE_SHARED",
    "RELEASE",
    "RELEASE_SHARED",
    "RELEASE_GENERIC",
    "TRY_ACQUIRE",
    "TRY_ACQUIRE_SHARED",
    "EXCLUDES",
    "ASSERT_CAPABILITY",
    "ASSERT_SHARED_CAPABILITY",
    "RETURN_CAPABILITY",
    "NO_THREAD_SAFETY_ANALYSIS",
    "AE_NO_TSAN",
    "MOODYCAMEL_MAYBE_ALIGN_TO_CACHELINE",
];
impl Parser<'_> {
    pub(super) fn annotation_end(&self, p: usize, end: usize) -> Option<usize> {
        if p >= end
            || self.token(p).kind != Kind::Identifier
            || !ANNOTATIONS.contains(&self.word(p))
        {
            return None;
        }
        if p + 1 < end && self.word(p + 1) == "(" {
            self.close(p + 1).filter(|e| *e < end)
        } else if matches!(
            self.word(p),
            "SCOPED_CAPABILITY"
                | "NO_THREAD_SAFETY_ANALYSIS"
                | "AE_NO_TSAN"
                | "MOODYCAMEL_MAYBE_ALIGN_TO_CACHELINE"
        ) {
            Some(p)
        } else {
            None
        }
    }
    pub(super) fn record_annotations(&mut self, start: usize, end: usize) {
        let mut p = start;
        while p < end {
            if let Some(close) = self.annotation_end(p, end) {
                let span = self.span(p, close);
                if !self.index.annotations.iter().any(|a| a.span == span) {
                    self.index.annotations.push(Annotation {
                        name: self.word(p).into(),
                        span,
                    });
                }
                p = close + 1;
            } else {
                p += 1;
            }
        }
    }
}
