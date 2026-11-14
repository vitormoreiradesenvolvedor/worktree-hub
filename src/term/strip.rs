//! The scroll strip of a pane running a full-screen program.
//!
//! Such a program keeps its own history and never says how much of it there
//! is, so the strip works that out by itself: it sends notches of the wheel,
//! fingerprints the rows before and after, and learns both how far one notch
//! goes and where the ends are. That is what lets one drag of the handle cover
//! the whole history instead of nudging it a screen at a time.

use std::time::{Duration, Instant};

use egui::Rect;

/// What the screen looked like before a batch of notches went out, so the
/// answer can be measured against it.
pub struct ScrollProbe {
    pub before: Vec<u64>,
    /// Notches sent, counted positive toward the top.
    pub notches: i32,
    /// When it went out, since a program answers on its own time.
    pub sent: Instant,
    /// Where the strip believed the view stood when the batch left.
    pub pos_before: f32,
}

/// How long a program is given to answer before an unchanged screen is taken
/// as its word that there is nothing left that way. Programs that redraw a
/// whole conversation take their time, and calling an end too early would nail
/// the strip to a place it could never leave.
const PATIENCE: Duration = Duration::from_millis(500);

/// How long one batch holds the floor when its answer cannot be read.
const BEAT: Duration = Duration::from_millis(120);

/// Notches in one batch sent to a program whose scrolling cannot be read.
/// Such a program is counted in batches rather than in lines — a batch is the
/// one unit it answers in — so this size is fixed for as long as a pane
/// lives: change it and every place already counted would mean something
/// else. Small enough to be one step of scrolling, big enough to be seen.
const STEP: i32 = 1;

/// How long a program that cannot be read is given to answer one batch before
/// an unchanged screen is taken as its word. One batch is in flight at a
/// time here, so there is no backlog to wait out.
const REPLY: Duration = Duration::from_millis(700);

/// Steps a drag may be ahead of the text on a program that cannot be read:
/// what has been asked for and not yet seen on the screen. Enough that a
/// slow program does not make the handle crawl, few enough that the text
/// stops when the hand does.
const OWED: f32 = 4.0;

/// How many steps of such a program one screenful of text is taken to be.
/// Nothing can say what a step is worth in lines, but the handle has to show
/// the screen as a slice of the history, and both have to be in the same
/// unit. Four notches move a good part of a screen in the programs this is
/// for, so a screen is a handful of steps.
const SCREEN_STEPS: f32 = 1.0;

/// Most steps one batch may carry, and the most a drag may run ahead of a
/// program still drawing the last one.
const MOST_STEPS: f32 = 32.0;

/// The handle never gets shorter than this, however long the history grows: a
/// handle the mouse cannot catch is no handle.
const MIN_HANDLE: f32 = 28.0;

/// How a program scrolls, when the program is one the strip already knows.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Habit {
    /// It scrolls by lines, this many to a notch of the wheel.
    Lines(f32),
    /// It scrolls by whole blocks of text and redraws everything, so no
    /// reading of a notch means anything: `claude` and its kind.
    Blocks,
}

/// What a program is known to do with a notch of the wheel. Working this out
/// by trial takes a drag or two and gets it wrong on programs that redraw
/// everything, and the name of the program in charge of the pane is right
/// there for the asking — so it is asked.
pub fn habit_of(program: &str) -> Option<Habit> {
    match program {
        // Pagers move a line at a time, and `git` is here because its pager
        // is one of them.
        "less" | "more" | "most" | "man" | "git" | "bat" | "delta" => Some(Habit::Lines(1.0)),
        // Editors move three.
        "vim" | "nvim" | "vi" | "helix" | "hx" | "kak" => Some(Habit::Lines(3.0)),
        // Conversations and the like: a notch is a block of text, and the
        // whole screen is redrawn around it.
        "claude" | "aider" | "codex" | "gemini" => Some(Habit::Blocks),
        _ => None,
    }
}

/// What the strip has worked out about a program's own scrolling.
///
/// Nothing is known at the start — not where the view stands, not how much
/// there is — so everything here is measured. The place is counted in lines
/// from wherever the pane happened to be when it opened, growing downward, and
/// the two ends are learned the moment the program refuses to move.
pub struct ScrollTrack {
    /// Fingerprint of each visible row, kept from the last frame.
    pub rows_hash: Vec<u64>,
    /// A batch of notches waiting to find out what it caused.
    pub probe: Option<ScrollProbe>,
    /// Lines of text one notch of the wheel is worth, as measured.
    gain: f32,
    /// Where the view stands, in lines from that first place.
    pos: f32,
    /// The place where the program stopped going up, once it has said so.
    top: Option<f32>,
    /// The place where it stopped going down.
    bot: Option<f32>,
    /// The highest and lowest places actually measured, which is what the
    /// track spreads over while an end is still unknown.
    seen: (f32, f32),
    /// How many measurements in a row agreed with the reading of a notch. A
    /// gain that has proved itself earns bigger batches; one that has not
    /// keeps them down to a screen, so a wrong guess cannot overshoot far
    /// enough to be dragged back — which is what made the handle fight.
    steady: u8,
    /// When the last batch went out, so a drag keeps a steady pace instead of
    /// waiting on an answer that large jumps can never give.
    beat: Option<Instant>,
    /// Batches in a row whose answer could not be read at all. Some programs
    /// repaint every line as they scroll, so their moves can never be
    /// measured; after a few of those the strip stops waiting for a proof it
    /// will not get and works with the reading it has.
    blind: u8,
    /// Latched once this program has shown that its scrolling cannot be read.
    /// A few of its batches land on a screen that happens to be readable, and
    /// without the latch the strip would swap between map and throttle from
    /// one frame to the next — which is exactly the fight the hand feels. Only
    /// a reading that proves itself twice in a row takes it off again.
    murky: bool,
    /// Readings in a row that disagreed with the one before. A notch is
    /// worth what it is worth; a number that keeps changing means the program
    /// is not moving by notches at all.
    wobble: u8,
    /// Batches in a row that moved nothing at all. A program busy drawing
    /// looks exactly like one that has nothing left, so an end is only
    /// believed when it is said twice.
    stalled: u8,
    /// Frames in a row in which not one row of the screen changed. A program
    /// that has stopped answering is not moving, and a guess that kept
    /// marching on would have the strip measuring history that is not there.
    still: u8,
    /// How far past the end of the known history to reach while the handle is
    /// held against the rail. It doubles each frame, so a history nobody has
    /// walked yet is crossed in a moment instead of a screen at a time.
    push: f32,
    /// True when this program moves by blocks: readable, but never the same
    /// distance twice. Its batches stay small and strictly one at a time.
    blocky: bool,
    /// Steps asked for and not yet seen on the screen, positive toward the
    /// top. They are counted the moment the program draws something, and
    /// struck off when it turns out it had nothing left to draw.
    owed: f32,
    /// Which way the last batch of a drag went, to damp the shiver of a
    /// reversal that is worth less than a screen.
    last_dir: i8,
    /// Where the pointer is holding the handle during a drag. The handle is
    /// drawn there and nowhere else: the program answers in its own time, and
    /// a handle that followed those answers would jitter under the finger.
    pub held_at: Option<f32>,
    /// Pointer position of the last frame of a fine drag.
    pub grab: Option<f32>,
    /// Leftover pixels of a fine drag, so a slow drag still adds up to a line.
    pub rest: f32,
    /// Where on the handle it was taken, as a share of its height: the handle
    /// shrinks as more history turns up, and a grip kept in pixels would slide
    /// off it and keep the pointer from ever reaching the ends.
    hold: f32,
}

