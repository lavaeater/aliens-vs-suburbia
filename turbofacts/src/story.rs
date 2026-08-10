use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

use super::consequence::Consequence;
use super::criterion::Criterion;
use super::fact_value::FactValue;
use super::facts_resource::Facts;

/// A named conjunction of [`Criterion`]s — passes when all criteria pass (AND).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rule {
    #[serde(default = "default_rule_name")]
    pub name: String,
    pub criteria: Vec<Criterion>,
}

fn default_rule_name() -> String {
    "Rule".to_string()
}

impl Rule {
    pub fn passes(&self, facts: &Facts) -> bool {
        self.criteria.iter().all(|c| c.evaluate(facts))
    }
}

/// A data-driven story: fires its consequences once all its rules pass. Mirrors Kotlin's
/// `TurboStory`, including the `repeat`/`exclusive` flags, a silent initializer that seeds
/// facts, and the fire-once latch (`finished`/`needs_init`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Story {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub repeat: bool,
    #[serde(default)]
    pub exclusive: bool,
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub consequences: Vec<Consequence>,
    /// Facts seeded silently when the story is (re)initialized. The analog of the Kotlin
    /// `initializer` block's `silent { setFact... }` calls.
    #[serde(default)]
    pub init_facts: Vec<(String, FactValue)>,

    #[serde(skip, default = "default_true")]
    needs_init: bool,
    #[serde(skip)]
    finished: bool,
}

fn default_true() -> bool {
    true
}

impl Story {
    /// Builds a story with default flags (repeat=true, exclusive=false, no init facts).
    /// The [`crate::facts::builder`] DSL is the ergonomic front end for this.
    pub fn new(name: impl Into<String>, rules: Vec<Rule>, consequences: Vec<Consequence>) -> Self {
        Story {
            name: name.into(),
            description: String::new(),
            repeat: true,
            exclusive: false,
            rules,
            consequences,
            init_facts: Vec::new(),
            needs_init: true,
            finished: false,
        }
    }

    /// Total criteria count — the specificity score used to order stories most-specific-first.
    pub fn specificity(&self) -> usize {
        self.rules.iter().map(|r| r.criteria.len()).sum()
    }

    pub fn passes(&self, facts: &Facts) -> bool {
        self.rules.iter().all(|r| r.passes(facts))
    }

    /// Seeds `init_facts` (silently) if this story needs initialization, and clears the
    /// finished latch for repeatable stories.
    pub fn initialize(&mut self, facts: &mut Facts) {
        if !self.needs_init {
            return;
        }
        self.needs_init = false;
        if self.repeat && self.finished {
            self.finished = false;
        }
        let seeds = self.init_facts.clone();
        facts.silent(|f| {
            for (key, value) in seeds {
                f.apply_value(&key, value);
            }
        });
    }

    /// Checks the rules and, if they pass, applies consequences. Returns whether it fired.
    /// A fired story latches `finished` (and re-arms `needs_init` so a repeatable story is
    /// re-initialized before it can fire again).
    pub fn check_and_apply(&mut self, facts: &mut Facts, effects: &mut Vec<String>) -> bool {
        if self.finished {
            return false;
        }
        if !self.passes(facts) {
            return false;
        }
        self.finished = true;
        self.needs_init = true;
        for consequence in &self.consequences {
            consequence.apply(facts, effects);
        }
        true
    }
}

/// Holds all stories (kept sorted most-specific-first), plus the active flag and the
/// dirty `needs_checking` gate. The Rust analog of `TurboStoryManager`.
#[derive(Resource, Default)]
pub struct StoryStore {
    pub stories: Vec<Story>,
    pub active: bool,
    pub needs_checking: bool,
}

impl StoryStore {
    pub fn add(&mut self, story: Story) {
        self.stories.push(story);
        self.stories
            .sort_by_key(|a| std::cmp::Reverse(a.specificity()));
    }

    pub fn add_all(&mut self, stories: impl IntoIterator<Item = Story>) {
        for story in stories {
            self.add(story);
        }
    }

    /// Activates the store and initializes every story (seeding their `init_facts`). Also
    /// arms a check so stories evaluate on the next `check_stories` run.
    pub fn activate(&mut self, facts: &mut Facts) {
        self.active = true;
        self.needs_checking = true;
        for story in &mut self.stories {
            story.initialize(facts);
        }
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }
}
