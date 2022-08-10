# type_cosplay

**What it does:** Checks that all deserialized types have a proper discriminant so that
all types are guaranteed to deserialize differently.

Instead of searching for equivalent types and checking to make sure those specific
types have a discriminant, this lint takes a more strict approach and instead enforces
all deserialized types it collects, to have a discriminant, regardless of whether the
types are equivalent or not.

We define a proper discriminant as an enum with as many variants as there are struct
types in the program. Further, the discriminant should be the first field of every
struct in order to avoid overwrite by arbitrary length fields, like vectors.

A second case of a proper discriminant is when a single enum contains as variants all the struct
types that will be deserialized. This "umbrella" enum essentially has a built-in
discriminant. If it is the only type that is deserialized, then all struct types
are guaranteed to be unique since the program will have to match a specific variant.

**Why is this bad?**
The type cosplay issue is when one account type can be substituted for another account type.
This occurs when a type deserializes exactly the same as another type, such that you can't
tell the difference between deserialized type `X` and deserialized type `Y`. This allows a
malicious user to substitute `X` for `Y` or vice versa, and the code may perform unauthorized
actions with the bytes.

**Known problems:** In the case when only one enum is deserialized, this lint by default
regards that as secure. However, this is not always the case. For example, if the program
defines another enum and serializes, but never deserializes it, a user could create this enum,
and, if it deserializes the same as the first enum, then this may be a possible vulnerability.

Furthermore, one may have alternative definitions of a discriminant, such as using a bool,
or u8, and not an enum. This will flag a false positive.

## Note on Tests
**insecure-anchor**: insecure because `User` type derives Discriminator trait (via `#[account]`),
thus one may expect this code to be secure. However, the program tries to deserialize with
`try_from_slice`, the default borsh deserialization method, which does _not_ check for the
discriminator. Thus, one could potentially serialize a `Metadata` struct, and then later
deserialize without any problem into a `User` struct, leading to a type-cosplay vulnerability.

**recommended**: this is secure code because all structs have an `#[account]` macro attributed
on them, thus deriving the `Discriminator` trait for each. Further, unlike the insecure-anchor
example, the program uses the proper deserialization method, `try_deserialize`, to deserialize
bytes as `User`. This is "proper" because in the derived implementation of `try_deserialize`,
the discriminator of the type is checked first.