impl Default for ScrollTrack {
    fn default() -> Self {
        Self {
            rows_hash: Vec::new(),
            probe: None,
            gain: 1.0,
            pos: 0.0,
            top: None,
            bot: None,
            seen: (0.0, 0.0),
            steady: 0,
            beat: None,
            blind: 0,
            murky: false,
            wobble: 0,
            stalled: 0,
            still: 0,
            push: 0.0,
            blocky: false,
            owed: 0.0,
            last_dir: 0,
            held_at: None,
            grab: None,
            rest: 0.0,
            hold: 0.0,
        }
    }
}

impl ScrollTrack {
    /// Takes what is already known about the program now in charge of the
    /// pane, instead of working it out notch by notch. A program nobody has
    /// heard of is still learned the hard way.
    pub fn taught(&mut self, habit: Option<Habit>) {
        let kept = std::mem::take(&mut self.rows_hash);
        *self = Self {
            rows_hash: kept,
            ..Self::default()
        };
        match habit {
            Some(Habit::Lines(per_notch)) => {
                self.gain = per_notch;
                // Known, so there is nothing to prove: full batches from the
                // first drag.
                self.steady = 3;
                // A pager or an editor opens at the beginning of what it is
                // showing, so one end of the track is known before the first
                // notch is sent.
                self.top = Some(0.0);
            }
            Some(Habit::Blocks) => {
                // It moves a line or four to a notch, never the same number
                // twice, so no single reading of a notch will ever prove
                // itself. What it does do is move the text in a way that can
                // be read off the screen, so every batch is measured and the
                // place comes from the measurement, not from a count of
                // notches. The batches stay small and go out one at a time,
                // which is what keeps each answer attributable to the batch
                // that caused it.
                self.blocky = true;
                self.gain = 2.0;
                // A conversation opens on its newest line: that is one end,
                // and it is known for free.
                self.bot = Some(0.0);
            }
            None => {}
        }
    }

    /// Lines of text one notch of the wheel moves, as last measured. On a
    /// program whose moves cannot be read at all, a notch is simply worth one
    /// step: the track then counts in notches instead of lines, which is the
    /// one unit that stays the same whatever the program does with them.
    pub fn lines_per_notch(&self) -> f32 {
        if self.murky {
            return 1.0;
        }
        self.gain.clamp(0.2, 8.0)
    }

    /// The two ends the track stands for: the measured ones where they are
    /// known, and a screen beyond the view on the side that is not, so that
    /// dragging that way keeps uncovering more until the program says stop.
    fn ends(&self, rows: usize) -> (f32, f32) {
        let reach = if self.murky {
            SCREEN_STEPS
        } else {
            rows.max(1) as f32
        };
        // Where the view has been, measured or believed: a jump too long to
        // be measured still happened, and a track that ignored it would stop
        // offering anywhere left to go.
        let low = self.seen.0.min(self.pos);
        let high = self.seen.1.max(self.pos);
        // An end nobody has reached is not one screen away — it is unknown,
        // and a track that pretended otherwise would put the handle on the
        // rail end with text still to come. The unknown side is given as much
        // room as the history already walked, so there is always visibly more
        // that way until the program itself says there is not.
        let room = (high - low).max(reach);
        let lo = match self.top {
            Some(end) => end.min(low),
            None => low - room,
        };
        let hi = match self.bot {
            Some(end) => end.max(high),
            None => high + room,
        };
        (lo, hi.max(lo + 1.0))
    }

    /// How much history the track spreads over, in lines.
    pub fn span(&self, rows: usize) -> f32 {
        let (lo, hi) = self.ends(rows);
        hi - lo
    }

