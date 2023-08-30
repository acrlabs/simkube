use k8s_openapi::api::core::v1 as corev1;
use reqwest::Url;

const TRACE_VOLUME_NAME: &str = "trace-data";
const TRACE_PATH: &str = "/trace-data";

pub(super) fn get_local_trace_volume(path: &Url) -> (corev1::VolumeMount, corev1::Volume, String) {
    let fp: String = match path.to_file_path() {
        Ok(p) => p.to_str().unwrap().into(),
        _ => path.as_str().into(),
    };
    let mount_path = format!("/{}/{}", TRACE_PATH, fp);
    return (
        corev1::VolumeMount {
            name: TRACE_VOLUME_NAME.into(),
            mount_path: mount_path.clone(),
            ..corev1::VolumeMount::default()
        },
        corev1::Volume {
            name: TRACE_VOLUME_NAME.into(),
            host_path: Some(corev1::HostPathVolumeSource { path: fp, type_: Some("File".into()) }),
            ..corev1::Volume::default()
        },
        mount_path,
    );
}
