use super::sender::Sender;
use crate::{
    collections::watched::WatchFuture,
    protocols::tcp::SeqNumber,
    runtime::Runtime
};
use std::fmt::Debug;

mod cubic;
mod none;
mod options;
pub use self::{
    cubic::Cubic,
    none::None,
    options::{
        Options,
        OptionValue,
    },
};

pub trait SlowStartCongestionAvoidance<RT: Runtime> {
    fn get_cwnd(&self) -> u32 { u32::MAX }
    fn watch_cwnd(&self) -> (u32,  WatchFuture<'_, u32>) { (u32::MAX, WatchFuture::Pending) }

    // Called immediately before the cwnd check is performed before data is sent
    fn on_cwnd_check_before_send(&self, _sender: &Sender<RT>) {}

    fn on_ack_received(&self, _sender: &Sender<RT>, _ack_seq_no: SeqNumber) {}
    
    // Called immediately before retransmit after RTO
    fn on_rto(&self, _sender: &Sender<RT>) {}

    // Called immediately before a segment is sent for the 1st time
    fn on_send(&self, _sender: &Sender<RT>, _num_sent_bytes: u32) {}
}

pub trait FastRetransmitRecovery<RT: Runtime> where Self: SlowStartCongestionAvoidance<RT> {
    fn get_duplicate_ack_count(&self) -> u32 { 0 }

    fn get_retransmit_now_flag(&self) -> bool { false }
    fn watch_retransmit_now_flag(&self) -> (bool, WatchFuture<'_, bool>) { (false, WatchFuture::Pending) }

    fn on_fast_retransmit(&self, _sender: &Sender<RT>) {}
    fn on_base_seq_no_wraparound(&self, _sender: &Sender<RT>) {}
}

pub trait LimitedTransmit<RT: Runtime> where Self: SlowStartCongestionAvoidance<RT> {
    fn get_limited_transmit_cwnd_increase(&self) -> u32 { 0 }
    fn watch_limited_transmit_cwnd_increase(&self) -> (u32, WatchFuture<'_, u32>) {(0, WatchFuture::Pending) }
} 


pub trait CongestionControl<RT: Runtime>: SlowStartCongestionAvoidance<RT> +
                             FastRetransmitRecovery<RT> +
                             LimitedTransmit<RT> +
                             Debug {
    fn new(mss: usize, seq_no: SeqNumber, options: Option<options::Options>) -> Box<dyn CongestionControl<RT>> where Self: Sized;
}

pub type CongestionControlConstructor<T> = fn(usize, SeqNumber, Option<options::Options>) -> Box<dyn CongestionControl<T>>;
