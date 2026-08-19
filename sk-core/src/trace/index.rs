use std::collections::{
    BTreeMap,
    HashMap,
};

use serde::de::Deserializer;
use serde::ser::{
    SerializeMap,
    Serializer,
};
use serde_with::ser::SerializeAsWrap;
use serde_with::{
    As,
    DisplayFromStr,
    Same,
};

use crate::k8s::{
    GVK,
    KubeResourceId,
};
use crate::trace::pod_sim_data::PodSimData;

pub type TraceIndexEntry = BTreeMap<i64, Vec<PodSimData>>;
pub type TraceIndex = HashMap<KubeResourceId, TraceIndexEntry>;

// This is ugly as fuck but this type lets us convert the i64 mtime values from the TraceIndex into
// String values in the serialized result, so that we can roundtrip a tracefile to JSON and back;
// this makes hand modifications _much_ easier, and was something that bit us all the time with the
// v2 trace format.
type SerializedIndexEntryByPod = BTreeMap<Same, BTreeMap<DisplayFromStr, Same>>;

// When we serialize the TraceIndex, we want to nest the GVK and ns_name fields, so that the trace
// file isn't repeating a bunch of unneeded information, and to make it easier to read/look at.  But
// interally it's easier/more ergonomic to work with KubeResourceIds, so these serialize/deserialize
// methods seamlessly convert back and forth between these formats.
pub fn serialize<S>(map: &HashMap<KubeResourceId, TraceIndexEntry>, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // Convert the ns_names to a BTreeMap so that they are sorted in the final output; GVKs don't
    // implement Ord and it's a mild pain to make them implement Ord so we just leave these unsorted
    // in the output
    let mut grouped: HashMap<GVK, BTreeMap<String, TraceIndexEntry>> = HashMap::new();
    for (resource_id, buckets) in map {
        grouped
            .entry(resource_id.gvk.clone())
            .or_default()
            .insert(resource_id.ns_name.clone(), buckets.clone());
    }
    let mut m = ser.serialize_map(Some(grouped.len()))?;
    for (gvk, objs) in &grouped {
        m.serialize_entry(
            gvk,
            &SerializeAsWrap::<BTreeMap<String, TraceIndexEntry>, SerializedIndexEntryByPod>::new(objs),
        )?;
    }
    m.end()
}

pub fn deserialize<'de, D>(de: D) -> Result<HashMap<KubeResourceId, TraceIndexEntry>, D::Error>
where
    D: Deserializer<'de>,
{
    let nested: HashMap<GVK, BTreeMap<String, TraceIndexEntry>> =
        As::<HashMap<Same, SerializedIndexEntryByPod>>::deserialize(de)?;
    Ok(nested
        .into_iter()
        .flat_map(|(gvk, objs)| {
            objs.into_iter()
                .map(move |(ns_name, entry)| (KubeResourceId::new(gvk.clone(), ns_name), entry))
        })
        .collect())
}
