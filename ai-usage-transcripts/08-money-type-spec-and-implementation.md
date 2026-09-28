# Designing and implementing a precise Money type

I wanted to investigate what an owned type for four-precision-point money would
look like. I asked AI to create a lightweight implementation, and then to
implement that. I did not like the results, and will be moving with a ready-made
crate instead of tweaking the implementation. If I had more intention on using
the implementation, I would work on refining the specification and tweaking the
output code. But I just wanted to see what it would look like, and thus this
quick interaction sufficed.

The LLM's implementation is persisted in a different branch: `draft-money-type`
as `8180c3a`.

# Prompt 1:

Context:

This crate currently uses a type alias "Money" that is simply an i64. 

In order to support a precision of four points after the decimal, we are going to create a new type to represent the money. 

This type should internally always use integers, and it should support addition and subtraction. Values of this type will be constructed from
floating points with up to four points after the decimal. 

This is meant to be used by a banking system, so accuracy is fundamental.

Prompt:

Create a lightweight specification with the design and implementation plan of this new type. Additionally, assess the refactoring risk of updating the project to use it.

# Output 1:

Created the [Money type specification](/Users/crs/Development/payment-processing/artifacts/money-spec.md). It defines integer storage at four decimal places, checked addition and subtraction, construction from decimal text and `f64`, a migration plan, and a refactoring risk assessment.

The key accuracy limit is at input: a fallible float constructor can reject excess precision, but it cannot recover decimal digits already lost in a floating-point value. The spec therefore recommends decimal text for banking inputs. I reviewed the document and ran `git diff --check`; no code was changed.

# Prompt 2:

Implement the new money type following the specification. Persist any implementation errors and ambiguities you find along the way.

# Output 2:

Implemented the [Money type](/Users/crs/Development/payment-processing/src/types.rs) with integer storage at four decimal places, checked addition and subtraction, exact decimal parsing, and fallible `f64` construction. The [processor](/Users/crs/Development/payment-processing/src/payment_processor.rs) now rejects overflow without partially applying an event.

The full suite passed: **9 tests**, including the state-machine test. Focused tests also passed after the final type cleanup. Formatting and `git diff --check` pass. Clippy succeeds with existing warnings; its strict `-D warnings` mode does not.

Implementation decisions and remaining ambiguities, especially the limits of float input and the undefined policy for zero or negative transaction amounts, are recorded in [money-problems.md](/Users/crs/Development/payment-processing/artifacts/money-problems.md).
