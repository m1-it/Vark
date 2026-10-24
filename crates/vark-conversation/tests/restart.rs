use std::fs;
use vark_conversation::Conversation;
use vark_memory::EpisodicMemory;
use vark_neurons::LifNetwork;

#[test]
fn facts_survive_restart_and_network_checkpoint() {
    let root = std::env::temp_dir().join(format!("vark_restart_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let memory_path = root.join("memory.jsonl");
    let checkpoint = root.join("network.json");
    {
        let mut c = Conversation::new(EpisodicMemory::open(&memory_path).unwrap(), 64).unwrap();
        c.respond("koreksi: VARK menyimpan fakta secara persisten.")
            .unwrap();
    }
    let restored = Conversation::new(EpisodicMemory::open(&memory_path).unwrap(), 64).unwrap();
    let mut answer = restored;
    assert!(!answer.memory.load_records().unwrap().is_empty());
    let restored_answer = answer
        .respond("Apa VARK menyimpan fakta secara persisten?")
        .unwrap();
    assert!(restored_answer.grounded);
    assert!(restored_answer.text.contains("VARK menyimpan fakta"));
    let network = LifNetwork::new(8, 1.0, 10.0, 1);
    network.save_atomic(&checkpoint).unwrap();
    assert_eq!(LifNetwork::load(&checkpoint).unwrap().neurons.len(), 8);
    fs::remove_dir_all(root).unwrap();
}