    /// Remembers that a batch of notches went out: the handle moves with the
    /// drag at once, and the next frames check it against what really happened.
    pub fn note(&mut self, up_notches: i32, rows: usize) {
        self.beat = Some(Instant::now());
        let before = self.pos;
        let reach = rows.max(1) as f32;
        // A guess, and only a guess: a program slow to repaint may not have
        // moved yet, and one against its end never will. What corrects it is
        // the probe: a batch that changed nothing hands the view back to
        // where it was when that batch went out.
        if self.murky {
            // Not one step is counted here. What the notches did is read off
            // the screen when it is drawn, and a program sitting on its end
            // draws nothing — which is the only honest way to tell the two
            // apart on a program that redraws everything it shows.
            self.owed += up_notches as f32 / STEP as f32;
            if self.probe.is_none() && !self.rows_hash.is_empty() {
                self.probe = Some(ScrollProbe {
                    before: self.rows_hash.clone(),
                    notches: up_notches,
                    sent: Instant::now(),
                    pos_before: self.pos,
                });
            }
            return;
        }
        let mut next = if self.murky {
            // Counted in steps of STEP notches. Nothing here can say how far
            // the text went, but a batch that gets answered has moved
            // something, and twice the notches move roughly twice as much —
            // which is all the rail needs to stay in proportion. A batch that
            // is never answered is struck off whole, so none of this can run
            // away from the text.
            before - (up_notches.signum() as f32)
        } else {
            before - up_notches as f32 * self.lines_per_notch()
        };
        // Only an end the program itself named may hold the guess back, and
        // even that one by a screen: an end that is merely unknown must not,
        // or every long drag would come out counted far too short. A program
        // counted in batches gets one batch of slack instead of a screen: it
        // is all that is needed to push an end that was named too soon back
        // out, and anything more is a count running away from the text.
        let slack = if self.murky { 1.0 } else { reach };
        if let Some(top) = self.top {
            next = next.max(top - slack);
        }
        if let Some(bot) = self.bot {
            next = next.min(bot + slack);
        }
        self.pos = next;
        // Where the track has taken the view, so the history it has walked is
        // not forgotten the moment the view comes back from an end: without
        // this the span would shrink under the hand on the way back, and the
        // same place would answer to two different spots on the rail.
        self.seen = (self.seen.0.min(next), self.seen.1.max(next));
        if self.probe.is_none() && !self.rows_hash.is_empty() {
            self.probe = Some(ScrollProbe {
                before: self.rows_hash.clone(),
                notches: up_notches,
                sent: Instant::now(),
                pos_before: before,
            });
        }
    }

    /// Settles the pending batch against this frame's rows: how far it went,
    /// or that it went nowhere because the program is sitting on an end.
    pub fn measure(&mut self) {
        let Some(probe) = self.probe.take() else {
            return;
        };

        if self.murky {
            if fresh_share(&probe.before, &self.rows_hash) >= 0.25 {
                // Text that was not on the screen before: it moved. These
                // programs redraw themselves for all sorts of reasons — a
                // spinner, a hint, a token arriving — and a redraw that
                // brings no new lines with it has moved nothing, whatever it
                // repainted. Counting those is what had the view counted
                // past the newest line while it sat on it.
                //
                // What moved is everything asked for since the last drawing,
                // since a program that took a while to answer answers for
                // the whole pile at once.
                self.pos -= std::mem::take(&mut self.owed);
                self.seen = (self.seen.0.min(self.pos), self.seen.1.max(self.pos));
                self.stalled = 0;
                return;
            }
            if self.still < 12 || probe.sent.elapsed() < REPLY {
                // Still drawing, maybe. Hold the batch and wait; nothing else
                // goes out while it is in flight.
                self.probe = Some(probe);
                return;
            }
            // Not one row changed in all that time: the notches moved
            // nothing, so they are struck off and the view is where it
            // already was.
            self.owed = 0.0;
            self.stalled = self.stalled.saturating_add(1);
            // Once is a program catching its breath; twice is an end.
            if self.stalled < 2 {
                return;
            }
            // A refusal only names an end where the view has actually been:
            // a program busy drawing refuses exactly like one with nothing
            // left, and an end taken from the middle of a history already
            // walked would fold the whole rail into a few steps.
            let at_edge = if probe.notches > 0 {
                self.pos <= self.seen.0 + 1.0
            } else {
                self.pos >= self.seen.1 - 1.0
            };
            if !at_edge {
                return;
            }
            if probe.notches > 0 {
                self.top = Some(self.pos);
                self.seen.0 = self.pos;
                self.seen.1 = self.seen.1.max(self.pos);
            } else {
                self.bot = Some(self.pos);
                self.seen.1 = self.pos;
                self.seen.0 = self.seen.0.min(self.pos);
            }
            return;
        }

        if let Some(shift) = shift_between(&probe.before, &self.rows_hash) {
            if probe.notches != 0 {
                let measured = (shift.abs() as f32 / probe.notches.abs() as f32).clamp(0.2, 8.0);
                let ratio = measured / self.lines_per_notch();
                if (0.75..1.34).contains(&ratio) {
                    // The reading held up: trust it a little more each time.
                    self.steady = self.steady.saturating_add(1).min(3);
                    self.gain = self.gain * 0.7 + measured * 0.3;
                    self.wobble = 0;
                    // A program that was found unreadable does not become
                    // readable halfway through a drag: two readings that
                    // happen to agree are a coincidence these programs throw
                    // up often, and swapping back on one is what had the
                    // handle changing its mind under the hand.
                    if self.steady >= 4 {
                        self.murky = false;
                    }
                } else {
                    self.steady = 0;
                    // Half of the new reading, so one odd frame cannot throw
                    // the feel of the strip off.
                    self.gain = (self.gain + measured) / 2.0;
                    // Readings that keep contradicting each other are a
                    // program that moves by blocks rather than by notches.
                    // It is readable only by accident, and treating it as a
                    // map is what had the handle jumping about.
                    self.wobble = self.wobble.saturating_add(1);
                    if self.wobble >= 4 && self.steady < 2 && !self.blocky {
                        self.murky = true;
                    }
                }
            }
            self.blind = 0;
            self.stalled = 0;
            // A screen that slid up means the view travelled downward.
            self.pos = probe.pos_before + shift as f32;
            self.seen = (self.seen.0.min(self.pos), self.seen.1.max(self.pos));
            // Text can appear beyond an end that was named earlier, and an end
            // named too soon is simply wrong: either way the end moves with
            // the text rather than holding the view back.
            if self.top.is_some_and(|top| self.pos < top) {
                self.top = Some(self.pos);
            }
            if self.bot.is_some_and(|bot| self.pos > bot) {
                self.bot = Some(self.pos);
            }
            return;
        }

        // On a program that cannot be read, a screen that did not change one
        // row after a screenful of notches is a program with nothing left
        // that way. There is no second opinion to wait for here — waiting is
        // what let the count run away from the text — so the batch is struck
        // off at once and the end goes where the view actually is. If the
        // program was merely slow, the next push past the end moves it again
        // and the end moves back out with it.
        // A screen that changed in some way we cannot read — a program that
        // repaints every line, say — says nothing about the move. The guess
        // stands, but the batch still holds the floor for a moment: firing a
        // fresh one every frame would bury the program in notches.
        if !nearly_same(&probe.before, &self.rows_hash) {
            self.blind = self.blind.saturating_add(1);
            if self.blind >= 3 && self.steady < 2 && !self.blocky {
                self.murky = true;
            }
            self.stalled = 0;
            if probe.sent.elapsed() < BEAT {
                self.probe = Some(probe);
            }
            return;
        }

        // Nothing but a status line changed, and the program has had its time:
        // it is sitting
        // on an end, and that is where the strip puts the end of its track.
        if probe.sent.elapsed() >= PATIENCE && self.still >= 6 {
            self.pos = probe.pos_before;
            self.stalled = self.stalled.saturating_add(1);
            // Once is a program catching its breath; twice is an end.
            if self.stalled < 2 {
                return;
            }
            // The reckoning drifts on a long trip, so where the view has been
            // is no longer worth anything on this side: the end the program
            // just named is the only anchor left. An end never comes inward,
            // though — the far side has been reached before, and a drifted
            // reading of it must not shrink the history to nothing.
            // The notches that went nowhere are struck off the record: what
            // the track counted past this point never happened, and a span
            // stretched by it would leave a dead stretch of rail. Only the
            // refusal that names the end says that, though. A program that
            // stops short of an end it has already gone past is catching its
            // breath, and forgetting the history beyond it would move the
            // same text to a different place on the rail.
            if probe.notches > 0 {
                let end = self.top.map_or(self.pos, |end| end.min(self.pos));
                self.top = Some(end);
                if end >= self.pos {
                    self.seen.0 = self.pos;
                }
                self.seen.1 = self.seen.1.max(self.pos);
            } else {
                let end = self.bot.map_or(self.pos, |end| end.max(self.pos));
                self.bot = Some(end);
                if end <= self.pos {
                    self.seen.1 = self.pos;
                }
                self.seen.0 = self.seen.0.min(self.pos);
            }
            return;
        }

        self.probe = Some(probe);
    }

