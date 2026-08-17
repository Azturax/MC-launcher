//! Dependency graph + lockfile. Transitive required deps are walked;
//! incompatible edges and two versions of the same project are conflicts.
//! Pins always win over "latest".

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockfileEntry {
    pub path: String,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for LockfileEntry {
    fn default() -> Self {
        Self {
            path: String::new(),
            sha1: None,
            sha512: None,
            source: String::new(),
            project_id: None,
            version_id: None,
            filename: None,
            pinned: false,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lockfile {
    pub version: u32,
    pub instance_id: String,
    pub files: Vec<LockfileEntry>,
}

impl Lockfile {
    pub fn empty(instance_id: impl Into<String>) -> Self {
        Self {
            version: 1,
            instance_id: instance_id.into(),
            files: vec![],
        }
    }

    pub fn read_from(game_dir: &Path) -> AppResult<Self> {
        let path = game_dir.join("aureum.lock.json");
        if !path.is_file() {
            return Ok(Self::empty(""));
        }
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }

    pub fn hash(&self) -> AppResult<String> {
        let bytes = serde_json::to_vec(self)?;
        let digest = Sha256::digest(bytes);
        Ok(hex::encode(digest))
    }

    pub fn write_to(&self, game_dir: &Path) -> AppResult<String> {
        let path = game_dir.join("aureum.lock.json");
        std::fs::write(&path, serde_json::to_vec_pretty(self)?)?;
        self.hash()
    }

    pub fn upsert_mod(&mut self, entry: LockfileEntry) {
        if let Some(pid) = entry.project_id.as_deref() {
            self.files.retain(|f| f.project_id.as_deref() != Some(pid));
        }
        self.files.push(entry);
    }

    pub fn remove_project(&mut self, project_id: &str) {
        self.files
            .retain(|f| f.project_id.as_deref() != Some(project_id));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DepKind {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

impl DepKind {
    pub fn parse(raw: &str) -> Self {
        match raw {
            "optional" => Self::Optional,
            "incompatible" => Self::Incompatible,
            "embedded" => Self::Embedded,
            _ => Self::Required,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepRef {
    pub project_id: String,
    pub version_id: Option<String>,
    pub kind: DepKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub deps: Vec<DepRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub project_id: String,
    pub version_id: Option<String>,
    pub pin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub project_id: String,
    pub left: String,
    pub right: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveResult {
    pub instance_id: String,
    pub selected: Vec<(String, String)>,
    pub conflicts: Vec<Conflict>,
}

/// Resolve a closed set of packages. `packages` is keyed by (project_id, version_id).
/// `latest` is the version to use when a required dep has no version_id and no pin.
/// Pins always override latest and dep-requested versions.
pub fn resolve(
    instance_id: &str,
    roots: &[Selection],
    packages: &HashMap<(String, String), Package>,
    latest: &HashMap<String, String>,
    pins: &HashMap<String, String>,
) -> AppResult<ResolveResult> {
    let mut selected: HashMap<String, String> = HashMap::new();
    let mut conflicts = Vec::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut preferred: HashMap<String, String> = HashMap::new();

    for root in roots {
        queue.push_back(root.project_id.clone());
        if let Some(v) = &root.version_id {
            preferred.insert(root.project_id.clone(), v.clone());
        }
        if root.pin {
            if let Some(v) = &root.version_id {
                preferred.insert(root.project_id.clone(), v.clone());
            }
        }
    }

    let mut seen_queue: HashSet<String> = HashSet::new();

    while let Some(project_id) = queue.pop_front() {
        if !seen_queue.insert(project_id.clone()) && selected.contains_key(&project_id) {
            continue;
        }
        let version_id = pins
            .get(&project_id)
            .cloned()
            .or_else(|| preferred.get(&project_id).cloned())
            .or_else(|| latest.get(&project_id).cloned())
            .ok_or_else(|| {
                AppError::msg(format!("No version available for project {project_id}"))
            })?;

        if let Some(existing) = selected.get(&project_id) {
            if existing != &version_id {
                conflicts.push(Conflict {
                    project_id: project_id.clone(),
                    left: existing.clone(),
                    right: version_id,
                    reason: "two required versions of the same project".into(),
                });
            }
            continue;
        }
        selected.insert(project_id.clone(), version_id.clone());

        let Some(pkg) = packages.get(&(project_id.clone(), version_id.clone())) else {
            return Err(AppError::msg(format!(
                "Missing package metadata for {project_id}@{version_id}"
            )));
        };

        for dep in &pkg.deps {
            match dep.kind {
                DepKind::Required => {
                    if let Some(vid) = &dep.version_id {
                        if let Some(prev) = preferred.get(&dep.project_id) {
                            if prev != vid
                                && pins.get(&dep.project_id).is_none()
                                && selected.get(&dep.project_id).is_none()
                            {
                                preferred.insert(dep.project_id.clone(), vid.clone());
                            } else if prev != vid && selected.get(&dep.project_id).is_none() {
                                // keep first preferred unless pinned
                            }
                        } else {
                            preferred.insert(dep.project_id.clone(), vid.clone());
                        }
                    }
                    queue.push_back(dep.project_id.clone());
                }
                DepKind::Incompatible => {
                    if selected.contains_key(&dep.project_id)
                        || roots.iter().any(|r| r.project_id == dep.project_id)
                    {
                        conflicts.push(Conflict {
                            project_id: dep.project_id.clone(),
                            left: project_id.clone(),
                            right: dep.project_id.clone(),
                            reason: "incompatible dependency".into(),
                        });
                    }
                }
                DepKind::Optional | DepKind::Embedded => {}
            }
        }
    }

    // Second pass: required deps that requested a specific version after the
    // project was already selected with a different one.
    for ((pid, vid), pkg) in packages {
        if selected.get(pid) != Some(vid) {
            continue;
        }
        for dep in &pkg.deps {
            if dep.kind != DepKind::Required {
                continue;
            }
            if let (Some(want), Some(got)) = (&dep.version_id, selected.get(&dep.project_id)) {
                if want != got && pins.get(&dep.project_id).is_none() {
                    conflicts.push(Conflict {
                        project_id: dep.project_id.clone(),
                        left: got.clone(),
                        right: want.clone(),
                        reason: "transitive version mismatch".into(),
                    });
                }
            }
        }
    }

    Ok(ResolveResult {
        instance_id: instance_id.to_string(),
        selected: selected.into_iter().collect(),
        conflicts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(id: &str, ver: &str, deps: Vec<DepRef>) -> ((String, String), Package) {
        (
            (id.into(), ver.into()),
            Package {
                project_id: id.into(),
                version_id: ver.into(),
                name: id.into(),
                deps,
            },
        )
    }

    fn req(id: &str, ver: Option<&str>) -> DepRef {
        DepRef {
            project_id: id.into(),
            version_id: ver.map(|s| s.to_string()),
            kind: DepKind::Required,
        }
    }

    #[test]
    fn transitive_required_is_included() {
        let packages = HashMap::from([
            pkg("sodium", "1", vec![req("fabric-api", Some("2"))]),
            pkg("fabric-api", "2", vec![]),
        ]);
        let latest = HashMap::from([
            ("sodium".into(), "1".into()),
            ("fabric-api".into(), "2".into()),
        ]);
        let result = resolve(
            "inst",
            &[Selection {
                project_id: "sodium".into(),
                version_id: Some("1".into()),
                pin: false,
            }],
            &packages,
            &latest,
            &HashMap::new(),
        )
        .unwrap();
        assert!(result.conflicts.is_empty());
        let map: HashMap<_, _> = result.selected.into_iter().collect();
        assert_eq!(map.get("sodium").map(String::as_str), Some("1"));
        assert_eq!(map.get("fabric-api").map(String::as_str), Some("2"));
    }

    #[test]
    fn conflicting_versions_are_reported() {
        let packages = HashMap::from([
            pkg("a", "1", vec![req("lib", Some("1"))]),
            pkg("b", "1", vec![req("lib", Some("2"))]),
            pkg("lib", "1", vec![]),
            pkg("lib", "2", vec![]),
        ]);
        let latest = HashMap::from([
            ("a".into(), "1".into()),
            ("b".into(), "1".into()),
            ("lib".into(), "1".into()),
        ]);
        let result = resolve(
            "inst",
            &[
                Selection {
                    project_id: "a".into(),
                    version_id: Some("1".into()),
                    pin: false,
                },
                Selection {
                    project_id: "b".into(),
                    version_id: Some("1".into()),
                    pin: false,
                },
            ],
            &packages,
            &latest,
            &HashMap::new(),
        )
        .unwrap();
        assert!(
            result
                .conflicts
                .iter()
                .any(|c| c.project_id == "lib" || c.reason.contains("mismatch") || c.reason.contains("two required"))
        );
    }

    #[test]
    fn pin_wins_over_latest() {
        let packages = HashMap::from([pkg("mod", "old", vec![]), pkg("mod", "new", vec![])]);
        let latest = HashMap::from([("mod".into(), "new".into())]);
        let pins = HashMap::from([("mod".into(), "old".into())]);
        let result = resolve(
            "inst",
            &[Selection {
                project_id: "mod".into(),
                version_id: None,
                pin: true,
            }],
            &packages,
            &latest,
            &pins,
        )
        .unwrap();
        let map: HashMap<_, _> = result.selected.into_iter().collect();
        assert_eq!(map.get("mod").map(String::as_str), Some("old"));
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn incompatible_edge_conflicts() {
        let packages = HashMap::from([
            pkg(
                "a",
                "1",
                vec![DepRef {
                    project_id: "b".into(),
                    version_id: None,
                    kind: DepKind::Incompatible,
                }],
            ),
            pkg("b", "1", vec![]),
        ]);
        let latest = HashMap::from([("a".into(), "1".into()), ("b".into(), "1".into())]);
        let result = resolve(
            "inst",
            &[
                Selection {
                    project_id: "a".into(),
                    version_id: Some("1".into()),
                    pin: false,
                },
                Selection {
                    project_id: "b".into(),
                    version_id: Some("1".into()),
                    pin: false,
                },
            ],
            &packages,
            &latest,
            &HashMap::new(),
        )
        .unwrap();
        assert!(result.conflicts.iter().any(|c| c.reason.contains("incompatible")));
    }
}
