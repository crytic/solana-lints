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

### insecure

This is the canonical example of type-cosplay. The program tries to deserialize
bytes from `AccountInfo.data` into the `User` type. However, a malicious user could pass in
an account that has in it's data field the `Metadata` type. This type is equivalent to the
`User` type, and the data bytes will thus successfully deserialize as a `User` type. The
program performs no checks whatsoever, and will continue on operating with a pubkey that it
believes to be a `User` pubkey, not a `Metadata` pubkey.

### insecure-2

This is insecure because the program tries to deserialize from multiple enum types.
Here, `UserInfo` and `MetadataInfo` enums are both being deserialized. Note that both of these
enums contain a single variant, with the struct type nested inside it. This evades the in-built
discriminant of an enum. A `Metadata` type could be deserialized into a `UserInfo::User(User)`,
and a `User` could be deserialized into a `MetadataInfo::Metadata(Metadata)`.

Only deserializing from a single enum is safe since enums contain a natural, in-built discriminator.
If _all_ types are nested under a variant of this enum, then when deserializing, the enum variant
must be matched first, thus guaranteeing differentiation between types.

However, deserializing from multiple enums partitions the "set of types" and is thus not exhaustive
in discriminating between all types. If multiple enums are used to encompass the types, there may
be two equivalent types that are variants under different enums, as seen in this example.

### insecure-3

This example is insecure because `AccountWithDiscriminant` could be deserialized as a
`User`, if the variant is `Extra(Extra)`. The first byte would be 0, to indicate the discriminant
in both cases, and the next 32 bytes would be the pubkey. The problem here is similar to
the insecure-2 example--not all types are nested under a single enum type. Except here,
instead of using another enum, the program also tries to deserialize `User`.

This illustrates that in order to properly take advantage of the enums natural built-in
discriminator, you must nest _all_ types in your program as variants of this enum, and
only serialize and deserialize this enum type.

### insecure-anchor

Insecure because `User` type derives Discriminator trait (via `#[account]`),
thus one may expect this code to be secure. However, the program tries to deserialize with
`try_from_slice`, the default borsh deserialization method, which does _not_ check for the
discriminator. Thus, one could potentially serialize a `Metadata` struct, and then later
deserialize without any problem into a `User` struct, leading to a type-cosplay vulnerability.

### recommended

The recommended way to address the type-cosplay issue. It adds an `#[account]` macro to each
struct, which adds a discriminant to each struct. It doesn't actually perform any deserializations,
which is why the `recommended-2` was created.

### recommended-2

This is secure code because all structs have an `#[account]` macro attributed
on them, thus deriving the `Discriminator` trait for each. Further, unlike the insecure-anchor
example, the program uses the proper deserialization method, `try_deserialize`, to deserialize
bytes as `User`. This is "proper" because in the derived implementation of `try_deserialize`,
the discriminator of the type is checked first.

_Note: this example differs from the Sealevel [recommended](https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/3-type-cosplay/recommended/src/lib.rs) example in that it actually attempts_
_to perform a deserialization in the function body, and then uses the struct. It provides_
_a more realistic and concrete example of what might happen in real programs_

### secure

This example is from the Sealevel [example](https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/3-type-cosplay/secure/src/lib.rs). It fixes the insecure case by adding a `discriminant`
field to each struct, and further this discriminant is "proper" because it contains the
necessary amount of variants in order to differentiate each type. In the code, there is
an explicit check to make sure the discriminant is as expected.

### secure-2

This example fixes both the insecure and insecure-2 examples. It is secure because it only deserializes
from a single enum, and that enum encapsulates all of the user-defined types. Since enums contain
an implicit discriminant, this program will always be secure as long as all types are defined under the enum.
