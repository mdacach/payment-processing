# Investigating Serde for CSV event parsing

I'm not sure why it didn't use serde first of all, given it's so popular and a good fit.
The answers here were not really good, too, and I needed to push back against it a bit.

# Prompt 1:

I want you to investigate the serde crate. Could we use it for parsing the events from the csvs more cleanly?

# Output 1:

Serde can deserialize the CSV fields into a typed row, replacing positional lookups and ID parsing. The event conversion still needs to select the variant and apply the existing money rules. I recommended a `CsvRow` conversion and initially said that deserializing directly into `Event` would not work.

# Prompt 2:

What are the limits of the csv crate's limited enum deserialization?

# Output 2:

The `csv` crate documents unit enums for a single field and single-value variants through untagged deserialization. A probe confirmed that deriving an internally tagged `Event` with `#[serde(tag = "type")]` fails when reading the CSV row. I suggested deserializing the `type` column into an event-kind enum inside a row struct.

# Prompt 3:

We really can't do it directly from the CSV? That's a bit weird.

# Output 3:

I corrected the earlier claim: `reader.deserialize::<Event>()` can work with `#[serde(try_from = "CsvRow")]` on `Event` and a `TryFrom<CsvRow>` implementation. I verified that approach with deposit and dispute rows in a small probe. The conversion remains necessary to select and validate the event variant.

# Prompt 4:

Couldn't we provide custom deserializer for the amount through serde's API?

# Output 4:

Yes. `#[serde(deserialize_with = "deserialize_amount")]` can parse the CSV amount field into `Option<Money>`: an empty field becomes `None`, and a nonempty field uses the existing precision and range validation. I noted that this validates a nonempty amount even on dispute-related rows, which differs from the current parser's behavior.

# Prompt 5:

But that's fine, no? We don't expect nonempty malformed amounts on disputes — they're guaranteed to be empty. In that case, we would hit the branch returning None, and things work out.

# Output 5:

I agreed. With trimmed CSV fields, an empty amount becomes `None`. The row-to-event conversion should require `Some(Money)` for deposits and withdrawals and `None` for disputes, resolves, and chargebacks, making the stated CSV rule explicit.
