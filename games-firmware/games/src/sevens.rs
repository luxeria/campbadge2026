//! The Sevens card game (a.k.a. Fan Tan / Parliament).
//!
//! Pure rules logic with no I/O, so the same engine drives the badge's
//! rotating hot-seat mode, the coordinating server, and the AI players. The
//! logic is `no_std` and allocation free, using fixed arrays throughout.

use raylib_camp::rand::Prng;

/// The four card suits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Suit {
    /// Spades.
    Spades,
    /// Hearts.
    Hearts,
    /// Diamonds.
    Diamonds,
    /// Clubs,
    Clubs,
}

/// All suits in a fixed order.
pub const SUITS: [Suit; 4] = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];

impl Suit {
    /// Index of the suit, matching its position in [`SUITS`].
    pub fn index(self) -> usize {
        self as usize
    }

    /// One-letter label for the suit (`S`, `H`, `D`, `C`).
    pub fn label(self) -> &'static str {
        match self {
            Suit::Spades => "S",
            Suit::Hearts => "H",
            Suit::Diamonds => "D",
            Suit::Clubs => "C",
        }
    }
}

/// Ace rank value (the lowest card in a pile).
pub const ACE: u8 = 1;
/// Anchor rank value: a pile is opened by a 7.
pub const SEVEN: u8 = 7;
/// King rank value (the highest card in a pile).
pub const KING: u8 = 13;

/// A single playing card, `rank` in `1..=13` where 1 is Ace and 11..13 are the
/// face cards.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Card {
    /// The card's suit.
    pub suit: Suit,
    /// The card's rank, rank 1 is Ace and rank 13 is King.
    pub rank: u8,
}

impl Card {
    /// Builds a card, returning `None` if the rank is outside `1..=13`.
    pub const fn new(suit: Suit, rank: u8) -> Option<Card> {
        if ACE <= rank && rank <= KING {
            Some(Card { suit, rank })
        } else {
            None
        }
    }

    /// True if the rank is a face card (Jack, Queen or King).
    pub fn is_face(self) -> bool {
        (11..=KING).contains(&self.rank)
    }

    /// Points this card is worth left in a hand at the end of a round.
    pub fn points(self) -> u32 {
        match self.rank {
            SEVEN => 50,
            ACE => 15,
            11..=KING => 10,
            _ => 5,
        }
    }

    /// Short label for the rank (`A`, `2`..`10`, `J`, `Q`, `K`).
    pub fn rank_label(self) -> &'static str {
        match self.rank {
            1 => "A",
            11 => "J",
            12 => "Q",
            13 => "K",
            _ => {
                // A tiny static table avoids allocating while still labelling 2..10.
                const DIGITS: [&str; 10] = ["2", "3", "4", "5", "6", "7", "8", "9", "10", "?"];
                DIGITS[(self.rank - 2) as usize]
            }
        }
    }

    /// Compact textual form of the card, e.g. `"8H"` or `"AS"`.
    pub fn to_string_compact(self) -> CardString {
        CardString {
            label: [self.rank_label().as_bytes(), self.suit.label().as_bytes()],
        }
    }
}

/// A fixed stack buffer holding a card's compact textual label.
pub struct CardString {
    label: [&'static [u8]; 2],
}

impl CardString {
    /// Exposes the two label segments (rank, suit).
    pub fn parts(&self) -> (&'static [u8], &'static [u8]) {
        (self.label[0], self.label[1])
    }
}

/// A deck of fifty-two cards, dealt from the top.
pub struct Deck {
    cards: [Option<Card>; 52],
    count: usize,
}

impl Deck {
    /// Builds a fresh full deck and shuffles it with the given generator.
    pub fn new_shuffled(rng: &mut Prng) -> Deck {
        let mut cards = [None; 52];
        let mut index = 0;
        for suit in SUITS {
            for rank in ACE..=KING {
                cards[index] = Card::new(suit, rank);
                index += 1;
            }
        }
        for position in (1..52).rev() {
            let swap = rng.next_range((position + 1) as u32) as usize;
            cards.swap(position, swap);
        }
        Deck { cards, count: 52 }
    }

