// TEST CASES
// two equal structs, no ADT fields
// two equal structs, with struct fields
// two equal structs, with enum fields
// two unequal structs, no ADT fields
// unequal structs, with struct fields
// unequal structs, with enum fields

pub struct EqNoADT1 {
    authority: u8,
    field2: [u8; 32],
}

pub struct EqNoADT2 {
    field: u8,
    x: [u8; 32]
}

// Even though the two structs have y with a different type, the type
// has the same primitive fields so it will serialize the same way. Thus,
// the structs are equal.
pub struct EqWithStructField1 {
    x: u8,
    y: EqNoADT1,
}

pub struct EqWithStructField2 {
    x: u8,
    y: EqNoADT2,
}

// The structs are equal because the second field should serialize the same,
// even though one is wrapped in a struct.
// NOTE: current impl doesn't support this
pub struct Pubkey([u8; 32]);
pub struct EqWithStructField3 {
    y: u8,
    z: [u8; 32]
}
pub struct EqWithStructField4 {
    x: u8,
    y: Pubkey
}

pub struct Rgb(u32, u32, u32);
pub struct Tricky1 {
    x: Rgb,
}
pub struct Tricky2 {
    x: (u32, u32, u32),
}

// The same applies to the following test. The two structs have a field
// with different enum types, yet those types should serialize the same way
// and thus the structs should be equal.
pub enum Listy {
    Jimi,
    Hendrix,
}

pub enum Enumy {
    Jimi,
    Hendrix,
}

pub struct EqWithEnumField1 {
    x: Vec<u8>,
    y: Listy
}

pub struct EqWithEnumField2 {
    x: Vec<u8>,
    y: Enumy
}

// The following types are unequal
pub struct UnEqNoADT1 {
    x: u32,
}

pub struct UnEqNoADT2 {
    y: u32,
    x: Vec<u8>,
}

// The following structs are unequal due to the sub-struct being unequal
pub struct UnEqStructField1 {
    x: UnEqNoADT1,
    y: u8,
}

pub struct UnEqStructField2 {
    z: UnEqNoADT2,
    y: u8,
}