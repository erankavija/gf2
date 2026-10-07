//@ charon-args=--remove-associated-types=*

use std::ops::Mul;

trait FiniteField: Sized + Clone + Mul<Output = Self> {
    type Characteristic;
}

trait ConstField: FiniteField {}

trait ExtConfig: 'static {
    type BaseField: ConstField;

    fn square(x: Self::BaseField) -> Self::BaseField {
        x.clone() * x
    }
}
