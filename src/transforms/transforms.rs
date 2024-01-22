use super::quadratic::QuadraticTransformOpts;
use super::s::STransformOpts;

#[derive(Clone, Debug)]
pub enum StretchingFunction<'a> {
    Quadratic(Option<QuadraticTransformOpts<'a>>),
    S(Option<STransformOpts<'a>>),
    // Shchepetkin2005(Option<Shchepetkin2005TransformOpts>),
    // Geyer(Option<GeyerTransformOpts>),
    // Shchepetkin2010(Option<Shchepetkin2010TransformOpts>),
    // FixedZ(FixedZOpts)
}

// impl StretchingFunction {
//     pub fn nvrt(&self) {
//         // match
//     }
// }

// a_vqs(m)=max(-1.d0,a_vqs0-(m-1)*0.03)
// tmp=a_vqs(m)*sigma*sigma+(1+a_vqs(m))*sigma !transformed sigma
// z_mas(k,m)=tmp*(etal+hsm(m))+etal
