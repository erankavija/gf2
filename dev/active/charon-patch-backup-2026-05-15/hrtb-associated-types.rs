//@ charon-args=--remove-associated-types=*
use std::ops::{Add, Sub};

trait FiniteField:
    Sized
    + Add<Output = Self>
    + for<'a> Add<&'a Self, Output = Self>
    + Sub<Output = Self>
    + for<'a> Sub<&'a Self, Output = Self>
{
    type Characteristic;
}

trait ConstField: FiniteField {}

trait ExtConfig: 'static {
    type BaseField: ConstField;
}

fn use_base_field<C: ExtConfig>() -> C::BaseField
where
    C::BaseField: Default,
{
    Default::default()
}
