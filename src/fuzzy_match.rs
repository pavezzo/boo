use crate::byte_storage::FilePath;

const GAP_PENALTY: i32 = 20;
const BONUS_FIRST_LETTER: i32 = 20;
const BONUS_RIGHT_LETTER: i32 = 10;

pub struct FuzzyMatcher {
    scoring_matrix: Vec<i32>,
}

impl FuzzyMatcher {
    pub fn new() -> Self {
        Self {
            scoring_matrix: Vec::new(),
        }
    }

    pub fn smith_waterman(&mut self, input: &[u8], to_match: FilePath) -> i32 {
        if let Some(points) = self.instant_match(input, to_match) {
            return points;
        }
        let to_match = to_match.data;
        self.scoring_matrix.fill(0);
        self.scoring_matrix.resize((input.len() + 1) * (to_match.len() + 1), 0);

        let mut max = 0;
        let mut gap_length = 0;

        for row in 1..=input.len() {
            for col in 1..=to_match.len() {
                let matching = if input[row-1] == to_match[col-1] {
                    gap_length = 0;
                    self.scoring_matrix[(row-1) * to_match.len() + col - 1] + BONUS_RIGHT_LETTER + if row == 1 { BONUS_FIRST_LETTER } else { 0 }
                } else {
                    gap_length += 1;
                    self.scoring_matrix[(row-1) * to_match.len() + col - 1] - GAP_PENALTY
                };

                let deleting = self.scoring_matrix[(row-1) * to_match.len() + col] - GAP_PENALTY * gap_length;
                let inserting = self.scoring_matrix[row * to_match.len() + col- 1] - GAP_PENALTY * gap_length;

                self.scoring_matrix[row * to_match.len() + col] = *[0, matching, deleting, inserting].iter().max().unwrap();
                max = max.max(self.scoring_matrix[row * to_match.len() + col]);
            }
        }

        max
    }

    fn instant_match(&self, input: &[u8], to_match: FilePath) -> Option<i32> {
        // for part in to_match.into_iter().filter(|part| part.len() == input.len()) {
        for part in to_match.into_iter() {
            if part == input {
                return Some(i32::MAX);
            }
        }
        None
    }
}

