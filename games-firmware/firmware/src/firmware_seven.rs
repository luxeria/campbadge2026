//! Sevens (Fan Tan / Parliament) firmware, driven by the `games::sevens` engine.
//!
//! Work in progress: the current player's hand is a bottom rack, the four
//! suited piles sit in the middle, and the opponents' card-back stacks are on
//! the other three sides of the table.

use raylib_camp::canvas::Canvas;
use raylib_camp::color::{palette, Color};
use raylib_camp::input::Button;
use raylib_camp::rand::Prng;

use games::sevens::{Card, Game, PlayOutcome, Suit, MAX_HAND, PLAYER_COUNT, SUITS};

use crate::board::Board;

/// Sorts a slice of cards by suit, then by descending rank within each suit.
fn sort_cards(cards: &mut [Card]) {
    let comes_after = |a: Card, b: Card| -> bool {
        a.suit.index() > b.suit.index() || (a.suit.index() == b.suit.index() && a.rank < b.rank)
    };
    for index in 1..cards.len() {
        let mut cursor = index;
        while cursor > 0 && comes_after(cards[cursor - 1], cards[cursor]) {
            cards.swap(cursor - 1, cursor);
            cursor -= 1;
        }
    }
}

/// Appends a card's compact rank+suit label (e.g. `"8H"`) into a stack buffer.
fn card_label(card: Card, out: &mut [u8; 3]) -> &str {
    let rank = card.rank_label().as_bytes();
    let suit = card.suit.label().as_bytes();
    let mut count = 0;
    for &byte in rank {
        if count < 2 {
            out[count] = byte;
            count += 1;
        }
    }
    out[count] = suit[0];
    count += 1;
    core::str::from_utf8(&out[..count]).unwrap_or("?")
}

/// Draws a small card face with an optional highlight (the selected rack card).
fn draw_card(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32, card: Card, selected: bool) {
    let background = if selected {
        palette::YELLOW
    } else {
        palette::WHITE
    };
    canvas.rect_filled(x, y, w, h, background);
    canvas.rect(x, y, w, h, palette::GREY);
    let face_color = if card.suit == Suit::Hearts || card.suit == Suit::Diamonds {
        palette::RED
    } else {
        palette::BLACK
    };
    let mut label = [0u8; 3];
    let text = card_label(card, &mut label);
    let text_width = canvas.measure_text(text, 1);
    canvas.draw_text(text, x + (w - text_width) / 2, y + 1, 1, face_color);
}

