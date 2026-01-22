use pest::Parser;

use super::*;

#[rstest]
#[case("# foo bar")]
#[case("remove(status);")]
#[case("remove(*, spec.template.spec);")]
#[case("remove(spec.template.spec.nodeSelector.\"simkube.dev/foo\" == \"bar\", metadata.labels);")]
#[case(
    "remove(@t >= 12345
        && @t <= 45677
        && spec.template.spec.nodeSelector.\"simkube.dev/foo\" == \"bar\",
      metadata);"
)]
#[case("remove(!exists(spec.template.tolerations), metadata.labels);")]
#[case("remove(@t == 1 && exists(spec.template.tolerations), metadata.labels);")]
#[case(
    "remove(exists(spec.template.spec.containers[*].env[*].valueFrom.secretKeyRef), metadata.labels.\"simkube.io/dev\");"
)]
#[case("remove($x := spec.template.spec.containers[*].env[*] | exists($x.valueFrom.secretKeyRef), $x);")]
#[case("remove($x := spec.template.spec.containers[*].env[*] | !exists($x.valueFrom.secretKeyRef), $x);")]
#[case(
    "remove(@t >= 10m
        && spec.template.spec.nodeSelector.\"simkube.dev/foo\" == \"bar\"
        && $p := spec.template.spec.tolerations[*] | $p.key <= 1234,
      $p.value);"
)]
#[case(
    "apply(@t >= 10m && spec.template.spec.nodeSelector.\"simkube.dev/foo\" == \"bar\",
      spec.template.spec.nodeSelector.\"simkube.dev/foo\" = \"baz\");"
)]
fn test_skel_should_parse(#[case] command: &str) {
    assert_ok!(SkelParser::parse(Rule::skel, command));
}

#[rstest]
#[case("remove(status")]
#[case("remove(* && status == \"foo\", metadata)")]
#[case("remove(@status)")]
#[case("remove(sta%tus)")]
fn test_skel_should_not_parse(#[case] command: &str) {
    assert_err!(SkelParser::parse(Rule::skel, command));
}