    /// Deals the top card, or `None` if the deck is exhausted.
    pub fn deal(&mut self) -> Option<Card> {
        if self.count == 0 {
            None
        } else {
            self.count -= 1;
            self.cards[self.count].take()
        }
    }
}

/// Upper bound on a single hand's size.
pub const MAX_HAND: usize = 52;

/// A player's hand of cards, stored in an unsorted fixed array.
pub struct Hand {
    cards: [Option<Card>; MAX_HAND],
    len: usize,
}

impl Hand {
    /// Creates an empty hand.
    pub fn new() -> Self {
        Hand {
            cards: [None; MAX_HAND],
            len: 0,
        }
    }

    /// Number of cards currently in the hand.
    pub fn len(&self) -> usize {
        self.len
    }

    /// True when the hand holds no cards.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Adds a card to the hand.
    pub fn push(&mut self, card: Card) {
        if self.len < MAX_HAND {
            self.cards[self.len] = Some(card);
            self.len += 1;
        }
    }

    /// True if the hand contains the card.
    pub fn contains(&self, card: Card) -> bool {
        self.cards[..self.len].contains(&Some(card))
    }

    /// Iterates the cards currently held.
    pub fn iter(&self) -> impl Iterator<Item = Card> + '_ {
        self.cards[..self.len].iter().filter_map(|c| *c)
    }

    /// Removes the given card, returning whether it was present.
    pub fn remove(&mut self, card: Card) -> bool {
        for index in 0..self.len {
            if self.cards[index] == Some(card) {
                self.cards[index] = self.cards[self.len - 1].take();
                self.len -= 1;
                return true;
            }
        }
        false
    }

    /// Removes a random card, returning it, or `None` for an empty hand.
    pub fn random_remove(&mut self, rng: &mut Prng) -> Option<Card> {
        if self.len == 0 {
            return None;
        }
        let index = rng.next_range(self.len as u32) as usize;
        let card = self.cards[index].take().unwrap();
        self.cards[index] = self.cards[self.len - 1].take();
        self.len -= 1;
        Some(card)
    }
}

impl Default for Hand {
    fn default() -> Self {
        Hand::new()
    }
}

/// One suited pile of the board, opened by its 7 and expanding in rank order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pile {
    open: bool,
    low: u8,
    high: u8,
}

impl Pile {
    fn new() -> Self {
        Pile {
            open: false,
            low: 0,
            high: 0,
        }
    }

    /// True if a card of this rank can be played onto the pile.
    pub fn can_place(&self, rank: u8) -> bool {
        if !self.open {
            rank == SEVEN
        } else {
            (self.low > ACE && rank == self.low - 1) || (self.high < KING && rank == self.high + 1)
        }
    }

    fn place(&mut self, rank: u8) {
        if !self.open {
            self.open = true;
            self.low = rank;
            self.high = rank;
        } else if self.low > ACE && rank == self.low - 1 {
            self.low = rank;
        } else if self.high < KING && rank == self.high + 1 {
            self.high = rank;
        }
    }
}

/// The four suited piles of the board.
pub struct Board {
    piles: [Pile; 4],
}

impl Board {
    fn new() -> Self {
        Board {
            piles: [Pile::new(), Pile::new(), Pile::new(), Pile::new()],
        }
    }

    /// True if the card is currently playable on the board.
    pub fn playable(&self, card: Card) -> bool {
        self.piles[card.suit.index()].can_place(card.rank)
    }

    fn play(&mut self, card: Card) {
        self.piles[card.suit.index()].place(card.rank);
    }
}

/// Outcome of attempting to play a card.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayOutcome {
    /// The card was played and the turn continues.
    Played,
    /// The card emptied the player's hand, finishing the round.
    RoundFinished,
}

/// Error when a card cannot be played.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayError {
    /// It is not this card's turn (the card belongs to another hand).
    WrongPlayer,
    /// The card is not in the current player's hand.
    NotInHand,
    /// The card is not legal on the board right now.
    IllegalMove,
    /// The round or game has already finished.
    GameOver,
}

/// Number of players.
pub const PLAYER_COUNT: usize = 4;

/// The complete state of one Sevens game, spanning possibly many rounds.
pub struct Game {
    hands: [Hand; PLAYER_COUNT],
    board: Board,
    scores: [u32; PLAYER_COUNT],
    current: usize,
    round_over: bool,
    game_over: bool,
}

