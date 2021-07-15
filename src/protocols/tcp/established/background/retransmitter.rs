use super::super::state::ControlBlock;
use crate::{fail::Fail, runtime::Runtime};
use futures::{
    future::{self, Either},
    FutureExt,
};
use std::rc::Rc;

pub enum RetransmitCause {
    TimeOut,
    FastRetransmit,
}

pub async fn retransmit<RT: Runtime>(
    cause: RetransmitCause,
    cb: &Rc<ControlBlock<RT>>,
    ) -> Result<(), Fail> {
    // Our retransmission timer fired, so we need to resend a packet.
    let remote_link_addr = cb.arp.query(cb.remote.address()).await?;

    let mut unacked_queue = cb.sender.unacked_queue.borrow_mut();
    let mut rto = cb.sender.rto.borrow_mut();

    let seq_no = cb.sender.base_seq_no.get();
    let segment = match unacked_queue.front_mut() {
        Some(s) => s,
        None => panic!("Retransmission timer set with empty acknowledge queue"),
    };

    // TODO: Repacketization

    // NOTE: Congestion Control Don't think we record a failure on Fast Retransmit, but can't find a definitive source.
    match cause {
        RetransmitCause::TimeOut => rto.record_failure(),
        RetransmitCause::FastRetransmit => (),
    };

    // Unset the initial timestamp so we don't use this for RTT estimation.
    segment.initial_tx.take();

    let mut header = cb.tcp_header();
    header.seq_num = seq_no;
    cb.emit(header, segment.bytes.clone(), remote_link_addr);

    // Set new retransmit deadline
    let deadline = cb.rt.now() + rto.estimate();
    cb.sender.retransmit_deadline.set(Some(deadline));
    Ok(())
}

pub async fn retransmitter<RT: Runtime>(cb: Rc<ControlBlock<RT>>) -> Result<!, Fail> {
    loop {
        let (rtx_deadline, rtx_deadline_changed) = cb.sender.retransmit_deadline.watch();
        futures::pin_mut!(rtx_deadline_changed);

        // I assume any change to the fast retransmit flag is an instruction to transmit, because I use `set_without_notify` to change it
        // back to false (which I am acutely aware is hack...).
        let (_rtx_fast_retransmit, rtx_fast_retransmit_changed) =
            cb.sender.congestion_ctrl.watch_retransmit_now_flag();
        futures::pin_mut!(rtx_fast_retransmit_changed);

        let rtx_future = match rtx_deadline {
            Some(t) => Either::Left(cb.rt.wait_until(t).fuse()),
            None => Either::Right(future::pending()),
        };
        futures::pin_mut!(rtx_future);
        futures::select_biased! {
            _ = rtx_deadline_changed => continue,
            _ = rtx_future => {
                cb.sender.congestion_ctrl.on_rto(&cb.sender);
                retransmit(RetransmitCause::TimeOut, &cb).await?;
            },
            _ = rtx_fast_retransmit_changed => {
                cb.sender.congestion_ctrl.on_fast_retransmit(&cb.sender);
                retransmit(RetransmitCause::FastRetransmit, &cb).await?;
            }
        }
    }
}
