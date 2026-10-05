//@ charon-args=--remove-associated-types=*

trait Config: 'static {
    type Field: Default + Clone + std::ops::Mul<Output = Self::Field>;
    fn double(x: Self::Field) -> Self::Field {
        x.clone() * x
    }
}

fn use_config<C: Config>() -> C::Field {
    C::double(Default::default())
}
