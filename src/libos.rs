use crate::{
    engine::{
        Engine,
        Protocol,
    },
    fail::Fail,
    file_table::FileDescriptor,
    interop::{
        dmtr_qresult_t,
        dmtr_sgarray_t,
    },
    protocols::ipv4::Endpoint,
    runtime::Runtime,
    scheduler::{
        Operation,
        SchedulerHandle,
    },
    operations::OperationResult,
};
use must_let::must_let;
use libc::c_int;
use std::{
    time::Instant,
};

const TIMER_RESOLUTION: usize = 64;
const MAX_RECV_ITERS: usize = 2;

pub type QToken = u64;

pub struct LibOS<RT: Runtime> {
    engine: Engine<RT>,
    rt: RT,

    ts_iters: usize,
}

impl<RT: Runtime> LibOS<RT> {
    pub fn new(rt: RT) -> Result<Self, Fail> {
        let engine = Engine::new(rt.clone())?;
        Ok(Self {
            engine,
            rt,
            ts_iters: 0,
        })
    }

    pub fn rt(&self) -> &RT {
        &self.rt
    }

    pub fn socket(
        &mut self,
        domain: c_int,
        socket_type: c_int,
        _protocol: c_int,
    ) -> Result<FileDescriptor, Fail> {
        if domain != libc::AF_INET {
            return Err(Fail::Invalid {
                details: "Invalid domain",
            });
        }
        let engine_protocol = match socket_type {
            libc::SOCK_STREAM => Protocol::Tcp,
            libc::SOCK_DGRAM => Protocol::Udp,
            _ => {
                return Err(Fail::Invalid {
                    details: "Invalid socket type",
                })
            },
        };
        Ok(self.engine.socket(engine_protocol))
    }

    ///
    /// **Brief**
    ///
    /// Binds the socket referred to by `fd` to the local endpoint specified by
    /// `local`.
    ///
    /// **Return Value**
    ///
    /// Upon successful completion, `Ok()` is returned. Upon failure, `Fail` is
    /// returned instead.
    ///
    pub fn bind(&mut self, fd: FileDescriptor, local: Endpoint) -> Result<(), Fail> {
        trace!("bind(): fd={:?} local={:?}", fd, local);
        self.engine.bind(fd, local)
    }

    ///
    /// **Brief**
    ///
    /// Marks the socket referred to by `fd` as a socket that will be used to
    /// accept incoming connection requests using [accept]. The `fd` should
    /// refer to a socket of type `SOCK_STREAM`. The `backlog` argument defines
    /// the maximum length to which the queue of pending connections for `fd`
    /// may grow. If a connection request arrives when the queue is full, the
    /// client may receive an error with an indication that the connection was
    /// refused.
    ///
    /// **Return Value**
    ///
    /// Upon successful completion, `Ok()` is returned. Upon failure, `Fail` is
    /// returned instead.
    ///
    pub fn listen(&mut self, fd: FileDescriptor, backlog: usize) -> Result<(), Fail> {
        trace!{"listen(): fd={:?} backlog={:?}", fd, backlog}
        self.engine.listen(fd, backlog)
    }

    ///
    /// **Brief**
    ///
    /// Accepts an incoming connection request on the queue of pending
    /// connections for the listening socket referred to by `fd`.
    ///
    /// **Return Value**
    ///
    /// Upon successful completion, a queue token is returned. This token can be
    /// used to wait for a connection request to arrive. Upon failure, `Fail` is
    /// returned instead.
    ///
    pub fn accept(&mut self, fd: FileDescriptor) -> Result<QToken, Fail> {
        trace!("accept(): {:?}", fd);
        let future = self.engine.accept(fd)?;
        Ok(self.rt.scheduler().insert(future).into_raw())
    }

    ///
    /// **Brief**
    ///
    /// Connects the socket referred to by `fd` to the remote endpoint specified by `remote`.
    ///
    /// **Return Value**
    ///
    /// Upon successful completion, a queue token is returned. This token can be
    /// used to push and pop data to/from the queue that connects the local and
    /// remote endpoints. Upon failure, `Fail` is
    /// returned instead.
    ///
    pub fn connect(&mut self, fd: FileDescriptor, remote: Endpoint) -> Result<QToken, Fail> {
        trace!("connect(): fd={:?} remote={:?}", fd, remote);
        let future = self.engine.connect(fd, remote)?;
        Ok(self.rt.scheduler().insert(future).into_raw())
    }

    ///
    /// **Brief**
    ///
    /// Closes a connection referred to by `fd`.
    ///
    /// **Return Value**
    ///
    /// Upon successful completion, `Ok()` is returned. Upon failure, `Fail` is
    /// returned instead.
    ///
    pub fn close(&mut self, fd: FileDescriptor) -> Result<(), Fail> {
        trace!("close(): fd={:?}", fd);
        self.engine.close(fd)
    }

    pub fn push(&mut self, fd: FileDescriptor, sga: &dmtr_sgarray_t) -> Result<QToken, Fail> {
        trace!("push(): fd={:?}", fd);
        let buf = self.rt.clone_sgarray(sga);
        let future = self.engine.push(fd, buf)?;
        Ok(self.rt.scheduler().insert(future).into_raw())
    }