    /// True when the program is known to move by whole blocks of text, like
    /// a conversation that redraws its screen. Nothing honest can be said
    /// about where the view stands in such a history, so the pane shows no
    /// scroll rail for it: the wheel is the only way through, a notch at a
    /// time with Alt held.
    pub fn by_blocks(&self) -> bool {
        self.blocky
    }

    /// True when this program's scrolling cannot be read at all: it repaints
    /// every line, or it moves by whole blocks rather than by lines, so no
    /// reading of a notch can be trusted. `claude` does both. The track then
    /// stops measuring and keeps its own count instead: one notch, one step,
    /// with the ends found by the program refusing to move. The handle still
    /// shows where the view stands and still follows the hand.
    pub fn unsure(&self) -> bool {
        self.murky
    }

    /// How tall the handle is: as big a slice of the track as the screen is
    /// of the history it has found so far.
    fn handle_height(&self, track: Rect, rows: usize) -> f32 {
        let span = self.span(rows);
        let screen = if self.murky {
            SCREEN_STEPS
        } else {
            rows as f32
        };
        let share = screen / (screen + span);
        (track.height() * share).clamp(MIN_HANDLE.min(track.height()), track.height())
    }

    /// Height and middle of the handle on a track: short when the history is
    /// long, and placed where the view stands in it.
    pub fn handle(&self, track: Rect, rows: usize) -> (f32, f32) {
        let height = self.handle_height(track, rows);
        // Whatever the handle is asked to show, it stays on its rail: the
        // height can change under a held handle, and half of it hanging over
        // the edge of the pane would be the first thing the eye catches.
        let rail = |y: f32| {
            y.clamp(
                track.top() + height / 2.0,
                (track.bottom() - height / 2.0).max(track.top() + height / 2.0),
            )
        };
        if let Some(y) = self.held_at {
            return (height, rail(y));
        }
        let travel = (track.height() - height).max(0.0);
        let (lo, hi) = self.ends(rows);
        let from_top = ((self.pos - lo) / (hi - lo)).clamp(0.0, 1.0);
        (height, rail(track.top() + height / 2.0 + from_top * travel))
    }

