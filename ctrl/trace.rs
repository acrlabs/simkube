use std::path::PathBuf;

use anyhow::anyhow;
use k8s_openapi::api::core::v1 as corev1;
use reqwest::Url;

const TRACE_VOLUME_NAME: &str = "trace-data";
const TRACE_PATH: &str = "/trace-data";

pub(super) fn get_local_trace_volume(path: &Url) -> anyhow::Result<(corev1::VolumeMount, corev1::Volume, String)> {
    let fp = path.to_file_path().map_err(|_| anyhow!("could not parse path"))?;

    let mut mount_path = PathBuf::from(TRACE_PATH);
    mount_path.push(fp.clone());
    let mount_path_str = mount_path.to_str().unwrap();

    Ok((
        corev1::VolumeMount {
            name: TRACE_VOLUME_NAME.into(),
            mount_path: mount_path_str.into(),
            ..Default::default()
        },
        corev1::Volume {
            name: TRACE_VOLUME_NAME.into(),
            host_path: Some(corev1::HostPathVolumeSource {
                path: fp.into_os_string().into_string().unwrap(),
                type_: Some("File".into()),
            }),
            ..Default::default()
        },
        mount_path_str.into(),
    ))
}