    pub fn push2(&mut self, fd: FileDescriptor, buf: RT::Buf) -> Result<QToken, Fail> {
        trace!("push2(): fd={:?}", fd);
        let future = self.engine.push(fd, buf)?;
        Ok(self.rt.scheduler().insert(future).into_raw())
    }

    pub fn pushto(&mut self, fd: FileDescriptor, sga: &dmtr_sgarray_t, to: Endpoint) -> Result<QToken, Fail> {
        let buf = self.rt.clone_sgarray(sga);
        let future = self.engine.pushto(fd, buf, to)?;
        Ok(self.rt.scheduler().insert(future).into_raw())
    }

    pub fn pushto2(&mut self, fd: FileDescriptor, buf: RT::Buf, to: Endpoint) -> Result<QToken, Fail> {
        let future = self.engine.pushto(fd, buf, to)?;
        Ok(self.rt.scheduler().insert(future).into_raw())
    }

    ///
    /// **Brief**
    ///
    /// Invalidates the queue token referred to by `qt`. Any operations on this
    /// operations will fail.
    ///
    pub fn drop_qtoken(&mut self, qt: QToken) {
        drop(self.rt.scheduler().from_raw_handle(qt).unwrap());
    }

    pub fn pop(&mut self, fd: FileDescriptor) -> Result<QToken, Fail> {
        trace!("pop(): fd={:?}", fd);
        let future = self.engine.pop(fd)?;
        Ok(self.rt.scheduler().insert(future).into_raw())
    }

    // If this returns a result, `qt` is no longer valid.
    pub fn poll(&mut self, qt: QToken) -> Option<dmtr_qresult_t> {
        trace!("poll(): qt={:?}", qt);
        self.poll_bg_work();
        let handle = match self.rt.scheduler().from_raw_handle(qt) {
            None => {
                panic!("Invalid handle {}", qt);
            },
            Some(h) => h,
        };
        if !handle.has_completed() {
            handle.into_raw();
            return None;
        }
        let (qd, r) = self.take_operation(handle);
        Some(dmtr_qresult_t::pack(&self.rt, r, qd, qt))
    }

    pub fn wait(&mut self, qt: QToken) -> dmtr_qresult_t {
        trace!("wait(): qt={:?}", qt);
        let (qd, r) = self.wait2(qt);
        dmtr_qresult_t::pack(&self.rt, r, qd, qt)
    }

    pub fn wait2(&mut self, qt: QToken) -> (FileDescriptor, OperationResult<RT>) {
        trace!("wait2(): qt={:?}", qt);
        let handle = self.rt.scheduler().from_raw_handle(qt).unwrap();
        loop {
            self.poll_bg_work();
            if handle.has_completed() {
                return self.take_operation(handle);
            }
        }

    }

    pub fn wait_all_pushes(&mut self, qts: &mut Vec<QToken>) {
        trace!("wait_all_pushes(): qts={:?}", qts);
        self.poll_bg_work();
        for qt in qts.drain(..) {
            let handle = self.rt.scheduler().from_raw_handle(qt).unwrap();
            assert!(handle.has_completed());
            must_let!(let (_, OperationResult::Push) = self.take_operation(handle));
        }
    }

    pub fn wait_any(&mut self, qts: &[QToken]) -> (usize, dmtr_qresult_t) {
        trace!("wait_any(): qts={:?}", qts);
        loop {
            self.poll_bg_work();
            for (i, &qt) in qts.iter().enumerate() {
                let handle = self.rt.scheduler().from_raw_handle(qt).unwrap();
                if handle.has_completed() {
                    let (qd, r) = self.take_operation(handle);
                    return (i, dmtr_qresult_t::pack(&self.rt, r, qd, qt));
                }
                handle.into_raw();
            }
        }
    }

    pub fn wait_any2(&mut self, qts: &[QToken]) -> (usize, FileDescriptor, OperationResult<RT>) {
        trace!("wait_any2(): qts={:?}", qts);
        loop {
            self.poll_bg_work();
            for (i, &qt) in qts.iter().enumerate() {
                let handle = self.rt.scheduler().from_raw_handle(qt).unwrap();
                if handle.has_completed() {
                    let (qd, r) = self.take_operation(handle);
                    return (i, qd, r);
                }
                handle.into_raw();
            }
        }
    }

    pub fn is_qd_valid(&self, _fd: FileDescriptor) -> bool {
        unimplemented!();
    }

    fn take_operation(&mut self, handle: SchedulerHandle) -> (FileDescriptor, OperationResult<RT>) {
        match self.rt.scheduler().take(handle) {
            Operation::Tcp(f) => f.expect_result(),
            Operation::Udp(f) => f.expect_result(),
            Operation::Background(..) => panic!("Polled background operation"),
        }
    }

    fn poll_bg_work(&mut self) {
        self.rt.scheduler().poll();
        for _ in 0..MAX_RECV_ITERS {
            let batch = self.rt.receive();
            if batch.is_empty() {
                break;
            }
            for pkt in batch {
                if let Err(e) = self.engine.receive(pkt) {
                    warn!("Dropped packet: {:?}", e);
                }
            }
        }
        if self.ts_iters == 0 {
            self.rt.advance_clock(Instant::now());
        }
        self.ts_iters = (self.ts_iters + 1) % TIMER_RESOLUTION;
    }
}
