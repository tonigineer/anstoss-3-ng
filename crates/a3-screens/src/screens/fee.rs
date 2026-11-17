//! The talk over a transfer's fee for its human seller (dialog 308,
//! `CVerhandlungUeberAbloese`, `IDD_VERHANDLUNG_UEBER_ABLOESE`), a round
//! at a time: the player, his club and the bidder, the fee offered and
//! the fee asked, each on the fee slider's steps (`DAT_0088cb50`). The
//! seller sets his asking on the right slider and asks it ("Verhandeln",
//! option 0) or takes the offer ("Unterschreiben", option 1).
//! The dialog edits the running talk in place (`FUN_0076fddd`), which the
//! cycle plays on ([`a3_core::fee_talk`]). Still to come: the partner
//! manager's portrait (4092) and its face by his patience
//! (`FUN_005bccb0`).

use a3_core::cycle::dialogs::FEE_TALK;
use a3_core::fee_talk::{FeeTalk, amount, step};

use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::bitmap::Picture;
use crate::pointer::Target;
use crate::view::View;

/// The player's line: name, position, strength, age, market value, his
/// contract (`FUN_005bb967`).
const NAME: u16 = 4070;
const POSITION: u16 = 4071;
const STRENGTH: u16 = 4072;
const AGE: u16 = 4073;
const VALUE: u16 = 4074;
const CONTRACT: u16 = 4075;
/// The seller ("Bisheriger Verein") and the bidder ("Interessent"):
/// crest, name, league.
const SELLER_CREST: u16 = 4077;
const SELLER: u16 = 4078;
const SELLER_LEAGUE: u16 = 4079;
const BUYER_CREST: u16 = 4081;
const BUYER: u16 = 4082;
const BUYER_LEAGUE: u16 = 4083;
/// The manager's words (+0x520), the fee offered and asked (+0x12e8,
/// +0x13ac), their sliders (+0x1470, +0x1608).
const WORDS: u16 = 4084;
const OFFERED: u16 = 4086;
const ASKED: u16 = 4088;
const OFFER_SLIDER: u16 = 4089;
const ASK_SLIDER: u16 = 4090;
/// "Spielerinfo", the options (`OnClicked_IDC_UEBER_ABLOESE_ANNEHMEN`:
/// 0 "Verhandeln", asking; 1 "Unterschreiben", taking the offer).
const PLAYER_INFO: u16 = 4096;
const CHOICES: u16 = 4095;
const ASK: usize = 0;
const TAKE: usize = 1;
/// Text 0x935: the asking past its top, the man not for sale.
const NOT_FOR_SALE: u16 = 0x935;
/// "Verhandlungspartner" and, in brackets, whether he would come at once
/// (texts 0x1258, 0x1259 by the bidder's +0x3a 0x20); the partner's
/// manager (`FUN_00637cbb`); the seller's question (0xa84).
const PARTNER: u16 = 4091;
const AT_ONCE: u16 = 0x1258;
const NOT_AT_ONCE: u16 = 0x1259;
const MANAGER: u16 = 4097;
const SELLERS_QUESTION: u16 = 0xa84;
/// The caption, in the frame: the title, its "%s" empty
/// (`FUN_005bc49d`).
const CAPTION_ID: u16 = 9922;

/// Puts the dialog's caption in its frame.
pub(crate) fn compose(dialogs: &mut std::collections::HashMap<u16, a3_files::rch::Dialog>) {
    let Some(dialog) = dialogs.get_mut(&FEE_TALK) else { return };
    let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.replacen("%s", "", 1));
    dialog.controls.insert(0, caption);
}

#[derive(Default)]
pub struct FeeTalkScreen {
    /// The asking at the right slider (+0x27c0), the offer at its step
    /// (+0x27bc), the right slider's top (+0x27ba).
    asked: i32,
    offered: i32,
    top: i32,
    pressing: bool,
}

fn talk<'a>(cx: &'a Cx) -> Option<&'a FeeTalk> {
    cx.state.season.as_ref()?.fee_talk.as_ref()
}

impl Screen for FeeTalkScreen {
    fn ids(&self) -> &[u16] {
        &[FEE_TALK]
    }

