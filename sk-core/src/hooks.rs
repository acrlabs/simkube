use std::fs;
use std::process::Stdio;

use anyhow::{
    anyhow,
    bail,
};
use sk_api::v1::SimulationHooksConfig;
use tokio::io::{
    AsyncWriteExt,
    BufWriter,
};
use tokio::process::Command;
use tracing::*;

use crate::prelude::*;

#[derive(Debug)]
pub enum Type {
    PreStart,
    PreRun,
    PostRun,
    PostStop,
}

pub fn merge_hooks(maybe_files: &Option<Vec<String>>) -> anyhow::Result<Option<SimulationHooksConfig>> {
    let Some(files) = maybe_files else {
        return Ok(None);
    };
    if files.is_empty() {
        return Ok(None);
    }

    Some(files.iter().try_fold(SimulationHooksConfig::default(), |mut merged_hooks, f| {
        let next = serde_yaml::from_slice::<SimulationHooksConfig>(
            &fs::read(f).map_err(|e| anyhow!("error reading hook {f}: {e}"))?,
        )
        .map_err(|e| anyhow!("error parsing hook {f}: {e}"))?;
        merge_vecs(&mut merged_hooks.pre_start_hooks, next.pre_start_hooks);
        merge_vecs(&mut merged_hooks.pre_run_hooks, next.pre_run_hooks);
        merge_vecs(&mut merged_hooks.post_run_hooks, next.post_run_hooks);
        merge_vecs(&mut merged_hooks.post_stop_hooks, next.post_stop_hooks);
        Ok(merged_hooks)
    }))
    .transpose()
}

pub async fn execute(sim: &Simulation, type_: Type) -> EmptyResult {
    let maybe_hooks = match &sim.spec.hooks {
        Some(hooks_config) => match type_ {
            Type::PreStart => hooks_config.pre_start_hooks.as_ref(),
            Type::PreRun => hooks_config.pre_run_hooks.as_ref(),
            Type::PostRun => hooks_config.post_run_hooks.as_ref(),
            Type::PostStop => hooks_config.post_stop_hooks.as_ref(),
        },
        _ => None,
    };

    if let Some(hooks) = maybe_hooks {
        info!("Executing {:?} hooks", type_);

        for hook in hooks {
            info!("Running `{}` with args {:?}", hook.cmd, hook.args);
            let mut child = Command::new(hook.cmd.clone())
                .args(hook.args.clone().unwrap_or_default())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            if let Some(true) = hook.send_sim {
                let mut stdin = BufWriter::new(child.stdin.take().ok_or(anyhow!("could not take stdin"))?);
                stdin.write_all(&serde_json::to_vec(sim)?).await?;
                stdin.flush().await?;
            }
            let output = child.wait_with_output().await?;
            info!("Hook output: {:?}", output);
            match hook.ignore_failure {
                Some(true) => (),
                _ => {
                    if !output.status.success() {
                        bail!("hook failed");
                    }
                },
            }
        }
        info!("Done executing {:?} hooks", type_);
    };

    Ok(())
}

fn merge_vecs<T>(maybe_v1: &mut Option<Vec<T>>, maybe_v2: Option<Vec<T>>) {
    if let Some(v2) = maybe_v2 {
        if let Some(v1) = maybe_v1 {
            v1.extend(v2)
        } else {
            *maybe_v1 = Some(v2)
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use assert_fs::prelude::*;
    use rstest::*;
    use sk_testutils::*;
    use tracing_test::*;

    use super::*;

    const HOOK1: &str = r#"
---
preStartHooks:
  - cmd: prestart1
    args:
      - prestart-arg1
      - prestart-arg2
preRunHooks:
  - cmd: prerun1
    args:
      - prerun-arg1
postRunHooks:
  - cmd: postrun1
    args:
      - postrun-arg1
"#;

    const HOOK2: &str = r#"
---
preStartHooks:
  - cmd: prestart2
  - cmd: prestart3
preRunHooks:
  - cmd: prerun2
    args:
      - prerun-arg2
postStopHooks:
  - cmd: poststop1
    args:
      - poststop-arg1
"#;

    const HOOK3: &str = r#"
---
preRunHooks:
  - cmd: prerun3
    args:
      - prerun-arg3
postRunHooks:
  - cmd: postrun2
    args:
      - prerun-arg2
"#;

    const EXPECTED_MERGED: &str = r#"
---
preStartHooks:
  - cmd: prestart1
    args:
      - prestart-arg1
      - prestart-arg2
  - cmd: prestart2
  - cmd: prestart3
preRunHooks:
  - cmd: prerun1
    args:
      - prerun-arg1
  - cmd: prerun2
    args:
      - prerun-arg2
  - cmd: prerun3
    args:
      - prerun-arg3
postRunHooks:
  - cmd: postrun1
    args:
      - postrun-arg1
  - cmd: postrun2
    args:
      - prerun-arg2
postStopHooks:
  - cmd: poststop1
    args:
      - poststop-arg1
"#;

    #[rstest]
    fn test_merge_hooks() {
        let temp = assert_fs::TempDir::new().unwrap();
        let hook1 = temp.child("hook1.yml");
        hook1.write_str(HOOK1).unwrap();
        let hook2 = temp.child("hook2.yml");
        hook2.write_str(HOOK2).unwrap();
        let hook3 = temp.child("hook3.yml");
        hook3.write_str(HOOK3).unwrap();

        let merged_config = merge_hooks(&Some(vec![
            hook1.path().to_str().unwrap().into(),
            hook2.path().to_str().unwrap().into(),
            hook3.path().to_str().unwrap().into(),
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(merged_config, serde_yaml::from_str(EXPECTED_MERGED).unwrap());
    }

    #[rstest]
    #[traced_test]
    #[tokio::test]
    async fn test_execute_hooks(test_sim: Simulation) {
        // Should print "foo"
        let res = execute(&test_sim, Type::PreStart).await;
        assert!(res.is_ok());

        // No PreStop hook defined
        let res = execute(&test_sim, Type::PostStop).await;
        assert!(res.is_ok());

        // PreRun hook calls bad command
        let res = execute(&test_sim, Type::PreRun).await;
        assert!(res.is_err());
    }
}
