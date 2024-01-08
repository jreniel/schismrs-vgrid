pub trait BuildVQS {
    fn nvrt(&self) -> usize;
    fn bottom_level_indices(&self) -> Vec<usize>;
    fn iter_level_values(&self) -> IterLevelValues;
    fn values_at_level(&self, level: usize) -> Vec<f64>;
}

pub struct IterLevelValues<'a> {
    vqs: &'a dyn BuildVQS,
    level: usize,
}

impl<'a> Iterator for IterLevelValues<'a> {
    type Item = (usize, Vec<f64>);

    fn next(&mut self) -> Option<Self::Item> {
        let values = self.vqs.values_at_level(self.level);
        self.level += 1;
        Some((self.level - 1, values))
    }
}
