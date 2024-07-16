use std::process::Stdio;

use anyhow::{
    anyhow,
    bail,
};
use tokio::io::{
    AsyncWriteExt,
    BufWriter,
};
use tokio::process::Command;

use crate::prelude::*;

#[derive(Debug)]
pub enum Type {
    PreStart,
    PreRun,
    PostRun,
    PostStop,
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
                .args(hook.args.clone())
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

#[cfg(test)]
mod test {
    use rstest::*;
    use tracing_test::*;

    use super::*;
    use crate::k8s::testutils::test_sim;

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
