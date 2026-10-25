use crate::core::prefs::{DownloadList, Prefs};
use crate::core::schedule::{
    fmt_hm, list_rules, move_selection, parse_hm, plan_limit_actions, schedule_events,
    until_window_open, window_len, window_open, ListRule,
};
use crate::core::service::normalize_lists;
use crate::downloads::in_view;
use crate::engine::{Category, Snapshot, Status};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

    #[test]
    fn prefs_round_trip_keeps_lists() {
        let json = r#"{"dir":"D:/x","conns":4,"queued":false,"lists":[{"name":"Movies","items":["a","b"],"color":3,"max_parallel":2,"sched_start":480,"sched_end":1320},{"name":"Work","items":[]}]}"#;
        let p: Prefs = serde_json::from_str(json).unwrap();
        assert_eq!(p.lists.len(), 2);
        assert_eq!(p.lists[0].items, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(p.lists[0].color, 3);
        assert_eq!(p.lists[0].max_parallel, 2);
        assert_eq!(p.lists[0].sched_start, Some(480));
        assert_eq!(p.lists[0].sched_end, Some(1320));
        assert!(p.lists[1].items.is_empty());
        assert_eq!(p.lists[1].color, 0, "missing colour defaults to the first palette entry");
        assert_eq!(p.lists[1].max_parallel, 0, "missing cap defaults to unlimited");
        assert_eq!(p.lists[1].sched_start, None, "missing schedule defaults to none");
        assert_eq!(p.lists[1].sched_end, None);
        assert!(!p.lists[0].shutdown_at_end, "missing flag defaults to off");
        assert_eq!(p.font, "Segoe UI", "missing font defaults to Segoe UI");
        let out = serde_json::to_string(&p).unwrap();
        let p2: Prefs = serde_json::from_str(&out).unwrap();
        assert_eq!(p2.lists[0].name, "Movies");
        assert_eq!(p2.lists[0].items.len(), 2);
        assert_eq!(p2.lists[0].color, 3);
        assert_eq!(p2.lists[0].max_parallel, 2);
        assert_eq!(p2.lists[0].sched_start, Some(480));
        assert_eq!(p2.lists[0].sched_end, Some(1320));
    }

    fn snap(id: &str, status: Status, category: Category) -> Snapshot {
        Snapshot {
            id: id.into(),
            url: String::new(),
            name: String::new(),
            dest: PathBuf::new(),
            status,
            total: None,
            received: 0,
            speed: 0,
            error: None,
            added: 0,
            finished: None,
            category,
            connections: 0,
            parts: Vec::new(),
            order: 0,
        }
    }

    #[test]
    fn selected_list_defines_the_whole_view() {
        let ids: HashSet<String> = ["a".to_string()].into_iter().collect();
        let done = snap("a", Status::Done, Category::Compressed);
        let other = snap("b", Status::Downloading, Category::Media);
        // In a list: membership wins, the category filter is ignored.
        assert!(in_view(Some(&ids), Category::Active, &done));
        assert!(!in_view(Some(&ids), Category::All, &other));
        // No list: the category filter applies as before.
        assert!(in_view(None, Category::Done, &done));
        assert!(!in_view(None, Category::Done, &other));
        assert!(in_view(None, Category::All, &other));
        assert!(in_view(None, Category::Active, &other));
    }

    #[test]
    fn prefs_without_lists_still_load() {
        let json = r#"{"dir":"D:/x","conns":8,"queued":true,"retry_parts":true,"retry_after":30,"limit_kbps":512}"#;
        let p: Prefs = serde_json::from_str(json).unwrap();
        assert!(p.lists.is_empty());
        assert_eq!(p.limit_kbps, 512);
    }

    fn task(id: &str, status: Status, order: u64) -> Snapshot {
        let mut s = snap(id, status, Category::Other);
        s.added = order;
        s.order = order;
        s
    }

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn list_cap_pauses_latest_queue_entry_and_resumes_earliest() {
        let snaps = vec![
            task("a", Status::Downloading, 10),
            task("b", Status::Downloading, 20),
            task("c", Status::Downloading, 30),
            task("d", Status::Paused, 40),
            task("e", Status::Paused, 50),
        ];
        let items = ids(&["a", "b", "c", "d", "e"]);
        let queued: HashSet<String> = ["d".into(), "e".into()].into_iter().collect();

        // Over the cap: the download latest in the queue waits.
        let rules = [ListRule { items: &items, limit: 2, scheduled: false }];
        let (pause, resume) = plan_limit_actions(&rules, &snaps, &queued, &HashSet::new(), &HashSet::new());
        assert_eq!(pause, vec!["c".to_string()]);
        assert!(resume.is_empty());

        // A freed slot goes to the earliest waiting download.
        let rules = [ListRule { items: &items, limit: 4, scheduled: false }];
        let (pause, resume) = plan_limit_actions(&rules, &snaps, &queued, &HashSet::new(), &HashSet::new());
        assert!(pause.is_empty());
        assert_eq!(resume, vec!["d".to_string()]);

        // A download the user paused is never auto-resumed.
        let user_paused: HashSet<String> = ["e".into()].into_iter().collect();
        let (_, resume) = plan_limit_actions(&rules, &snaps, &queued, &user_paused, &HashSet::new());
        assert_eq!(resume, vec!["d".to_string()]);
    }

    #[test]
    fn shared_task_is_not_resumed_into_a_full_list() {
        let snaps = vec![
            task("a", Status::Downloading, 10),
            task("b", Status::Downloading, 20),
            task("e", Status::Paused, 30),
        ];
        let one = ids(&["a", "b", "e"]);
        let two = ids(&["e"]);
        let rules = [
            ListRule { items: &one, limit: 2, scheduled: false },
            ListRule { items: &two, limit: 1, scheduled: false },
        ];
        let queued: HashSet<String> = ["e".into()].into_iter().collect();
        let (pause, resume) = plan_limit_actions(&rules, &snaps, &queued, &HashSet::new(), &HashSet::new());
        // List one is at its cap, so e must stay paused even though list two has a slot.
        assert!(pause.is_empty());
        assert!(resume.is_empty());
    }

    #[test]
    fn schedule_window_logic() {
        assert!(window_open(10 * 60, 8 * 60, 22 * 60), "10:00 inside 08:00-22:00");
        assert!(!window_open(7 * 60, 8 * 60, 22 * 60));
        assert!(!window_open(22 * 60, 8 * 60, 22 * 60), "end is exclusive");
        // Overnight window 22:00-06:00.
        assert!(window_open(23 * 60, 22 * 60, 6 * 60));
        assert!(window_open(5 * 60, 22 * 60, 6 * 60));
        assert!(!window_open(12 * 60, 22 * 60, 6 * 60));
        // Equal start and end means the window is always closed.
        assert!(!window_open(10 * 60, 600, 600));
    }

    #[test]
    fn default_list_is_created_and_membership_is_single() {
        let list = |name: &str, items: &[&str], is_default: bool| DownloadList {
            name: name.into(),
            items: ids(items),
            color: 0,
            max_parallel: 0,
            sched_start: None,
            sched_end: None,
            shutdown_at_end: false,
            is_default,
        };

        let mut lists = vec![list("a", &["x"], false), list("b", &["x", "y"], false)];
        assert!(normalize_lists(&mut lists));
        assert_eq!(lists[0].name, "main");
        assert!(lists[0].is_default);
        assert_eq!(lists[1].items, ids(&["x"]));
        assert_eq!(lists[2].items, ids(&["y"]));

        // Renaming the default keeps it default and adds no second list.
        lists[0].name = "downloads".into();
        assert!(!normalize_lists(&mut lists));
        assert_eq!(lists.len(), 3);
        assert!(lists[0].is_default);

        // Older prefs with a list called main adopt it instead of duplicating.
        let mut legacy = vec![list("Main", &[], false), list("other", &[], false)];
        assert!(normalize_lists(&mut legacy));
        assert_eq!(legacy.len(), 2);
        assert!(legacy[0].is_default);
    }

    #[test]
    fn schedule_remaining_and_duration() {
        // Day window 10:00-12:00.
        assert_eq!(until_window_open(8 * 60, 10 * 60, 12 * 60), 2 * 60);
        assert_eq!(until_window_open(11 * 60, 10 * 60, 12 * 60), 0);
        assert_eq!(until_window_open(13 * 60, 10 * 60, 12 * 60), 21 * 60);
        assert_eq!(window_len(10 * 60, 12 * 60), 2 * 60);

        // Overnight window 22:00-06:00.
        assert_eq!(window_len(22 * 60, 6 * 60), 8 * 60);
        assert_eq!(until_window_open(12 * 60, 22 * 60, 6 * 60), 10 * 60);
        assert_eq!(until_window_open(23 * 60, 22 * 60, 6 * 60), 0);
    }

    #[test]
    fn closed_schedule_pauses_everything_in_list() {
        let snaps = vec![
            task("a", Status::Downloading, 10),
            task("b", Status::Downloading, 20),
            task("c", Status::Paused, 30),
        ];
        let items = ids(&["a", "b", "c"]);
        // A closed schedule window acts as a zero cap.
        let rules = [ListRule { items: &items, limit: 0, scheduled: true }];
        let (pause, resume) = plan_limit_actions(
            &rules,
            &snaps,
            &HashSet::new(),
            &HashSet::new(),
            &HashSet::new(),
        );
        assert_eq!(pause.len(), 2);
        assert!(resume.is_empty());
    }

    #[test]
    fn manual_start_overrides_closed_schedule() {
        let snaps = vec![
            task("a", Status::Downloading, 10),
            task("b", Status::Downloading, 20),
            task("c", Status::Downloading, 30),
        ];
        let items = ids(&["a", "b", "c"]);
        let user_started: HashSet<String> = ["b".into()].into_iter().collect();
        let rules = [ListRule { items: &items, limit: 0, scheduled: true }];
        let (pause, resume) = plan_limit_actions(
            &rules,
            &snaps,
            &HashSet::new(),
            &HashSet::new(),
            &user_started,
        );
        assert_eq!(pause.len(), 2);
        assert!(pause.contains(&"a".to_string()));
        assert!(pause.contains(&"c".to_string()));
        assert!(!pause.contains(&"b".to_string()), "manual start must keep running");
        assert!(resume.is_empty());
    }

    #[test]
    fn scheduled_list_starts_its_paused_rows() {
        let snaps = vec![
            task("a", Status::Paused, 10),
            task("b", Status::Paused, 20),
            task("c", Status::Downloading, 30),
        ];
        let items = ids(&["a", "b", "c"]);
        let empty = HashSet::new();

        // Unlimited scheduled list: every paused member starts on its own.
        let rules = [ListRule { items: &items, limit: u32::MAX, scheduled: true }];
        let (_, resume) = plan_limit_actions(&rules, &snaps, &empty, &empty, &empty);
        assert_eq!(resume, vec!["a".to_string(), "b".to_string()]);

        // The cap still limits how many start at once.
        let rules = [ListRule { items: &items, limit: 2, scheduled: true }];
        let (pause, resume) = plan_limit_actions(&rules, &snaps, &empty, &empty, &empty);
        assert!(pause.is_empty());
        assert_eq!(resume, vec!["a".to_string()]);

        // Unscheduled lists only start rows the limiter queued.
        let rules = [ListRule { items: &items, limit: u32::MAX, scheduled: false }];
        let (_, resume) = plan_limit_actions(&rules, &snaps, &empty, &empty, &empty);
        assert!(resume.is_empty());

        // A manual pause wins over the schedule.
        let user: HashSet<String> = ["a".into()].into_iter().collect();
        let rules = [ListRule { items: &items, limit: u32::MAX, scheduled: true }];
        let (_, resume) = plan_limit_actions(&rules, &snaps, &empty, &user, &empty);
        assert_eq!(resume, vec!["b".to_string()]);
    }

    #[test]
    fn scheduled_unlimited_lists_get_rules() {
        let list = |name: &str, max_parallel: u32, sched: Option<(u32, u32)>| DownloadList {
            name: name.into(),
            items: Vec::new(),
            color: 0,
            max_parallel,
            sched_start: sched.map(|s| s.0),
            sched_end: sched.map(|s| s.1),
            shutdown_at_end: false,
            is_default: false,
        };
        let sched = list("night", 0, Some((60, 120)));
        let rules = list_rules(std::slice::from_ref(&sched), 90);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].limit, u32::MAX);
        assert!(rules[0].scheduled);
        let rules = list_rules(std::slice::from_ref(&sched), 30);
        assert_eq!(rules[0].limit, 0, "outside the window everything waits");
        let plain = list("plain", 0, None);
        assert!(list_rules(std::slice::from_ref(&plain), 90).is_empty());
    }

    #[test]
    fn opening_window_releases_paused_rows() {
        let list = DownloadList {
            name: "night".into(),
            items: ids(&["a", "b"]),
            color: 0,
            max_parallel: 0,
            sched_start: Some(60),
            sched_end: Some(120),
            shutdown_at_end: false,
            is_default: false,
        };
        let mut was_open = HashMap::new();
        // Closed: nothing is released.
        let ev = schedule_events(std::slice::from_ref(&list), 30, &mut was_open);
        assert!(ev.opened.is_empty());
        assert!(!ev.shutdown);
        // The opening moment releases every member, even manually paused ones.
        let ev = schedule_events(std::slice::from_ref(&list), 90, &mut was_open);
        assert!(ev.opened.contains("a") && ev.opened.contains("b"));
        // While the window stays open, manual pauses are left alone.
        let ev = schedule_events(std::slice::from_ref(&list), 100, &mut was_open);
        assert!(ev.opened.is_empty());
        // Closing and reopening releases them again.
        let ev = schedule_events(std::slice::from_ref(&list), 130, &mut was_open);
        assert!(ev.opened.is_empty());
        assert!(!ev.shutdown, "shutdown was not requested for this list");
        let ev = schedule_events(std::slice::from_ref(&list), 90, &mut was_open);
        assert!(ev.opened.contains("a") && ev.opened.contains("b"));
    }

    #[test]
    fn closing_a_scheduled_list_can_request_shutdown() {
        let list = DownloadList {
            name: "night".into(),
            items: ids(&["a"]),
            color: 0,
            max_parallel: 0,
            sched_start: Some(60),
            sched_end: Some(120),
            shutdown_at_end: true,
            is_default: false,
        };
        let mut was_open = HashMap::new();
        let ev = schedule_events(std::slice::from_ref(&list), 90, &mut was_open);
        assert!(!ev.shutdown, "no shutdown at the opening moment");
        let ev = schedule_events(std::slice::from_ref(&list), 130, &mut was_open);
        assert!(ev.shutdown);
        // The transition fires once, not on every later frame.
        let ev = schedule_events(std::slice::from_ref(&list), 131, &mut was_open);
        assert!(!ev.shutdown);
    }

    #[test]
    fn hm_round_trip() {
        assert_eq!(parse_hm("08:30"), Some(8 * 60 + 30));
        assert_eq!(fmt_hm(8 * 60 + 30), "08:30");
        assert_eq!(parse_hm(" 23:59 "), Some(23 * 60 + 59));
        assert_eq!(parse_hm("24:00"), None);
        assert_eq!(parse_hm("12:60"), None);
        assert_eq!(parse_hm("12"), None);
        assert_eq!(parse_hm("ab:cd"), None);
    }

    #[test]
    fn move_selection_shifts_the_block_as_one() {
        let mut order = ids(&["a", "b", "c", "d"]);
        let selected: HashSet<String> = ["b".into(), "c".into()].into_iter().collect();

        assert!(move_selection(&mut order, &selected, true));
        assert_eq!(order, ids(&["b", "c", "a", "d"]));
        assert!(!move_selection(&mut order, &selected, true), "already at the top");

        assert!(move_selection(&mut order, &selected, false));
        assert_eq!(order, ids(&["a", "b", "c", "d"]));
        assert!(move_selection(&mut order, &selected, false));
        assert_eq!(order, ids(&["a", "d", "b", "c"]));
        assert!(!move_selection(&mut order, &selected, false), "already at the bottom");
    }

    #[test]
    fn move_selection_skips_unselected_rows() {
        let mut order = ids(&["a", "b", "c"]);
        let only_c: HashSet<String> = ["c".into()].into_iter().collect();
        assert!(move_selection(&mut order, &only_c, true));
        assert_eq!(order, ids(&["a", "c", "b"]));
        assert!(move_selection(&mut order, &only_c, true));
        assert_eq!(order, ids(&["c", "a", "b"]));
    }
