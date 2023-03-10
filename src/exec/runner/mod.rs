use std::{
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{bail, Context};

use crate::utils::{
    clock::SteadyClock,
    mpsc,
    sync::{ClockSync, OFClockSync},
};

use self::container::ServerContainer;

use super::{
    server::{SendGameServer, ServerKind},
    DEFAULT_RECV_TIMEOUT,
};

pub mod container;

pub enum FromRunnerMsg {
    MoveServer(Option<SendGameServer>),
}
pub enum ToRunnerMsg {
    RequestServer(ServerKind),
    MoveServer(SendGameServer),
    SetFrequency(f64),
    Stop,
}

#[derive(Default)]
pub struct Runner {
    pub container: ServerContainer,
    pub sync: OFClockSync<SteadyClock>,
    pub frequency: f64,
}

impl Runner {
    pub fn run_single(&mut self, is_main_runner: bool) -> anyhow::Result<()> {
        self.container.run_single(is_main_runner, self.frequency)?;
        self.sync.sync(self.frequency);
        Ok(())
    }
}

pub struct ThreadRunner {
    base: Runner,
    sender: mpsc::Sender<FromRunnerMsg>,
    receiver: mpsc::Receiver<ToRunnerMsg>,
}

pub struct ThreadRunnerHandle {
    join_handle: JoinHandle<()>,
    sender: mpsc::Sender<ToRunnerMsg>,
    receiver: mpsc::Receiver<FromRunnerMsg>,
}

impl ThreadRunner {
    fn send(&self, msg: FromRunnerMsg) -> anyhow::Result<()> {
        self.sender
            .send(msg)
            .map_err(|e| anyhow::format_err!("{}", e))
    }

    pub fn run(mut self) {
        loop {
            let pending_msgs = self
                .receiver
                .try_iter((!self.base.container.does_run()).then_some(DEFAULT_RECV_TIMEOUT))
                .expect("thread runner channel was unexpectedly closed");
            for msg in pending_msgs {
                match msg {
                    ToRunnerMsg::Stop => return,
                    ToRunnerMsg::MoveServer(server) => self
                        .base
                        .container
                        .emplace_server_check(server)
                        .expect("error emplacing server"),
                    ToRunnerMsg::RequestServer(kind) => {
                        let server = self
                            .base
                            .container
                            .take_server(kind)
                            .expect("error taking server");
                        self.send(FromRunnerMsg::MoveServer(server))
                            .expect("thread runner channel was unexpectedly closed");
                    }
                    ToRunnerMsg::SetFrequency(frequency) => self.base.frequency = frequency,
                }
            }

            self.base
                .run_single(false)
                .expect("error while running servers");
        }
    }
}

impl ThreadRunnerHandle {
    pub fn new(id: RunnerId) -> Self {
        let (to_send, to_recv) = mpsc::channels();
        let (from_send, from_recv) = mpsc::channels();
        Self {
            join_handle: thread::Builder::new()
                .name(format!("runner thread {id}"))
                .spawn(move || {
                    ThreadRunner {
                        base: Runner::default(),
                        sender: from_send,
                        receiver: to_recv,
                    }
                    .run()
                })
                .expect("failed to spawn thread"),
            sender: to_send,
            receiver: from_recv,
        }
    }

    fn send(&self, msg: ToRunnerMsg) -> anyhow::Result<()> {
        self.sender
            .send(msg)
            .map_err(|e| anyhow::format_err!("{}", e))
            .context("thread runner channel was unexpectedly closed")
    }

    fn recv(&mut self) -> anyhow::Result<Option<FromRunnerMsg>> {
        self.receiver
            .recv_timeout(DEFAULT_RECV_TIMEOUT)
            .context("thread runner channel was unexpectedly closed")
    }

    pub fn stop(&self) -> anyhow::Result<()> {
        self.send(ToRunnerMsg::Stop)
    }

    pub fn join(self) -> bool {
        self.join_handle.join().is_err()
    }

    pub fn set_frequency(&self, frequency: f64) -> anyhow::Result<()> {
        self.send(ToRunnerMsg::SetFrequency(frequency))
    }
}

pub trait ServerMover {
    fn take_server(&mut self, kind: ServerKind) -> anyhow::Result<Option<SendGameServer>>;
    fn emplace_server(&mut self, server: SendGameServer) -> anyhow::Result<()>;

    fn take_server_check(&mut self, kind: ServerKind) -> anyhow::Result<SendGameServer> {
        self.take_server(kind)?.ok_or_else(|| {
            anyhow::format_err!(
                "{} server not found in container",
                match kind {
                    ServerKind::Audio => "audio",
                    ServerKind::Draw => "draw",
                    ServerKind::Update => "update",
                }
            )
        })
    }

    fn emplace_server_check(&mut self, server: SendGameServer) -> anyhow::Result<()> {
        debug_assert!(
            self.take_server(server.server_kind())
                .context("checking for existing server, expected None, but an error occurred")?
                .is_none(),
            "invalid state: server already existed before emplacement"
        );
        self.emplace_server(server)
    }
}

impl ServerMover for MainRunner {
    fn take_server(&mut self, kind: ServerKind) -> anyhow::Result<Option<SendGameServer>> {
        self.base.container.take_server(kind)
    }

    fn emplace_server(&mut self, server: SendGameServer) -> anyhow::Result<()> {
        self.base.container.emplace_server(server)
    }
}

impl ServerMover for ThreadRunnerHandle {
    #[allow(irrefutable_let_patterns)]
    fn take_server(&mut self, kind: ServerKind) -> anyhow::Result<Option<SendGameServer>> {
        self.send(ToRunnerMsg::RequestServer(kind))
            .context("unable to request server from runner thread")?;
        let sent = Instant::now();
        let mut warn = false;
        if let FromRunnerMsg::MoveServer(server) = loop {
            if let Some(msg) = self
                .recv()
                .context("unable to receive server from runner thread")?
            {
                break msg;
            }

            if sent.elapsed() > Duration::from_secs(100) && !warn {
                warn = true;
                tracing::warn!("taking server taking an unexpectedly long amount of time...");
            }
        } {
            Ok(server)
        } else {
            bail!("invalid thread runner response")
        }
    }

    fn emplace_server(&mut self, server: SendGameServer) -> anyhow::Result<()> {
        self.send(ToRunnerMsg::MoveServer(server))
    }
}

pub struct MainRunner {
    pub base: Runner,
}

pub type RunnerId = u8;
pub const MAIN_RUNNER_ID: RunnerId = 3;
