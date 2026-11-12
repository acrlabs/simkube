use json_patch_ext::{
    PointerBuf,
    format_ptr,
};
use sk_core::k8s::GVK;
use sk_core::trace::config::TracerConfig;

// Defines which paths should be prepended to a rules path ptrs before matching
// note that each path will be
pub enum RuleScope {
    Empty,
    PodTemplates,
    #[allow(unused)]
    Paths(Vec<&'static str>),
    // It seems incredibly common that we want to check multiple subpaths
    // Theres a world here in which it might be nice to have a "path set" object, which represents a
    // path as a list of lists of path components, with each enumerating against every other, such
    // that syntax such as /this/that/{a,b,c}/d are supported, or to explicitly support {} syntax in
    // some other way. the list form would also make escaping trivial
}
impl RuleScope {
    pub fn get_scoped_paths(&self, ptrs: &Vec<&str>, config: &TracerConfig, gvk: &GVK) -> Vec<PointerBuf> {
        match &self {
            RuleScope::Empty => ptrs.iter().map(|p| format_ptr!("{p}")).collect(),
            // if this rule is slow we can lazylock Paths or memoize PodTemplates
            RuleScope::PodTemplates => config
                .pod_spec_template_paths(gvk)
                .into_iter()
                .flatten()
                .flat_map(|scope| ptrs.iter().map(move |ptr| format_ptr!("{scope}{ptr}")))
                .collect(),
            RuleScope::Paths(paths) => paths
                .iter()
                .flat_map(|scope| ptrs.iter().map(move |ptr| format_ptr!("{scope}{ptr}")))
                .collect(),
        }
    }
}
