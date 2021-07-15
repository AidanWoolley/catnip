// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::super::sender::Sender;
use super::{
    CongestionControl, FastRetransmitRecovery, LimitedTransmit, Options,
    SlowStartCongestionAvoidance,
};
use crate::runtime::Runtime;
use crate::{
    collections::watched::{WatchFuture, WatchedValue},
    protocols::tcp::SeqNumber,
};
use std::{
    cell::Cell,
    cmp::{max, min},
    convert::TryInto,
    fmt::Debug,
    num::Wrapping,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct Cubic {
    pub mss: u32, // Just for convenience, otherwise we have `as u32` or `.try_into().unwrap()` scattered everywhere...
    // Slow Start / Congestion Avoidance State
    pub ca_start: Cell<Instant>, // The time we started the current congestion avoidance
    pub cwnd: WatchedValue<u32>, // Congestion window: Maximum number of bytes that may be in flight ot prevent congestion
    pub fast_convergence: bool, // Should we employ the fast convergence algorithm (Only recommended if there are multiple CUBIC streams on the same network, in which case we'll cede capacity to new ones faster)
    pub initial_cwnd: u32, // The initial value of cwnd, which gets used if the connection ever resets
    pub last_send_time: Cell<Instant>, // The moment at which we last sent data
    pub last_congestion_was_rto: Cell<bool>, // A flag for whether the last congestion event was detected by RTO
    pub retransmitted_packets_in_flight: Cell<u32>, // A flag for if there is currently a retransmitted packet in flight
    pub rtt_at_last_send: Cell<Duration>,           // The RTT at the moment we last sent data
    pub ssthresh: Cell<u32>, // The size of cwnd at which we will change from using slow start to congestion avoidance
    pub w_max: Cell<u32>,    // The size of cwnd before the previous congestion event

    // Fast Recovery / Fast Retransmit State
    pub duplicate_ack_count: Cell<u32>, // The number of consecutive duplicate ACKs we've received
    pub fast_retransmit_now: WatchedValue<bool>, // Flag to cause the retransmitter to retransmit a segment now
    pub in_fast_recovery: Cell<bool>, // Are we currently in the `fast recovery` algorithm
    pub prev_ack_seq_no: Cell<SeqNumber>, // The previous highest ACK sequence number
    pub recover: Cell<SeqNumber>, // If we receive dup ACKs with sequence numbers greater than this we'll attempt fast recovery

    pub limited_transmit_cwnd_increase: WatchedValue<u32>, // The amount by which cwnd should be increased due to the limited transit algorithm
}

impl<RT: Runtime> CongestionControl<RT> for Cubic {
    fn new(
        mss: usize,
        seq_no: SeqNumber,
        options: Option<Options>,
    ) -> Box<dyn CongestionControl<RT>> {
        let mss: u32 = mss.try_into().unwrap();
        // The initial value of cwnd is set according to RFC5681, section 3.1, page 7
        let initial_cwnd = match mss {
            0..=1095 => 4 * mss,
            1096..=2190 => 3 * mss,
            _ => 2 * mss,
        };

        let options: Options = options.unwrap_or_default();
        let fast_convergence = options.get_bool("fast_convergence").unwrap_or(true);

        Box::new(Self {
            mss,
            // Slow Start / Congestion Avoidance State
            ca_start: Cell::new(Instant::now()), // record the start time of the congestion avoidance period
            cwnd: WatchedValue::new(initial_cwnd),
            fast_convergence,
            initial_cwnd,
            last_send_time: Cell::new(Instant::now()),
            retransmitted_packets_in_flight: Cell::new(0),
            rtt_at_last_send: Cell::new(Duration::new(1, 0)), // The default RTT is 1 sec
            ssthresh: Cell::new(u32::MAX), // According to RFC5681 ssthresh should be initialised 'arbitrarily high'
            w_max: Cell::new(0), // Because ssthresh is u32::MAX, this will be set appropriately during the 1st congestion event
            last_congestion_was_rto: Cell::new(false),

            in_fast_recovery: Cell::new(false),
            fast_retransmit_now: WatchedValue::new(false),
            recover: Cell::new(seq_no), // Recover set to initial send sequence number according to RFC6582
            prev_ack_seq_no: Cell::new(seq_no), // RFC6582 doesn't specify the initial value, but this seems sensible
            duplicate_ack_count: Cell::new(0),

            limited_transmit_cwnd_increase: WatchedValue::new(0),
        })
    }
}

impl Cubic {
    // Cubic const parameters
    const C: f32 = 0.4;
    const BETA_CUBIC: f32 = 0.7;

    const DUP_ACK_THRESHOLD: u32 = 3;

    fn fast_convergence(&self) {
        // The fast convergence algorithm assumes that w_max and cwnd are stored in units of mss, so we do this
        // integer division to prevent it being applied too often
        let cwnd = self.cwnd.get();

        if (cwnd / self.mss) < self.w_max.get() / self.mss {
            self.w_max
                .set((cwnd as f32 * (1. + Self::BETA_CUBIC) / 2.) as u32);
        } else {
            self.w_max.set(cwnd);
        }
    }

    fn increment_dup_ack_count(&self) -> u32 {
        let duplicate_ack_count = self.duplicate_ack_count.get() + 1;
        self.duplicate_ack_count.set(duplicate_ack_count);
        if duplicate_ack_count < Self::DUP_ACK_THRESHOLD {
            self.limited_transmit_cwnd_increase
                .modify(|ltci| ltci + self.mss);
        }
        duplicate_ack_count
    }

    fn on_dup_ack_received<RT: Runtime>(&self, sender: &Sender<RT>, ack_seq_no: SeqNumber) {
        // Get and increment the duplicate ACK count, and store the updated value
        let duplicate_ack_count = self.increment_dup_ack_count();

        let prev_ack_seq_no = self.prev_ack_seq_no.get();
        let ack_seq_no_diff = if ack_seq_no > prev_ack_seq_no {
            (ack_seq_no - prev_ack_seq_no).0
        } else {
            // Handle the case where the current ack_seq_no has wrapped and the previous hasn't
            (prev_ack_seq_no - ack_seq_no).0
        };
        let cwnd = self.cwnd.get();
        let ack_covers_recover = ack_seq_no - Wrapping(1) > self.recover.get();
        let retransmitted_packet_dropped_heuristic =
            cwnd > self.mss && ack_seq_no_diff as u32 <= 4 * self.mss;

        if duplicate_ack_count == Self::DUP_ACK_THRESHOLD
            && (ack_covers_recover || retransmitted_packet_dropped_heuristic)
        {
            // Check against recover specified in RFC6582
            self.in_fast_recovery.set(true);
            self.recover.set(sender.sent_seq_no.get());
            let reduced_cwnd = (cwnd as f32 * Self::BETA_CUBIC) as u32;

            if self.fast_convergence {
                self.fast_convergence();
            } else {
                self.w_max.set(cwnd);
            }
            self.ssthresh.set(max(reduced_cwnd, 2 * self.mss));
            self.cwnd.set(reduced_cwnd);
            self.fast_retransmit_now.set(true);
            // We don't reset ca_start here even though cwnd has been shrunk because we aren't going
            // straight back into congestion avoidance.
        } else if duplicate_ack_count > Self::DUP_ACK_THRESHOLD || self.in_fast_recovery.get() {
            self.cwnd.modify(|c| c + self.mss);
        }
    }

    fn on_ack_received_fast_recovery<RT: Runtime>(
        &self,
        sender: &Sender<RT>,
        ack_seq_no: SeqNumber,
    ) {
        let bytes_outstanding = sender.sent_seq_no.get() - sender.base_seq_no.get();
        let bytes_acknowledged = ack_seq_no - sender.base_seq_no.get();
        let mss = self.mss;

        if ack_seq_no > self.recover.get() {
            // Full acknowledgement
            self.cwnd.set(min(
                self.ssthresh.get(),
                max(bytes_outstanding.0, mss) + mss,
            ));
            // Record the time we go back into congestion avoidance
            self.ca_start.set(Instant::now());
            // Record that we didn't enter CA from a timeout
            self.last_congestion_was_rto.set(false);
            self.in_fast_recovery.set(false);
        } else {
            // Partial acknowledgement
            self.fast_retransmit_now.set(true);
            if bytes_acknowledged.0 >= mss {
                self.cwnd.modify(|c| c - bytes_acknowledged.0 + mss);
            } else {
                self.cwnd.modify(|c| c - bytes_acknowledged.0);
            }
            // We stay in fast recovery mode here because we haven't acknowledged all data up to `recovery`
            // Thus, we don't reset ca_start here either.
        }
    }

    fn k(&self, w_max: f32) -> f32 {
        // While we store w_max in terms of bytes, we have pre-normalised it to units of MSS
        // for compatibility with RFC8312
        if self.last_congestion_was_rto.get() {
            0.0
        } else {
            (w_max * (1. - Self::BETA_CUBIC) / Self::C).cbrt()
        }
    }

    fn w_cubic(&self, w_max: f32, t: f32, k: f32) -> f32 {
        // While we store w_max in terms of bytes, we have pre-normalised it to units of MSS
        // for compatibility with RFC8312
        (Self::C) * (t - k).powi(3) + w_max
    }

    fn w_est(&self, w_max: f32, t: f32, rtt: f32) -> f32 {
        // While we store w_max in terms of bytes, we have pre-normalised it to units of MSS
        // for compatibility with RFC8312
        let bc = Self::BETA_CUBIC;
        w_max * bc + ((3. * (1. - bc) / (1. + bc)) * t / rtt)
    }

    fn on_ack_received_ss_ca<RT: Runtime>(&self, sender: &Sender<RT>, ack_seq_no: SeqNumber) {
        let bytes_acknowledged = ack_seq_no - sender.base_seq_no.get();
        let mss = self.mss;
        let cwnd = self.cwnd.get();
        let ssthresh = self.ssthresh.get();

        if cwnd < ssthresh {
            // Slow start
            self.cwnd.modify(|c| c + min(bytes_acknowledged.0, mss));
        } else {
            // Congestion avoidance
            let t = self.ca_start.get().elapsed().as_secs_f32();
            let rtt = sender.current_rto().as_secs_f32();
            let mss_f32 = mss as f32;
            let normalised_w_max = self.w_max.get() as f32 / mss_f32;
            let k = self.k(normalised_w_max);
            let w_est = self.w_est(normalised_w_max, t, rtt);
            if self.w_cubic(normalised_w_max, t, k) < w_est {
                // w_est return units of MSS which we multiply back up to get bytes
                self.cwnd.set((w_est * mss_f32) as u32);
            } else {
                let cwnd_f32 = cwnd as f32;
                // Again, do everythin in terms of units of MSS
                let normalised_cwnd = cwnd_f32 / mss_f32;
                let cwnd_inc = ((self.w_cubic(normalised_w_max, t + rtt, k) - normalised_cwnd)
                    / normalised_cwnd)
                    * mss_f32;
                self.cwnd.modify(|c| c + cwnd_inc as u32);
            }
        }
    }

    fn on_rto_ss_ca(&self) {
        let cwnd = self.cwnd.get();

        if self.fast_convergence {
            self.fast_convergence();
        } else {
            self.w_max.set(cwnd);
        }
        self.cwnd.set(self.mss);

        let rpif = self.retransmitted_packets_in_flight.get();
        if rpif == 0 {
            // If we lost a retransmitted packet, we don't shrink ssthresh.
            // So we have to check if a retransmitted packet was in flight before we shrink it.
            self.ssthresh
                .set(max((cwnd as f32 * Self::BETA_CUBIC) as u32, 2 * self.mss));
        }

        // Used to decide whether to shrink ssthresh on rto
        // We're just about to retransmit a packet, so increment the counter
        self.retransmitted_packets_in_flight.set(rpif + 1);

        // Used to decide whether to set K to 0 for w_cubic
        self.last_congestion_was_rto.set(true);
    }

    fn on_rto_fast_recovery<RT: Runtime>(&self, sender: &Sender<RT>) {
        // Exit fast recovery/retransmit
        self.recover.set(sender.sent_seq_no.get());
        self.in_fast_recovery.set(false);
    }
}

impl<RT: Runtime> SlowStartCongestionAvoidance<RT> for Cubic {
    fn get_cwnd(&self) -> u32 {
        self.cwnd.get()
    }
    fn watch_cwnd(&self) -> (u32, WatchFuture<'_, u32>) {
        self.cwnd.watch()
    }

    fn on_cwnd_check_before_send(&self, _sender: &Sender<RT>) {
        let long_time_since_send =
            Instant::now().duration_since(self.last_send_time.get()) > self.rtt_at_last_send.get();
        if long_time_since_send {
            let restart_window = min(self.initial_cwnd, self.cwnd.get());
            self.cwnd.set(restart_window);
            self.limited_transmit_cwnd_increase.set_without_notify(0);
        }
    }

    fn on_send(&self, sender: &Sender<RT>, num_bytes_sent: u32) {
        self.last_send_time.set(Instant::now());
        self.rtt_at_last_send.set(sender.current_rto());
        self.limited_transmit_cwnd_increase.set_without_notify(
            self.limited_transmit_cwnd_increase
                .get()
                .saturating_sub(num_bytes_sent),
        );
    }

    fn on_ack_received(&self, sender: &Sender<RT>, ack_seq_no: SeqNumber) {
        let bytes_acknowledged = ack_seq_no - sender.base_seq_no.get();
        if bytes_acknowledged.0 == 0 {
            // ACK is a duplicate
            self.on_dup_ack_received(sender, ack_seq_no);
            // We attempt to keep track of the number of retransmitted packets in flight because we do not alter
            // ssthresh if a packet is lost when it has been retransmitted. There is almost certainly a better way.
            self.retransmitted_packets_in_flight
                .set(self.retransmitted_packets_in_flight.get().saturating_sub(1));
        } else {
            self.duplicate_ack_count.set(0);

            if self.in_fast_recovery.get() {
                // Fast Recovery response to new data
                self.on_ack_received_fast_recovery(sender, ack_seq_no);
            } else {
                self.on_ack_received_ss_ca(sender, ack_seq_no);
            }
            // Used to handle dup ACKs after timeout
            self.prev_ack_seq_no.set(ack_seq_no);
        }
    }

    fn on_rto(&self, sender: &Sender<RT>) {
        // Handle timeout for any of the algorithms we could currently be using
        self.on_rto_ss_ca();
        self.on_rto_fast_recovery(sender);
    }
}

impl<RT: Runtime> FastRetransmitRecovery<RT> for Cubic {
    fn get_duplicate_ack_count(&self) -> u32 {
        self.duplicate_ack_count.get()
    }

    fn get_retransmit_now_flag(&self) -> bool {
        self.fast_retransmit_now.get()
    }
    fn watch_retransmit_now_flag(&self) -> (bool, WatchFuture<'_, bool>) {
        self.fast_retransmit_now.watch()
    }

    fn on_fast_retransmit(&self, _sender: &Sender<RT>) {
        // NOTE: Could we potentially miss FastRetransmit requests with just a flag?
        // I suspect it doesn't matter because we only retransmit on the 3rd repeat ACK precisely...
        // I should really use some other mechanism here just because it would be nicer...
        self.fast_retransmit_now.set_without_notify(false);
    }

    fn on_base_seq_no_wraparound(&self, _sender: &Sender<RT>) {
        // This still won't let us enter fast recovery if base_seq_no wraps to precisely 0, but there's nothing to be done in that case.
        self.recover.set(Wrapping(0));
    }
}

impl<RT: Runtime> LimitedTransmit<RT> for Cubic {
    fn get_limited_transmit_cwnd_increase(&self) -> u32 {
        self.limited_transmit_cwnd_increase.get()
    }
    fn watch_limited_transmit_cwnd_increase(&self) -> (u32, WatchFuture<'_, u32>) {
        self.limited_transmit_cwnd_increase.watch()
    }
}
