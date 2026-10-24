use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Entity {
    pub id: String,
    pub label: String,
    pub entity_type: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Relation {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub polarity: bool,
    pub confidence: u8,
    pub source: String,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    pub entities: HashMap<String, Entity>,
    pub relations: Vec<Relation>,
    pub exclusive: HashSet<String>,
}
impl KnowledgeGraph {
    pub fn new() -> Self {
        let mut g = Self::default();
        for p in ["is", "type", "located_in", "born_in", "has_color", "is_a"] {
            g.exclusive.insert(p.into());
        }
        g
    }
    pub fn upsert_entity(&mut self, label: &str, kind: &str) -> String {
        let id = label.to_lowercase();
        self.entities.entry(id.clone()).or_insert(Entity {
            id: id.clone(),
            label: label.into(),
            entity_type: kind.into(),
        });
        id
    }
    pub fn insert(&mut self, r: Relation) -> bool {
        if self.relations.iter().any(|x| x == &r) {
            return false;
        }
        self.relations.push(r);
        true
    }
    pub fn replace_conflicts(&mut self, r: Relation) -> Vec<Relation> {
        let removed = self.conflicts_for(&r);
        self.relations
            .retain(|x| !removed.iter().any(|old| old == x));
        self.insert(r);
        removed
    }
    pub fn query(
        &self,
        s: Option<&str>,
        p: Option<&str>,
        o: Option<&str>,
        polarity: Option<bool>,
    ) -> Vec<Relation> {
        self.relations
            .iter()
            .filter(|r| {
                s.map(|x| x == r.subject).unwrap_or(true)
                    && p.map(|x| x == r.predicate).unwrap_or(true)
                    && o.map(|x| x == r.object).unwrap_or(true)
                    && polarity.map(|x| x == r.polarity).unwrap_or(true)
            })
            .cloned()
            .collect()
    }
    pub fn conflicts_for(&self, r: &Relation) -> Vec<Relation> {
        self.relations
            .iter()
            .filter(|x| {
                x.subject == r.subject
                    && x.predicate == r.predicate
                    && (x.object != r.object || x.polarity != r.polarity)
                    && (self.exclusive.contains(&r.predicate)
                        || self.exclusive.contains(&x.predicate))
            })
            .cloned()
            .collect()
    }
    pub fn contradictions(&self) -> Vec<(Relation, Relation)> {
        let mut out = vec![];
        for a in &self.relations {
            for b in self.conflicts_for(a) {
                if a != &b {
                    out.push((a.clone(), b))
                }
            }
        }
        out
    }
    pub fn reachable(&self, start: &str, depth: usize) -> HashSet<String> {
        let mut seen = HashSet::new();
        let mut q = VecDeque::from([(start.to_string(), 0)]);
        while let Some((node, d)) = q.pop_front() {
            if d > depth || !seen.insert(node.clone()) {
                continue;
            }
            for r in self
                .relations
                .iter()
                .filter(|r| r.polarity && r.subject == node)
            {
                q.push_back((r.object.clone(), d + 1));
            }
        }
        seen
    }
}
pub fn extract_relations(text: &str, source: &str) -> Vec<Relation> {
    let n = text.to_lowercase().replace(['.', '!', '?'], "");
    let words = n.split_whitespace().collect::<Vec<_>>();
    if words.len() < 3 {
        return vec![];
    }
    let negative = words
        .iter()
        .any(|w| matches!(*w, "bukan" | "tidak" | "not" | "never"));
    let predicate = if words
        .iter()
        .any(|w| matches!(*w, "adalah" | "ialah" | "is"))
    {
        "is"
    } else if words
        .iter()
        .any(|w| matches!(*w, "memiliki" | "punya" | "has"))
    {
        "has"
    } else if words.iter().any(|w| matches!(*w, "di" | "in")) {
        "located_in"
    } else {
        "related_to"
    };
    let pivot = words
        .iter()
        .position(|w| {
            *w == "adalah"
                || *w == "ialah"
                || *w == "is"
                || *w == "memiliki"
                || *w == "punya"
                || *w == "has"
                || *w == "di"
                || *w == "in"
        })
        .unwrap_or(1);
    let subject = words[..pivot].join(" ");
    let object = words[pivot + 1..].join(" ");
    vec![Relation {
        subject,
        predicate: predicate.into(),
        object,
        polarity: !negative,
        confidence: 70,
        source: source.into(),
    }]
}
#[derive(Debug, Clone)]
pub struct ContradictionChecker {
    pub graph: KnowledgeGraph,
}
impl ContradictionChecker {
    pub fn check(&self, r: &Relation) -> bool {
        !self.graph.conflicts_for(r).is_empty()
    }
    pub fn conflicts(&self) -> Vec<(Relation, Relation)> {
        self.graph.contradictions()
    }
}
