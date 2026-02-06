use std::collections::BTreeMap;

use serde_json::json;
use sk_core::prelude::*;
use sk_store::TraceEvent;

use super::*;

#[rstest]
#[case::implicit_match_star(
    "remove(status);",
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({"status": {"foo": "bar"}}),
        }],
        deleted_objs: vec![],
    },
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({}),
        }],
        deleted_objs: vec![],
    },
)]
#[case::match_star(
    "remove(*, status);",
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({"status": {"foo": "bar"}}),
        }],
        deleted_objs: vec![],
    },
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({}),
        }],
        deleted_objs: vec![],
    },
)]
#[case::quoted_label_match(
    "remove(metadata.labels.\"simkube.dev/foo\" == \"bar\", status);",
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({"status": {"foo": "bar"}}),
        },
        DynamicObject{
            types: None,
            metadata: metav1::ObjectMeta{
                labels: Some(BTreeMap::from([("simkube.dev/foo".into(), "bar".into())])),
                ..Default::default()
            },
            data: json!({"status": {"foo": "bar"}}),
        }],
        deleted_objs: vec![],
    },
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({"status": {"foo": "bar"}}),
        },
        DynamicObject{
            types: None,
            metadata: metav1::ObjectMeta{
                labels: Some(BTreeMap::from([("simkube.dev/foo".into(), "bar".into())])),
                ..Default::default()
            },
            data: json!({}),
        }],
        deleted_objs: vec![],
    },
)]
#[case::exists(
    "remove(exists(metadata.labels.\"simkube.dev/foo\"), status);",
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({"status": {"foo": "bar"}}),
        },
        DynamicObject{
            types: None,
            metadata: metav1::ObjectMeta{
                labels: Some(BTreeMap::from([("simkube.dev/foo".into(), "bar".into())])),
                ..Default::default()
            },
            data: json!({"status": {"foo": "bar"}}),
        }],
        deleted_objs: vec![],
    },
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({"status": {"foo": "bar"}}),
        },
        DynamicObject{
            types: None,
            metadata: metav1::ObjectMeta{
                labels: Some(BTreeMap::from([("simkube.dev/foo".into(), "bar".into())])),
                ..Default::default()
            },
            data: json!({}),
        }],
        deleted_objs: vec![],
    },
)]
#[case::not_exists(
    "remove(!exists(metadata.labels.\"simkube.dev/foo\"), status);",
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({"status": {"foo": "bar"}}),
        },
        DynamicObject{
            types: None,
            metadata: metav1::ObjectMeta{
                labels: Some(BTreeMap::from([("simkube.dev/foo".into(), "bar".into())])),
                ..Default::default()
            },
            data: json!({"status": {"foo": "bar"}}),
        }],
        deleted_objs: vec![],
    },
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({}),
        },
        DynamicObject{
            types: None,
            metadata: metav1::ObjectMeta{
                labels: Some(BTreeMap::from([("simkube.dev/foo".into(), "bar".into())])),
                ..Default::default()
            },
            data: json!({"status": {"foo": "bar"}}),
        }],
        deleted_objs: vec![],
    },
)]
#[case::multi_conditional_1(
    "remove(@t >= 123 && metadata.namespace == \"baz\", status);",
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({"status": {"foo": "bar"}}),
        },
        DynamicObject{
            types: None,
            metadata: metav1::ObjectMeta{
                namespace: Some("baz".into()),
                ..Default::default()
            },
            data: json!({"status": {"foo": "bar"}}),
        }],
        deleted_objs: vec![],
    },
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({"status": {"foo": "bar"}}),
        },
        DynamicObject{
            types: None,
            metadata: metav1::ObjectMeta{
                namespace: Some("baz".into()),
                ..Default::default()
            },
            data: json!({}),
        }],
        deleted_objs: vec![],
    },
)]
#[case::multi_conditional_2(
    "remove(@t < 123 && metadata.namespace == \"baz\", status);",
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({"status": {"foo": "bar"}}),
        },
        DynamicObject{
            types: None,
            metadata: metav1::ObjectMeta{
                namespace: Some("baz".into()),
                ..Default::default()
            },
            data: json!({"status": {"foo": "bar"}}),
        }],
        deleted_objs: vec![],
    },
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({"status": {"foo": "bar"}}),
        },
        DynamicObject{
            types: None,
            metadata: metav1::ObjectMeta{
                namespace: Some("baz".into()),
                ..Default::default()
            },
            data: json!({"status": {"foo": "bar"}}),
        }],
        deleted_objs: vec![],
    },
)]
#[case::wildcards(
    "remove(spec.template.spec.containers[*]);",
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({
                "spec": {
                    "template": {
                        "spec": {
                            "containers": [
                                {
                                    "env": [
                                        {"valueFrom": {"secretKeyRef": "SOME_SECRET"}},
                                        {"FOO": "BAR"},
                                    ],
                                },
                                {"env": [{"ASDF": "QWERTY"}]},
                                {
                                    "env": [
                                        {"FOO": "BAR"},
                                        {"valueFrom": {"secretKeyRef": "SOME_SECRET"}},
                                        {"valueFrom": {"secretKeyRef": "SOME_SECRET_2"}},
                                    ],
                                },
                            ],
                        },
                    },
                },
                "status": {"foo": "bar"},
            }),
        }],
        deleted_objs: vec![],
    },
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({
                "spec": {"template": {"spec": {"containers": []}}},
                "status": {"foo": "bar"},
            }),
        }],
        deleted_objs: vec![],
    },
)]
#[case::variable_def_1(
    "remove($x := spec.template.spec.containers[*].env[*] | exists($x.valueFrom.secretKeyRef), $x);",
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({
                "spec": {
                    "template": {
                        "spec": {
                            "containers": [
                                {
                                    "env": [
                                        {"valueFrom": {"secretKeyRef": "SOME_SECRET"}},
                                        {"FOO": "BAR"},
                                    ],
                                },
                                {"env": [{"ASDF": "QWERTY"}]},
                                {
                                    "env": [
                                        {"FOO": "BAR"},
                                        {"valueFrom": {"secretKeyRef": "SOME_SECRET"}},
                                        {"valueFrom": {"secretKeyRef": "SOME_SECRET_2"}},
                                    ],
                                },
                            ],
                        },
                    },
                },
                "status": {"foo": "bar"},
            }),
        }],
        deleted_objs: vec![],
    },
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({
                "spec": {
                    "template": {
                        "spec": {
                            "containers": [
                                {"env": [{"FOO": "BAR"}]},
                                {"env": [{"ASDF": "QWERTY"}]},
                                {"env": [{"FOO": "BAR"}]},
                            ],
                        },
                    },
                },
                "status": {"foo": "bar"},
            }),
        }],
        deleted_objs: vec![],
    },
)]
#[case::variable_def_2(
    "remove($x := spec.template.spec.containers[*].env[*] | exists($x.valueFrom.secretKeyRef), $x.valueFrom);",
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({
                "spec": {
                    "template": {
                        "spec": {
                            "containers": [
                                {
                                    "env": [
                                        {"valueFrom": {"secretKeyRef": "SOME_SECRET"}, "valueTo": 42},
                                        {"FOO": "BAR"},
                                    ],
                                },
                                {"env": [{"ASDF": "QWERTY"}]},
                                {
                                    "env": [
                                        {"FOO": "BAR"},
                                        {"valueFrom": {"secretKeyRef": "SOME_SECRET"}, "valueTo": 67},
                                        {"valueFrom": {"secretKeyRef": "SOME_SECRET_2"}, "valueTo": 99},
                                    ],
                                },
                            ],
                        },
                    },
                },
                "status": {"foo": "bar"},
            }),
        }],
        deleted_objs: vec![],
    },
    TraceEvent{
        ts: 1234,
        applied_objs: vec![DynamicObject{
            types: None,
            metadata: Default::default(),
            data: json!({
                "spec": {
                    "template": {
                        "spec": {
                            "containers": [
                                {"env": [{"valueTo": 42}, {"FOO": "BAR"}]},
                                {"env": [{"ASDF": "QWERTY"}]},
                                {"env": [{"FOO": "BAR"}, {"valueTo": 67}, {"valueTo": 99}]},
                            ],
                        },
                    },
                },
                "status": {"foo": "bar"},
            }),
        }],
        deleted_objs: vec![],
    },
)]
fn test_remove_command(#[case] cmd_str: &str, #[case] evt: TraceEvent, #[case] expected: TraceEvent) {
    let mut skel = SkelParser::parse(Rule::skel, &cmd_str).unwrap();
    let cmd = parse_command(skel.next().unwrap(), 1234).unwrap();
    let res = apply_command_to_event(&cmd, evt).unwrap();
    assert_eq!(res, expected);
}
