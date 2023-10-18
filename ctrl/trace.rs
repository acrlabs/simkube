use std::path::PathBuf;

use anyhow::anyhow;
use reqwest::Url;
use simkube::prelude::*;

const TRACE_VOLUME_NAME: &str = "trace-data";
const TRACE_PATH: &str = "/trace-data";

pub(super) fn get_local_trace_volume(path: &Url) -> anyhow::Result<(corev1::VolumeMount, corev1::Volume, String)> {
    let fp = path
        .to_file_path()
        .map_err(|_| anyhow!("could not parse trace path: {}", path))?;

    let host_path_str = fp
        .clone()
        .into_os_string()
        .into_string()
        .map_err(|osstr| anyhow!("could not parse host path: {:?}", osstr))?;

    let mut mount_path = PathBuf::from(TRACE_PATH);
    mount_path.push(fp);
    let mount_path_str = mount_path
        .to_str()
        .ok_or(anyhow!("could not parse trace mount path: {}", mount_path.display()))?;

    Ok((
        corev1::VolumeMount {
            name: TRACE_VOLUME_NAME.into(),
            mount_path: mount_path_str.into(),
            ..Default::default()
        },
        corev1::Volume {
            name: TRACE_VOLUME_NAME.into(),
            host_path: Some(corev1::HostPathVolumeSource { path: host_path_str, type_: Some("File".into()) }),
            ..Default::default()
        },
        mount_path_str.into(),
    ))
}
