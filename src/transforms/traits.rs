pub trait BuildVQS {
    fn nvrt(&self) -> u8;
    fn bottom_level_indices(&self) -> Vec<u8>;
    fn iter_level_values(&self) -> IterLevelValues;
    fn values_at_level(&self, level: u8) -> Vec<f64>;
}

pub struct IterLevelValues<'a> {
    vqs: &'a dyn BuildVQS,
    level: u8,
}

impl<'a> Iterator for IterLevelValues<'a> {
    type Item = (u8, Vec<f64>);

    fn next(&mut self) -> Option<Self::Item> {
        let values = self.vqs.values_at_level(self.level);
        self.level += 1;
        Some((self.level - 1, values))
    }
}