    /// Takes the grip: grabbing the handle keeps it where it was taken, while
    /// grabbing the bare track picks it up by its middle.
    pub fn grip(&mut self, track: Rect, rows: usize, pointer_y: f32) {
        let (height, middle) = self.handle(track, rows);
        self.hold = if (pointer_y - middle).abs() <= height / 2.0 {
            ((pointer_y - middle) / height.max(1.0)).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        self.rest = 0.0;
        self.grab = Some(pointer_y);
        self.last_dir = 0;
        self.push = 0.0;
    }

    /// Most notches one batch may carry. Until the reading of a notch has
    /// proved itself, a batch moves at most a screen, so the worst a wrong
    /// guess can do is overshoot by a screen instead of sending the text
    /// flying and dragging it back.
    pub fn cap(&self) -> i32 {
        if self.blocky {
            // Small enough that one batch moves a readable amount, which is
            // the only way the place stays measured rather than guessed.
            return 8;
        }
        if self.steady < 2 && self.blind < 3 && !self.murky {
            // Small enough that the move stays inside what a fingerprint can
            // still recognise, which is the only way the reading of a notch
            // ever gets proved.
            return 8;
        }
        if self.murky {
            // The hand sets the pace here: what goes out is what the drag
            // asked for, and the cap is only there so a flick of the wrist
            // cannot bury the program.
            return 600;
        }
        // One size wherever the handle is. What a notch is worth turns out to
        // depend on how thickly the notches arrive, so a trip that crossed
        // the history in big batches and one that crept across in small ones
        // would not agree on where the middle is. The pace, not the size,
        // keeps a program drawing every step of the way.
        600
    }

    /// Takes this frame's rows and counts how long the screen has stood still.
    pub fn mark(&mut self, rows_hash: &[u64]) {
        self.still = if nearly_same(&self.rows_hash, rows_hash) {
            self.still.saturating_add(1)
        } else {
            0
        };
    }

    /// True when the next batch of a drag may go out. One batch a frame is
    /// plenty, and a program that cannot be read still deserves a pace.
    pub fn ready(&self) -> bool {
        // Nothing goes out while a batch is still unanswered, unless the
        // program has already proved what a notch is worth. A guess repeated
        // sixty times a second is what had the count running past the text
        // and the handle landing on a rail end with history still to come.
        if self.murky {
            // A drag may run a few steps ahead of a slow program, never more.
            return self.owed.abs() < OWED;
        }
        if (self.blocky || self.steady < 2) && self.probe.is_some() {
            return false;
        }
        match self.beat {
            // Nothing has gone out yet.
            None => true,
            // One batch at a time: the next waits until the screen has moved
            // for the last one. Pouring notches into a program faster than it
            // can draw them only builds a backlog, and a program busy working
            // through that backlog looks exactly like one that has stopped.
            Some(last) => {
                let waited = last.elapsed();
                (self.still == 0 && waited >= Duration::from_millis(8))
                    || waited >= Duration::from_millis(250)
            }
        }
    }
    /// Follows a drag: the handle is pinned to the pointer and the lines the
    /// view still has to travel to catch up come back, positive toward the top.
    pub fn drag_to(&mut self, track: Rect, rows: usize, pointer_y: f32) -> f32 {
        let (height, _) = self.handle(track, rows);
        let wanted = (pointer_y - self.hold * height)
            .clamp(track.top() + height / 2.0, track.bottom() - height / 2.0);
        self.held_at = Some(wanted);
        if self.murky {
            return self.by_hand(track, rows, pointer_y);
        }
        let mut lines = self.aim(track, rows, pointer_y);
        // Held against an end of the rail, the hand is asking for everything
        // that way; the strip reaches further with every frame it is held.
        match self.rail(track, rows, pointer_y) {
            Some(way) => {
                let reach = rows.max(1) as f32;
                // Reaching further only makes sense while the text is still
                // answering; against a wall it would only pile up notches.
                if self.murky {
                    // Reaching further than a screen here would only pile up
                    // notches the program may never use, and the count would
                    // leave the text behind.
                    self.push = reach;
                } else if self.still < 2 {
                    self.push = (self.push * 2.0).clamp(reach, 4000.0);
                }
                lines += way as f32 * self.push.max(reach);
            }
            None => self.push = 0.0,
        }
        // Inside a notch there is nothing to ask for, and asking anyway is how
        // a handle starts fidgeting.
        if lines.abs() < self.lines_per_notch() {
            return 0.0;
        }
        // Turning back costs a screen: without that, a guess corrected by a
        // late measurement would send the text the other way for one frame
        // and then back again, which is the fight the hand feels.
        let dir = if lines > 0.0 { 1 } else { -1 };
        if self.last_dir != 0 && dir != self.last_dir && lines.abs() < rows as f32 {
            return 0.0;
        }
        self.last_dir = dir;
        lines
    }

    /// How much text one pixel of the rail is worth on a program that cannot
    /// be measured: enough that one drag down the whole rail covers what the
    /// hand has already walked through, and a few screens before it has
    /// walked through anything.
    fn by_pixel(&self, track: Rect, rows: usize) -> f32 {
        let height = self.handle_height(track, rows);
        let travel = (track.height() - height).max(1.0);
        // The span the handle is drawn against, so that a pixel of drag is a
        // pixel of handle: the text keeps step with the hand, and the handle
        // keeps step with the text. Counted in this program's own steps, the
        // whole rail is the whole history and never more than one step per
        // pixel, however little of it has been found so far.
        // Until both ends are known the span is only what has been walked so
        // far, and a rate taken from it would have a drag across the whole
        // rail asking for almost nothing. A rail is worth a few screens at
        // the very least.
        (self.span(rows).max(SCREEN_STEPS * 8.0) / travel).clamp(0.001, 1.0)
    }

    /// Follows a drag on a program that cannot be measured: the text moves by
    /// as much as the hand did and no more, so the speed is always the hand's.
    fn by_hand(&mut self, track: Rect, rows: usize, pointer_y: f32) -> f32 {
        let moved = pointer_y - self.grab.unwrap_or(pointer_y);
        self.grab = Some(pointer_y);
        // What the hand asked for, in steps of this program's own scrolling.
        self.rest += -moved * self.by_pixel(track, rows);
        // Held against an end of the rail, the hand is asking for whatever
        // lies past the history found so far, a step at a time. One step per
        // answered batch is the fastest honest pace there is: these programs
        // take a burst of notches as one scroll, so a bigger batch would be
        // counted as more travel than it caused.
        // A hand holding the handle against the end of the rail is asking for
        // whatever is past the history found so far. The handle is what has
        // to reach the end, not the pointer, so the edge is as wide as half a
        // handle — a few pixels of rail would be a thing nobody could hit.
        let edge = (self.handle_height(track, rows) / 2.0).max(12.0);
        let (lo, hi) = self.ends(rows);
        if pointer_y <= track.top() + edge && self.pos > lo {
            self.rest = self.rest.max(1.0);
        } else if pointer_y >= track.bottom() - edge && self.pos < hi {
            self.rest = self.rest.min(-1.0);
        }
        // A hand can ask faster than a program can draw. What it asks for
        // meanwhile is kept, but only so much: a bigger backlog would go on
        // scrolling after the hand had stopped.
        self.rest = self.rest.clamp(-MOST_STEPS, MOST_STEPS);
        let steps = self.rest.trunc().clamp(-1.0, 1.0);
        if steps == 0.0 {
            return 0.0;
        }
        self.rest -= steps;
        self.last_dir = if steps > 0.0 { 1 } else { -1 };
        steps * STEP as f32
    }

    /// Which end of the rail the pointer is pinned against, if either: `1`
    /// for the top, `-1` for the bottom, in the same signs as the lines.
    fn rail(&self, track: Rect, rows: usize, pointer_y: f32) -> Option<i8> {
        let (height, _) = self.handle(track, rows);
        let travel = (track.height() - height).max(1.0);
        let wanted = pointer_y - self.hold * height;
        let from_top = (wanted - height / 2.0 - track.top()) / travel;
        if from_top <= 0.001 || pointer_y <= track.top() + 2.0 {
            Some(1)
        } else if from_top >= 0.999 || pointer_y >= track.bottom() - 2.0 {
            Some(-1)
        } else {
            None
        }
    }

    /// Lines the view still has to travel for the handle to reach the pointer,
    /// counted positive toward the top.
    pub fn aim(&self, track: Rect, rows: usize, pointer_y: f32) -> f32 {
        let (height, _) = self.handle(track, rows);
        let travel = (track.height() - height).max(1.0);
        let wanted = pointer_y - self.hold * height;
        let from_top = ((wanted - height / 2.0 - track.top()) / travel).clamp(0.0, 1.0);
        let (lo, hi) = self.ends(rows);
        // Held hard against an end of the track, the strip keeps asking for
        // one more screen. If the program really is finished nothing happens,
        // and if an end was named too early this is what takes it back. What
        // counts as the end is where the pointer is, not where the handle got
        // to: the handle always stops half its own height short of the rail.
        let reach = rows.max(1) as f32;
        let target = if from_top <= 0.001 || pointer_y <= track.top() + 2.0 {
            lo - reach
        } else if from_top >= 0.999 || pointer_y >= track.bottom() - 2.0 {
            hi + reach
        } else {
            lo + from_top * (hi - lo)
        };
        self.pos - target
    }
}

/// True when two screens are the same but for a line or two — the clock in a
/// corner, the `(END)` a pager writes at the foot of a file. A program that
/// has stopped scrolling still repaints such lines, and holding out for two
/// identical screens would mean never finding that end at all.
fn nearly_same(before: &[u64], now: &[u64]) -> bool {
    if before.len() != now.len() || before.is_empty() {
        return false;
    }
    let same = before
        .iter()
        .zip(now)
        .filter(|(one, other)| one == other)
        .count();
    same * 10 >= before.len() * 9
}

/// How much of the screen is text that was not there before, as a share of
/// its rows. A program that scrolled brings a good part of a screen with it;
/// one that merely repainted itself brings a line or two at most.
fn fresh_share(before: &[u64], now: &[u64]) -> f32 {
    if now.is_empty() {
        return 0.0;
    }
    let had: std::collections::HashSet<u64> = before.iter().copied().collect();
    let fresh = now
        .iter()
        .filter(|row| **row != 0 && !had.contains(row))
        .count();
    fresh as f32 / now.len() as f32
}

/// How many rows the screen moved between two fingerprints, or `None` when the
/// two do not look like the same text at all — a clock ticking in a corner is
/// fine, a whole new screen is not.
pub fn shift_between(before: &[u64], now: &[u64]) -> Option<i32> {
    if before.len() != now.len() || before.len() < 4 {
        return None;
    }
    let rows = before.len() as i32;
    // A whole screen of travel can still be recognised: programs move three
    // or more lines per notch, and a reach of a couple of lines would never
    // see it, leaving the strip to guess at a number it could have measured.
    let reach = (rows - 2).min(64);
    // How much of the screen is right where it was. A shift only counts as a
    // reading if it explains the screen better than standing still does:
    // text with many lines alike — a boxed prompt, a column of bullets — can
    // line up with itself one row over and tell the strip the view moved when
    // nothing did at all.
    let staying = before
        .iter()
        .zip(now)
        .filter(|&(one, other)| one == other && *one != 0)
        .count();
    let mut best: Option<(i32, usize)> = None;
    for shift in -reach..=reach {
        if shift == 0 {
            continue;
        }
        let mut matched = 0usize;
        let mut compared = 0usize;
        for row in 0..rows {
            let other = row + shift;
            if other < 0 || other >= rows {
                continue;
            }
            // Blank lines match everything, so they say nothing about the move.
            if before[other as usize] == 0 {
                continue;
            }
            compared += 1;
            if now[row as usize] == before[other as usize] {
                matched += 1;
            }
        }
        if compared < 3 || matched * 10 < compared * 7 || matched <= staying {
            continue;
        }
        if best.is_none_or(|(_, seen)| matched > seen) {
            best = Some((shift, matched));
        }
    }
    best.map(|(shift, _)| shift)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, Rect};