    /// `FUN_005bb967` and `FUN_005bc49d` for the seller's turn.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self::default();
        mark_inert(view, dialog, |t| match t {
            Target::Option { control: c, item } => dialog.controls[c].id == Some(CHOICES) && item <= TAKE,
            _ => false,
        });
        if let Some(c) = control(dialog, PLAYER_INFO) {
            view.inert.push(Target::Button(c));
        }
        if let Some(options) = control(dialog, CHOICES).and_then(|c| view.options_mut(c)) {
            options.actions = true;
        }
        let Some(fee) = talk(cx).cloned() else { return };
        let (Some(records), Some(season)) = (cx.state.records.as_ref(), cx.state.season.as_ref()) else {
            return;
        };
        // The right slider runs to the first asking, a step past it
        // without a release fee: the man not for sale.
        self.top = step(fee.asking) + i32::from(fee.fixed.is_none());
        self.pressing = fee.pressing();
        self.asked = amount(step(fee.demand));
        self.offered = amount(step(fee.offer));
        if let Some(slider) = control(dialog, OFFER_SLIDER).and_then(|c| view.value_mut(c)) {
            slider.range = (0, step(fee.affordable));
            slider.set(step(fee.offer));
        }
        if let Some(slider) = control(dialog, ASK_SLIDER).and_then(|c| view.value_mut(c)) {
            slider.range = (0, self.top);
            slider.set(if self.pressing { self.top } else { step(fee.demand) });
        }
        // The seller's turn: the bidder's slider hidden (`FUN_005bc49d`),
        // the question (0xa84), the partner's manager.
        if let Some(c) = control(dialog, OFFER_SLIDER) {
            view.controls[c].hidden = true;
        }
        set(view, dialog, WORDS, game.text(SELLERS_QUESTION).to_owned());
        let partner = control(dialog, PARTNER).map_or(String::new(), |c| dialog.controls[c].text.clone());
        let at_once = fee.at_once;
        set(view, dialog, PARTNER, format!("{partner} ({})", game.text(if at_once { AT_ONCE } else { NOT_AT_ONCE })));
        if let Some(team) = records.team(fee.buyer.0.into(), fee.buyer.1 as u16) {
            let [manager_trainer, _, manager] = team.staff();
            let id = if manager_trainer != 0 { manager_trainer } else { manager };
            set(view, dialog, MANAGER, records.person_name(id));
        }
        set(view, dialog, OFFERED, super::finances::money32(game, (self.offered * 1000) as u32));
        self.show_asked(cx, view);
        // The player.
        let today = season.date();
        if let Some((player, _, _)) = a3_core::load::any_player(records, &game.data, fee.player)
            && let Some(r) = records.player(fee.player)
        {
            let template = |id: u16| control(dialog, id).map_or(String::new(), |c| dialog.controls[c].text.clone());
            let years = (i32::from(r.contract() & 0xfff) - i32::from(records.globals.start_year())) as i8;
            let words = &game.rules.words;
            let position = words.positions.get(usize::from(player.positions[0]).min(10)).map_or("", |&p| game.text(p));
            set(view, dialog, NAME, format!("{} {}", player.first_name, player.last_name));
            set(view, dialog, POSITION, position.to_owned());
            set(view, dialog, STRENGTH, super::finances::printf(&template(STRENGTH), &[&player.strength]));
            set(view, dialog, AGE, super::finances::printf(&template(AGE), &[&player.age(today)]));
            let stars = records.globals.stars();
            let value = super::playerinfo::market_value(game, &player, r, stars, i32::from(years), today);
            set(view, dialog, VALUE, super::finances::printf(&template(VALUE), &[&super::finances::money64(game, value)]));
            set(view, dialog, CONTRACT, super::poaching::contract_text(cx, years, today));
        }
        // The clubs.
        for ((country, number), crest_id, name_id, league_id) in
            [(fee.seller, SELLER_CREST, SELLER, SELLER_LEAGUE), (fee.buyer, BUYER_CREST, BUYER, BUYER_LEAGUE)]
        {
            set(view, dialog, name_id, game.club_name(Some(records), country, number));
            set(view, dialog, league_id, super::tournament::league(game, records, (country, number)));
            let crest = game.world.slot_of(country.into()).and_then(|slot| {
                game.rules.crest(&game.world, Some(records), (slot, usize::try_from(number).ok()?))
            });
            if let Some(c) = control(dialog, crest_id) {
                view.set_picture(c, crest.map(Picture::file));
            }
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let id = |c: usize| cx.dialog.controls[c].id.unwrap_or(0);
        match target {
            // `FUN_005bcdc3`: below the top the asking at its step; at the
            // top holding out, the asking as it was.
            Target::SliderStep { control: c, .. } | Target::SliderAt { control: c, .. } | Target::SliderTrack(c)
                if id(c) == ASK_SLIDER =>
            {
                let position = view.value(c).map_or(0, |v| v.value);
                self.pressing = position >= self.top;
                if !self.pressing {
                    self.asked = amount(position);
                }
                self.show_asked(cx, view);
                Outcome::Stay
            }
            Target::Option { control: c, item } if id(c) == CHOICES => {
                let asked = match item {
                    ASK => self.asked,
                    TAKE => self.offered,
                    _ => return Outcome::Stay,
                };
                let pressing = item == ASK && self.pressing;
                if let Some(fee) = cx.state.season.as_mut().and_then(|s| s.fee_talk.as_mut()) {
                    fee.ask(asked, pressing);
                }
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }
}

impl FeeTalkScreen {
    /// `FUN_005bd1d0`: the asking, or past the top with a release fee
    /// "not for sale".
    fn show_asked(&self, cx: &Cx, view: &mut View) {
        let fixed = talk(cx).is_some_and(|f| f.fixed.is_some());
        let text = if self.pressing && fixed {
            cx.game.text(NOT_FOR_SALE).to_owned()
        } else {
            super::finances::money32(cx.game, (self.asked * 1000) as u32)
        };
        set(view, cx.dialog, ASKED, text);
    }
}
