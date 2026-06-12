//! Turbofacts: a data-driven game-logic engine. Systems write typed "facts" about the
//! world into the [`Facts`] resource; declarative "stories" watch those facts via rule
//! criteria and fire consequences when their rules pass. See `docs/turbofacts.md`.

pub mod builder;
pub mod consequence;
pub mod criterion;
pub mod fact_value;
pub mod facts_plugin;
pub mod facts_resource;
pub mod keys;
pub mod messages;
pub mod persistence;
pub mod stories;
pub mod story;
pub mod systems;

pub use consequence::*;
pub use criterion::*;
pub use fact_value::*;
pub use facts_plugin::*;
pub use facts_resource::*;
pub use messages::*;
pub use story::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fact_key_joins_with_dots() {
        assert_eq!(fact_key(&["a", "b", "c"]), "a.b.c");
        assert_eq!(fact_key(&["solo"]), "solo");
    }

    #[test]
    fn missing_facts_read_as_defaults() {
        let facts = Facts::default();
        assert_eq!(facts.bool("nope"), false);
        assert_eq!(facts.int("nope"), 0);
        assert_eq!(facts.float("nope"), 0.0);
        assert_eq!(facts.text("nope"), "");
        assert_eq!(facts.try_bool("nope"), None);
    }

    #[test]
    fn set_and_get_scalars() {
        let mut facts = Facts::default();
        facts.set_bool("b", true);
        facts.set_int("i", 42);
        facts.set_float("f", 1.5);
        facts.set_text("s", "hello");

        assert_eq!(facts.bool("b"), true);
        assert_eq!(facts.int("i"), 42);
        assert_eq!(facts.float("f"), 1.5);
        assert_eq!(facts.text("s"), "hello");
    }

    #[test]
    fn add_to_int_accumulates() {
        let mut facts = Facts::default();
        assert_eq!(facts.add_to_int("score", 5), 5);
        assert_eq!(facts.add_to_int("score", 3), 8);
        assert_eq!(facts.int("score"), 8);
    }

    #[test]
    fn or_default_inserts_once() {
        let mut facts = Facts::default();
        assert_eq!(facts.int_or_default("wave", 1), 1);
        facts.set_int("wave", 5);
        // default no longer applies once the fact exists
        assert_eq!(facts.int_or_default("wave", 1), 5);
    }

    #[test]
    fn mutations_record_dirty_keys() {
        let mut facts = Facts::default();
        facts.set_bool("a", true);
        facts.set_int("b", 1);
        let dirty = facts.drain_dirty();
        assert_eq!(dirty, vec!["a".to_string(), "b".to_string()]);
        // draining clears the list
        assert!(facts.drain_dirty().is_empty());
    }

    #[test]
    fn silent_block_suppresses_dirty() {
        let mut facts = Facts::default();
        facts.silent(|f| {
            f.set_bool("a", true);
            f.set_int("b", 2);
        });
        assert!(facts.drain_dirty().is_empty());
        // and reads still work
        assert_eq!(facts.bool("a"), true);
        assert_eq!(facts.int("b"), 2);
    }

    #[test]
    fn text_list_add_and_remove() {
        let mut facts = Facts::default();
        facts.add_to_text_list("zones", "market");
        facts.add_to_text_list("zones", "docks");
        assert_eq!(facts.text_list("zones"), &["market", "docks"]);
        facts.remove_from_text_list("zones", "market");
        assert_eq!(facts.text_list("zones"), &["docks"]);
    }

    #[test]
    fn text_set_dedupes() {
        let mut facts = Facts::default();
        facts.add_to_text_set("npcs", "bob");
        facts.add_to_text_set("npcs", "bob");
        facts.add_to_text_set("npcs", "alice");
        assert_eq!(facts.text_set_len("npcs"), 2);
        assert!(facts.text_set_contains("npcs", "bob"));
        facts.remove_from_text_set("npcs", "bob");
        assert!(!facts.text_set_contains("npcs", "bob"));
    }

    #[test]
    fn query_substring_match() {
        let mut facts = Facts::default();
        facts.set_bool("enemy.boss.dead", true);
        facts.set_bool("enemy.grunt.dead", false);
        facts.set_bool("player.alive", true);

        let mut keys: Vec<_> = facts.query("enemy").map(|(k, _)| k.clone()).collect();
        keys.sort();
        assert_eq!(keys, vec!["enemy.boss.dead", "enemy.grunt.dead"]);
    }

    fn test_app() -> bevy::app::App {
        let mut app = bevy::app::App::new();
        app.add_plugins(FactsPlugin);
        app
    }

    fn drain_changes(app: &mut bevy::app::App) -> Vec<FactChanged> {
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<FactChanged>>()
            .drain()
            .collect()
    }

    #[test]
    fn emit_fact_changes_drains_dirty_into_messages() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Facts>().set_int("score", 7);
        app.update();

        let changes = drain_changes(&mut app);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].key, "score");
        assert_eq!(changes[0].value, FactValue::Int(7));

        // a second frame with no mutation emits nothing
        app.update();
        assert!(drain_changes(&mut app).is_empty());
    }

    #[test]
    fn silent_writes_do_not_emit_changes() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<Facts>()
            .silent(|f| f.set_bool("seeded", true));
        app.update();
        assert!(drain_changes(&mut app).is_empty());
        assert_eq!(app.world().resource::<Facts>().bool("seeded"), true);
    }

    #[test]
    fn set_fact_message_applies_and_emits() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<SetFact>>()
            .write(SetFact::AddInt("coins".to_string(), 10));
        app.update();

        assert_eq!(app.world().resource::<Facts>().int("coins"), 10);
        let changes = drain_changes(&mut app);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].key, "coins");
        assert_eq!(changes[0].value, FactValue::Int(10));
    }

    #[test]
    fn criterion_bool_and_int() {
        let mut facts = Facts::default();
        facts.set_bool("flag", true);
        facts.set_int("kills", 10);

        assert!(Criterion::BoolIs { key: "flag".into(), expected: true }.evaluate(&facts));
        assert!(!Criterion::BoolIs { key: "flag".into(), expected: false }.evaluate(&facts));
        // missing bool reads as false
        assert!(Criterion::BoolIs { key: "missing".into(), expected: false }.evaluate(&facts));

        assert!(Criterion::IntCmp { key: "kills".into(), op: NumOp::Gt, value: 5 }.evaluate(&facts));
        assert!(Criterion::IntCmp { key: "kills".into(), op: NumOp::Lt, value: 20 }.evaluate(&facts));
        assert!(Criterion::IntCmp { key: "kills".into(), op: NumOp::Eq, value: 10 }.evaluate(&facts));
        assert!(!Criterion::IntCmp { key: "kills".into(), op: NumOp::Gt, value: 10 }.evaluate(&facts));
    }

    #[test]
    fn criterion_int_vs_int() {
        let mut facts = Facts::default();
        facts.set_int("kills", 12);
        facts.set_int("target", 5);
        assert!(Criterion::IntVsInt { lhs: "kills".into(), op: NumOp::Gt, rhs: "target".into() }.evaluate(&facts));
        assert!(!Criterion::IntVsInt { lhs: "target".into(), op: NumOp::Gt, rhs: "kills".into() }.evaluate(&facts));
    }

    #[test]
    fn criterion_text_and_collections() {
        let mut facts = Facts::default();
        facts.set_text("context", "town");
        facts.add_to_text_list("zones", "market");
        facts.add_to_text_list("zones", "docks");
        facts.add_to_text_set("npcs", "bob");

        assert!(Criterion::TextEq { key: "context".into(), value: "town".into() }.evaluate(&facts));
        assert!(Criterion::TextContains { key: "context".into(), value: "ow".into() }.evaluate(&facts));
        assert!(Criterion::ListContains { key: "zones".into(), value: "market".into() }.evaluate(&facts));
        assert!(Criterion::ListSize { key: "zones".into(), op: NumOp::Eq, value: 2 }.evaluate(&facts));
        assert!(Criterion::SetContains { key: "npcs".into(), value: "bob".into() }.evaluate(&facts));
        assert!(Criterion::SetSize { key: "npcs".into(), op: NumOp::Eq, value: 1 }.evaluate(&facts));
    }

    #[test]
    fn criterion_any_all_over_query() {
        let mut facts = Facts::default();
        facts.set_bool("enemy.a.dead", true);
        facts.set_bool("enemy.b.dead", false);

        assert!(Criterion::AnyBool { pattern: "enemy.*.dead".into(), expected: true }.evaluate(&facts));
        assert!(!Criterion::AllBool { pattern: "enemy.*.dead".into(), expected: true }.evaluate(&facts));

        facts.set_bool("enemy.b.dead", true);
        assert!(Criterion::AllBool { pattern: "enemy.*.dead".into(), expected: true }.evaluate(&facts));
    }

    #[test]
    fn all_over_empty_query_is_false() {
        let facts = Facts::default();
        // "all matching are true" must require at least one match
        assert!(!Criterion::AllBool { pattern: "nothing.*.here".into(), expected: true }.evaluate(&facts));
    }

    #[test]
    fn criterion_round_trips_through_ron() {
        let c = Criterion::IntCmp { key: "kills".into(), op: NumOp::Gt, value: 5 };
        let s = ron::to_string(&c).unwrap();
        let back: Criterion = ron::from_str(&s).unwrap();
        assert_eq!(c, back);
    }

    fn rule(criteria: Vec<Criterion>) -> Rule {
        Rule { name: "r".into(), criteria }
    }

    #[test]
    fn story_fires_once_and_applies_consequence() {
        let mut facts = Facts::default();
        facts.set_bool("ready", true);
        let mut story = Story::new(
            "win",
            vec![rule(vec![Criterion::BoolIs { key: "ready".into(), expected: true }])],
            vec![Consequence::SetFact { key: "done".into(), value: FactValue::Bool(true) }],
        );

        let mut effects = Vec::new();
        assert!(story.check_and_apply(&mut facts, &mut effects));
        assert_eq!(facts.bool("done"), true);
        // latched: does not fire again even though rules still pass
        assert!(!story.check_and_apply(&mut facts, &mut effects));
    }

    #[test]
    fn story_does_not_fire_until_rules_pass() {
        let mut facts = Facts::default();
        let mut story = Story::new(
            "win",
            vec![rule(vec![Criterion::IntCmp { key: "kills".into(), op: NumOp::Gt, value: 3 }])],
            vec![Consequence::Emit { effect: "victory".into() }],
        );
        let mut effects = Vec::new();
        assert!(!story.check_and_apply(&mut facts, &mut effects));
        facts.set_int("kills", 5);
        assert!(story.check_and_apply(&mut facts, &mut effects));
        assert_eq!(effects, vec!["victory".to_string()]);
    }

    #[test]
    fn store_sorts_by_specificity_desc() {
        let mut store = StoryStore::default();
        store.add(Story::new("one", vec![rule(vec![Criterion::BoolIs { key: "a".into(), expected: true }])], vec![]));
        store.add(Story::new(
            "three",
            vec![rule(vec![
                Criterion::BoolIs { key: "a".into(), expected: true },
                Criterion::BoolIs { key: "b".into(), expected: true },
                Criterion::BoolIs { key: "c".into(), expected: true },
            ])],
            vec![],
        ));
        let names: Vec<_> = store.stories.iter().map(|s| s.name.clone()).collect();
        assert_eq!(names, vec!["three", "one"]);
    }

    #[test]
    fn activate_seeds_init_facts_silently() {
        let mut facts = Facts::default();
        let mut store = StoryStore::default();
        let mut story = Story::new("s", vec![rule(vec![Criterion::BoolIs { key: "x".into(), expected: true }])], vec![]);
        story.init_facts = vec![("seeded".into(), FactValue::Int(7))];
        store.add(story);

        store.activate(&mut facts);
        assert_eq!(facts.int("seeded"), 7);
        // seeding was silent — no dirty keys
        assert!(facts.drain_dirty().is_empty());
    }

    #[test]
    fn check_stories_system_fires_and_cascades() {
        let mut app = test_app();
        {
            let mut store = app.world_mut().resource_mut::<StoryStore>();
            store.add(Story::new(
                "complete",
                vec![rule(vec![Criterion::BoolIs { key: keys::BOSS_IS_DEAD.into(), expected: true }])],
                vec![
                    Consequence::SetFact { key: keys::LEVEL_COMPLETE.into(), value: FactValue::Bool(true) },
                    Consequence::Emit { effect: "level_complete".into() },
                ],
            ));
            let mut facts = Facts::default();
            store.activate(&mut facts);
        }
        // trigger: boss dies
        app.world_mut().resource_mut::<Facts>().set_bool(keys::BOSS_IS_DEAD, true);

        // frame 1: emit FactChanged(boss) + arm. frame 2: check_stories fires.
        app.update();
        app.update();

        assert_eq!(app.world().resource::<Facts>().bool(keys::LEVEL_COMPLETE), true);
        let effects: Vec<_> = app
            .world_mut()
            .resource_mut::<bevy::ecs::message::Messages<StoryEffect>>()
            .drain()
            .map(|e| e.effect)
            .collect();
        assert!(effects.contains(&"level_complete".to_string()));
    }

    #[test]
    fn inactive_store_does_not_fire() {
        let mut app = test_app();
        {
            let mut store = app.world_mut().resource_mut::<StoryStore>();
            store.add(Story::new(
                "complete",
                vec![rule(vec![Criterion::BoolIs { key: "ready".into(), expected: true }])],
                vec![Consequence::SetFact { key: "done".into(), value: FactValue::Bool(true) }],
            ));
            // deliberately not activated
        }
        app.world_mut().resource_mut::<Facts>().set_bool("ready", true);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Facts>().bool("done"), false);
    }

    #[test]
    fn builder_constructs_expected_story() {
        let s = builder::story("Level Complete")
            .exclusive(true)
            .repeat(false)
            .rule("win", |r| {
                r.is_true(keys::LEVEL_STARTED).is_true(keys::LEVEL_COMPLETE);
            })
            .set_true(keys::GOTO_NEXT_LEVEL)
            .emit("level_complete")
            .build();

        assert_eq!(s.name, "Level Complete");
        assert!(s.exclusive);
        assert!(!s.repeat);
        assert_eq!(s.specificity(), 2);
        assert_eq!(s.consequences.len(), 2);
    }

    #[test]
    fn ported_level_complete_story_fires() {
        let mut facts = Facts::default();
        facts.set_bool(keys::LEVEL_STARTED, true);
        facts.set_bool(keys::LEVEL_COMPLETE, true);
        let mut s = stories::level_complete_story();
        let mut effects = Vec::new();
        assert!(s.check_and_apply(&mut facts, &mut effects));
        assert_eq!(facts.bool(keys::GOTO_NEXT_LEVEL), true);
        assert_eq!(effects, vec!["level_complete".to_string()]);
    }

    #[test]
    fn aliens_cleared_story_wins_when_all_dead() {
        let mut facts = Facts::default();
        let mut s = stories::aliens_cleared_story();
        let mut effects = Vec::new();

        // Started but aliens still around -> no win.
        facts.set_bool(keys::LEVEL_STARTED, true);
        facts.set_bool(keys::ALL_ALIENS_DEAD, false);
        assert!(!s.check_and_apply(&mut facts, &mut effects));

        facts.set_bool(keys::ALL_ALIENS_DEAD, true);
        assert!(s.check_and_apply(&mut facts, &mut effects));
        assert_eq!(facts.bool(keys::LEVEL_COMPLETE), true);
    }

    #[test]
    fn level_failed_stories_cover_both_lose_conditions() {
        // All players dead.
        let mut facts = Facts::default();
        facts.set_bool(keys::LEVEL_STARTED, true);
        facts.set_bool(keys::ALL_PLAYERS_DEAD, true);
        let mut effects = Vec::new();
        assert!(stories::level_failed_story().check_and_apply(&mut facts, &mut effects));
        assert_eq!(facts.bool(keys::LEVEL_FAILED), true);
        assert!(effects.contains(&"level_failed".to_string()));

        // Too many aliens escaped.
        let mut facts = Facts::default();
        facts.set_bool(keys::LEVEL_STARTED, true);
        facts.set_bool(keys::TOO_MANY_ALIENS_ESCAPED, true);
        let mut effects = Vec::new();
        assert!(stories::level_failed_escaped_story().check_and_apply(&mut facts, &mut effects));
        assert_eq!(facts.bool(keys::LEVEL_FAILED), true);
    }

    #[test]
    fn base_stories_cascade_start_to_win() {
        // Drives the live base-stories set through a full level-flow: start -> clear -> goto.
        let mut facts = Facts::default();
        let mut store = StoryStore::default();
        store.add_all(stories::base_stories());
        store.activate(&mut facts); // seeds LevelStarted=false etc, arms a check

        // First check: Level Start fires, flipping LevelStarted and emitting level_starting.
        let mut effects = Vec::new();
        for s in &mut store.stories {
            s.check_and_apply(&mut facts, &mut effects);
        }
        assert_eq!(facts.bool(keys::LEVEL_STARTED), true);
        assert!(effects.contains(&"level_starting".to_string()));
        assert_eq!(facts.bool(keys::LEVEL_COMPLETE), false);

        // World derives the win condition; the win + complete stories then cascade.
        facts.set_bool(keys::ALL_ALIENS_DEAD, true);
        let mut effects = Vec::new();
        for s in &mut store.stories {
            s.check_and_apply(&mut facts, &mut effects);
        }
        assert_eq!(facts.bool(keys::LEVEL_COMPLETE), true);
        assert_eq!(facts.bool(keys::GOTO_NEXT_LEVEL), true);
        assert!(effects.contains(&"level_complete".to_string()));
    }

    #[test]
    fn ported_kill_count_story_uses_int_vs_int() {
        let mut facts = Facts::default();
        let mut store = StoryStore::default();
        store.add(stories::enemy_kill_count_story());
        store.activate(&mut facts); // seeds kill=0, target=3
        facts.set_bool(keys::LEVEL_STARTED, true);

        let mut effects = Vec::new();
        // not enough kills yet
        assert!(!store.stories[0].check_and_apply(&mut facts, &mut effects));
        facts.set_int(keys::ENEMY_KILL_COUNT, 5);
        assert!(store.stories[0].check_and_apply(&mut facts, &mut effects));
        assert_eq!(facts.bool(keys::LEVEL_COMPLETE), true);
    }

    #[test]
    fn ported_boss_story_seeds_init_facts() {
        let mut facts = Facts::default();
        let mut store = StoryStore::default();
        store.add(stories::boss_and_objectives_story());
        store.activate(&mut facts);
        assert_eq!(facts.bool(keys::BOSS_IS_DEAD), false);
        assert!(facts.contains(keys::ALL_OBJECTIVES_TOUCHED));
    }

    #[test]
    fn facts_persist_scalars_only_round_trip() {
        let mut facts = Facts::default();
        facts.set_bool("b", true);
        facts.set_int("i", 9);
        facts.set_float("f", 2.5);
        facts.set_text("s", "hi");
        facts.add_to_text_list("list", "skip"); // runtime-only, must not persist

        let ron_str = persistence::facts_to_ron(&facts).unwrap();
        assert!(!ron_str.contains("list"));

        let mut loaded = Facts::default();
        persistence::facts_from_ron(&mut loaded, &ron_str).unwrap();
        assert_eq!(loaded.bool("b"), true);
        assert_eq!(loaded.int("i"), 9);
        assert_eq!(loaded.float("f"), 2.5);
        assert_eq!(loaded.text("s"), "hi");
        assert_eq!(loaded.text_list("list"), &[] as &[String]);
        // loading is silent
        assert!(loaded.drain_dirty().is_empty());
    }

    #[test]
    fn stories_parse_from_handwritten_ron() {
        let ron_str = r#"[
            (
                name: "Test Win",
                exclusive: true,
                rules: [
                    ( name: "enough kills", criteria: [ IntCmp(key: "kills", op: Gt, value: 3) ] ),
                ],
                consequences: [ SetFact(key: "won", value: Bool(true)) ],
            ),
        ]"#;
        let mut stories = persistence::stories_from_ron(ron_str).unwrap();
        assert_eq!(stories.len(), 1);
        assert_eq!(stories[0].name, "Test Win");
        assert!(stories[0].exclusive);

        let mut facts = Facts::default();
        facts.set_int("kills", 4);
        let mut effects = Vec::new();
        assert!(stories[0].check_and_apply(&mut facts, &mut effects));
        assert_eq!(facts.bool("won"), true);
    }

    #[test]
    fn example_asset_parses() {
        // guards the designer-facing template against format drift
        let stories = persistence::load_stories("assets/stories/example.ron").unwrap();
        assert_eq!(stories.len(), 1);
        assert_eq!(stories[0].name, "Reach the Kill Count");
        assert_eq!(stories[0].init_facts.len(), 2);
    }

    #[test]
    fn stories_round_trip_through_ron() {
        let original = vec![stories::enemy_kill_count_story()];
        let ron_str = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default()).unwrap();
        let parsed = persistence::stories_from_ron(&ron_str).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, original[0].name);
        assert_eq!(parsed[0].specificity(), original[0].specificity());
        assert_eq!(parsed[0].init_facts.len(), original[0].init_facts.len());
    }

    #[test]
    fn query_wildcard_prefix_suffix() {
        let mut facts = Facts::default();
        facts.set_bool("enemy.boss.dead", true);
        facts.set_bool("enemy.grunt.dead", true);
        facts.set_bool("enemy.boss.alive", false);

        let mut keys: Vec<_> = facts
            .query("enemy.*.dead")
            .map(|(k, _)| k.clone())
            .collect();
        keys.sort();
        assert_eq!(keys, vec!["enemy.boss.dead", "enemy.grunt.dead"]);
    }
}