    fn track() -> Rect {
        Rect::from_min_max(Pos2::new(0.0, 100.0), Pos2::new(6.0, 300.0))
    }

    /// A strip that has already learned a long history, both ends included.
    fn measured(lines: f32, pos: f32) -> ScrollTrack {
        ScrollTrack {
            top: Some(0.0),
            bot: Some(lines),
            pos,
            ..Default::default()
        }
    }

    #[test]
    fn a_screen_that_slid_up_reports_how_far() {
        let before: Vec<u64> = (1..=20).collect();
        let now: Vec<u64> = (4..=23).collect();
        assert_eq!(shift_between(&before, &now), Some(3));
    }

    #[test]
    fn a_screen_that_slid_down_reports_a_negative_move() {
        let before: Vec<u64> = (10..=29).collect();
        let now: Vec<u64> = (8..=27).collect();
        assert_eq!(shift_between(&before, &now), Some(-2));
    }

    #[test]
    fn a_screen_replaced_by_another_is_not_a_move() {
        let before: Vec<u64> = (1..=20).collect();
        let now: Vec<u64> = (1..=20).map(|n| n * 977 + 13).collect();
        assert_eq!(shift_between(&before, &now), None);
    }

    #[test]
    fn with_nothing_measured_yet_the_handle_waits_in_the_middle() {
        let strip = ScrollTrack::default();
        let (height, middle) = strip.handle(track(), 20);
        assert!((middle - track().center().y).abs() < 0.01);
        // A screen either way is all it dares assume, so the handle is a
        // third of the track: the screen itself plus those two guesses.
        assert!((height - track().height() / 3.0).abs() < 0.01);
    }

