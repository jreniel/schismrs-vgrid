use ndarray::Array2;

pub trait Transform {
    fn zmas(&self) -> &Array2<f64>;
    fn etal(&self) -> &f64;
    fn a_vqs0(&self) -> &f64;
}