/// Draws a decimal number centred around the given horizontal point.
fn draw_number(canvas: &mut Canvas, x: i32, y: i32, value: u32, color: Color) {
    let mut buffer = [0u8; 8];
    let mut n = 0;
    if value == 0 {
        buffer[n] = b'0';
        n += 1;
    }
    let mut v = value;
    while v > 0 && n < buffer.len() {
        buffer[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    buffer[..n].reverse();
    if let Ok(text) = core::str::from_utf8(&buffer[..n]) {
        let text_width = canvas.measure_text(text, 1);
        canvas.draw_text(text, x - text_width / 2, y, 1, color);
    }
}

/// Draws `text` centred at the given baseline y.
fn draw_centered(canvas: &mut Canvas, y: i32, text: &str, color: Color) {
    let text_width = canvas.measure_text(text, 1);
    canvas.draw_text(text, 120 - text_width / 2, y, 1, color);
}

/// Builds a `low-high` range label such as `"6-K"`.
fn range_label(low: u8, high: u8, out: &mut [u8; 7]) -> &str {
    let mut count = 0;
    for &byte in rank_label(low).as_bytes() {
        out[count] = byte;
        count += 1;
    }
    out[count] = b'-';
    count += 1;
    for &byte in rank_label(high).as_bytes() {
        out[count] = byte;
        count += 1;
    }
    core::str::from_utf8(&out[..count]).unwrap_or("")
}

/// Short label for a rank value (Ace..King).
fn rank_label(rank: u8) -> &'static str {
    match rank {
        1 => "A",
        11 => "J",
        12 => "Q",
        13 => "K",
        value => {
            const DIGITS: [&str; 9] = ["2", "3", "4", "5", "6", "7", "8", "9", "10"];
            DIGITS[(value - 2) as usize]
        }
    }
}

/// Draws the four suited piles in a row across the middle of the round panel.
fn draw_board(canvas: &mut Canvas, game: &Game) {
    const PILE_W: i32 = 46;
    const PILE_H: i32 = 34;
    let start_x = (240 - ((4 * PILE_W) + (3 * 8))) / 2;
    for (index, suit) in SUITS.iter().enumerate() {
        let x = start_x + index as i32 * (PILE_W + 8);
        match game.pile_state(index) {
            Some((low, high)) => {
                canvas.rect_filled(x, 96, PILE_W, PILE_H, palette::WHITE);
                canvas.rect(x, 96, PILE_W, PILE_H, palette::GREY);
                canvas.draw_text(suit.label(), x + 2, 97, 1, palette::BLACK);
                let mut range = [0u8; 7];
                let range_text = range_label(low, high, &mut range);
                let rw = canvas.measure_text(range_text, 1);
                canvas.draw_text(range_text, x + (PILE_W - rw) / 2, 114, 1, palette::BLACK);
            }
            None => {
                canvas.rect_filled(x, 96, PILE_W, PILE_H, palette::GREY);
                canvas.draw_text(suit.label(), x + 2, 97, 1, palette::WHITE);
            }
        }
    }
}

/// Renders the three opponent card-back stacks around the table.
fn draw_opponents(canvas: &mut Canvas, game: &Game, current: usize) {
    let right = (current + 1) % PLAYER_COUNT;
    let top = (current + 2) % PLAYER_COUNT;
    let left = (current + 3) % PLAYER_COUNT;

    draw_opponent(canvas, 116, 6, game.hand(top).len());
    draw_opponent(canvas, 6, 120, game.hand(left).len());
    draw_opponent(canvas, 220, 120, game.hand(right).len());
}

/// Draws a single opponent's card-back stack with its card count.
fn draw_opponent(canvas: &mut Canvas, x: i32, y: i32, count: usize) {
    canvas.rect_filled(x, y, 11, 15, palette::BLUE);
    canvas.rect(x, y, 11, 15, palette::GREY);
    draw_number(canvas, x + 20, y + 2, count as u32, palette::WHITE);
}

/// Draws the current player's hand rack at the bottom plus the status line.
fn draw_hand_rack(canvas: &mut Canvas, cards: &[Card], cursor: usize, game: &Game) {
    let count = cards.len();
    if count > 0 {
        let card_w = (228 / count as i32).clamp(8, 22);
        let start_x = (240 - count as i32 * card_w) / 2;
        const BASE_Y: i32 = 212;
        for (index, card) in cards.iter().enumerate() {
            let y = if index == cursor { BASE_Y - 6 } else { BASE_Y };
            draw_card(
                canvas,
                start_x + index as i32 * card_w,
                y,
                card_w - 1,
                26,
                *card,
                index == cursor,
            );
        }
    }

    if game.round_over() {
        if game.game_over() {
            draw_centered(canvas, 150, "GAME OVER", palette::RED);
        } else {
            draw_centered(canvas, 150, "ROUND OVER - B8", palette::RED);
        }
    } else if game.current_must_draw() {
        draw_centered(canvas, 196, "No move - press B6", palette::ORANGE);
    }
}

/// Plays Sevens until power is removed.
pub fn main_seven(mut board: Board) -> ! {
    let mut rng = Prng::new(0x53e7);
    let mut game = Game::new(&mut rng);
    let mut hand_cursor: usize = 0;

    loop {
        board.frame();

        let current = game.current_player();
        let mut hand_cards = [Card::new(Suit::Spades, 1).unwrap(); MAX_HAND];
        let mut hand_len = 0;
        for card in game.hand(current).iter() {
            hand_cards[hand_len] = card;
            hand_len += 1;
        }
        sort_cards(&mut hand_cards[..hand_len]);
        if hand_cursor >= hand_len {
            hand_cursor = hand_len.saturating_sub(1);
        }

        if !game.round_over() && !game.game_over() {
            if board.input.just_pressed(Button::Btn1) {
                hand_cursor = (hand_cursor + hand_len - 1) % hand_len.max(1);
            }
            if board.input.just_pressed(Button::Btn3) {
                hand_cursor = (hand_cursor + 1) % hand_len.max(1);
            }

            if board.input.just_pressed(Button::Btn5) || board.input.just_pressed(Button::Btn7) {
                let selection = hand_cards
                    .get(hand_cursor.min(hand_len.saturating_sub(1)))
                    .copied();
                if let Some(card) = selection {
                    match game.play(card) {
                        Ok(PlayOutcome::Played) => hand_cursor = 0,
                        Ok(PlayOutcome::RoundFinished) => {}
                        Err(_) => {}
                    }
                }
            }

            if board.input.just_pressed(Button::Btn6) {
                game.draw_from_others(&mut rng);
                hand_cursor = 0;
            }
        } else if board.input.just_pressed(Button::Btn8) {
            if game.game_over() {
                game = Game::new(&mut rng);
            } else {
                game.next_round(&mut rng);
            }
            hand_cursor = 0;
        }

        board.canvas.clear(palette::BLACK);
        draw_board(&mut board.canvas, &game);
        draw_opponents(&mut board.canvas, &game, current);
        draw_hand_rack(
            &mut board.canvas,
            &hand_cards[..hand_len],
            hand_cursor,
            &game,
        );

        board.flush();
        board.delay.delay_millis(33);
    }
}