    #[test]
    fn a_long_history_leaves_a_short_handle_that_still_reaches_both_ends() {
        let strip = measured(4000.0, 0.0);
        let (height, top) = strip.handle(track(), 20);
        assert_eq!(height, MIN_HANDLE);
        assert!((top - height / 2.0 - track().top()).abs() < 0.01);

        let strip = measured(4000.0, 4000.0);
        let (_, bottom) = strip.handle(track(), 20);
        assert!((bottom + height / 2.0 - track().bottom()).abs() < 0.01);
    }

    #[test]
    fn one_drag_of_the_track_covers_the_whole_history() {
        let strip = measured(1000.0, 1000.0);
        // From the bottom end of the track to the top end, in one gesture.
        let lines = strip.aim(track(), 20, track().top());
        assert!(lines >= 1000.0, "aimed at {lines} lines");
        // Half way up the track is half way through the history.
        let middle = strip.aim(track(), 20, track().center().y);
        assert!((middle - 500.0).abs() < 20.0, "aimed at {middle} lines");
    }

    #[test]
    fn held_against_an_end_it_keeps_asking_for_one_more_screen() {
        // An end named too early would nail the strip in place; asking again
        // costs nothing when the program really has nothing left.
        let strip = measured(1000.0, 0.0);
        assert_eq!(strip.aim(track(), 20, track().top()), 20.0);
    }

    #[test]
    fn an_end_nobody_has_found_yet_stays_a_screen_away() {
        // Dragging that way keeps asking for one more screen until the
        // program says it has nothing left.
        let strip = ScrollTrack {
            top: Some(0.0),
            pos: 0.0,
            ..Default::default()
        };
        let lines = strip.aim(track(), 20, track().bottom());
        assert_eq!(lines, -40.0);
    }

    #[test]
    fn a_program_that_will_not_budge_marks_the_end_of_the_track() {
        let mut strip = ScrollTrack {
            rows_hash: (1..=20).collect(),
            ..Default::default()
        };
        strip.note(5, 20);
        assert!(strip.pos < 0.0);
        // The same screen, frame after frame: still only patience.
        for _ in 0..8 {
            strip.mark(&(1..=20).collect::<Vec<u64>>());
        }
        strip.measure();
        assert!(strip.probe.is_some());
        assert_eq!(strip.top, None);
        // Once the program has had its time, nothing moved means an end —
        // said twice, because a program catching its breath looks the same.
        for _ in 0..2 {
            if let Some(probe) = strip.probe.as_mut() {
                probe.sent = Instant::now() - PATIENCE;
            }
            strip.measure();
            if strip.top.is_none() {
                strip.note(5, 20);
            }
        }
        assert_eq!(strip.top, Some(0.0));
        assert_eq!(strip.pos, 0.0);
        let (height, middle) = strip.handle(track(), 20);
        assert!((middle - height / 2.0 - track().top()).abs() < 0.01);
    }

    #[test]
    fn the_handle_stays_under_the_finger_while_it_is_dragged() {
        let mut strip = measured(4000.0, 4000.0);
        strip.drag_to(track(), 20, track().top() + 40.0);
        let (_, middle) = strip.handle(track(), 20);
        assert!((middle - (track().top() + 40.0)).abs() < 0.01);
        // Whatever the program answers meanwhile, the handle does not jump.
        strip.pos = 2500.0;
        let (_, still) = strip.handle(track(), 20);
        assert_eq!(middle, still);
        // Letting go hands it back to the text.
        strip.held_at = None;
        let (_, after) = strip.handle(track(), 20);
        assert!(after > middle);
    }

    #[test]
    fn a_reading_that_has_not_proved_itself_keeps_batches_short() {
        let mut strip = ScrollTrack::default();
        // Short enough that the move it causes can still be recognised.
        assert_eq!(strip.cap(), 8);
        // Held against the rail it may jump far; anywhere else it stays
        // within a screen, where the move can still be measured.
        strip.steady = 3;
        assert!(strip.cap() > 100);
    }

    #[test]
    fn a_long_guess_is_not_cut_short_by_an_end_nobody_has_found() {
        let mut strip = ScrollTrack {
            rows_hash: (1..=52).collect(),
            ..Default::default()
        };
        strip.note(-600, 52);
        assert_eq!(strip.pos, 600.0);
    }

    #[test]
    fn a_guess_stops_at_an_end_the_program_named() {
        let mut strip = measured(1000.0, 1000.0);
        strip.rows_hash = (1..=52).collect();
        strip.note(-600, 52);
        // A screen of slack past the end, no more.
        assert_eq!(strip.pos, 1052.0);
    }

    #[test]
    fn a_status_line_is_not_a_change_of_screen() {
        let before: Vec<u64> = (1..=20).collect();
        let mut after = before.clone();
        after[19] = 999;
        assert!(nearly_same(&before, &after));
        after[0] = 998;
        after[1] = 997;
        assert!(!nearly_same(&before, &after));
    }

    #[test]
    fn a_program_nobody_can_measure_gets_full_batches() {
        let mut strip = ScrollTrack {
            rows_hash: (1..=20).collect(),
            ..Default::default()
        };
        assert_eq!(strip.cap(), 8);
        for n in 0..3 {
            strip.note(-1, 20);
            // A screen that came back unrecognisable, three times over.
            strip.rows_hash = (1..=20).map(|row| row * 7 + n * 1000).collect();
            strip.measure();
        }
        // The hand sets the pace on such a program, so the batch only has to
        // be big enough to carry whatever the drag asked for.
        assert!(strip.cap() > 100);
    }

    #[test]
    fn a_screen_of_lookalike_rows_that_did_not_move_reads_as_no_move() {
        // A boxed prompt: rows that are each other's twins. Lined up one row
        // over, the screen matches itself almost perfectly, and a reading
        // taken from that would have the view drifting while it sits still.
        let frame: Vec<u64> = (0..20)
            .map(|row| if row % 2 == 0 { 7 } else { 9 })
            .collect();
        assert_eq!(shift_between(&frame, &frame), None);

        // Text that really slid up is still read, twins or not.
        let before: Vec<u64> = (1..=20).collect();
        let now: Vec<u64> = (4..=23).collect();
        assert_eq!(shift_between(&before, &now), Some(3));
    }

