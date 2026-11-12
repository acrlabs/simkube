use json_patch_ext::{PointerBuf, format_ptr};
use sk_core::k8s::GVK;
use sk_core::trace::config::TracerConfig;

type PathStr = &'static str;
/// Defines which paths should be prepended to a rules path ptrs before matching
/// If you want a pointer to be scoped by a rule scope you must call .add
/// and then `get_scoped_paths`
///
/// The entire RuleScope construct exists because we must lazily scope paths
/// based on the config, and gvk ie pod_spec_template_paths
/// which can only be obtained during rule evaluation for a resource
/// Ideally the pointers would include the resource type and just fail to match on wrong resources
/// And a rulescope could just build a list of *all* the paths once
/// this would require our pointers to be able to filter on the resource type
///
/// It would be nice if we could support things like Regex paths
/// but then we'd need to pass in the entire event to search
#[derive(std::fmt::Debug)]
pub enum RuleScope {
    /// Root Scope is equivalent to bare paths
    /// Nothing is prepended
    Root,
    /// Searches relative to pod templates based on config.pod_spec_template_paths
    Templates,
    #[allow(unused)]
    /// Searches relative to the given path strings
    /// note here that scoped by will take the *target paths* of its rulepath
    /// so to search in /foo/bar for /baz you want ScopedBy(Root().add(/foo/bar)).add(baz)
    ScopedBy(Box<RulePath>),
    /// gates a rule to apply only if the resource kind matches the list
    /// if it does not this rule will always return an empty list of PointerBufs
    /// this should nest correctly with ScopedBy
    OnlyOnKinds(Vec<String>, Box<RulePath>),
    /// Searches relative to all RuleScopes in its contained RulePaths
    /// To get the paths stored by RulePath, use the RulePath level caller
    Multi(Vec<RulePath>),
}
fn join_ptrs(root: &str, target: &str) -> PointerBuf {
    format_ptr!("{}/{}", root.trim_end_matches("/"), target.trim_start_matches("/"))
}
impl RuleScope {
    /// returns exactly the paths passed in with the scopes basepaths appened
    pub fn apply_scope(&self, ptrs: &Vec<PathStr>, config: &TracerConfig, gvk: &GVK) -> Vec<PointerBuf> {
        match &self {
            RuleScope::Root => ptrs.iter().map(|p| format_ptr!("{p}")).collect(),
            // if this rule is slow we can lazylock Paths or memoize PodTemplates
            RuleScope::Templates => config
                .pod_spec_template_paths(gvk)
                .into_iter()
                .flatten()
                .flat_map(|scope| ptrs.iter().map(move |ptr| join_ptrs(scope, ptr)))
                .collect(),
            RuleScope::ScopedBy(rule_path) => rule_path
                .get_scoped_paths(config, gvk)
                .iter()
                .flat_map(|scope| ptrs.iter().map(move |ptr| join_ptrs(scope.as_str(), ptr)))
                .collect(),
            // Note this is for the RuleScope::Multi type and not the item Multi type, as such we dont
            // carry forward the extra metadata associated with Items in MultiScope
            RuleScope::Multi(rule_paths) => rule_paths
                .iter()
                .flat_map(|rule| rule.scope.apply_scope(ptrs, config, gvk))
                .collect(),
            RuleScope::OnlyOnKinds(valid_kinds, rule_paths) => {
                if valid_kinds.contains(&gvk.kind) {
                    rule_paths.scope.apply_scope(ptrs, config, gvk)
                } else {
                    Default::default()
                }
            },
        }
    }
}

/// A type representing a full rulepath to search split into target path and scope
/// New target paths should be added with .add
#[derive(std::fmt::Debug)]
pub struct RulePath {
    /// A set of paths to search for
    target_paths: Vec<PathStr>,
    /// A scope to prepend to each search path
    /// each scope path gets prepended to each target path
    scope: RuleScope,
}
impl RulePath {
    /// Constructor for RulePath with no scope ie "Root"
    pub fn root() -> RulePath {
        RulePath { target_paths: Vec::new(), scope: RuleScope::Root }
    }
    /// Constructor for RulePath relative to pod templates based on the resource kind
    pub fn templates() -> RulePath {
        RulePath {
            target_paths: Vec::new(),
            scope: RuleScope::Templates,
        }
    }
    /// Constructor for RulePath scoped by the vec of `paths` passed in
    /// note here that scoped by will take the *target paths* of its rulepath this is not my
    /// so to search in /foo/bar for /baz you want ScopedBy(Root().add(/foo/bar)).add(baz)
    /// You can get quite complex behavior by passing in a multi path
    pub fn scoped_by(rule_path: RulePath) -> RulePath {
        RulePath {
            target_paths: Vec::new(),
            scope: RuleScope::ScopedBy(Box::new(rule_path)),
        }
    }
    /// Constructor for RulePath, which contains multiple groups of paths
    /// Each subpath may have their own targets, or may exclusively be used for combining
    /// multiple scopes
    /// any added paths at this level will be recursed to each contained path
    /// the complete set of paths will include all subpaths
    pub fn multi(rule_paths: Vec<RulePath>) -> RulePath {
        RulePath {
            target_paths: Vec::new(),
            scope: RuleScope::Multi(rule_paths),
        }
    }
    pub fn only_on_kinds(allowed_kinds: Vec<&str>, rule_path: RulePath) -> RulePath {
        RulePath {
            target_paths: Vec::new(),
            // allow an extra clone because ergonomics matters more then perf here
            scope: RuleScope::OnlyOnKinds(allowed_kinds.iter().map(|s| s.to_string()).collect(), Box::new(rule_path)),
        }
    }
    /// adds a target path to be searched for in the RulePaths scope
    pub fn add(mut self, targets: Vec<PathStr>) -> Self {
        for p in &targets {
            PointerBuf::parse(*p).unwrap();
        }
        self.target_paths.append(&mut targets.clone());
        self
    }
    pub fn get_scoped_paths(&self, config: &TracerConfig, gvk: &GVK) -> Vec<PointerBuf> {
        match &self.scope {
            RuleScope::Multi(paths) => paths
                .iter()
                .flat_map(|paths| paths.get_scoped_paths(config, gvk))
                // Unfortunately this cant just be a flatmap collect because the Multi scope itself can hold
                // paths, this is desirable as some paths will truely be across everything in the scope
                // but it means we need to add them in here
                .chain(self.scope.apply_scope(&self.target_paths, config, gvk))
                .collect(),
            RuleScope::OnlyOnKinds(kinds, rule_path) => {
                if kinds.contains(&gvk.kind) {
                    rule_path
                        .get_scoped_paths(config, gvk)
                        .into_iter()
                        .chain(self.scope.apply_scope(&self.target_paths, config, gvk))
                        .collect()
                } else {
                    // return empty vec if path doesnt match
                    Vec::new()
                }
            },
            RuleScope::Templates | RuleScope::Root | RuleScope::ScopedBy(_) => {
                self.scope.apply_scope(&self.target_paths, config, gvk)
            },
        }
    }
}
