The type cosplay issue is when one account type is confused for another account type.
This occurs when a type deserializes exactly the same as another type, such that you can't
tell the difference between deserialized type X and deserialized type Y. This can have catastrophic
consequences because type Y may be deserialized, when what was expected was a type X.

For example, imagine X and Y have a single field that is of type `Pubkey`. `Pubkey`s have no meaning
to the machine, they are all just 32 byte sequences, but they have semantic meaning for us. Suppose
X's pubkey is the owner's pubkey, while Y's pubkey is just any random pubkey. The code may deserialize
the data, assuming it is getting a struct of type X, and then may do things only the owner
is authorized to do. But since Y deserializes the same as X, a malicious user can pass in an account
of type Y, which will deserialize correctly, and then the code will perform authorized actions when
it shouldn't be.

Usually, when two types deserialize the same, they probably have the same type as well. Thus, one
might prevent this attack by designing a lint to detect if any struct types are equivalent in the code.
However distinct types do not always imply distinct deserialization! There may be two distinct types
that serialize the same.

We cannot just detect whether two types are the same to flag the lint, as this does not address the
core issue. The core issue is whether two types deserialize the same or not. Thus we should hunt for
any two types that deserialize the same. However, this is kinda impossible to do, since it is dependent
on what data format is being used. Two types that deserialize the same in one data format might not
 in another.

So the strategy we've come up with is first gather all the types that are being deserialized in the code.
Say `n` types are collected. Then one of the following constraints must hold:

1. `n=1`, and the type is an enum.
2. All types are structs AND
each struct has a field that is the enum type found before AND
the number of variants in the enum must be at least `n-1`.

These two constraints encode two scenarios where the code will be safe from the type-cosplay issue:

1. If there is only 1 enum being deserialized, then all types will be unique.
2. 

Two circumstances avoid the type cosplay attack.
// 1. All structs are variants of a single enum type--an enum effectively has a
// built in discriminator because each enum variant is defined to be unique. Thus, if all
// struct types in the code are defined as a variant under a single enum, then each type
// can be distinguished.

# Test Cases
Whenever we refer to a type, we refer to whether it was deserialized in the program, not
to the type definition.
- single deserialized type; is enum => SECURE
- single deserialized type; is not enum; has discriminant => SECURE
- single deserialized type; is not enum; no discriminant => INSECURE (insecure-1)
NOTE: do we really need to check if one is an enum?
- multiple deserialized types; one is enum; all structs have discriminant => SECURE
- multiple deserialized types; one is enum; some struct doesn't have discriminant => INSECURE
- multiple deserialized types; multiple enums => INSECURE (insecure-2)
- multiple deserialized types; no enums; all structs have discriminant => SECURE
- multiple deserialized types; no enums; some struct doesn't have discriminant => INSECURE