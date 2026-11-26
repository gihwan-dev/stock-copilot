use std::collections::BTreeSet;

use domain::{Nanosecond, SpanSeconds, TimeSpan, Timestamp, UntrustedText};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeadlineKey(String);

impl HeadlineKey {
    #[must_use]
    pub fn of(title: &UntrustedText) -> Self {
        let trimmed = title.as_untrusted_str().trim();
        let core = match trimmed.rsplit_once(" - ") {
            Some((head, tail))
                if !head.trim().is_empty()
                    && !tail.trim().is_empty()
                    && tail.chars().count() <= 24 =>
            {
                head
            }
            Some(_) | None => trimmed,
        };
        Self(
            core.chars()
                .filter(|character| character.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect(),
        )
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn similarity(&self, other: &Self) -> StorySimilarity {
        if self.0.is_empty() || other.0.is_empty() {
            return StorySimilarity::Different;
        }
        if self == other {
            return StorySimilarity::SameStory;
        }
        let left: BTreeSet<_> = self.0.chars().zip(self.0.chars().skip(1)).collect();
        let right: BTreeSet<_> = other.0.chars().zip(other.0.chars().skip(1)).collect();
        let union = left.union(&right).count();
        let shared = left.intersection(&right).count();
        if union > 0 && shared.saturating_mul(100) >= union.saturating_mul(45) {
            StorySimilarity::SameStory
        } else {
            StorySimilarity::Different
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorySimilarity {
    SameStory,
    Different,
}

#[must_use]
pub fn same_story_window() -> Option<TimeSpan> {
    TimeSpan::try_new(SpanSeconds::new(48 * 3_600), Nanosecond::new(0)).ok()
}

fn within_window(first: Timestamp, second: Timestamp) -> StorySimilarity {
    let (earlier, later) = if first <= second {
        (first, second)
    } else {
        (second, first)
    };
    match (earlier.elapsed_until(later), same_story_window()) {
        (Ok(elapsed), Some(window)) if elapsed <= window => StorySimilarity::SameStory,
        (Ok(_) | Err(_), _) => StorySimilarity::Different,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadlineAdmission {
    New,
    Duplicate,
}

#[derive(Debug, Default)]
pub struct RecentHeadlines {
    entries: Vec<(HeadlineKey, Timestamp)>,
}

impl RecentHeadlines {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn admit(&mut self, title: &UntrustedText, at: Timestamp) -> HeadlineAdmission {
        self.entries
            .retain(|(_key, seen_at)| within_window(*seen_at, at) == StorySimilarity::SameStory);
        let key = HeadlineKey::of(title);
        if key.0.is_empty() {
            return HeadlineAdmission::New;
        }
        if self.entries.iter().any(|(seen, _at)| *seen == key) {
            return HeadlineAdmission::Duplicate;
        }
        if self.entries.len() >= 4_000 {
            self.entries = self.entries.iter().skip(1).cloned().collect();
        }
        self.entries.push((key, at));
        HeadlineAdmission::New
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryGroup<T> {
    pub representative: T,
    pub related: Vec<T>,
}

pub fn group_same_story<T, Title, When>(
    newest_first: Vec<T>,
    title: Title,
    at: When,
) -> Vec<StoryGroup<T>>
where
    Title: Fn(&T) -> &UntrustedText,
    When: Fn(&T) -> Timestamp,
{
    let mut groups: Vec<(HeadlineKey, Timestamp, StoryGroup<T>)> = Vec::new();
    for item in newest_first {
        let key = HeadlineKey::of(title(&item));
        let when = at(&item);
        let joined = groups.iter_mut().find(|(representative, rep_at, _group)| {
            within_window(*rep_at, when) == StorySimilarity::SameStory
                && representative.similarity(&key) == StorySimilarity::SameStory
        });
        match joined {
            Some((_key, _at, group)) => group.related.push(item),
            None => groups.push((
                key,
                when,
                StoryGroup {
                    representative: item,
                    related: Vec::new(),
                },
            )),
        }
    }
    groups.into_iter().map(|(_key, _at, group)| group).collect()
}
