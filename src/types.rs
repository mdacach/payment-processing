// TODO: consider using newtypes here.
pub type TxId = u32;
pub type ClientId = u16;

// TODO: consider wrapping into a new type to make these units explicit in the API.
pub type Money = primitive_fixed_point_decimal::ConstScaleFpdec<i64, 4>;