impl Game {
    /// Deals a fresh game: thirteen cards to each player, finding whoever holds
    /// the 7 of spades to act first.
    pub fn new(rng: &mut Prng) -> Game {
        Game::deal_round(rng)
    }

    fn deal_round(rng: &mut Prng) -> Game {
        let mut deck = Deck::new_shuffled(rng);
        let mut hands = [Hand::new(), Hand::new(), Hand::new(), Hand::new()];
        for _ in 0..13 {
            for hand in hands.iter_mut() {
                if let Some(card) = deck.deal() {
                    hand.push(card);
                }
            }
        }
        // The player who opens first is whichever seat holds a seven.
        let mut current = 0;
        'find_opener: for (seat, hand) in hands.iter().enumerate() {
            for suit in SUITS {
                if hand.contains(Card::new(suit, SEVEN).unwrap()) {
                    current = seat;
                    break 'find_opener;
                }
            }
        }
        Game {
            hands,
            board: Board::new(),
            scores: [0; PLAYER_COUNT],
            current,
            round_over: false,
            game_over: false,
        }
    }

    /// Index of the player whose turn it is.
    pub fn current_player(&self) -> usize {
        self.current
    }

    /// Whether the round has finished.
    pub fn round_over(&self) -> bool {
        self.round_over
    }

    /// Whether the whole game has finished (somebody reached 250 points).
    pub fn game_over(&self) -> bool {
        self.game_over
    }

    /// A player's hand.
    pub fn hand(&self, player: usize) -> &Hand {
        &self.hands[player]
    }

    /// The played rank range of a suit's pile, as `(low, high)`, or `None` if
    /// that suit has not been opened yet.
    pub fn pile_state(&self, index: usize) -> Option<(u8, u8)> {
        let pile = &self.board.piles[index.min(3)];
        if pile.open {
            Some((pile.low, pile.high))
        } else {
            None
        }
    }

    /// The accumulated score of a player across rounds.
    pub fn score(&self, player: usize) -> u32 {
        self.scores[player]
    }

    /// Whether the current player has no legal card and therefore must draw.
    pub fn current_must_draw(&self) -> bool {
        !self.round_over && self.legal_cards().1 == 0
    }

    /// The set of legal cards for the current player, plus how many there are.
    pub fn legal_cards(&self) -> ([Card; MAX_HAND], usize) {
        let mut legal = [Card::new(Suit::Spades, ACE).expect("ace exists"); MAX_HAND];
        let mut count = 0;
        for card in self.hands[self.current].iter() {
            if self.board.playable(card) && count < MAX_HAND {
                legal[count] = card;
                count += 1;
            }
        }
        (legal, count)
    }

    /// Plays a card for the current player.
    pub fn play(&mut self, card: Card) -> Result<PlayOutcome, PlayError> {
        if self.round_over || self.game_over {
            return Err(PlayError::GameOver);
        }
        let (legal, count) = self.legal_cards();
        if !legal[..count].contains(&card) {
            // Distinguish "not in hand" from "not legal" for good error text.
            return if self.hands[self.current].contains(card) {
                Err(PlayError::IllegalMove)
            } else {
                Err(PlayError::NotInHand)
            };
        }
        self.hands[self.current].remove(card);
        self.board.play(card);

        if self.hands[self.current].is_empty() {
            self.round_over = true;
            self.finish_round();
            return Ok(PlayOutcome::RoundFinished);
        }
        self.advance_turn();
        Ok(PlayOutcome::Played)
    }

    /// The current player draws one random card from each other player.
    pub fn draw_from_others(&mut self, rng: &mut Prng) {
        for seat in 0..PLAYER_COUNT {
            if seat == self.current {
                continue;
            }
            if let Some(card) = self.hands[seat].random_remove(rng) {
                self.hands[self.current].push(card);
            }
        }
        self.advance_turn();
    }

    /// Begins a new round after the previous one finished and the game is not
    /// over yet.
    pub fn next_round(&mut self, rng: &mut Prng) {
        let mut dealt = Game::deal_round(rng);
        dealt.scores = self.scores;
        *self = dealt;
    }

    fn advance_turn(&mut self) {
        self.current = (self.current + 1) % PLAYER_COUNT;
    }

    fn finish_round(&mut self) {
        for seat in 0..PLAYER_COUNT {
            if seat == self.current {
                continue;
            }
            let points: u32 = self.hands[seat].iter().map(Card::points).sum();
            self.scores[seat] = self.scores[seat].saturating_add(points);
        }
        self.game_over = self.scores.iter().any(|&score| score >= 250);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffled_deck_has_52_unique_cards() {
        let mut rng = Prng::new(7);
        let mut deck = Deck::new_shuffled(&mut rng);
        let mut seen = [false; 52];
        let mut count = 0;
        while let Some(card) = deck.deal() {
            let key = card.suit.index() * 13 + (card.rank as usize - 1);
            assert!(!seen[key], "duplicate card dealt {card:?}");
            seen[key] = true;
            count += 1;
        }
        assert_eq!(count, 52);
    }

    #[test]
    fn dealing_gives_each_player_thirteen() {
        let mut rng = Prng::new(3);
        let game = Game::new(&mut rng);
        let total: usize = (0..PLAYER_COUNT).map(|p| game.hand(p).len()).sum();
        assert_eq!(total, 52);
        assert!((0..PLAYER_COUNT).all(|p| game.hand(p).len() == 13));
    }

    #[test]
    fn card_points_follow_the_table() {
        assert_eq!(Card::new(Suit::Spades, 7).unwrap().points(), 50);
        assert_eq!(Card::new(Suit::Spades, 1).unwrap().points(), 15);
        assert_eq!(Card::new(Suit::Spades, 11).unwrap().points(), 10);
        assert_eq!(Card::new(Suit::Spades, 13).unwrap().points(), 10);
        assert_eq!(Card::new(Suit::Spades, 3).unwrap().points(), 5);
    }

    #[test]
    fn pile_expands_around_the_seven() {
        let mut pile = Pile::new();
        assert!(pile.can_place(SEVEN));
        assert!(!pile.can_place(6));
        pile.place(SEVEN);
        assert!(pile.can_place(6));
        assert!(pile.can_place(8));
        assert!(!pile.can_place(5));
        pile.place(6);
        assert!(pile.can_place(5));
        pile.place(8);
        assert!(pile.can_place(9));
    }

    #[test]
    fn starting_player_holds_a_seven_to_open() {
        let mut rng = Prng::new(11);
        let game = Game::new(&mut rng);
        // The opening player must be able to open: they hold at least one 7.
        let has_seven = game
            .hand(game.current_player())
            .iter()
            .any(|c| c.rank == SEVEN);
        assert!(has_seven, "opening player should hold a seven");
        // Every 7 they hold is a legal opening choice - they may pick freely.
        let (legal, count) = game.legal_cards();
        assert!(count >= 1);
        assert!(legal[..count].iter().any(|c| c.rank == SEVEN));
    }

    #[test]
    fn any_seven_can_open_a_suit() {
        let mut rng = Prng::new(5);
        let mut game = Game::new(&mut rng);
        let opener = game.legal_cards().0[0];
        assert_eq!(opener.rank, SEVEN);
        match game.play(opener) {
            Ok(PlayOutcome::Played) => {}
            other => panic!("opening should play cleanly, got {other:?}"),
        }
        assert!(!game.round_over());
        assert!(
            game.pile_state(opener.suit.index()).is_some(),
            "That suit's pile is now open."
        );
    }

    #[test]
    fn emptying_a_hand_scores_the_round() {
        let mut rng = Prng::new(13);
        let mut game = Game::new(&mut rng);
        // Reduce the current player to a single card and force it into a legal
        // play to trigger a round finish at the very start.
        let current = game.current;
        let legal_card = game.legal_cards().0[0];
        game.hands[current] = Hand::new();
        game.hands[current].push(legal_card);
        let outcome = game.play(legal_card).unwrap();
        assert_eq!(outcome, PlayOutcome::RoundFinished);
        assert!(game.round_over());
        assert!(
            game.scores[current] == 0,
            "The player who finished scores nothing; the others score their hands."
        );
    }
}
