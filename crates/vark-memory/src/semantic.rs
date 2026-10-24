use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

pub mod extraction;
pub use extraction::extract_relations;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Entity {
    pub id: String,
    pub label: String,
    pub entity_type: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Quantifier {
    All,
    Some,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relation {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub polarity: bool,
    pub confidence: u8,
    pub source: String,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub qualifiers: Vec<(String, String)>,
    #[serde(default)]
    pub quantifier: Option<Quantifier>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    #[serde(default)]
    pub entities: HashMap<String, Entity>,
    #[serde(default)]
    pub relations: Vec<Relation>,
    #[serde(default)]
    pub exclusive: HashSet<String>,
}

impl KnowledgeGraph {
    pub fn new() -> Self {
        let mut graph = Self::default();
        for predicate in [
            "is",
            "is_a",
            "type",
            "located_in",
            "born_in",
            "has_color",
            "has",
            "greater_than",
            "less_than",
        ] {
            graph.exclusive.insert(predicate.into());
        }
        graph
    }

    pub fn upsert_entity(&mut self, label: &str, kind: &str) -> String {
        let id = normalize_entity(label);
        self.entities.entry(id.clone()).or_insert(Entity {
            id: id.clone(),
            label: label.trim().to_owned(),
            entity_type: kind.to_owned(),
            aliases: Vec::new(),
        });
        id
    }

    pub fn add_alias(&mut self, entity_id: &str, alias: &str) -> bool {
        let id = normalize_entity(entity_id);
        let Some(entity) = self.entities.get_mut(&id) else {
            return false;
        };
        let alias = alias.trim().to_owned();
        if alias.is_empty()
            || normalize_entity(&alias) == normalize_entity(&entity.label)
            || entity
                .aliases
                .iter()
                .any(|existing| normalize_entity(existing) == normalize_entity(&alias))
        {
            return false;
        }
        entity.aliases.push(alias);
        true
    }

    pub fn resolve_entity(&self, label: &str) -> Option<&Entity> {
        let normalized = normalize_entity(label);
        self.entities.values().find(|entity| {
            normalize_entity(&entity.id) == normalized
                || normalize_entity(&entity.label) == normalized
                || entity
                    .aliases
                    .iter()
                    .any(|alias| normalize_entity(alias) == normalized)
        })
    }

    fn canonical_entity(&self, label: &str) -> String {
        self.resolve_entity(label)
            .map(|entity| entity.id.clone())
            .unwrap_or_else(|| normalize_entity(label))
    }

    fn canonicalize_relation(&self, relation: &mut Relation) {
        relation.subject = self.canonical_entity(&relation.subject);
        relation.object = self.canonical_entity(&relation.object);
    }

    pub fn insert(&mut self, mut relation: Relation) -> bool {
        self.canonicalize_relation(&mut relation);
        if self.relations.iter().any(|existing| existing == &relation) {
            return false;
        }
        self.relations.push(relation);
        true
    }

    pub fn replace_conflicts(&mut self, mut relation: Relation) -> Vec<Relation> {
        self.canonicalize_relation(&mut relation);
        let removed = self.conflicts_for(&relation);
        self.relations
            .retain(|existing| !removed.iter().any(|old| old == existing));
        self.insert(relation);
        removed
    }

    pub fn query(
        &self,
        subject: Option<&str>,
        predicate: Option<&str>,
        object: Option<&str>,
        polarity: Option<bool>,
    ) -> Vec<Relation> {
        let subject = subject.map(|value| self.canonical_entity(value));
        let object = object.map(|value| self.canonical_entity(value));
        self.relations
            .iter()
            .filter(|relation| {
                subject
                    .as_deref()
                    .is_none_or(|value| value == relation.subject)
                    && predicate.is_none_or(|value| value == relation.predicate)
                    && object
                        .as_deref()
                        .is_none_or(|value| value == relation.object)
                    && polarity.is_none_or(|value| value == relation.polarity)
            })
            .cloned()
            .collect()
    }

    pub fn conflicts_for(&self, relation: &Relation) -> Vec<Relation> {
        let subject = self.canonical_entity(&relation.subject);
        let object = self.canonical_entity(&relation.object);
        self.relations
            .iter()
            .filter(|existing| {
                existing.subject == subject
                    && existing.predicate == relation.predicate
                    && (existing.object != object || existing.polarity != relation.polarity)
                    && (self.exclusive.contains(&relation.predicate)
                        || self.exclusive.contains(&existing.predicate))
            })
            .cloned()
            .collect()
    }

    pub fn contradictions(&self) -> Vec<(Relation, Relation)> {
        let mut output = Vec::new();
        for relation in &self.relations {
            for conflict in self.conflicts_for(relation) {
                if relation != &conflict {
                    output.push((relation.clone(), conflict));
                }
            }
        }
        output
    }

    pub fn reachable(&self, start: &str, depth: usize) -> HashSet<String> {
        let mut seen = HashSet::new();
        let mut queue = VecDeque::from([(self.canonical_entity(start), 0)]);
        while let Some((node, distance)) = queue.pop_front() {
            if distance > depth || !seen.insert(node.clone()) {
                continue;
            }
            for relation in self
                .relations
                .iter()
                .filter(|relation| relation.polarity && relation.subject == node)
            {
                queue.push_back((relation.object.clone(), distance + 1));
            }
        }
        seen
    }
}

pub fn normalize_entity(label: &str) -> String {
    let mut words = label
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric()
                || character.is_whitespace()
                || matches!(character, '_' | '-')
            {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    while words.first().is_some_and(|word| {
        matches!(
            word.as_str(),
            "sebuah" | "seekor" | "si" | "sang" | "the" | "a" | "an"
        )
    }) {
        words.remove(0);
    }
    words.join(" ")
}

#[derive(Debug, Clone)]
pub struct ContradictionChecker {
    pub graph: KnowledgeGraph,
}

impl ContradictionChecker {
    pub fn check(&self, relation: &Relation) -> bool {
        !self.graph.conflicts_for(relation).is_empty()
    }

    pub fn conflicts(&self) -> Vec<(Relation, Relation)> {
        self.graph.contradictions()
    }
}

#[cfg(test)]
mod tests {
    use super::{KnowledgeGraph, Relation};

    fn relation(subject: &str, object: &str) -> Relation {
        Relation {
            subject: subject.to_owned(),
            predicate: "is_a".to_owned(),
            object: object.to_owned(),
            polarity: true,
            confidence: 70,
            source: "unit test".to_owned(),
            value: None,
            unit: None,
            qualifiers: Vec::new(),
            quantifier: None,
        }
    }

    #[test]
    fn aliases_resolve_to_the_same_entity_id_in_graph_queries() {
        let mut graph = KnowledgeGraph::new();
        let entity_id = graph.upsert_entity("New York", "place");
        assert!(graph.add_alias(&entity_id, "NYC"));
        graph.insert(relation("NYC", "kota"));
        assert_eq!(graph.query(Some("the New York"), None, None, None).len(), 1);
        assert_eq!(graph.relations[0].subject, "new york");
        assert_eq!(graph.resolve_entity("NYC").unwrap().id, entity_id);
    }

    #[test]
    fn legacy_entity_and_relation_records_deserialize_with_defaults() {
        let old_entity = r#"{"id":"rust","label":"Rust","entity_type":"language"}"#;
        let entity: super::Entity = serde_json::from_str(old_entity).unwrap();
        assert!(entity.aliases.is_empty());
        let old_relation = r#"{"subject":"rust","predicate":"is_a","object":"language","polarity":true,"confidence":70,"source":"legacy"}"#;
        let relation: Relation = serde_json::from_str(old_relation).unwrap();
        assert_eq!(relation.value, None);
        assert_eq!(relation.unit, None);
        assert!(relation.qualifiers.is_empty());
        assert_eq!(relation.quantifier, None);
    }
}
