use std::env;
use std::process::{Child, Command};

use rdaw_core::sync::spsc::IpcChannel;

enum ProgramKind {
    SenderServer,
    ReceiverClient,
    ReceiverServer,
    SenderClient,
}

impl ProgramKind {
    fn as_str(&self) -> &'static str {
        match self {
            ProgramKind::SenderServer => "SenderServer",
            ProgramKind::ReceiverClient => "ReceiverClient",
            ProgramKind::ReceiverServer => "ReceiverServer",
            ProgramKind::SenderClient => "SenderClient",
        }
    }

    fn from_str(s: &str) -> Option<ProgramKind> {
        Some(match s {
            "SenderServer" => ProgramKind::SenderServer,
            "ReceiverClient" => ProgramKind::ReceiverClient,
            "ReceiverServer" => ProgramKind::ReceiverServer,
            "SenderClient" => ProgramKind::SenderClient,
            _ => return None,
        })
    }
}

fn spawn(kind: ProgramKind) -> Child {
    let exe = env::current_exe().unwrap();
    Command::new(exe).arg(kind.as_str()).spawn().unwrap()
}

fn spawn_env(kind: ProgramKind, key: &str, value: &str) -> Child {
    let exe = env::current_exe().unwrap();
    Command::new(exe)
        .arg(kind.as_str())
        .env(key, value)
        .spawn()
        .unwrap()
}

fn wait(mut child: Child) {
    assert!(child.wait().unwrap().success());
}

fn main_default() {
    println!();

    println!("running sender server");
    wait(spawn(ProgramKind::SenderServer));

    println!("running receiver server");
    wait(spawn(ProgramKind::ReceiverServer));

    println!();
}

fn main_sender_server() {
    let channel = IpcChannel::<u8>::create("test", 128).unwrap();
    let child = spawn_env(ProgramKind::ReceiverClient, "CHANNEL_ID", channel.id());
    let mut sender = channel.sender().unwrap();

    assert_eq!(sender.send(1), Ok(()));
    assert_eq!(sender.send(2), Ok(()));
    assert_eq!(sender.send(3), Ok(()));
    assert_eq!(sender.send(4), Ok(()));

    wait(child);
}

fn main_receiver_client() {
    let id = env::var("CHANNEL_ID").unwrap();
    let mut receiver = unsafe { IpcChannel::<u8>::open(&id) }
        .unwrap()
        .receiver()
        .unwrap();

    assert_eq!(receiver.recv(), Ok(1));
    assert_eq!(receiver.recv(), Ok(2));
    assert_eq!(receiver.recv(), Ok(3));
    assert_eq!(receiver.recv(), Ok(4));
}

fn main_receiver_server() {
    let channel = IpcChannel::<u8>::create("test", 128).unwrap();
    let child = spawn_env(ProgramKind::SenderClient, "CHANNEL_ID", channel.id());
    let mut receiver = channel.receiver().unwrap();

    assert_eq!(receiver.recv(), Ok(1));
    assert_eq!(receiver.recv(), Ok(2));
    assert_eq!(receiver.recv(), Ok(3));
    assert_eq!(receiver.recv(), Ok(4));

    wait(child);
}

fn main_sender_client() {
    let id = env::var("CHANNEL_ID").unwrap();
    let mut sender = unsafe { IpcChannel::<u8>::open(&id) }
        .unwrap()
        .sender()
        .unwrap();

    assert_eq!(sender.send(1), Ok(()));
    assert_eq!(sender.send(2), Ok(()));
    assert_eq!(sender.send(3), Ok(()));
    assert_eq!(sender.send(4), Ok(()));
}

fn main() {
    let Some(kind) = env::args().nth(1).and_then(|s| ProgramKind::from_str(&s)) else {
        return main_default();
    };

    match kind {
        ProgramKind::SenderServer => main_sender_server(),
        ProgramKind::ReceiverClient => main_receiver_client(),
        ProgramKind::ReceiverServer => main_receiver_server(),
        ProgramKind::SenderClient => main_sender_client(),
    }
}
