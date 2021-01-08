use std::{
    fs::File,
    io::{BufWriter, Write},
    sync::mpsc::{channel, Receiver, Sender},
};

use crate::{
    emitter::emitter::Emitter,
    protocol::types::{Edge, Element, Entry, NumberOrString, Vertex, ID},
};

const DEFAULT_BUF_SIZE: usize = 64 * 1024;

/// An `Emitter` that emits data to a file, in a new OS thread.
pub struct FileEmitter {
    id: ID,
    entry_sender: Sender<Entry>,
}

impl FileEmitter {
    fn next_id(&mut self) -> ID {
        self.id += 1;
        self.id
    }

    /// Creates and return a new `FileEmitter` and a `Receiver` that should be used
    /// to receive a signal indicating that the emitter has finished emitting all
    /// the data.
    ///
    /// This method spawn a new thread that waits for data to emit until the `end` method 
    /// is called.
    ///
    /// It is the reponsibiliy of the user of this struct to call `end` when there is
    /// no more data to be emitted and then wait for the flush signal.
    pub(crate) fn new(file: File) -> (Self, Receiver<()>) {
        let (signal_sender, signal_receiver) = channel();
        let (entry_sender, entry_receiver) = channel();

        std::thread::spawn(move || {
            Self::run(
                entry_receiver,
                signal_sender,
                BufWriter::with_capacity(DEFAULT_BUF_SIZE, file),
            );
        });

        (
            Self {
                id: 0,
                entry_sender,
            },
            signal_receiver,
        )
    }

    fn run(
        entry_receiver: Receiver<Entry>,
        signal_sender: Sender<()>,
        mut buf_writer: BufWriter<File>,
    ) {
        for entry in entry_receiver {
            let line = serde_json::to_vec(&entry).unwrap();
            buf_writer.write(&line).unwrap();
            buf_writer.write(b"\n").unwrap();
        }

        buf_writer.flush().unwrap();
        signal_sender.send(()).unwrap();
    }
}

impl Emitter for FileEmitter {
    fn emit_vertex<V: Into<Vertex>>(&mut self, v: V) -> u64 {
        let id = self.next_id();
        let entry = Entry {
            id: NumberOrString::Number(id),
            data: Element::Vertex(v.into()),
        };

        self.entry_sender.send(entry).unwrap();

        id
    }

    fn emit_edge<E: Into<Edge>>(&mut self, e: E) -> u64 {
        let id = self.next_id();
        let entry = Entry {
            id: NumberOrString::Number(id),
            data: Element::Edge(e.into()),
        };

        self.entry_sender.send(entry).unwrap();

        id
    }

    fn end(&mut self) {
        // to close the channel we need to take it and drop it
        let mut entry_sender = channel().0;
        std::mem::swap(&mut entry_sender, &mut self.entry_sender);
        drop(entry_sender);
    }
}
