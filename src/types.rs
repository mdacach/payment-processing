// TODO: consider using newtypes instead of type aliases. a newtype is harder to get wrong.
pub type TxId = u32;
pub type ClientId = u16;

// TODO: consider wrapping into a newtype instead of simply using a type alias.
// TODO: consider having a different type for allowing negative values.
pub type Money = primitive_fixed_point_decimal::ConstScaleFpdec<i64, 4>;
