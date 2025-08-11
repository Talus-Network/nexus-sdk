# `xyz.taluslabs.math.i64.add@1`

Standard Nexus Tool that adds two [`prim@i64`] numbers and returns the result.

## Input

**`a`: [`prim@i64`]**

The first number to add.

**`b`: [`prim@i64`]**

The second number to add.

## Output Variants & Ports

**`ok`**

The addition was successful.

- **`ok.result`: [`prim@i64`]** - The result of the addition.

**`err`**

The addition failed due to overflow.

- **`err.reason`: [`String`]** - The reason for the error. This is always overflow.

---

# `xyz.taluslabs.math.i64.mul@1`

Standard Nexus Tool that multiplies two [`prim@i64`] numbers and returns the result.

## Input

**`a`: [`prim@i64`]**

The first number to multiply.

**`b`: [`prim@i64`]**

The second number to multiply.

## Output Variants & Ports

**`ok`**

The multiplication was successful.

- **`ok.result`: [`prim@i64`]** - The result of the multiplication.

**`err`**

The multiplication failed due to overflow.

- **`err.reason`: [`String`]** - The reason for the error. This is always overflow.

---

# `xyz.taluslabs.math.i64.cmp@1`

Standard Nexus Tool that compares two [`prim@i64`] numbers and returns the result.

## Input

**`a`: [`prim@i64`]**

The first number to compare.

**`b`: [`prim@i64`]**

The second number to compare.

## Output Variants & Ports

**`gt`**

The first number is greater than the second.

- **`gt.a`: [`prim@i64`]** - The first number.
- **`gt.b`: [`prim@i64`]** - The second number.

**`eq`**

The first number is equal to the second.

- **`eq.a`: [`prim@i64`]** - The first number.
- **`eq.b`: [`prim@i64`]** - The second number.

**`lt`**

The first number is less than the second.

- **`lt.a`: [`prim@i64`]** - The first number.
- **`lt.b`: [`prim@i64`]** - The second number.

---

# `xyz.taluslabs.math.i64.sum@1`

Standard Nexus Tool that sums an array of [`i64`] numbers and returns the result.

## Input

**`vec`: [`Vec<prim@i64>`]**

The array of numbers to sum.

## Output Variants & Ports

**`ok`**

The summation was successful.

- **`ok.result`: [`prim@i64`]** - The result of the summation.

**`err`**

The summation failed due to overflow.

- **`err.reason`: [`String`]** - The reason for the error. This is always overflow.
