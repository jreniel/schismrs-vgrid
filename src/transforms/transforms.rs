use super::quadratic::QuadraticTransformOpts;

#[derive(Clone)]
pub enum StretchingFunction {
    Quadratic(QuadraticTransformOpts),
    // S,
    // Shchepetkin2005,
    // Geyer,
    // Shchepetkin2010,
}

// impl StretchingFunction {
//     pub fn nvrt(&self) {
//         // match
//     }
// }

// a_vqs(m)=max(-1.d0,a_vqs0-(m-1)*0.03)
// tmp=a_vqs(m)*sigma*sigma+(1+a_vqs(m))*sigma !transformed sigma
// z_mas(k,m)=tmp*(etal+hsm(m))+etal