    #[test]
    fn an_end_nobody_has_reached_keeps_room_on_the_rail() {
        let mut strip = ScrollTrack {
            bot: Some(0.0),
            pos: -50.0,
            seen: (-50.0, 0.0),
            ..Default::default()
        };
        // The newest line is known and the handle sits on that rail end.
        strip.pos = 0.0;
        let (height, newest) = strip.handle(track(), 20);
        assert!((newest - (track().bottom() - height / 2.0)).abs() < 0.01);

        // As far back as anyone has been, the handle still has rail left:
        // nothing has said this is the beginning.
        strip.pos = -50.0;
        let (height, oldest) = strip.handle(track(), 20);
        assert!(
            oldest - height / 2.0 - track().top() > track().height() * 0.2,
            "the handle went to the rail end with history still unexplored"
        );

        // Once the program refuses to go further, the rail ends there.
        strip.top = Some(-50.0);
        strip.seen.0 = -50.0;
        let (height, beginning) = strip.handle(track(), 20);
        assert!((beginning - (track().top() + height / 2.0)).abs() < 0.01);
    }

    #[test]
    fn a_program_the_strip_knows_needs_no_learning() {
        assert_eq!(habit_of("less"), Some(Habit::Lines(1.0)));
        assert_eq!(habit_of("nvim"), Some(Habit::Lines(3.0)));
        assert_eq!(habit_of("claude"), Some(Habit::Blocks));
        assert_eq!(habit_of("some-program-nobody-wrote-down"), None);

        // A pager: a notch is a line and the strip may say so from the first
        // drag, without the small careful batches of a guess.
        let mut strip = ScrollTrack::default();
        strip.taught(habit_of("less"));
        assert_eq!(strip.lines_per_notch(), 1.0);
        assert!(!strip.unsure());
        assert!(strip.cap() > 100);

        // A conversation: counted in its own steps from the start, so a first
        // drag cannot land anywhere on the strength of a reading that was
        // never going to mean anything.
        let mut strip = ScrollTrack::default();
        strip.taught(habit_of("claude"));
        // Readable, so it is measured like anything else — but in small
        // batches, one at a time, because what a notch is worth changes from
        // one block of text to the next.
        assert!(!strip.unsure());
        assert_eq!(strip.cap(), 8);
        // It opens on its newest line, so that end of the rail is known and
        // the handle starts there instead of guessing at the middle.
        let (height, start) = strip.handle(track(), 20);
        assert!((start - (track().bottom() - height / 2.0)).abs() < 0.01);
        // Readings that disagree are what this program does; they must not
        // talk the strip out of measuring it.
        strip.rows_hash = (1..=20).collect();
        for n in 0..6 {
            strip.note(-1, 20);
            strip.rows_hash = (1..=20).map(|row| row * 7 + n * 100).collect();
            strip.measure();
        }
        assert!(!strip.unsure());

        // And a program it has never heard of is still learned the hard way.
        let mut strip = ScrollTrack::default();
        strip.taught(habit_of("surprise"));
        assert!(!strip.unsure());
        assert_eq!(strip.cap(), 8);
    }

    #[test]
    fn a_program_that_cannot_be_read_is_dragged_by_hand() {
        let mut strip = ScrollTrack {
            rows_hash: (1..=20).collect(),
            ..Default::default()
        };
        for n in 0..3 {
            strip.note(-1, 20);
            strip.rows_hash = (1..=20).map(|row| row * 7 + n * 1000).collect();
            strip.measure();
        }
        assert!(strip.unsure());

        // A history already walked from end to end: twenty steps of this
        // program's own scrolling, with the view on the newest text.
        strip.top = Some(-20.0);
        strip.bot = Some(0.0);
        strip.seen = (-20.0, 0.0);
        strip.pos = 0.0;
        strip.probe = None;
        strip.stalled = 0;

        // A hand that has not moved asks for nothing.
        let (_, start) = strip.handle(track(), 20);
        strip.grip(track(), 20, start);
        assert_eq!(strip.drag_to(track(), 20, start), 0.0);

        // Dragging to the top of the rail walks the history up a step at a
        // time, each step answered by the program before the next goes out.
        let mut batches = 0;
        for _ in 0..200 {
            let asked = strip.drag_to(track(), 20, track().top());
            if asked != 0.0 {
                assert!(asked > 0.0, "dragging up heads for older text");
                strip.note(asked.round() as i32, 20);
                batches += 1;
                // The program draws something: the batch was answered.
                strip.rows_hash = (1..=20).map(|row| row + batches * 31).collect();
            }
            strip.measure();
        }
        assert!(
            (strip.pos - -20.0).abs() < 1.5,
            "the drag landed on {} instead of the top",
            strip.pos
        );
        strip.held_at = None;
        let (height, middle) = strip.handle(track(), 20);
        assert!((middle - (track().top() + height / 2.0)).abs() < 2.0);

        // Nothing is asked of a program that has stopped answering: the count
        // cannot run past text that never moved.
        let frozen = strip.rows_hash.clone();
        for _ in 0..30 {
            if strip.drag_to(track(), 20, track().top()) != 0.0 {
                strip.note(1, 20);
            }
            strip.mark(&frozen);
            strip.measure();
        }
        assert!(
            strip.pos >= -21.0,
            "the count ran to {} against a frozen screen",
            strip.pos
        );
    }

    #[test]
    fn a_measured_move_teaches_how_much_a_notch_is_worth() {
        let mut strip = ScrollTrack {
            rows_hash: (1..=20).collect(),
            ..Default::default()
        };
        strip.note(-2, 20);
        // Two notches down moved the screen six lines: three lines a notch.
        strip.rows_hash = (7..=26).collect();
        strip.measure();
        assert!(strip.lines_per_notch() > 1.5);
        assert_eq!(strip.pos, 6.0);
    }
}
