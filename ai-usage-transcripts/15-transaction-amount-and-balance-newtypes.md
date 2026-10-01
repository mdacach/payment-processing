# Replacing Money with TransactionAmount and Balance newtypes

Mostly mechanical change that is a good fit for LLMs.

# Prompt 1:

What do you think about creating a newtype for Balance (that can be negative) and ValidatedAmount (that must be positive and bigger than 0.0001)?

# Output 1:

I recommended separate types for transaction inputs and account balances. I noted that the existing minimum includes `0.0001`; a strictly greater requirement would make `0.0002` the smallest valid amount. I initially suggested `PositiveAmount` as a name and checked arithmetic for balances.

# Prompt 2:

What about TransactionAmount, rather than PositiveAmount? Then we can document the requirements.

# Output 2:

I agreed that `TransactionAmount` names the domain role well. I suggested a private field and fallible construction enforcing the inclusive `0.0001` minimum, with `Balance` representing signed account values.

# Prompt 3:

What if we removed Money altogether? And used fixed decimal values for both TransactionAmount and Balance?

# Output 3:

I recommended removing the public `Money` alias and wrapping the fixed-point decimal directly in `TransactionAmount` and `Balance`. I noted that the underlying `i64` range still requires checked arithmetic.

# Prompt 4:

Create an implementation plan for making this change. Also consider what needs to change in the csv parsing main.

# Output 4:

I planned to define the two newtypes, migrate events, account state, and errors, remove the `Money` export, and update tests and documentation. The plan kept four-decimal truncation in `main.rs` as a CSV policy while moving the positive-minimum rule into `TransactionAmount`.

# Prompt 5:

Go ahead and implement this change.

# Output 5:

I implemented the newtypes and migrated the processor and CSV parser. `TransactionAmount` enforces the inclusive `0.0001` minimum; `Balance` permits negative values and uses checked arithmetic. I removed the public `Money` alias, updated tests and the README, and reported passing compilation, formatting, CLI, scenario, constructor, and state-machine checks.
